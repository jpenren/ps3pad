//! Device discovery and controller access.

use std::time::Duration;

use ps3pad_core::{Connection, InputReport, OutputReport};

use crate::error::PlatformError;

/// A physical controller discovered on the system.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Opaque platform path used to open the device.
    pub path: String,
    /// Device product name as reported by the OS.
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub connection: Connection,
    /// Bluetooth address or USB serial, when the OS exposes one.
    pub serial: Option<String>,
    /// Physical topology hint (`usb-0000:00:14.0-1/input3` on Linux).
    pub phys: Option<String>,
}

impl DeviceInfo {
    pub fn is_bluetooth(&self) -> bool {
        self.connection.is_wireless()
    }
}

/// Extra information obtained while initialising the controller.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InitInfo {
    /// Bluetooth address of the controller itself, read from the USB
    /// calibration report when available.
    pub device_mac: Option<[u8; 6]>,
    /// Feature reports that were successfully exchanged.
    pub feature_reports: Vec<u8>,
}

/// An open DualShock 3.
pub struct Controller {
    inner: imp::Controller,
    info: DeviceInfo,
}

impl Controller {
    /// Discovers every supported controller.
    pub fn discover() -> Result<Vec<DeviceInfo>, PlatformError> {
        imp::discover()
    }

    /// Opens the first supported controller.
    pub fn open_first() -> Result<Self, PlatformError> {
        let info = Self::discover()?
            .into_iter()
            .next()
            .ok_or(PlatformError::NoSupportedDevice)?;
        Self::open(&info)
    }

    /// Opens a controller previously returned by [`Controller::discover`].
    pub fn open(info: &DeviceInfo) -> Result<Self, PlatformError> {
        let inner = imp::Controller::open(info)?;
        Ok(Self {
            inner,
            info: info.clone(),
        })
    }

    pub fn info(&self) -> &DeviceInfo {
        &self.info
    }

    /// Puts the controller into full report mode.
    ///
    /// Must be called once after opening. USB controllers are switched to
    /// operational mode with feature reports `0xf2` and `0xf5`; Bluetooth
    /// controllers are switched with feature report `0xf4`.
    pub fn initialize(&mut self) -> Result<InitInfo, PlatformError> {
        self.inner.initialize()
    }

    /// Reads one input report, waiting at most `timeout`.
    ///
    /// Returns `Ok(None)` when the timeout expires.
    pub fn read(&mut self, timeout: Duration) -> Result<Option<InputReport>, PlatformError> {
        let connection = self.info.connection;
        match self.read_raw(timeout)? {
            Some(report) => Ok(Some(InputReport::parse(&report, connection)?)),
            None => Ok(None),
        }
    }

    /// Reads one raw report, waiting at most `timeout`.
    pub fn read_raw(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, PlatformError> {
        self.inner.read_raw(timeout)
    }

    /// Sends an LED/rumble output report.
    pub fn send_output(&mut self, output: &OutputReport) -> Result<(), PlatformError> {
        self.inner.write_output(&output.to_bytes())
    }

    /// Sends an arbitrary output report, report ID included.
    pub fn write_output(&mut self, report: &[u8]) -> Result<(), PlatformError> {
        self.inner.write_output(report)
    }

    /// Reads a feature report. `len` includes the report ID byte.
    pub fn get_feature(&mut self, report_id: u8, len: usize) -> Result<Vec<u8>, PlatformError> {
        self.inner.get_feature(report_id, len)
    }

    /// Writes a feature report. The first byte must be the report ID.
    pub fn set_feature(&mut self, report: &[u8]) -> Result<(), PlatformError> {
        self.inner.set_feature(report)
    }

    /// Returns the controller's own Bluetooth address, when known.
    pub fn device_mac(&mut self) -> Result<Option<[u8; 6]>, PlatformError> {
        if self.info.connection != Connection::Usb {
            return Ok(None);
        }
        let report = self.get_feature(0xf2, 17)?;
        if report.len() < 10 {
            return Ok(None);
        }
        // The address starts at offset 4 and is stored big-endian.
        let mut mac = [0u8; 6];
        for (index, byte) in report[4..10].iter().enumerate() {
            mac[5 - index] = *byte;
        }
        Ok(Some(mac))
    }
}

#[cfg(target_os = "linux")]
mod imp {
    pub use crate::linux::{discover, Controller};
}

#[cfg(not(target_os = "linux"))]
mod imp {
    use std::time::Duration;

    use ps3pad_core::InputReport;

    use crate::device::{DeviceInfo, InitInfo};
    use crate::error::PlatformError;

    pub struct Controller;

    impl Controller {
        pub fn open(_info: &DeviceInfo) -> Result<Self, PlatformError> {
            Err(PlatformError::Unsupported {
                os: std::env::consts::OS,
            })
        }

        pub fn initialize(&mut self) -> Result<InitInfo, PlatformError> {
            Err(PlatformError::Unsupported {
                os: std::env::consts::OS,
            })
        }

        pub fn read_raw(&mut self, _timeout: Duration) -> Result<Option<Vec<u8>>, PlatformError> {
            Err(PlatformError::Unsupported {
                os: std::env::consts::OS,
            })
        }

        pub fn write_output(&mut self, _report: &[u8]) -> Result<(), PlatformError> {
            Err(PlatformError::Unsupported {
                os: std::env::consts::OS,
            })
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

    pub fn discover() -> Result<Vec<DeviceInfo>, PlatformError> {
        Err(PlatformError::Unsupported {
            os: std::env::consts::OS,
        })
    }

    #[allow(dead_code)]
    fn _assert_input_report_is_used(_: InputReport) {}
}
