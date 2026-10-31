//! `ps3pad-core` — dependency-light implementation of the Sony DualShock 3 /
//! SIXAXIS wire protocol.
//!
//! The crate deliberately contains no platform code and no unsafe blocks. It is
//! shared by the Linux daemon, the Windows/macOS bridges and the egui tester so
//! that every platform decodes the exact same bytes in the exact same way.
//!
//! References used while writing this crate (facts only, independent code):
//! * USB HID 1.11 specification and the HID Usage Tables.
//! * Public PS3 `sixaxis` protocol notes (ps2dev wiki, USB Host Shield wiki).
//! * Linux `hid-sony.c` and SDL `SDL_hidapi_ps3.c` as behavioural references.
//! * DsHidMini documentation and research notes.
//!
//! See `docs/PROTOCOL.md` for the byte-level documentation and
//! `docs/REVERSE_ENGINEERING.md` for provenance.

pub mod accel;
pub mod buttons;
pub mod descriptor;
pub mod dpad;
pub mod error;
pub mod input;
pub mod output;

pub use accel::{AccelCalibration, AccelSample, Tilt};
pub use buttons::{Button, Buttons};
pub use descriptor::{
    generic_gamepad_descriptor, VirtualPadState, GENERIC_GAMEPAD_INPUT_LEN, GENERIC_GAMEPAD_NAME,
    GENERIC_GAMEPAD_OUTPUT_LEN, GENERIC_GAMEPAD_REPORT_ID, VIRTUAL_PRODUCT_ID, VIRTUAL_VENDOR_ID,
};
pub use dpad::DPad;
pub use error::ParseError;
pub use input::{
    parse, Battery, BatteryState, Connection, InputReport, Pressure, Stick, ACCEL_X_OFFSET,
    ACCEL_Y_OFFSET, ACCEL_Z_OFFSET, BATTERY_OFFSET, GYRO_OFFSET, INPUT_REPORT_ID, INPUT_REPORT_LEN,
};
pub use output::{
    host_mac_from_report, host_mac_report, OutputReport, FEATURE_ENABLE_BT, FEATURE_ENABLE_USB,
    FEATURE_REPORT_0F2_LEN, FEATURE_REPORT_0F5_LEN, OUTPUT_REPORT_ID, OUTPUT_REPORT_LEN,
};

/// Sony Interactive Entertainment USB vendor ID.
pub const SONY_VENDOR_ID: u16 = 0x054c;
/// DualShock 3 / SIXAXIS product ID (USB and Bluetooth).
pub const DS3_PRODUCT_ID: u16 = 0x0268;
/// PlayStation 3 Navigation Controller product ID.
pub const NAVIGATION_PRODUCT_ID: u16 = 0x0269;
/// PlayStation Move Motion Controller product ID.
pub const MOVE_PRODUCT_ID: u16 = 0x03d5;

/// USB bus type as reported by the Linux HID stack (`BUS_USB`).
pub const BUS_USB: u16 = 0x03;
/// Bluetooth bus type as reported by the Linux HID stack (`BUS_BLUETOOTH`).
pub const BUS_BLUETOOTH: u16 = 0x05;

/// Returns `true` for the device IDs this project understands.
pub fn is_supported(vendor: u16, product: u16) -> bool {
    vendor == SONY_VENDOR_ID
        && matches!(
            product,
            DS3_PRODUCT_ID | NAVIGATION_PRODUCT_ID | MOVE_PRODUCT_ID
        )
}
