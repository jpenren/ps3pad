use std::fs;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use ps3pad_core::{Button, InputReport, OutputReport, ParseError};
use ps3pad_platform::{Controller, VirtualPad, VirtualPadEvent};
use serde_json::json;

use crate::cli::DumpArgs;
use crate::util::{connection_label, format_mac, parse_mac, select_device};

pub fn list(json_output: bool) -> Result<()> {
    let devices = Controller::discover().context("cannot enumerate controllers")?;

    if json_output {
        let items: Vec<_> = devices
            .iter()
            .enumerate()
            .map(|(index, device)| {
                json!({
                    "index": index,
                    "path": device.path,
                    "name": device.name,
                    "vendor_id": format!("{:04x}", device.vendor_id),
                    "product_id": format!("{:04x}", device.product_id),
                    "connection": device.connection,
                    "serial": device.serial,
                    "phys": device.phys,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&items)?);
        return Ok(());
    }

    if devices.is_empty() {
        println!("No supported controllers found.");
        println!("Connect a DualShock 3 over USB, or pair it over Bluetooth.");
        return Ok(());
    }

    println!(
        "{:<4} {:<18} {:<10} {:<10} {:<42} SERIAL",
        "ID", "PATH", "BUS", "VID:PID", "NAME"
    );
    for (index, device) in devices.iter().enumerate() {
        println!(
            "{:<4} {:<18} {:<10} {:<10} {:<42} {}",
            index,
            device.path,
            connection_label(device),
            format!("{:04x}:{:04x}", device.vendor_id, device.product_id),
            truncate(&device.name, 42),
            device.serial.as_deref().unwrap_or("-"),
        );
    }
    Ok(())
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        value.to_string()
    } else {
        let mut result: String = value.chars().take(width.saturating_sub(1)).collect();
        result.push('…');
        result
    }
}

pub fn dump(args: DumpArgs) -> Result<()> {
    let info = select_device(args.device.as_deref())?;
    let mut controller = Controller::open(&info).context("cannot open controller")?;

    if !args.no_init {
        let init = controller
            .initialize()
            .context("controller initialisation failed")?;
        if let Some(mac) = init.device_mac {
            eprintln!(
                "controller {} on {} (MAC {})",
                info.name,
                info.path,
                format_mac(mac)
            );
        } else {
            eprintln!("controller {} on {}", info.name, info.path);
        }
    }

    let started = Instant::now();
    let deadline = (args.seconds > 0).then(|| started + Duration::from_secs(args.seconds));
    let mut reports = 0u64;

    loop {
        let now = Instant::now();
        if let Some(deadline) = deadline {
            if now >= deadline {
                break;
            }
        }
        let timeout = match deadline {
            Some(deadline) => deadline
                .saturating_duration_since(now)
                .min(Duration::from_millis(250)),
            None => Duration::from_millis(250),
        };

        match controller.read(timeout) {
            Ok(Some(report)) => {
                reports += 1;
                if args.raw {
                    println!("{}", report.raw_hex());
                } else if args.json {
                    println!(
                        "{}",
                        serde_json::to_string(&json!({
                            "elapsed_ms": started.elapsed().as_millis() as u64,
                            "connection": info.connection,
                            "report": report,
                        }))?
                    );
                } else {
                    println!("{}", format_report(&report, started.elapsed()));
                }
            }
            Ok(None) => {}
            Err(ps3pad_platform::PlatformError::Protocol(ParseError::InvalidBluetoothReport)) => {
                // Harmless keep-alive/idle packet.
            }
            Err(ps3pad_platform::PlatformError::Protocol(error)) => {
                eprintln!("bad report: {error}");
            }
            Err(error) if error.is_disconnected() => {
                eprintln!("controller disconnected");
                break;
            }
            Err(error) => return Err(error).context("read failed"),
        }
    }

    if args.seconds > 0 {
        eprintln!("captured {reports} report(s) in {}s", args.seconds);
    }
    Ok(())
}

fn format_report(report: &InputReport, elapsed: Duration) -> String {
    let buttons = if report.buttons.is_empty() {
        "-".to_string()
    } else {
        report
            .buttons
            .iter()
            .map(Button::name)
            .collect::<Vec<_>>()
            .join("+")
    };
    let (lx, ly) = report.left_stick.normalized();
    let (rx, ry) = report.right_stick.normalized();
    let [ax, ay, az] = report.accel.to_g(&Default::default());
    let tilt = report.accel.tilt(&Default::default());

    format!(
        "[{:>7.2}s] {:>9}  btn:{buttons:<32}  L({lx:+.2},{ly:+.2}) R({rx:+.2},{ry:+.2})  L2:{:>3} R2:{:>3}  bat:{:>3}% {:<11}  accel({ax:+.2},{ay:+.2},{az:+.2})g  tilt({:+.1},{:+.1})°  gyro:{:>6}",
        elapsed.as_secs_f64(),
        report.connection.label(),
        report.pressure.l2,
        report.pressure.r2,
        report.battery.percent,
        format!("{:?}", report.battery.state),
        tilt.pitch,
        tilt.roll,
        report.gyro,
    )
}

pub fn doctor(json_output: bool) -> Result<()> {
    let devices = Controller::discover().context("cannot enumerate controllers")?;
    let platform = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);

    #[cfg(windows)]
    fn windows_checks(devices: &[ps3pad_platform::DeviceInfo]) -> (Vec<(&'static str, String)>, Vec<String>) {
        use std::process::Command;
        let mut checks: Vec<(&'static str, String)> = Vec::new();
        let mut hints: Vec<String> = Vec::new();

        // Check that DsHidMini-backed controllers appear as XInput devices.
        let dshidmini = devices.iter().any(|d| d.vendor_id == 0x054c && d.product_id == 0x0268);
        checks.push((
            "DsHidMini driver",
            if dshidmini {
                "installed (DS3 exposed as XInput)".to_string()
            } else {
                "not detected (install the DsHidMini driver package)".to_string()
            },
        ));
        if !dshidmini {
            hints.push(r"Install the DsHidMini driver package (signed UMDF driver for USB\VID_054C&PID_0268).".to_string());
        }

        // Check that the key services are running.
        for service in ["nssmkig", "mshidumdf", "WUDFRd"] {
            let output = Command::new("sc.exe")
                .args(["query", service])
                .output();
            let running = output
                .map(|o| String::from_utf8_lossy(&o.stdout).contains("RUNNING"))
                .unwrap_or(false);
            checks.push((
                service,
                if running { "running".to_string() } else { "not running".to_string() },
            ));
            if !running {
                hints.push(format!("Service {service} is not running."));
            }
        }

        (checks, hints)
    }

    #[cfg(not(windows))]
    fn linux_checks() -> (Vec<(&'static str, String)>, Vec<String>) {
        use std::path::Path;
        use std::fs;

        let uhid = check_uhid();
        let hidraw_count = count_hidraw_nodes();
        let module_loaded =
            Path::new("/sys/module/hid_sony").exists() || Path::new("/sys/module/sony").exists();
        let udev_rule = udev_rule_path();

        let mut checks: Vec<(&'static str, String)> = Vec::new();
        checks.push(("hidraw nodes", hidraw_count.to_string()));
        checks.push((
            "uhid",
            match (uhid.present, uhid.readable, uhid.writable) {
                (false, _, _) => "MISSING (load the uhid module)".to_string(),
                (true, true, true) => "present, read/write".to_string(),
                (true, true, false) => "present, read-only (check permissions)".to_string(),
                (true, false, _) => "present, not readable (check permissions)".to_string(),
            },
        ));
        checks.push((
            "hid_sony module",
            if module_loaded {
                "loaded".to_string()
            } else {
                "not loaded (optional)".to_string()
            },
        ));
        checks.push((
            "udev rule",
            udev_rule
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "not installed".to_string()),
        ));
        let mut hints: Vec<String> = Vec::new();
        if !uhid.present {
            hints.push("Load the module with: sudo modprobe uhid".to_string());
        }
        (checks, hints)
    }

    if json_output {
        #[cfg(windows)]
        {
            let (checks, _hints) = windows_checks(&devices);
            let json_checks: serde_json::Map<String, serde_json::Value> = checks
                .into_iter()
                .map(|(k, v)| (k.to_string(), serde_json::Value::String(v)))
                .collect();
            let checks = json!({
                "platform": platform,
                "supported_controllers": devices.len(),
                "checks": json_checks,
            });
            println!("{}", serde_json::to_string_pretty(&checks)?);
            return Ok(());
        }
        #[cfg(not(windows))]
        {
            let (checks, _hints) = linux_checks();
            let hidraw_count: usize = checks
                .iter()
                .find(|(k, _)| *k == "hidraw nodes")
                .and_then(|(_, v)| v.parse().ok())
                .unwrap_or(0);
            let uhid_present = checks
                .iter()
                .find(|(k, _)| *k == "uhid")
                .map(|(_, v)| v == "present, read/write")
                .unwrap_or(false);
            let checks = json!({
                "platform": platform,
                "hidraw_nodes": hidraw_count,
                "supported_controllers": devices.len(),
                "uhid": uhid_present,
                "hid_sony_module": checks
                    .iter()
                    .find(|(k, _)| *k == "hid_sony module")
                    .map(|(_, v)| v == "loaded")
                    .unwrap_or(false),
                "udev_rule": checks
                    .iter()
                    .find(|(k, _)| *k == "udev rule")
                    .and_then(|(_, v)| if v == "not installed" { None } else { Some(v.clone()) }),
            });
            println!("{}", serde_json::to_string_pretty(&checks)?);
            return Ok(());
        }
    }

    println!("ps3pad doctor");
    println!("  platform          {platform}");
    println!("  controllers       {} supported", devices.len());
    for (index, device) in devices.iter().enumerate() {
        println!(
            "    [{index}] {}  {}  {:04x}:{:04x}  {}",
            device.path,
            connection_label(device),
            device.vendor_id,
            device.product_id,
            device.name
        );
    }

    #[cfg(windows)]
    {
        let (checks, hints) = windows_checks(&devices);
        for (key, value) in &checks {
            println!("  {:<18} {}", key, value);
        }
        if devices.is_empty() {
            println!();
            println!("No controller detected. Connect a DualShock 3 over USB and ensure the DsHidMini driver is installed.");
        }
        for hint in &hints {
            println!();
            println!("  {hint}");
        }
    }
    #[cfg(not(windows))]
    {
        let (checks, hints) = linux_checks();
        for (key, value) in &checks {
            println!("  {:<18} {}", key, value);
        }
        if devices.is_empty() {
            println!();
            println!("No controller detected. USB: connect the cable. Bluetooth: pair and press PS.");
        }
        for hint in &hints {
            println!();
            println!("  {hint}");
        }
    }

    Ok(())
}

struct UhidStatus {
    present: bool,
    readable: bool,
    writable: bool,
}

fn check_uhid() -> UhidStatus {
    let path = Path::new("/dev/uhid");
    let present = path.exists();
    let readable = fs::OpenOptions::new().read(true).open(path).is_ok();
    let writable = fs::OpenOptions::new().write(true).open(path).is_ok();
    UhidStatus {
        present,
        readable,
        writable,
    }
}

fn count_hidraw_nodes() -> usize {
    fs::read_dir("/sys/class/hidraw")
        .map(|entries| entries.flatten().count())
        .unwrap_or(0)
}

fn udev_rule_path() -> Option<std::path::PathBuf> {
    [
        "/etc/udev/rules.d/60-ps3pad.rules",
        "/usr/lib/udev/rules.d/60-ps3pad.rules",
        "/lib/udev/rules.d/60-ps3pad.rules",
    ]
    .into_iter()
    .map(Path::new)
    .find(|path| path.exists())
    .map(Path::to_path_buf)
}

pub fn led(player: u8, device: Option<&str>) -> Result<()> {
    let info = select_device(device)?;
    let mut controller = Controller::open(&info).context("cannot open controller")?;
    controller
        .initialize()
        .context("controller initialisation failed")?;
    let output = OutputReport::default().with_player_led(player);
    controller
        .send_output(&output)
        .context("cannot set player LED")?;
    println!("set player {player} LED on {}", info.path);
    Ok(())
}

pub fn pair(host_mac: &str, device: Option<&str>) -> Result<()> {
    let mac = parse_mac(host_mac)?;
    let info = select_device(device)?;
    let mut controller = Controller::open(&info).context("cannot open controller")?;
    controller
        .initialize()
        .context("controller initialisation failed")?;

    let report = ps3pad_core::host_mac_report(mac);
    controller
        .set_feature(&report)
        .context("cannot write pairing feature report 0xf5")?;

    // Apply the stored address immediately where the controller supports it.
    let output = OutputReport::default().with_player_led(1);
    let _ = controller.send_output(&output);
    println!(
        "stored host address {} in {} (unplug and reconnect over Bluetooth)",
        format_mac(mac),
        info.path
    );
    Ok(())
}

pub fn uhid_test(seconds: u64) -> Result<()> {
    let mut pad = VirtualPad::create_default().context("cannot create virtual gamepad")?;
    println!(
        "created virtual gamepad: {} ({:04x}:{:04x})",
        ps3pad_core::GENERIC_GAMEPAD_NAME,
        ps3pad_core::VIRTUAL_VENDOR_ID,
        ps3pad_core::VIRTUAL_PRODUCT_ID
    );
    println!("holding it open for {seconds}s; press Ctrl-C to stop early");

    let started = Instant::now();
    let frame = Duration::from_millis(16);
    let mut state = ps3pad_core::VirtualPadState::default();
    let mut last = Instant::now();

    while started.elapsed() < Duration::from_secs(seconds) {
        // Animate the left stick so the device is visibly alive in jstest.
        let phase = started.elapsed().as_secs_f32();
        state.lx = (128.0 + phase.sin() * 100.0) as u8;
        state.ly = (128.0 + phase.cos() * 100.0) as u8;
        pad.send_input(&state.to_report())
            .context("cannot send virtual input report")?;

        while let Some(event) = pad.poll(Duration::ZERO).context("uhid poll failed")? {
            match event {
                VirtualPadEvent::Start { dev_flags } => {
                    println!("host started the device (flags {dev_flags:#x})")
                }
                VirtualPadEvent::Open => println!("host opened the device"),
                VirtualPadEvent::Close => println!("host closed the device"),
                VirtualPadEvent::Output { data, .. } => {
                    println!("host output report: {}", hex(&data))
                }
                VirtualPadEvent::Destroy => anyhow::bail!("host destroyed the virtual device"),
                other => println!("event: {other:?}"),
            }
        }

        last += frame;
        if let Some(delay) = last.checked_duration_since(Instant::now()) {
            thread::sleep(delay);
        } else {
            last = Instant::now();
        }
    }
    println!("virtual gamepad removed");
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}
