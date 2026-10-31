//! Linux backend: hidraw for physical controllers and `/dev/uhid` for virtual
//! devices. No libudev, hidapi or other runtime library is used.

mod hidraw;
mod sysfs;
mod uhid;

pub use hidraw::Controller;
pub use sysfs::discover;
pub use uhid::VirtualPad;

use std::io;
use std::os::unix::io::RawFd;
use std::time::Duration;

const IOC_NRBITS: u32 = 8;
const IOC_TYPEBITS: u32 = 8;
const IOC_SIZEBITS: u32 = 14;
const IOC_NRSHIFT: u32 = 0;
const IOC_TYPESHIFT: u32 = IOC_NRSHIFT + IOC_NRBITS;
const IOC_SIZESHIFT: u32 = IOC_TYPESHIFT + IOC_TYPEBITS;
const IOC_DIRSHIFT: u32 = IOC_SIZESHIFT + IOC_SIZEBITS;

const IOC_WRITE: u32 = 1;
const IOC_READ: u32 = 2;

/// Linux `_IOC` macro.
const fn ioc(dir: u32, ty: u8, nr: u8, size: usize) -> libc::c_ulong {
    ((dir << IOC_DIRSHIFT)
        | ((ty as u32) << IOC_TYPESHIFT)
        | ((nr as u32) << IOC_NRSHIFT)
        | ((size as u32) << IOC_SIZESHIFT)) as libc::c_ulong
}

/// `_IOR('H', nr, size)`.
const fn ior(nr: u8, size: usize) -> libc::c_ulong {
    ioc(IOC_READ, b'H', nr, size)
}

/// `_IOWR('H', nr, size)`.
const fn iowr(nr: u8, size: usize) -> libc::c_ulong {
    ioc(IOC_WRITE | IOC_READ, b'H', nr, size)
}

const HIDIOCGRAWINFO: libc::c_ulong = ior(0x03, std::mem::size_of::<HidrawDevinfo>());
const HIDIOCSFEATURE: fn(usize) -> libc::c_ulong = |len| iowr(0x06, len);
const HIDIOCGFEATURE: fn(usize) -> libc::c_ulong = |len| iowr(0x07, len);

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct HidrawDevinfo {
    bustype: u32,
    vendor: i16,
    product: i16,
}

/// Waits for `events` on `fd`. Returns `false` on timeout.
fn poll_fd(fd: RawFd, events: i16, timeout: Duration) -> io::Result<bool> {
    let mut pfd = libc::pollfd {
        fd,
        events,
        revents: 0,
    };
    let millis = timeout.as_millis().min(i32::MAX as u128) as i32;
    loop {
        let rc = unsafe { libc::poll(&mut pfd, 1, millis) };
        if rc < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        return Ok(rc > 0);
    }
}

fn last_error(context: &'static str) -> crate::PlatformError {
    crate::PlatformError::io(context, io::Error::last_os_error())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ioctl_numbers_match_the_kernel_abi() {
        // Values taken from linux/hidraw.h on x86_64.
        assert_eq!(HIDIOCGRAWINFO, 0x80084803);
        assert_eq!(HIDIOCSFEATURE(8), 0xc0084806);
        assert_eq!(HIDIOCGFEATURE(8), 0xc0084807);
    }
}
