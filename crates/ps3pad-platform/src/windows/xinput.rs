//! Windows XInput backend for `ps3pad-platform`.
//!
//! On Windows the physical DualShock 3 is driven by the signed DsHidMini UMDF
//! driver, which replaces `hidusb` for `USB\VID_054C&PID_0268` and exposes the
//! controller as a real XInput gamepad (`VID_045E&PID_02FF`). ps3pad therefore
//! talks XInput instead of raw HID: it polls `XInputGetState` on slots 0–3,
//! maps the gamepad state to a synthetic 49-byte DualShock 3 input report for
//! `InputReport::parse`, and routes rumble/LEDs through `XInputSetState`.
//!
//! Feature reports (`0xf2`/`0xf4`/`0xf5`) are not needed: DsHidMini already
//! puts the controller into full report mode, and XInput has no feature-report
//! concept. A virtual gamepad is likewise unnecessary because Windows already
//! exposes a genuine XInput device that games recognise natively.
//!
//! XInput functions are declared manually (no `windows-sys` dependency) to
//! avoid the import-library tooling that the GNU Rust toolchain lacks.

use std::time::Duration;

use crate::device::{DeviceInfo, InitInfo};
use crate::error::PlatformError;
use ps3pad_core::{Connection, DS3_PRODUCT_ID, SONY_VENDOR_ID};

/// Number of XInput slots to probe.
const XINPUT_SLOTS: u32 = 4;

/// XInput error codes.
const ERROR_SUCCESS: u32 = 0;
const ERROR_DEVICE_NOT_CONNECTED: u32 = 1167;

/// XInput button masks from `XINPUT_GAMEPAD.wButtons`.
const XINPUT_BUTTON_DPAD_UP: u16 = 0x0001;
const XINPUT_BUTTON_DPAD_DOWN: u16 = 0x0002;
const XINPUT_BUTTON_DPAD_LEFT: u16 = 0x0004;
const XINPUT_BUTTON_DPAD_RIGHT: u16 = 0x0008;
const XINPUT_BUTTON_START: u16 = 0x0010;
const XINPUT_BUTTON_BACK: u16 = 0x0020;
const XINPUT_BUTTON_LEFT_THUMB: u16 = 0x0040;
const XINPUT_BUTTON_RIGHT_THUMB: u16 = 0x0080;
const XINPUT_BUTTON_LEFT_SHOULDER: u16 = 0x0100;
const XINPUT_BUTTON_RIGHT_SHOULDER: u16 = 0x0200;
const XINPUT_BUTTON_A: u16 = 0x1000;
const XINPUT_BUTTON_B: u16 = 0x2000;
const XINPUT_BUTTON_X: u16 = 0x4000;
const XINPUT_BUTTON_Y: u16 = 0x8000;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
struct XInputGamepad {
    w_buttons: u16,
    b_left_trigger: u8,
    b_right_trigger: u8,
    s_thumb_lx: i16,
    s_thumb_ly: i16,
    s_thumb_rx: i16,
    s_thumb_ry: i16,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
struct XInputState {
    dw_packet_number: u32,
    gamepad: XInputGamepad,
    _pad: [u8; 2], // alignment padding
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
struct XInputVibration {
    w_left_motor_speed: u16,
    w_right_motor_speed: u16,
}

type XInputGetStateFn =
    unsafe extern "system" fn(u32, *mut XInputState) -> u32;
type XInputSetStateFn =
    unsafe extern "system" fn(u32, *mut XInputVibration) -> u32;

/// Dynamically loads `xinput1_4.dll` (falling back to `xinput1_3.dll`) and
/// resolves the two functions we need. This avoids any import-library
/// tooling that the GNU Rust toolchain may lack.
struct XInput {
    get_state: XInputGetStateFn,
    set_state: XInputSetStateFn,
    _module: isize, // keep the HMODULE alive
}

impl XInput {
    fn new() -> Result<Self, PlatformError> {
        unsafe {
            let module = load_library(b"xinput1_4\0".as_ptr());
            let module = if module == 0 {
                load_library(b"xinput1_3\0".as_ptr())
            } else {
                module
            };
            if module == 0 {
                return Err(PlatformError::io(
                    "LoadLibrary(xinput1_4/xinput1_3)",
                    std::io::Error::last_os_error(),
                ));
            }
            let get_state = get_proc_address(module, b"XInputGetState\0".as_ptr());
            if get_state.is_null() {
                free_library(module);
                return Err(PlatformError::io(
                    "GetProcAddress(XInputGetState)",
                    std::io::Error::last_os_error(),
                ));
            }
            let set_state = get_proc_address(module, b"XInputSetState\0".as_ptr());
            if set_state.is_null() {
                free_library(module);
                return Err(PlatformError::io(
                    "GetProcAddress(XInputSetState)",
                    std::io::Error::last_os_error(),
                ));
            }
            Ok(Self {
                get_state: std::mem::transmute(get_state),
                set_state: std::mem::transmute(set_state),
                _module: module,
            })
        }
    }

    fn get_state(&self, slot: u32, state: &mut XInputState) -> u32 {
        unsafe { (self.get_state)(slot, state) }
    }

    fn set_state(&self, slot: u32, vibration: &mut XInputVibration) -> u32 {
        unsafe { (self.set_state)(slot, vibration) }
    }
}

unsafe extern "system" {
    #[link_name = "LoadLibraryA"]
    fn load_library(libname: *const u8) -> isize;

    #[link_name = "GetProcAddress"]
    fn get_proc_address(module: isize, procname: *const u8) -> *mut std::ffi::c_void;

    #[link_name = "FreeLibrary"]
    fn free_library(module: isize) -> i32;
}

/// An open XInput slot backing a [`Controller`].
pub struct Controller {
    xinput: XInput,
    slot: u32,
}

impl Controller {
    /// Opens an XInput slot previously returned by [`Controller::discover`].
    /// The slot index is parsed from `info.path` (e.g. `"XInput slot 2"`).
    pub fn open(info: &DeviceInfo) -> Result<Self, PlatformError> {
        let slot = slot_from_path(&info.path)
            .ok_or_else(|| PlatformError::NoSupportedDevice)?;
        let xinput = XInput::new()?;
        let mut state = XInputState::default();
        let rc = xinput.get_state(slot, &mut state);
        if rc != ERROR_SUCCESS {
            return Err(PlatformError::NoSupportedDevice);
        }
        Ok(Self { xinput, slot })
    }

    /// XInput has no "enable" step: DsHidMini keeps the controller streaming.
    pub fn initialize(&mut self) -> Result<InitInfo, PlatformError> {
        Ok(InitInfo::default())
    }

    /// Reads one XInput state packet and converts it to a raw DS3 report.
    pub fn read_raw(&mut self, _timeout: Duration) -> Result<Option<Vec<u8>>, PlatformError> {
        let mut state = XInputState::default();
        let rc = self.xinput.get_state(self.slot, &mut state);
        if rc == ERROR_DEVICE_NOT_CONNECTED {
            return Err(PlatformError::Disconnected);
        }
        if rc != ERROR_SUCCESS {
            return Err(PlatformError::io("XInputGetState", std::io::Error::from_raw_os_error(rc as i32)));
        }
        Ok(Some(build_ds3_report(&state.gamepad)))
    }

    /// Routes an LED/rumble output report to `XInputSetState`.
    pub fn write_output(&mut self, report: &[u8]) -> Result<(), PlatformError> {
        if report.is_empty() || report[0] != 0x01 {
            return Err(PlatformError::Message(
                "output report must start with report ID 0x01".into(),
            ));
        }
        let rumble_left = report[5];
        let rumble_right = report[3] != 0;
        let mut vibration = XInputVibration {
            w_left_motor_speed: u16::from(rumble_left) * 257,
            w_right_motor_speed: if rumble_right { 65535 } else { 0 },
        };
        let rc = self.xinput.set_state(self.slot, &mut vibration);
        if rc != ERROR_SUCCESS {
            return Err(PlatformError::io("XInputSetState", std::io::Error::from_raw_os_error(rc as i32)));
        }
        Ok(())
    }

    pub fn get_feature(
        &mut self,
        _report_id: u8,
        _len: usize,
    ) -> Result<Vec<u8>, PlatformError> {
        Err(PlatformError::Unsupported {
            os: std::env::consts::OS,
        })
    }

    pub fn set_feature(&mut self, _report: &[u8]) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported {
            os: std::env::consts::OS,
        })
    }
}

/// Discovers every XInput slot with a connected controller.
pub fn discover() -> Result<Vec<DeviceInfo>, PlatformError> {
    let xinput = XInput::new()?;
    let mut devices = Vec::new();
    for slot in 0..XINPUT_SLOTS {
        let mut state = XInputState::default();
        if xinput.get_state(slot, &mut state) == ERROR_SUCCESS {
            devices.push(DeviceInfo {
                path: format!("XInput slot {slot}"),
                name: "XInput Controller (DsHidMini)".to_string(),
                vendor_id: SONY_VENDOR_ID,
                product_id: DS3_PRODUCT_ID,
                connection: Connection::Usb,
                serial: None,
                phys: None,
            });
        }
    }
    Ok(devices)
}

/// Parses the XInput slot index out of a device path like `"XInput slot 3"`.
fn slot_from_path(path: &str) -> Option<u32> {
    let suffix = path.strip_prefix("XInput slot ")?;
    suffix.trim().parse::<u32>().ok()
}

/// Converts an XINPUT_GAMEPAD into a 49-byte DualShock 3 input report so the
/// shared `InputReport::parse` can decode it.
fn build_ds3_report(gamepad: &XInputGamepad) -> Vec<u8> {
    let mut report = vec![0u8; ps3pad_core::input::INPUT_REPORT_LEN];
    report[0] = ps3pad_core::input::INPUT_REPORT_ID;

    let buttons = gamepad.w_buttons;

    // Byte 2: SELECT L3 R3 START UP RIGHT DOWN LEFT
    let mut b2: u8 = 0;
    if buttons & XINPUT_BUTTON_BACK != 0 {
        b2 |= 0x01;
    }
    if buttons & XINPUT_BUTTON_LEFT_THUMB != 0 {
        b2 |= 0x02;
    }
    if buttons & XINPUT_BUTTON_RIGHT_THUMB != 0 {
        b2 |= 0x04;
    }
    if buttons & XINPUT_BUTTON_START != 0 {
        b2 |= 0x08;
    }
    if buttons & XINPUT_BUTTON_DPAD_UP != 0 {
        b2 |= 0x10;
    }
    if buttons & XINPUT_BUTTON_DPAD_RIGHT != 0 {
        b2 |= 0x20;
    }
    if buttons & XINPUT_BUTTON_DPAD_DOWN != 0 {
        b2 |= 0x40;
    }
    if buttons & XINPUT_BUTTON_DPAD_LEFT != 0 {
        b2 |= 0x80;
    }
    report[2] = b2;

    // Byte 3: L1 R1 TRIANGLE CIRCLE CROSS SQUARE
    let mut b3: u8 = 0;
    if buttons & XINPUT_BUTTON_LEFT_SHOULDER != 0 {
        b3 |= 0x04;
    }
    if buttons & XINPUT_BUTTON_RIGHT_SHOULDER != 0 {
        b3 |= 0x08;
    }
    if buttons & XINPUT_BUTTON_Y != 0 {
        b3 |= 0x10;
    }
    if buttons & XINPUT_BUTTON_B != 0 {
        b3 |= 0x20;
    }
    if buttons & XINPUT_BUTTON_A != 0 {
        b3 |= 0x40;
    }
    if buttons & XINPUT_BUTTON_X != 0 {
        b3 |= 0x80;
    }
    report[3] = b3;

    // Sticks: XInput sThumb* is -32768..=32767; DS3 raw is 0..=255 with 128 centre.
    let scale = |value: i16| -> u8 {
        let v = value as i32 + 32768;
        (v / 257).clamp(0, 255) as u8
    };
    report[6] = scale(gamepad.s_thumb_lx);
    report[7] = scale(gamepad.s_thumb_ly);
    report[8] = scale(gamepad.s_thumb_rx);
    report[9] = scale(gamepad.s_thumb_ry);

    // Triggers map to L2/R2 pressure (and digital buttons in parse).
    report[18] = gamepad.b_left_trigger;
    report[19] = gamepad.b_right_trigger;

    // Digital face/dpad buttons also get pressure so the GUI shows them pressed.
    let press = |bit: u16| -> u8 { if buttons & bit != 0 { 255 } else { 0 } };
    report[14] = press(XINPUT_BUTTON_DPAD_UP);
    report[15] = press(XINPUT_BUTTON_DPAD_RIGHT);
    report[16] = press(XINPUT_BUTTON_DPAD_DOWN);
    report[17] = press(XINPUT_BUTTON_DPAD_LEFT);
    report[20] = press(XINPUT_BUTTON_LEFT_SHOULDER);
    report[21] = press(XINPUT_BUTTON_RIGHT_SHOULDER);
    report[22] = press(XINPUT_BUTTON_Y);
    report[23] = press(XINPUT_BUTTON_B);
    report[24] = press(XINPUT_BUTTON_A);
    report[25] = press(XINPUT_BUTTON_X);

    // Battery: XInput doesn't report it; default to full/charging.
    report[30] = 0xef;

    report
}
