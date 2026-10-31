use anyhow::{bail, Context, Result};
use ps3pad_platform::{Controller, DeviceInfo};

/// Resolves a user-supplied device selector.
///
/// The selector may be an index from `ps3pad list`, a full device path, a
/// path suffix (`hidraw3`), or a serial/Bluetooth address.
pub fn select_device(selector: Option<&str>) -> Result<DeviceInfo> {
    let devices = Controller::discover().context("cannot enumerate controllers")?;
    let Some(selector) = selector else {
        return devices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("no supported controller found"));
    };

    if let Ok(index) = selector.parse::<usize>() {
        if let Some(device) = devices.get(index) {
            return Ok(device.clone());
        }
        bail!(
            "device index {index} is out of range ({} controller(s) found)",
            devices.len()
        );
    }

    let wanted = selector.to_ascii_lowercase();
    let wanted_compact = wanted.replace([':', '-'], "");
    devices
        .into_iter()
        .find(|device| {
            let path = device.path.to_ascii_lowercase();
            path == wanted
                || path.ends_with(&wanted)
                || device
                    .serial
                    .as_ref()
                    .map(|serial| {
                        serial.to_ascii_lowercase() == wanted
                            || serial.to_ascii_lowercase().replace([':', '-'], "") == wanted_compact
                    })
                    .unwrap_or(false)
        })
        .ok_or_else(|| anyhow::anyhow!("no controller matches selector {selector:?}"))
}

pub fn format_mac(mac: [u8; 6]) -> String {
    format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}

pub fn parse_mac(value: &str) -> Result<[u8; 6]> {
    let compact: String = value
        .chars()
        .filter(|character| *character != ':' && *character != '-')
        .collect();
    if compact.len() != 12 {
        bail!("expected 12 hexadecimal digits, got {value:?}");
    }
    let mut mac = [0u8; 6];
    for (index, byte) in mac.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&compact[index * 2..index * 2 + 2], 16)
            .with_context(|| format!("invalid MAC address {value:?}"))?;
    }
    Ok(mac)
}

pub fn connection_label(device: &DeviceInfo) -> &'static str {
    device.connection.label()
}
