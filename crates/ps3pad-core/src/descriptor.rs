use serde::{Deserialize, Serialize};

use crate::input::{InputReport, TRIGGER_DIGITAL_THRESHOLD};

/// HID report ID used by the virtual gamepad.
pub const GENERIC_GAMEPAD_REPORT_ID: u8 = 1;
/// Length of a virtual gamepad input report, including the report ID.
pub const GENERIC_GAMEPAD_INPUT_LEN: usize = 10;
/// Length of a virtual gamepad output report, including the report ID.
pub const GENERIC_GAMEPAD_OUTPUT_LEN: usize = 9;
/// Human-readable virtual device name.
pub const GENERIC_GAMEPAD_NAME: &str = "PS3Pad Virtual Gamepad";
/// Vendor ID used by the virtual gamepad. `0x1209` is the pid.codes open
/// hardware VID; obtain your own PID before shipping a product.
pub const VIRTUAL_VENDOR_ID: u16 = 0x1209;
/// Product ID used by the virtual gamepad.
pub const VIRTUAL_PRODUCT_ID: u16 = 0x0001;

/// HID report descriptor for a standard 16-button, 6-axis gamepad with a hat
/// switch and an 8-byte vendor output report (rumble + LEDs).
///
/// Input report layout (10 bytes including report ID):
/// `[id, buttons_lo, buttons_hi, x, y, z, rx, ry, rz, hat]`.
///
/// Button bits (1-based in HID terms, 0-based here):
/// 0 Cross, 1 Circle, 2 Square, 3 Triangle, 4 L1, 5 R1, 6 L2, 7 R2,
/// 8 Select, 9 Start, 10 L3, 11 R3, 12 PS.
pub const GENERIC_GAMEPAD_DESCRIPTOR: &[u8] = &[
    0x05, 0x01, // Usage Page (Generic Desktop)
    0x09, 0x05, // Usage (Game Pad)
    0xa1, 0x01, // Collection (Application)
    0x85, 0x01, //   Report ID (1)
    0x05, 0x09, //   Usage Page (Button)
    0x19, 0x01, //   Usage Minimum (Button 1)
    0x29, 0x10, //   Usage Maximum (Button 16)
    0x15, 0x00, //   Logical Minimum (0)
    0x25, 0x01, //   Logical Maximum (1)
    0x75, 0x01, //   Report Size (1)
    0x95, 0x10, //   Report Count (16)
    0x81, 0x02, //   Input (Data, Variable, Absolute)
    0x05, 0x01, //   Usage Page (Generic Desktop)
    0x09, 0x30, //   Usage (X)
    0x09, 0x31, //   Usage (Y)
    0x09, 0x32, //   Usage (Z)
    0x09, 0x33, //   Usage (Rx)
    0x09, 0x34, //   Usage (Ry)
    0x09, 0x35, //   Usage (Rz)
    0x15, 0x00, //   Logical Minimum (0)
    0x26, 0xff, 0x00, // Logical Maximum (255)
    0x75, 0x08, //   Report Size (8)
    0x95, 0x06, //   Report Count (6)
    0x81, 0x02, //   Input (Data, Variable, Absolute)
    0x09, 0x39, //   Usage (Hat switch)
    0x15, 0x00, //   Logical Minimum (0)
    0x25, 0x07, //   Logical Maximum (7)
    0x35, 0x00, //   Physical Minimum (0)
    0x46, 0x3b, 0x01, // Physical Maximum (315)
    0x65, 0x14, //   Unit (English Rotation: degrees)
    0x75, 0x04, //   Report Size (4)
    0x95, 0x01, //   Report Count (1)
    0x81, 0x42, //   Input (Data, Variable, Absolute, Null State)
    0x75, 0x04, //   Report Size (4)
    0x95, 0x01, //   Report Count (1)
    0x81, 0x03, //   Input (Constant, Variable, Absolute)
    0x06, 0x00, 0xff, // Usage Page (Vendor Defined 0xFF00)
    0x09, 0x01, //   Usage (0x01)
    0x75, 0x08, //   Report Size (8)
    0x95, 0x08, //   Report Count (8)
    0x91, 0x02, //   Output (Data, Variable, Absolute)
    0xc0, // End Collection
];

/// Returns the generic virtual gamepad report descriptor.
pub const fn generic_gamepad_descriptor() -> &'static [u8] {
    GENERIC_GAMEPAD_DESCRIPTOR
}

/// State of the virtual gamepad, already in HID units.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VirtualPadState {
    pub buttons: u16,
    pub lx: u8,
    pub ly: u8,
    pub rx: u8,
    pub ry: u8,
    pub l2: u8,
    pub r2: u8,
    pub hat: u8,
}

impl Default for VirtualPadState {
    fn default() -> Self {
        Self {
            buttons: 0,
            lx: 128,
            ly: 128,
            rx: 128,
            ry: 128,
            l2: 0,
            r2: 0,
            hat: 8,
        }
    }
}

impl VirtualPadState {
    /// Maps a decoded DualShock 3 report to the virtual gamepad.
    pub fn from_input(input: &InputReport) -> Self {
        Self::from_input_with_threshold(input, TRIGGER_DIGITAL_THRESHOLD)
    }

    /// Same as [`VirtualPadState::from_input`] with a custom digital trigger
    /// threshold.
    pub fn from_input_with_threshold(input: &InputReport, threshold: u8) -> Self {
        use crate::Button;

        const MAPPING: [(Button, u16); 11] = [
            (Button::Cross, 0),
            (Button::Circle, 1),
            (Button::Square, 2),
            (Button::Triangle, 3),
            (Button::L1, 4),
            (Button::R1, 5),
            (Button::Select, 8),
            (Button::Start, 9),
            (Button::L3, 10),
            (Button::R3, 11),
            (Button::Ps, 12),
        ];

        let mut buttons = 0u16;
        for (button, bit) in MAPPING {
            if input.buttons.contains(button.bit()) {
                buttons |= 1 << bit;
            }
        }
        if input.pressure.l2 >= threshold {
            buttons |= 1 << 6;
        }
        if input.pressure.r2 >= threshold {
            buttons |= 1 << 7;
        }

        Self {
            buttons,
            lx: input.left_stick.x,
            ly: input.left_stick.y,
            rx: input.right_stick.x,
            ry: input.right_stick.y,
            l2: input.pressure.l2,
            r2: input.pressure.r2,
            hat: input.dpad.hat(),
        }
    }

    /// Serialises the full 10-byte input report, report ID included.
    pub fn to_report(self) -> [u8; GENERIC_GAMEPAD_INPUT_LEN] {
        [
            GENERIC_GAMEPAD_REPORT_ID,
            (self.buttons & 0xff) as u8,
            (self.buttons >> 8) as u8,
            self.lx,
            self.ly,
            self.l2,
            self.rx,
            self.ry,
            self.r2,
            self.hat,
        ]
    }

    /// Parses an 8-byte vendor output payload (without report ID).
    pub fn output_from_payload(payload: &[u8]) -> Option<crate::OutputReport> {
        crate::output::parse_virtual_output(payload)
    }
}
