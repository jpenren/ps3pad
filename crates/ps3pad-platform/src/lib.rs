//! Platform backends for `ps3pad`.
//!
//! The crate has two jobs:
//!
//! * open a physical DualShock 3 through the operating system's native HID
//!   API, without a runtime dependency such as hidapi or libudev;
//! * create a virtual HID gamepad through the operating system's native
//!   mechanism (`/dev/uhid` on Linux).
//!
//! Both halves are exposed through small, platform-neutral types so the CLI,
//! daemon and GUI share one code path.

pub mod device;
pub mod error;
pub mod virtual_pad;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "windows")]
mod windows;

pub use device::{Controller, DeviceInfo, InitInfo};
pub use error::PlatformError;
pub use virtual_pad::{VirtualPad, VirtualPadEvent};
