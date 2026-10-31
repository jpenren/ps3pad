use serde::{Deserialize, Serialize};

/// Report ID of the DualShock 3 output (LED/rumble) report.
pub const OUTPUT_REPORT_ID: u8 = 0x01;
/// Length of the output report as accepted by the controller over USB and
/// Bluetooth. Only the first 36 bytes are meaningful; the rest is padding.
pub const OUTPUT_REPORT_LEN: usize = 49;

/// Length of the calibration/status feature report `0xf2`.
pub const FEATURE_REPORT_0F2_LEN: usize = 17;
/// Length of the pairing/Bluetooth address feature report `0xf5`.
pub const FEATURE_REPORT_0F5_LEN: usize = 8;

/// Feature report used to put a USB-connected controller into full report mode.
/// This is the variant used by Sony's own driver and by the USB Host Shield
/// reference implementation.
pub const FEATURE_ENABLE_USB: [u8; 5] = [0xf4, 0x42, 0x0c, 0x00, 0x00];
/// Feature report used to put a Bluetooth-connected controller into full report
/// mode. The kernel and SDL use `0x03` as the second payload byte here.
pub const FEATURE_ENABLE_BT: [u8; 5] = [0xf4, 0x42, 0x03, 0x00, 0x00];

/// A blinking pattern for one player LED.
///
/// The controller's LED groups work in tenths of a second. Values are clamped
/// to 25.5 s, which is the maximum the hardware can express.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedBlink {
    pub on_ms: u16,
    pub off_ms: u16,
}

impl LedBlink {
    pub const fn new(on_ms: u16, off_ms: u16) -> Self {
        Self { on_ms, off_ms }
    }

    pub const fn slow() -> Self {
        Self {
            on_ms: 1_000,
            off_ms: 1_000,
        }
    }

    pub const fn fast() -> Self {
        Self {
            on_ms: 200,
            off_ms: 200,
        }
    }
}

/// Desired LED and rumble state.
///
/// `leds` is a 4-bit mask where bit 0 is the leftmost LED (player 1). The
/// controller itself uses bits 1..4 of the output report, so the conversion is
/// handled by [`OutputReport::to_bytes`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputReport {
    pub leds: u8,
    pub blink: [Option<LedBlink>; 4],
    /// Large/left motor, `0..=255`.
    pub rumble_left: u8,
    /// Small/right motor, on or off.
    pub rumble_right: bool,
}

impl OutputReport {
    /// Lights exactly one player LED (`player` is `1..=4`). Out-of-range values
    /// clear all LEDs.
    pub fn with_player_led(mut self, player: u8) -> Self {
        self.leds = if (1..=4).contains(&player) {
            1 << (player - 1)
        } else {
            0
        };
        self
    }

    pub fn set_player_led(&mut self, player: u8) {
        *self = self.with_player_led(player);
    }

    pub fn set_led(&mut self, led: u8, on: bool) {
        if led < 4 {
            if on {
                self.leds |= 1 << led;
            } else {
                self.leds &= !(1 << led);
            }
            self.leds &= 0x0f;
        }
    }

    pub fn led(&self, led: u8) -> bool {
        led < 4 && self.leds & (1 << led) != 0
    }

    /// Serialises the 49-byte HID output report.
    pub fn to_bytes(&self) -> [u8; OUTPUT_REPORT_LEN] {
        let mut report = DEFAULT_OUTPUT_REPORT;

        // Right (small) motor: duration 0xff = until changed, then on/off.
        report[2] = 0xff;
        report[3] = u8::from(self.rumble_right);
        // Left (large) motor: duration and force.
        report[4] = 0xff;
        report[5] = self.rumble_left;

        // LED bitmap. A zero mask needs the "all LEDs off" bit set, otherwise
        // some third-party controllers keep the previous pattern.
        let mut bitmap = (self.leds & 0x0f) << 1;
        if bitmap & 0x1e == 0 {
            bitmap |= 0x20;
        }
        report[10] = bitmap;

        // The four LED timing groups are stored in reverse order: group 0 is
        // LED 4, group 3 is LED 1.
        for (index, blink) in self.blink.iter().enumerate() {
            if let Some(blink) = blink {
                let base = 11 + (3 - index) * 5;
                report[base] = 0xff; // enabled until changed
                report[base + 1] = 0x27; // 39 deciseconds cycle
                report[base + 2] = 0x10;
                report[base + 3] = (blink.off_ms as u32 / 100).min(255) as u8;
                report[base + 4] = (blink.on_ms as u32 / 100).min(255) as u8;
            }
        }

        report
    }
}

/// Baseline output report. Mirrors the values the Linux driver uses, but is an
/// independent constant.
const DEFAULT_OUTPUT_REPORT: [u8; OUTPUT_REPORT_LEN] = [
    0x01, // report ID
    0x01, 0xff, 0x00, 0xff, 0x00, // rumble (off) + flag
    0x00, 0x00, 0x00, 0x00, // reserved
    0x00, // LED bitmap (patched by to_bytes)
    0xff, 0x27, 0x10, 0x00, 0x32, // LED 4 timing
    0xff, 0x27, 0x10, 0x00, 0x32, // LED 3 timing
    0xff, 0x27, 0x10, 0x00, 0x32, // LED 2 timing
    0xff, 0x27, 0x10, 0x00, 0x32, // LED 1 timing
    0x00, 0x00, 0x00, 0x00, 0x00, // reserved
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, // padding to 49 bytes
];

/// Builds the feature report that stores the host Bluetooth address in the
/// controller. `mac` is in normal human order (`aa:bb:cc:dd:ee:ff`); the
/// controller expects it reversed in the report.
pub fn host_mac_report(mac: [u8; 6]) -> [u8; FEATURE_REPORT_0F5_LEN] {
    [0xf5, 0x01, mac[5], mac[4], mac[3], mac[2], mac[1], mac[0]]
}

/// Extracts the Bluetooth address from a `0xf5` feature report.
pub fn host_mac_from_report(report: &[u8]) -> Option<[u8; 6]> {
    if report.len() < FEATURE_REPORT_0F5_LEN {
        return None;
    }
    Some([
        report[7], report[6], report[5], report[4], report[3], report[2],
    ])
}

/// Parses the 8-byte payload of a vendor output report produced by a virtual
/// gamepad (see [`crate::descriptor`]). Returns the LED/rumble state.
///
/// Layout: `[left_force, right_on, led_mask, reserved; 5]`.
pub fn parse_virtual_output(payload: &[u8]) -> Option<OutputReport> {
    if payload.len() < 3 {
        return None;
    }
    Some(OutputReport {
        leds: payload[2] & 0x0f,
        blink: [None; 4],
        rumble_left: payload[0],
        rumble_right: payload[1] != 0,
    })
}

/// Serialises the 8-byte vendor output payload understood by
/// [`parse_virtual_output`].
pub fn virtual_output_payload(state: &OutputReport) -> [u8; 8] {
    [
        state.rumble_left,
        u8::from(state.rumble_right),
        state.leds & 0x0f,
        0,
        0,
        0,
        0,
        0,
    ]
}
