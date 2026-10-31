use std::fs::{File, OpenOptions};
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::time::Duration;

use ps3pad_core::{Connection, FEATURE_ENABLE_BT};

use crate::device::{DeviceInfo, InitInfo};
use crate::error::PlatformError;

use super::{last_error, poll_fd, HidrawDevinfo, HIDIOCGFEATURE, HIDIOCGRAWINFO, HIDIOCSFEATURE};

/// An open hidraw node.
pub struct Controller {
    file: File,
    connection: Connection,
}

impl Controller {
    pub fn open(info: &DeviceInfo) -> Result<Self, PlatformError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(&info.path)
            .map_err(|error| PlatformError::io("cannot open hidraw device", error))?;

        // Refresh the bus type from the node itself. This also verifies that
        // the path still points at the expected device.
        let mut devinfo = HidrawDevinfo::default();
        if unsafe { libc::ioctl(file.as_raw_fd(), HIDIOCGRAWINFO, &mut devinfo) } < 0 {
            return Err(last_error("HIDIOCGRAWINFO failed"));
        }
        let connection = Connection::from_linux_bus(devinfo.bustype as u16);

        Ok(Self { file, connection })
    }

    fn fd(&self) -> i32 {
        self.file.as_raw_fd()
    }

    pub fn initialize(&mut self) -> Result<InitInfo, PlatformError> {
        let mut info = InitInfo::default();

        match self.connection {
            Connection::Usb => {
                // Feature report 0xf2 is the calibration report and contains
                // the controller MAC at offset 4.
                let calibration = self.get_feature(0xf2, ps3pad_core::FEATURE_REPORT_0F2_LEN)?;
                info.feature_reports.push(0xf2);
                if calibration.len() >= 10 {
                    let mut mac = [0u8; 6];
                    for (index, byte) in calibration[4..10].iter().enumerate() {
                        mac[5 - index] = *byte;
                    }
                    info.device_mac = Some(mac);
                }

                // Reading 0xf5 is the second half of the operational-mode
                // handshake. Some third-party pads only react after the
                // following one-byte interrupt write.
                let pairing = self.get_feature(0xf5, ps3pad_core::FEATURE_REPORT_0F5_LEN)?;
                info.feature_reports.push(0xf5);
                if let Some(&first) = pairing.first() {
                    // Failure is not fatal: several clones dislike this step.
                    let _ = self.write_report(&[first]);
                }
            }
            Connection::Bluetooth => {
                self.set_feature(&FEATURE_ENABLE_BT)?;
                info.feature_reports.push(0xf4);
            }
            Connection::Unknown => {
                // Try the Bluetooth handshake; harmless on most devices.
                let _ = self.set_feature(&FEATURE_ENABLE_BT);
            }
        }

        Ok(info)
    }

    pub fn read_raw(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, PlatformError> {
        if !poll_fd(self.fd(), libc::POLLIN, timeout)
            .map_err(|error| PlatformError::io("poll on hidraw failed", error))?
        {
            return Ok(None);
        }

        let mut buffer = [0u8; 4096];
        loop {
            let rc = unsafe {
                libc::read(
                    self.fd(),
                    buffer.as_mut_ptr() as *mut libc::c_void,
                    buffer.len(),
                )
            };
            if rc < 0 {
                let error = io::Error::last_os_error();
                match error.kind() {
                    io::ErrorKind::Interrupted => continue,
                    io::ErrorKind::WouldBlock => return Ok(None),
                    _ => return Err(PlatformError::io("read from hidraw failed", error)),
                }
            }
            if rc == 0 {
                return Err(PlatformError::Disconnected);
            }
            return Ok(Some(buffer[..rc as usize].to_vec()));
        }
    }

    pub fn write_output(&mut self, report: &[u8]) -> Result<(), PlatformError> {
        if report.is_empty() {
            return Err(PlatformError::message("empty output report"));
        }
        self.write_report(report)
    }

    fn write_report(&mut self, report: &[u8]) -> Result<(), PlatformError> {
        if !poll_fd(self.fd(), libc::POLLOUT, Duration::from_millis(1_000))
            .map_err(|error| PlatformError::io("poll for write failed", error))?
        {
            return Err(PlatformError::message("timed out writing output report"));
        }

        loop {
            let rc = unsafe {
                libc::write(
                    self.fd(),
                    report.as_ptr() as *const libc::c_void,
                    report.len(),
                )
            };
            if rc < 0 {
                let error = io::Error::last_os_error();
                match error.kind() {
                    io::ErrorKind::Interrupted => continue,
                    io::ErrorKind::WouldBlock => {
                        if !poll_fd(self.fd(), libc::POLLOUT, Duration::from_millis(1_000))
                            .map_err(|error| PlatformError::io("poll for write failed", error))?
                        {
                            return Err(PlatformError::message("timed out writing output report"));
                        }
                        continue;
                    }
                    _ => return Err(PlatformError::io("write to hidraw failed", error)),
                }
            }
            if (rc as usize) != report.len() {
                return Err(PlatformError::message(format!(
                    "short hidraw write: {} of {} bytes",
                    rc,
                    report.len()
                )));
            }
            return Ok(());
        }
    }

    pub fn get_feature(&mut self, report_id: u8, len: usize) -> Result<Vec<u8>, PlatformError> {
        if len == 0 || len > 4096 {
            return Err(PlatformError::message("invalid feature report length"));
        }
        let mut buffer = vec![0u8; len];
        buffer[0] = report_id;
        let rc = unsafe {
            libc::ioctl(
                self.fd(),
                HIDIOCGFEATURE(len),
                buffer.as_mut_ptr() as *mut libc::c_void,
            )
        };
        if rc < 0 {
            return Err(last_error("HIDIOCGFEATURE failed"));
        }
        buffer.truncate((rc as usize).min(len));
        Ok(buffer)
    }

    pub fn set_feature(&mut self, report: &[u8]) -> Result<(), PlatformError> {
        if report.is_empty() || report.len() > 4096 {
            return Err(PlatformError::message("invalid feature report length"));
        }
        let rc = unsafe {
            libc::ioctl(
                self.fd(),
                HIDIOCSFEATURE(report.len()),
                report.as_ptr() as *const libc::c_void,
            )
        };
        if rc < 0 {
            return Err(last_error("HIDIOCSFEATURE failed"));
        }
        Ok(())
    }
}
