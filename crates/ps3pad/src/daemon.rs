use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use ps3pad_core::{Connection, InputReport, OutputReport, VirtualPadState};
use ps3pad_platform::{Controller, DeviceInfo, VirtualPad};
use serde::Serialize;

use crate::cli::DaemonArgs;
use crate::util::{connection_label, format_mac, select_device};

#[derive(Debug)]
pub(crate) enum Command {
    Stop,
    Led(u8),
    Rumble { left: u8, right: bool },
}

#[derive(Clone, Debug, Serialize, Default)]
pub(crate) struct Status {
    connected: bool,
    path: Option<String>,
    name: Option<String>,
    connection: Option<Connection>,
    device_mac: Option<String>,
    battery_percent: u8,
    battery_state: String,
    buttons: Vec<String>,
    left_stick: Option<[u8; 2]>,
    right_stick: Option<[u8; 2]>,
    pressure_l2: u8,
    pressure_r2: u8,
    accel_g: Option<[f32; 3]>,
    gyro: i16,
    virtual_pad: bool,
    reports: u64,
    updated_ms: u128,
}

impl Status {
    fn update_report(&mut self, report: &InputReport) {
        self.buttons = report
            .buttons
            .iter()
            .map(|button| button.name().to_string())
            .collect();
        self.battery_percent = report.battery.percent;
        self.battery_state = format!("{:?}", report.battery.state);
        self.left_stick = Some([report.left_stick.x, report.left_stick.y]);
        self.right_stick = Some([report.right_stick.x, report.right_stick.y]);
        self.pressure_l2 = report.pressure.l2;
        self.pressure_r2 = report.pressure.r2;
        self.accel_g = Some(report.accel.to_g(&Default::default()));
        self.gyro = report.gyro;
        self.reports += 1;
        self.updated_ms = now_ms();
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

pub(crate) fn run(mut args: DaemonArgs) -> Result<()> {
    let mut info = select_device(args.device.as_deref())?;
    let mut pad: Option<VirtualPad> = None;
    #[cfg(not(windows))]
    {
        if !args.no_virtual {
            pad = Some(
                VirtualPad::create_default()
                    .context("cannot create virtual gamepad")?,
            );
            println!(
                "virtual gamepad: {} ({:04x}:{:04x})",
                ps3pad_core::GENERIC_GAMEPAD_NAME,
                ps3pad_core::VIRTUAL_VENDOR_ID,
                ps3pad_core::VIRTUAL_PRODUCT_ID
            );
        }
    }
    #[cfg(windows)]
    {
        let _ = args.no_virtual;
        println!("Windows: DsHidMini exposes the DS3 as a native XInput device; no virtual pad needed.");
    }

    let status = Arc::new(Mutex::new(Status {
        virtual_pad: pad.is_some(),
        ..Status::default()
    }));
    let (commands, command_rx) = mpsc::channel();

    #[cfg(unix)]
    {
        if args.socket.as_os_str().is_empty() {
            args.socket = default_socket_path();
        }
        match crate::socket::start(&args.socket, Arc::clone(&status), commands.clone()) {
            Ok(()) => println!("control socket: {}", args.socket.display()),
            Err(error) => eprintln!("control socket disabled: {error:#}"),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = &args.socket;
    }

    loop {
        match open_and_bridge(&info, &mut pad, &args, &status, &command_rx) {
            Ok(Exit::Stop) => {
                println!("stopping");
                let mut status = status.lock().unwrap();
                status.connected = false;
                return Ok(());
            }
            Ok(Exit::Disconnected) => {
                eprintln!("controller disconnected; waiting for it to return");
            }
            Err(error) => {
                eprintln!("bridge error: {error:#}");
            }
        }

        {
            let mut status = status.lock().unwrap();
            status.connected = false;
        }

        thread::sleep(Duration::from_secs(1));
        match select_device(args.device.as_deref().or(Some(info.path.as_str()))) {
            Ok(found) => info = found,
            Err(_) => match select_device(args.device.as_deref()) {
                Ok(found) => info = found,
                Err(error) => {
                    eprintln!("no controller available: {error:#}");
                    continue;
                }
            },
        }
        #[cfg(not(windows))]
        {
            if pad.is_none() && !args.no_virtual {
                pad = Some(
                    VirtualPad::create_default()
                        .context("cannot recreate virtual gamepad")?,
                );
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Exit {
    Stop,
    Disconnected,
}

fn open_and_bridge(
    info: &DeviceInfo,
    pad: &mut Option<VirtualPad>,
    args: &DaemonArgs,
    status: &Arc<Mutex<Status>>,
    commands: &Receiver<Command>,
) -> Result<Exit> {
    let mut controller = Controller::open(info).context("cannot open controller")?;
    let init = controller
        .initialize()
        .context("controller initialisation failed")?;

    {
        let mut status = status.lock().unwrap();
        status.connected = true;
        status.path = Some(info.path.clone());
        status.name = Some(info.name.clone());
        status.connection = Some(info.connection);
        status.device_mac = init.device_mac.map(format_mac);
    }

    println!(
        "connected: {} on {} ({}){}",
        info.name,
        info.path,
        connection_label(info),
        init.device_mac
            .map(|mac| format!(", MAC {}", format_mac(mac)))
            .unwrap_or_default()
    );

    let mut output = OutputReport::default().with_player_led(args.player);
    controller
        .send_output(&output)
        .context("cannot set initial LED state")?;

    let mut last_sent = output;
    let mut next_poll = Instant::now();

    loop {
        // Control requests from the socket.
        loop {
            match commands.try_recv() {
                Ok(Command::Stop) => return Ok(Exit::Stop),
                Ok(Command::Led(player)) => {
                    output.set_player_led(player);
                }
                Ok(Command::Rumble { left, right }) => {
                    output.rumble_left = left;
                    output.rumble_right = right;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
        }

        // Host output reports from the virtual pad (rumble and LEDs).
        if let Some(pad) = pad.as_mut() {
            while let Some(host) = pad
                .next_output(Duration::ZERO)
                .context("virtual pad event failed")?
            {
                if args.rumble {
                    output.rumble_left = host.rumble_left;
                    output.rumble_right = host.rumble_right;
                }
                if host.leds != 0 {
                    output.leds = host.leds;
                }
            }
        }

        if output != last_sent {
            match controller.send_output(&output) {
                Ok(()) => last_sent = output,
                Err(error) if error.is_disconnected() => return Ok(Exit::Disconnected),
                Err(error) => eprintln!("cannot send output report: {error}"),
            }
        }

        match controller.read(Duration::from_millis(4)) {
            Ok(Some(report)) => {
                if let Some(pad) = pad.as_mut() {
                    pad.send_input(&VirtualPadState::from_input(&report).to_report())
                        .context("cannot send virtual input report")?;
                }
                status.lock().unwrap().update_report(&report);
            }
            Ok(None) => {}
            Err(ps3pad_platform::PlatformError::Protocol(
                ps3pad_core::ParseError::InvalidBluetoothReport,
            )) => {}
            Err(ps3pad_platform::PlatformError::Protocol(error)) => {
                eprintln!("bad report: {error}");
            }
            Err(error) if error.is_disconnected() => return Ok(Exit::Disconnected),
            Err(error) => return Err(error).context("read failed"),
        }

        // Avoid a busy loop when the controller is silent.
        if next_poll > Instant::now() {
            thread::sleep(next_poll - Instant::now());
        }
        next_poll = Instant::now() + Duration::from_millis(2);
    }
}

#[cfg(unix)]
fn default_socket_path() -> std::path::PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
    base.join("ps3pad").join("control.sock")
}
