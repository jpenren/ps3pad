use thiserror::Error;

/// Errors returned while decoding an input report.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ParseError {
    #[error("expected a {expected}-byte report, got {actual} bytes")]
    WrongLength { expected: usize, actual: usize },

    #[error("not a DualShock 3 input report (report id 0x{0:02x})")]
    NotInputReport(u8),

    #[error("ignored a known-invalid Bluetooth report (byte 1 == 0xff)")]
    InvalidBluetoothReport,
}
