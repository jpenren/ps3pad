//! Windows backend: XInput for physical controllers (via DsHidMini).
//! No virtual gamepad is created because DsHidMini already exposes a genuine
//! XInput device that games recognise natively.

mod xinput;

pub use xinput::{Controller, discover};
