use serde::{Deserialize, Serialize};

use crate::accel::AccelSample;
use crate::buttons::{Button, Buttons};
use crate::dpad::DPad;
use crate::error::ParseError;

/// Every DualShock 3 input report starts with this report ID.
pub const INPUT_REPORT_ID: u8 = 0x01;
/// Length of a full DualShock 3 input report.
pub const INPUT_REPORT_LEN: usize = 49;

/// Offset of the battery charge byte.
pub const BATTERY_OFFSET: usize = 30;
/// Offset of the first accelerometer byte (X, big endian).
pub const ACCEL_X_OFFSET: usize = 41;
/// Offset of the Z accelerometer byte (big endian).
pub const ACCEL_Z_OFFSET: usize = 43;
/// Offset of the Y accelerometer byte (big endian).
pub const ACCEL_Y_OFFSET: usize = 45;
/// Offset of the experimental single-axis gyro (big endian).
pub const GYRO_OFFSET: usize = 47;

/// Digital trigger threshold applied to the analog L2/R2 values when building
/// a virtual gamepad. The raw analog values are always preserved.
pub const TRIGGER_DIGITAL_THRESHOLD: u8 = 27;

/// Transport used by the controller for the current connection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Connection {
    Usb,
    Bluetooth,
    #[default]
    Unknown,
}

impl Connection {
    /// Maps a Linux `bus` value (`BUS_USB`, `BUS_BLUETOOTH`, ...) to a
    /// connection kind.
    pub const fn from_linux_bus(bus: u16) -> Self {
        match bus {
            crate::BUS_USB => Self::Usb,
            crate::BUS_BLUETOOTH => Self::Bluetooth,
            _ => Self::Unknown,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Usb => "USB",
            Self::Bluetooth => "Bluetooth",
            Self::Unknown => "Unknown",
        }
    }

    pub const fn is_wireless(self) -> bool {
        matches!(self, Self::Bluetooth)
    }
}

/// Raw and normalised state of one analog stick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stick {
    /// Raw X value, `0` = left, `128` = centre, `255` = right.
    pub x: u8,
    /// Raw Y value, `0` = up, `128` = centre, `255` = down.
    pub y: u8,
}

impl Stick {
    pub const CENTER: Self = Self { x: 128, y: 128 };

    #[inline]
    pub const fn is_centered(self) -> bool {
        self.x == 128 && self.y == 128
    }

    /// Normalised `(x, y)` in `[-1, 1]`, positive Y pointing up. Useful for a
    /// UI; the virtual HID device uses the raw bytes instead.
    pub fn normalized(self) -> (f32, f32) {
        (
            (self.x as f32 - 128.0) / 127.0,
            (127.0 - self.y as f32) / 127.0,
        )
    }

    /// Normalised values with a radial dead zone in `[0, 1)`.
    pub fn normalized_deadzone(self, deadzone: f32) -> (f32, f32) {
        let (x, y) = self.normalized();
        let magnitude = (x * x + y * y).sqrt();
        if magnitude <= deadzone || magnitude == 0.0 {
            return (0.0, 0.0);
        }
        let scale = ((magnitude - deadzone) / (1.0 - deadzone)) / magnitude;
        (x * scale, y * scale)
    }
}

/// Analog pressure values reported for the buttons that are pressure
/// sensitive on a DualShock 3. `0` is released, `255` is fully pressed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pressure {
    pub up: u8,
    pub right: u8,
    pub down: u8,
    pub left: u8,
    pub l1: u8,
    pub r1: u8,
    pub l2: u8,
    pub r2: u8,
    pub triangle: u8,
    pub circle: u8,
    pub cross: u8,
    pub square: u8,
}

impl Pressure {
    /// Analog value for a button that has a pressure sensor, or `None` for
    /// buttons that are purely digital.
    pub const fn get(self, button: Button) -> Option<u8> {
        Some(match button {
            Button::Up => self.up,
            Button::Right => self.right,
            Button::Down => self.down,
            Button::Left => self.left,
            Button::L1 => self.l1,
            Button::R1 => self.r1,
            Button::L2 => self.l2,
            Button::R2 => self.r2,
            Button::Triangle => self.triangle,
            Button::Circle => self.circle,
            Button::Cross => self.cross,
            Button::Square => self.square,
            _ => return None,
        })
    }

    /// Maximum pressure across all analog buttons; handy for a summary bar.
    pub fn max(self) -> u8 {
        [
            self.up,
            self.right,
            self.down,
            self.left,
            self.l1,
            self.r1,
            self.l2,
            self.r2,
            self.triangle,
            self.circle,
            self.cross,
            self.square,
        ]
        .into_iter()
        .max()
        .unwrap_or(0)
    }
}

/// Battery state reported by the controller.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatteryState {
    #[default]
    Unknown,
    Discharging,
    Charging,
    Full,
}

/// Decoded battery information.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Battery {
    /// Raw battery byte from the report.
    pub raw: u8,
    /// Approximate charge percentage (`0..=100`).
    pub percent: u8,
    pub state: BatteryState,
}

impl Battery {
    /// Decodes the battery byte. The DualShock 3 reports a five-step charge
    /// level while discharging, `0xee` while charging and `0xef` when full.
    pub const fn from_raw(raw: u8) -> Self {
        const LEVELS: [u8; 6] = [0, 1, 25, 50, 75, 100];

        match raw {
            0xee => Self {
                raw,
                percent: 100,
                state: BatteryState::Charging,
            },
            0xef => Self {
                raw,
                percent: 100,
                state: BatteryState::Full,
            },
            0..=5 => Self {
                raw,
                percent: LEVELS[raw as usize],
                state: BatteryState::Discharging,
            },
            _ => Self {
                raw,
                percent: 0,
                state: BatteryState::Unknown,
            },
        }
    }

    pub const fn is_charging(self) -> bool {
        matches!(self.state, BatteryState::Charging | BatteryState::Full)
    }
}

/// A fully decoded DualShock 3 input report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputReport {
    /// The exact bytes received from the controller.
    pub raw: Vec<u8>,
    pub connection: Connection,
    pub buttons: Buttons,
    pub dpad: DPad,
    pub left_stick: Stick,
    pub right_stick: Stick,
    pub pressure: Pressure,
    pub battery: Battery,
    pub accel: AccelSample,
    /// Experimental single-axis gyro, raw counts.
    pub gyro: i16,
}

impl InputReport {
    /// Parses a 49-byte report received from a DualShock 3.
    pub fn parse(report: &[u8], connection: Connection) -> Result<Self, ParseError> {
        if report.len() != INPUT_REPORT_LEN {
            return Err(ParseError::WrongLength {
                expected: INPUT_REPORT_LEN,
                actual: report.len(),
            });
        }
        if report[0] != INPUT_REPORT_ID {
            return Err(ParseError::NotInputReport(report[0]));
        }
        // The controller occasionally emits an all-zero report with 0xff in
        // byte 1 over Bluetooth. It must not generate input events.
        if report[1] == 0xff {
            return Err(ParseError::InvalidBluetoothReport);
        }

        let mut buttons = Buttons::NONE;
        let b2 = report[2];
        if b2 & 0x01 != 0 {
            buttons.insert(Buttons::SELECT);
        }
        if b2 & 0x02 != 0 {
            buttons.insert(Buttons::L3);
        }
        if b2 & 0x04 != 0 {
            buttons.insert(Buttons::R3);
        }
        if b2 & 0x08 != 0 {
            buttons.insert(Buttons::START);
        }
        if b2 & 0x10 != 0 {
            buttons.insert(Buttons::UP);
        }
        if b2 & 0x20 != 0 {
            buttons.insert(Buttons::RIGHT);
        }
        if b2 & 0x40 != 0 {
            buttons.insert(Buttons::DOWN);
        }
        if b2 & 0x80 != 0 {
            buttons.insert(Buttons::LEFT);
        }

        let b3 = report[3];
        if b3 & 0x04 != 0 {
            buttons.insert(Buttons::L1);
        }
        if b3 & 0x08 != 0 {
            buttons.insert(Buttons::R1);
        }
        if b3 & 0x10 != 0 {
            buttons.insert(Buttons::TRIANGLE);
        }
        if b3 & 0x20 != 0 {
            buttons.insert(Buttons::CIRCLE);
        }
        if b3 & 0x40 != 0 {
            buttons.insert(Buttons::CROSS);
        }
        if b3 & 0x80 != 0 {
            buttons.insert(Buttons::SQUARE);
        }

        if report[4] & 0x01 != 0 {
            buttons.insert(Buttons::PS);
        }

        let pressure = Pressure {
            up: report[14],
            right: report[15],
            down: report[16],
            left: report[17],
            l1: report[20],
            r1: report[21],
            l2: report[18],
            r2: report[19],
            triangle: report[22],
            circle: report[23],
            cross: report[24],
            square: report[25],
        };

        // L2/R2 are analog-only in the raw report. Expose a digital button as
        // well when the trigger is meaningfully pressed.
        if pressure.l2 >= TRIGGER_DIGITAL_THRESHOLD {
            buttons.insert(Buttons::L2);
        }
        if pressure.r2 >= TRIGGER_DIGITAL_THRESHOLD {
            buttons.insert(Buttons::R2);
        }

        let accel = AccelSample {
            x: be_u16(report, ACCEL_X_OFFSET) as i16,
            z: be_u16(report, ACCEL_Z_OFFSET) as i16,
            y: be_u16(report, ACCEL_Y_OFFSET) as i16,
        };
        let gyro = i16::from_be_bytes([report[GYRO_OFFSET], report[GYRO_OFFSET + 1]]);

        Ok(Self {
            raw: report.to_vec(),
            connection,
            buttons,
            dpad: DPad::from_buttons(buttons),
            left_stick: Stick {
                x: report[6],
                y: report[7],
            },
            right_stick: Stick {
                x: report[8],
                y: report[9],
            },
            pressure,
            battery: Battery::from_raw(report[BATTERY_OFFSET]),
            accel,
            gyro,
        })
    }

    /// Convenience wrapper for [`InputReport::parse`].
    pub fn from_bytes(report: &[u8], connection: Connection) -> Result<Self, ParseError> {
        Self::parse(report, connection)
    }

    pub fn raw_hex(&self) -> String {
        let mut out = String::with_capacity(self.raw.len() * 3);
        for (index, byte) in self.raw.iter().enumerate() {
            if index != 0 {
                out.push(' ');
            }
            use std::fmt::Write as _;
            let _ = write!(out, "{byte:02x}");
        }
        out
    }
}

#[inline]
fn be_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([bytes[offset], bytes[offset + 1]])
}

/// Free-function form of [`InputReport::parse`].
pub fn parse(report: &[u8], connection: Connection) -> Result<InputReport, ParseError> {
    InputReport::parse(report, connection)
}
