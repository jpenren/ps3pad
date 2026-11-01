# Architecture

## Workspace layout

```
ps3pad/
├── crates/
│   ├── ps3pad-core/      # Protocol codec, no I/O, no platform code
│   ├── ps3pad-platform/  # Transport + virtual HID backends (Linux/Windows/macOS)
│   └── ps3pad/           # CLI, daemon, GUI
├── installer/
│   └── windows/          # Inno Setup installer + DsHidMini driver package
├── docs/
│   ├── ARCHITECTURE.md   # This file
│   ├── PROTOCOL.md       # Wire format
│   └── REVERSE_ENGINEERING.md
└── Cargo.toml            # Workspace root
```

## Crate responsibilities

### ps3pad-core

Pure Rust, no `std` dependencies beyond `alloc`. Contains:

- `Button`, `Buttons` (u32 bitflags, 17 buttons incl. PS, L2/R2 digital)
- `DPad` (4 bools derived from buttons)
- `Stick` (raw x/y, `normalized() -> (f32, f32)` in `[-1,1]`, Y up)
- `Pressure` (12 analog buttons, 0–255)
- `Battery` (raw byte → percent + state: Unknown/Discharging/Charging/Full)
- `AccelSample` (x/y/z raw counts, `to_g()` with 511 zero-g, 113 counts/g)
- `InputReport::parse(&[u8], Connection) -> Result<InputReport>`
- `OutputReport` builder + `to_bytes() -> [u8; 49]`
- `generic_gamepad_descriptor()` (VID 0x1209, PID 0x0001, 16 buttons, 6 axes, hat, 8-byte vendor output)
- `host_mac_report(mac) -> [u8; 8]` (pairing report `0xf5 0x01 mac[5..0]`)

### ps3pad-platform

Platform-neutral traits + per-OS implementations behind `cfg(target_os = "...")`.

**Physical controller API** (`device.rs`):

```rust
pub struct DeviceInfo { path, name, vendor_id, product_id, connection, serial, phys }
pub struct Controller {
    pub fn discover() -> Result<Vec<DeviceInfo>>
    pub fn open(&DeviceInfo) -> Result<Self>
    pub fn initialize(&mut self) -> Result<InitInfo>
    pub fn read(&mut self, Duration) -> Result<Option<InputReport>>
    pub fn read_raw(&mut self, Duration) -> Result<Option<Vec<u8>>>
    pub fn write_output(&mut self, &[u8]) -> Result<()>
    pub fn get_feature(&mut self, u8, usize) -> Result<Vec<u8>>
    pub fn set_feature(&mut self, &[u8]) -> Result<()>
}
```

**Virtual gamepad API** (`virtual_pad.rs`):

```rust
pub enum VirtualPadEvent {
    Start { dev_flags: u64 }, Stop, Open, Close,
    Output { report_type: u8, data: Vec<u8> },
    GetReport { id: u32, number: u8, report_type: u8 },
    SetReport { id: u32, number: u8, report_type: u8, data: Vec<u8> },
    Destroy,
}
pub struct VirtualPad {
    pub fn create_default() -> Result<Self>
    pub fn create(name, bus, vid, pid, version, country, descriptor) -> Result<Self>
    pub fn send_input(&mut self, &[u8]) -> Result<()>
    pub fn poll(&mut self, Duration) -> Result<Option<VirtualPadEvent>>
    pub fn reply_get_report(&mut self, id, number, type, data) -> Result<()>
    pub fn reply_set_report(&mut self, id, error) -> Result<()>
    pub fn next_output(&mut self, Duration) -> Result<Option<OutputReport>>
}
```

#### Linux implementation (`src/linux/`)

- **hidraw.rs**: Opens `/dev/hidrawN` O_RDWR|O_NONBLOCK|O_CLOEXEC. `poll()` + `read()`/`write()`. Feature reports via `HIDIOCGFEATURE`/`HIDIOCSFEATURE` ioctls. No libudev.
- **sysfs.rs**: Scans `/sys/class/hidraw/hidraw*/device/uevent` for `HID_ID=bbbb:vvvv:pppp`. Matches VID 0x054c, PID 0x0268. Bus 3 → USB, 5 → Bluetooth.
- **uhid.rs**: Opens `/dev/uhid`. `CREATE2` event (280-byte header + descriptor). `INPUT2` for reports. Parses `OUTPUT`, `GET_REPORT`, `SET_REPORT`, `START`/`STOP`/`OPEN`/`CLOSE`/`DESTROY`. Auto-replies to feature requests.

#### Windows implementation (`src/windows/`)

On Windows the physical DualShock 3 is **not** driven by ps3pad directly. The
stock `hidusb` driver rejects the DS3's `SET_REPORT` with report ID `0xF4`
(error 87) because that report is not in the HID descriptor, so a kernel-mode
or filter driver is required. ps3pad uses the signed **DsHidMini** UMDF driver
(Nefarius Software Solutions, BSD-3-Clause) which replaces `hidusb` for
`USB\VID_054C&PID_0268` and defaults to XInput mode (mode 5). Windows then
exposes the controller as a genuine XInput gamepad (`VID_045E&PID_02FF`)
that games recognise natively.

- **Physical**: XInput API (`XInputGetState`/`XInputSetState` from `xinput1_4.dll`,
  loaded dynamically at runtime). Slots 0–3 are probed; each connected slot
  maps to a synthetic 49-byte DualShock 3 report so `InputReport::parse` can
  decode buttons, sticks, triggers, and battery. Rumble/LEDs are routed through
  `XInputSetState`. Feature reports (`0xf2`/`0xf4`/`0xf5`) are unsupported —
  DsHidMini already keeps the controller in full report mode.
- **Virtual**: None. DsHidMini already exposes a real XInput device, so no
  virtual gamepad, no ViGEmBus, and no KMDF/VHF driver are needed.

`Controller::discover()` probes XInput slots 0–3 and returns one `DeviceInfo`
per connected slot. `Controller::open()` validates the slot with
`XInputGetState`. `Controller::read_raw()` returns a synthetic DS3 report.
`Controller::write_output()` converts an `OutputReport` to XInput vibration.

DLL declarations use manual `#[link_name]` FFI (no `windows-sys`) to avoid
import-library tooling issues with the GNU Rust toolchain.

#### macOS implementation (planned, `src/macos/`)

- **Physical**: `IOHIDManagerCreate` + `IOHIDManagerSetDeviceMatching` (VID/PID). `IOHIDManagerOpen` → `IOHIDDeviceCreate`. Register `IOHIDDeviceRegisterInputReportCallback` + `IOHIDDeviceRegisterRemovalCallback`. `IOHIDDeviceGetReport`/`SetReport` (report ID separate, buffer excludes it). Stable path: `DevSrvsID:<registry_entry_id>`. Transport property `"USB"` vs `"Bluetooth*"`.
- **Virtual**: `IOHIDUserDeviceCreateWithOptions` with `kIOHIDUserDeviceCreateOptionStartWhenScheduled`. Register `IOHIDUserDeviceReportCallback` (type, reportID, data, len). `IOHIDUserDeviceScheduleWithRunLoop` on dedicated thread with `CFRunLoopRun`. `IOHIDUserDeviceHandleReport` for input. Feature replies via callback.

### ps3pad (application)

- **commands.rs**: `list`, `dump` (raw/json/pretty), `doctor`, `led`, `pair`, `uhid-test`.
- **daemon.rs**: Opens controller → `initialize()` → creates virtual pad → loop: `read()` → map → `virtual.send_input()`; `virtual.next_output()` → `controller.send_output()`. Unix socket (JSON): `{"type":"ping"}`, `{"type":"status"}`, `{"type":"stop"}`, `{"type":"led","player":2}`, `{"type":"rumble","left":128,"right":true}`.
- **gui.rs**: egui/eframe 0.37. Device picker. Real-time: button grid, stick circles, pressure bars, battery gauge, accel/tilt/gyro readouts, report log. LED/radio buttons, rumble sliders, virtual-pad toggle. 60 fps target.

## Data flow

```
Physical DS3 (USB)
        │
        ▼
DsHidMini driver (replaces hidusb)
        │  exposes as XInput device (VID 045E, PID 02FF)
        ▼
ps3pad-platform (XInput: XInputGetState/XInputSetState)
        │  read_raw() → synthetic 49-byte DS3 report
        ▼
ps3pad-core: InputReport::parse()
        │
        ▼
Daemon / GUI
        │
        ├─► (Windows: no virtual pad needed — DsHidMini already exposes XInput)
        │    (Linux/macOS: Virtual pad: send_input() → uhid / IOHIDUserDevice)
        │
        └─► OutputReport (LED/rumble) → to_bytes() → write_output() → XInputSetState
```

## Virtual HID descriptor (generic gamepad)

- VID 0x1209 (Open Source Hardware), PID 0x0001
- Report ID 1 (numbered)
- 16 buttons (bits 0–15)
- 6 axes: X, Y, Z, Rx, Ry, Rz (8-bit each, 0–255, 128 center)
- 1 hat (4-bit, 0–7, 8 = null)
- 8-byte vendor-defined output report (ID 2) for LED/rumble passthrough

## Connection types

| Bus (Linux) | Windows | macOS | ps3pad-core enum |
|-------------|---------|-------|------------------|
| USB (3)     | XInput (via DsHidMini) | IOKit USB | `Connection::Usb` |
| Bluetooth (5)| HID over BT | IOKit BT | `Connection::Bluetooth` |

On Windows, Bluetooth is not yet supported by ps3pad; DsHidMini handles both
USB and Bluetooth DS3 controllers, but ps3pad's XInput backend only reports
`Connection::Usb`.

Initialization handshake differs:
- USB/Linux/macOS: GET 0xf2 (17B) → GET 0xf5 (8B) → 1-byte interrupt write
- Bluetooth/Linux/macOS: SET 0xf4 `[0x42,0x03,0x00,0x00]`
- Windows (DsHidMini): no handshake needed — DsHidMini keeps the controller streaming

## Error handling

- `PlatformError::Unsupported { os }` for missing backends.
- `PlatformError::Disconnected` on EOF/broken pipe.
- `PlatformError::Io { context, source }` wraps `std::io::Error`.
- `ParseError` from `ps3pad-core` for malformed reports.

## Testing

- `cargo test -p ps3pad-core` (10 protocol tests)
- `cargo test -p ps3pad-platform` (5 Linux ioctl/uhid ABI tests)
- `cargo test -p ps3pad` (integration tests require hardware)
- Cross-check: `cargo check --target x86_64-pc-windows-msvc --workspace`
- Cross-check: `cargo check --target x86_64-apple-darwin --workspace`