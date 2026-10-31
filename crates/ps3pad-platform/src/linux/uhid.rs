use std::fs::{File, OpenOptions};
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::time::Duration;

use crate::error::PlatformError;
use crate::virtual_pad::VirtualPadEvent;

use super::poll_fd;

const UHID_DESTROY: u32 = 1;
const UHID_START: u32 = 2;
const UHID_STOP: u32 = 3;
const UHID_OPEN: u32 = 4;
const UHID_CLOSE: u32 = 5;
const UHID_OUTPUT: u32 = 6;
const UHID_GET_REPORT: u32 = 9;
const UHID_GET_REPORT_REPLY: u32 = 10;
const UHID_CREATE2: u32 = 11;
const UHID_INPUT2: u32 = 12;
const UHID_SET_REPORT: u32 = 13;
const UHID_SET_REPORT_REPLY: u32 = 14;

const UHID_DATA_MAX: usize = 4096;
const EVENT_MAX: usize = 4380;
const CREATE2_HEADER: usize = 280;

/// A virtual HID device backed by `/dev/uhid`.
pub struct VirtualPad {
    file: File,
}

impl VirtualPad {
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        name: &str,
        bus: u16,
        vendor_id: u16,
        product_id: u16,
        version: u32,
        country: u32,
        descriptor: &[u8],
    ) -> Result<Self, PlatformError> {
        if descriptor.is_empty() || descriptor.len() > UHID_DATA_MAX {
            return Err(PlatformError::message(
                "invalid HID report descriptor length",
            ));
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_CLOEXEC)
            .open("/dev/uhid")
            .map_err(|error| PlatformError::io("cannot open /dev/uhid", error))?;

        let mut event = vec![0u8; CREATE2_HEADER + descriptor.len()];
        put_u32(&mut event, 0, UHID_CREATE2);
        put_cstring(&mut event[4..132], name);
        put_cstring(&mut event[132..196], "ps3pad/virtual");
        put_cstring(&mut event[196..260], "");
        put_u16(&mut event, 260, descriptor.len() as u16);
        put_u16(&mut event, 262, bus);
        put_u32(&mut event, 264, u32::from(vendor_id));
        put_u32(&mut event, 268, u32::from(product_id));
        put_u32(&mut event, 272, version);
        put_u32(&mut event, 276, country);
        event[CREATE2_HEADER..].copy_from_slice(descriptor);

        let pad = Self { file };
        pad.write_event(&event)?;
        Ok(pad)
    }

    pub fn send_input(&mut self, report: &[u8]) -> Result<(), PlatformError> {
        if report.is_empty() || report.len() > UHID_DATA_MAX {
            return Err(PlatformError::message(
                "invalid virtual input report length",
            ));
        }
        let mut event = vec![0u8; 6 + report.len()];
        put_u32(&mut event, 0, UHID_INPUT2);
        put_u16(&mut event, 4, report.len() as u16);
        event[6..].copy_from_slice(report);
        self.write_event(&event)
    }

    pub fn poll(&mut self, timeout: Duration) -> Result<Option<VirtualPadEvent>, PlatformError> {
        if !poll_fd(self.file.as_raw_fd(), libc::POLLIN, timeout)
            .map_err(|error| PlatformError::io("poll on /dev/uhid failed", error))?
        {
            return Ok(None);
        }

        let mut buffer = [0u8; EVENT_MAX];
        let rc = loop {
            let rc = unsafe {
                libc::read(
                    self.file.as_raw_fd(),
                    buffer.as_mut_ptr() as *mut libc::c_void,
                    buffer.len(),
                )
            };
            if rc < 0 {
                let error = io::Error::last_os_error();
                match error.kind() {
                    io::ErrorKind::Interrupted => continue,
                    io::ErrorKind::WouldBlock => return Ok(None),
                    _ => return Err(PlatformError::io("read from /dev/uhid failed", error)),
                }
            }
            break rc as usize;
        };
        if rc == 0 {
            return Err(PlatformError::Disconnected);
        }

        let event = &buffer[..rc];
        let event_type = get_u32(event, 0)
            .ok_or_else(|| PlatformError::message("truncated event from /dev/uhid"))?;

        let parsed = match event_type {
            UHID_DESTROY => VirtualPadEvent::Destroy,
            UHID_START => VirtualPadEvent::Start {
                dev_flags: get_u64(event, 4).unwrap_or(0),
            },
            UHID_STOP => VirtualPadEvent::Stop,
            UHID_OPEN => VirtualPadEvent::Open,
            UHID_CLOSE => VirtualPadEvent::Close,
            UHID_OUTPUT => {
                let size = get_u16(event, 4 + UHID_DATA_MAX).unwrap_or(0) as usize;
                let size = size.min(UHID_DATA_MAX);
                let report_type = event.get(4 + UHID_DATA_MAX + 2).copied().unwrap_or(0);
                let data = event
                    .get(4..4 + size)
                    .map(|slice| slice.to_vec())
                    .unwrap_or_default();
                VirtualPadEvent::Output { report_type, data }
            }
            UHID_GET_REPORT => VirtualPadEvent::GetReport {
                id: get_u32(event, 4).unwrap_or(0),
                number: event.get(8).copied().unwrap_or(0),
                report_type: event.get(9).copied().unwrap_or(0),
            },
            UHID_SET_REPORT => {
                let size = get_u16(event, 10).unwrap_or(0) as usize;
                let size = size.min(UHID_DATA_MAX);
                VirtualPadEvent::SetReport {
                    id: get_u32(event, 4).unwrap_or(0),
                    number: event.get(8).copied().unwrap_or(0),
                    report_type: event.get(9).copied().unwrap_or(0),
                    data: event
                        .get(12..12 + size)
                        .map(|slice| slice.to_vec())
                        .unwrap_or_default(),
                }
            }
            _ => return self.poll(Duration::ZERO),
        };

        Ok(Some(parsed))
    }

    pub fn reply_get_report(
        &mut self,
        id: u32,
        _number: u8,
        _report_type: u8,
        data: &[u8],
    ) -> Result<(), PlatformError> {
        let data = &data[..data.len().min(UHID_DATA_MAX)];
        let mut event = vec![0u8; 12 + data.len()];
        put_u32(&mut event, 0, UHID_GET_REPORT_REPLY);
        put_u32(&mut event, 4, id);
        put_u16(&mut event, 8, 0);
        put_u16(&mut event, 10, data.len() as u16);
        event[12..].copy_from_slice(data);
        self.write_event(&event)
    }

    pub fn reply_set_report(&mut self, id: u32, error: u16) -> Result<(), PlatformError> {
        let mut event = [0u8; 10];
        put_u32(&mut event, 0, UHID_SET_REPORT_REPLY);
        put_u32(&mut event, 4, id);
        put_u16(&mut event, 8, error);
        self.write_event(&event)
    }

    fn write_event(&self, event: &[u8]) -> Result<(), PlatformError> {
        let mut written = 0;
        while written < event.len() {
            let rc = unsafe {
                libc::write(
                    self.file.as_raw_fd(),
                    event[written..].as_ptr() as *const libc::c_void,
                    event.len() - written,
                )
            };
            if rc < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(PlatformError::io("write to /dev/uhid failed", error));
            }
            if rc == 0 {
                return Err(PlatformError::Disconnected);
            }
            written += rc as usize;
        }
        Ok(())
    }
}

impl Drop for VirtualPad {
    fn drop(&mut self) {
        let mut event = [0u8; 4];
        put_u32(&mut event, 0, UHID_DESTROY);
        let _ = self.write_event(&event);
    }
}

fn put_cstring(target: &mut [u8], value: &str) {
    let bytes = value.as_bytes();
    let len = bytes.len().min(target.len().saturating_sub(1));
    target[..len].copy_from_slice(&bytes[..len]);
}

fn put_u16(buffer: &mut [u8], offset: usize, value: u16) {
    buffer[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(buffer: &mut [u8], offset: usize, value: u32) {
    buffer[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn get_u16(buffer: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        buffer.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn get_u32(buffer: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        buffer.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn get_u64(buffer: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        buffer.get(offset..offset + 8)?.try_into().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create2_layout_matches_the_kernel_abi() {
        let descriptor = [0x05, 0x01, 0x09, 0x05, 0xc0];
        let mut event = vec![0u8; CREATE2_HEADER + descriptor.len()];
        put_u32(&mut event, 0, UHID_CREATE2);
        put_cstring(&mut event[4..132], "pad");
        put_u16(&mut event, 260, descriptor.len() as u16);
        assert_eq!(get_u32(&event, 0), Some(11));
        assert_eq!(get_u16(&event, 260), Some(5));
        assert_eq!(&event[4..8], b"pad\0");
    }

    #[test]
    fn parses_output_event() {
        let mut event = vec![0u8; 4 + UHID_DATA_MAX + 3];
        put_u32(&mut event, 0, UHID_OUTPUT);
        put_u16(&mut event, 4 + UHID_DATA_MAX, 9);
        event[4] = 1;
        event[5] = 0x40;
        event[4 + UHID_DATA_MAX + 2] = 2;

        let size = get_u16(&event, 4 + UHID_DATA_MAX).unwrap_or(0) as usize;
        assert_eq!(size, 9);
        assert_eq!(event.get(4 + UHID_DATA_MAX + 2), Some(&2));
    }
}
