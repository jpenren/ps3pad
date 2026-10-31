use std::fs;
use std::path::Path;

use ps3pad_core::{is_supported, Connection};

use crate::device::DeviceInfo;
use crate::error::PlatformError;

/// Enumerates supported controllers through `/sys/class/hidraw`.
pub fn discover() -> Result<Vec<DeviceInfo>, PlatformError> {
    let class = Path::new("/sys/class/hidraw");
    let entries = match fs::read_dir(class) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(PlatformError::io(
                "cannot enumerate /sys/class/hidraw",
                error,
            ))
        }
    };

    let mut devices = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let node = entry.file_name();
        let node = node.to_string_lossy();
        if !node.starts_with("hidraw") {
            continue;
        }

        let sys = entry.path();
        let uevent = read_trimmed(&sys.join("device/uevent")).unwrap_or_default();
        let Some((bus, vendor, product)) = parse_hid_id(&uevent) else {
            continue;
        };
        if !is_supported(vendor, product) {
            continue;
        }

        let name =
            uevent_value(&uevent, "HID_NAME").unwrap_or_else(|| "Sony DualShock 3".to_string());
        let uniq = read_trimmed(&sys.join("device/uniq"))
            .filter(|value| !value.is_empty())
            .or_else(|| uevent_value(&uevent, "HID_UNIQ").filter(|value| !value.is_empty()));
        let phys = read_trimmed(&sys.join("device/phys"))
            .filter(|value| !value.is_empty())
            .or_else(|| uevent_value(&uevent, "HID_PHYS").filter(|value| !value.is_empty()));

        devices.push(DeviceInfo {
            path: format!("/dev/{node}"),
            name,
            vendor_id: vendor,
            product_id: product,
            connection: Connection::from_linux_bus(bus),
            serial: uniq,
            phys,
        });
    }

    devices.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(devices)
}

fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
}

fn uevent_value(uevent: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    uevent
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .map(|value| value.trim().to_string())
}

/// Parses `HID_ID=0003:0000054C:00000268`.
fn parse_hid_id(uevent: &str) -> Option<(u16, u16, u16)> {
    let value = uevent_value(uevent, "HID_ID")?;
    let mut parts = value.split(':');
    let bus = u16::from_str_radix(parts.next()?, 16).ok()?;
    let vendor = u16::from_str_radix(parts.next()?, 16).ok()?;
    let product = u16::from_str_radix(parts.next()?, 16).ok()?;
    Some((bus, vendor, product))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hid_id() {
        let uevent = "DRIVER=sony\nHID_ID=0003:0000054C:00000268\nHID_NAME=Sony PLAYSTATION(R)3 Controller\n";
        assert_eq!(parse_hid_id(uevent), Some((3, 0x054c, 0x0268)));
        assert_eq!(
            uevent_value(uevent, "HID_NAME").as_deref(),
            Some("Sony PLAYSTATION(R)3 Controller")
        );
    }

    #[test]
    fn rejects_non_sony_devices() {
        assert_eq!(
            parse_hid_id("HID_ID=0003:0000046D:0000C52B\n"),
            Some((3, 0x046d, 0xc52b))
        );
    }
}
