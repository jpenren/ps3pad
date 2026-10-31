//! Virtual gamepad creation.

use std::time::Duration;

use ps3pad_core::OutputReport;

use crate::error::PlatformError;

/// Event delivered by the operating system to a virtual HID device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VirtualPadEvent {
    /// The host started the device.
    Start { dev_flags: u64 },
    /// The host stopped the device.
    Stop,
    /// A host process opened the device.
    Open,
    /// A host process closed the device.
    Close,
    /// The host sent an output report (rumble / LEDs).
    Output { report_type: u8, data: Vec<u8> },
    /// The host asked for a feature report.
    GetReport {
        id: u32,
        number: u8,
        report_type: u8,
    },
    /// The host wrote a feature report.
    SetReport {
        id: u32,
        number: u8,
        report_type: u8,
        data: Vec<u8>,
    },
    /// The device was destroyed.
    Destroy,
}

impl VirtualPadEvent {
    /// Interprets an output event as an LED/rumble state.
    ///
    /// Accepts both numbered reports (`[0x01, ...]`) and raw payloads.
    pub fn output_state(&self) -> Option<OutputReport> {
        let Self::Output { data, .. } = self else {
            return None;
        };
        let payload = if data.len() >= 9 && data[0] == 1 {
            &data[1..]
        } else {
            data.as_slice()
        };
        ps3pad_core::output::parse_virtual_output(payload)
    }
}

/// A virtual HID gamepad supplied by the platform.
pub struct VirtualPad {
    inner: imp::VirtualPad,
}

impl VirtualPad {
    /// Creates the default generic gamepad described by `ps3pad-core`.
    pub fn create_default() -> Result<Self, PlatformError> {
        Self::create(
            ps3pad_core::descriptor::GENERIC_GAMEPAD_NAME,
            ps3pad_core::BUS_USB,
            ps3pad_core::VIRTUAL_VENDOR_ID,
            ps3pad_core::VIRTUAL_PRODUCT_ID,
            0x0100,
            0,
            ps3pad_core::generic_gamepad_descriptor(),
        )
    }

    /// Creates a virtual HID device with an explicit descriptor.
    pub fn create(
        name: &str,
        bus: u16,
        vendor_id: u16,
        product_id: u16,
        version: u32,
        country: u32,
        descriptor: &[u8],
    ) -> Result<Self, PlatformError> {
        let inner = imp::VirtualPad::create(
            name, bus, vendor_id, product_id, version, country, descriptor,
        )?;
        Ok(Self { inner })
    }

    /// Sends an input report. The report must include its report ID when the
    /// descriptor is numbered.
    pub fn send_input(&mut self, report: &[u8]) -> Result<(), PlatformError> {
        self.inner.send_input(report)
    }

    /// Waits for an event from the host.
    pub fn poll(&mut self, timeout: Duration) -> Result<Option<VirtualPadEvent>, PlatformError> {
        self.inner.poll(timeout)
    }

    /// Answers a feature report request.
    pub fn reply_get_report(
        &mut self,
        id: u32,
        number: u8,
        report_type: u8,
        data: &[u8],
    ) -> Result<(), PlatformError> {
        self.inner.reply_get_report(id, number, report_type, data)
    }

    /// Acknowledges a feature report write.
    pub fn reply_set_report(&mut self, id: u32, error: u16) -> Result<(), PlatformError> {
        self.inner.reply_set_report(id, error)
    }

    /// Handles report requests automatically and returns output events.
    ///
    /// Feature report requests are answered with an empty report, which is
    /// correct for the default descriptor.
    pub fn next_output(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<OutputReport>, PlatformError> {
        loop {
            let Some(event) = self.poll(timeout)? else {
                return Ok(None);
            };
            match event {
                VirtualPadEvent::GetReport {
                    id,
                    number,
                    report_type,
                } => self.inner.reply_get_report(id, number, report_type, &[])?,
                VirtualPadEvent::SetReport { id, .. } => self.inner.reply_set_report(id, 0)?,
                VirtualPadEvent::Destroy => return Err(PlatformError::Disconnected),
                other => {
                    if let Some(state) = other.output_state() {
                        return Ok(Some(state));
                    }
                }
            }
        }
    }
}

#[cfg(target_os = "linux")]
mod imp {
    pub use crate::linux::VirtualPad;
}

#[cfg(not(target_os = "linux"))]
mod imp {
    use std::time::Duration;

    use crate::error::PlatformError;

    pub struct VirtualPad;

    impl VirtualPad {
        pub fn create(
            _name: &str,
            _bus: u16,
            _vendor_id: u16,
            _product_id: u16,
            _version: u32,
            _country: u32,
            _descriptor: &[u8],
        ) -> Result<Self, PlatformError> {
            Err(PlatformError::Unsupported {
                os: std::env::consts::OS,
            })
        }

        pub fn send_input(&mut self, _report: &[u8]) -> Result<(), PlatformError> {
            Err(PlatformError::Unsupported {
                os: std::env::consts::OS,
            })
        }

        pub fn poll(
            &mut self,
            _timeout: Duration,
        ) -> Result<Option<crate::virtual_pad::VirtualPadEvent>, PlatformError> {
            Err(PlatformError::Unsupported {
                os: std::env::consts::OS,
            })
        }

        pub fn reply_get_report(
            &mut self,
            _id: u32,
            _number: u8,
            _report_type: u8,
            _data: &[u8],
        ) -> Result<(), PlatformError> {
            Err(PlatformError::Unsupported {
                os: std::env::consts::OS,
            })
        }

        pub fn reply_set_report(&mut self, _id: u32, _error: u16) -> Result<(), PlatformError> {
            Err(PlatformError::Unsupported {
                os: std::env::consts::OS,
            })
        }
    }
}
