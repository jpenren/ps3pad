# PS3Pad

Self-contained Sony DualShock 3 / SIXAXIS driver and tester for Linux, Windows, and macOS.

## Features

- **Protocol support** (`ps3pad-core`): Full decode of USB/Bluetooth input reports (buttons, sticks, pressure, battery, accelerometer, gyro), LED/rumble output reports, pairing, and the official HID report descriptor.
- **Hybrid architecture** (`ps3pad-platform`):
  - Linux: Direct `/dev/hidraw*` access + virtual gamepad via `/dev/uhid` (no hidapi, no libudev).
  - Windows: Native SetupAPI/HidD/overlapped I/O for physical controllers; KMDF/VHF kernel driver for virtual gamepad.
  - macOS: IOKit/IOHIDManager FFI for physical controllers; `IOHIDUserDevice` (user-space, DriverKit-free) for virtual gamepad.
- **Applications** (`ps3pad`):
  - CLI: `list`, `dump`, `doctor`, `led`, `pair`, `uhid-test`.
  - Daemon: Bridges physical controller → virtual gamepad with Unix-socket JSON control.
  - GUI: Native egui tester with live visualization of buttons, sticks, pressure, battery, motion, and rumble/LED controls.

## Requirements

- **Linux**: Kernel ≥ 5.10 (`hidraw`, `uhid` modules), udev rules for non-root access (provided).
- **Windows**: Windows 10+ (VHF requires kernel-mode driver; user-mode VHF is not supported). Test-signing or attestation signing required for driver installation.
- **macOS**: macOS 12+ (Monterey), Xcode command-line tools. `IOHIDUserDevice` runs in-process on a CFRunLoop thread; no DriverKit entitlements needed.
- **Rust**: 1.85+ (MSRV tracked in workspace).

## Quick install (prebuilt binaries)

Prebuilt binaries for every supported platform are attached to the
[latest release](https://github.com/jpenren/ps3pad/releases/latest). The
`latest/download/...` URLs below always resolve to the newest release, so you
do not need to know the version number.

| Platform | Release asset |
| --- | --- |
| Linux x86_64 | `ps3pad-linux-x86_64.tar.gz` + `ps3pad-linux-udev.tar.gz` |
| Windows x86_64 | `ps3pad-windows-x86_64.zip` |
| macOS Apple Silicon | `ps3pad-macos-aarch64.tar.gz` |
| macOS Intel | `ps3pad-macos-x86_64.tar.gz` |
| macOS universal | `ps3pad-macos-universal.tar.gz` |

### Linux

```bash
# Binary
curl -LO https://github.com/jpenren/ps3pad/releases/latest/download/ps3pad-linux-x86_64.tar.gz
tar xzf ps3pad-linux-x86_64.tar.gz
sudo install -m 0755 ps3pad /usr/local/bin/ps3pad

# udev rules (required for non-root access)
curl -LO https://github.com/jpenren/ps3pad/releases/latest/download/ps3pad-linux-udev.tar.gz
tar xzf ps3pad-linux-udev.tar.gz
sudo install -m 0644 60-ps3pad.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
```

Verify the install with:

```bash
ps3pad list
```

The Linux binary is built against glibc 2.17, so it runs on most current
distributions (Debian 10+, Ubuntu 18.04+, Fedora, Arch, etc.) without requiring
a newer system libc.

### Windows

Download and extract
[`ps3pad-windows-x86_64.zip`](https://github.com/jpenren/ps3pad/releases/latest/download/ps3pad-windows-x86_64.zip),
then run `dist\ps3pad.exe` from the extracted folder:

```powershell
Invoke-WebRequest `
  -Uri https://github.com/jpenren/ps3pad/releases/latest/download/ps3pad-windows-x86_64.zip `
  -OutFile ps3pad-windows-x86_64.zip
Expand-Archive .\ps3pad-windows-x86_64.zip -DestinationPath .
.\dist\ps3pad.exe list
```

The virtual gamepad still needs the KMDF/VHF driver; see
[Windows driver (KMDF/VHF)](#windows-driver-kmdfvhf).

### macOS

Choose the archive that matches your Mac:

```bash
# Apple Silicon (M1/M2/M3/...)
curl -LO https://github.com/jpenren/ps3pad/releases/latest/download/ps3pad-macos-aarch64.tar.gz
tar xzf ps3pad-macos-aarch64.tar.gz
sudo install -m 0755 ps3pad-aarch64 /usr/local/bin/ps3pad

# Intel
curl -LO https://github.com/jpenren/ps3pad/releases/latest/download/ps3pad-macos-x86_64.tar.gz
tar xzf ps3pad-macos-x86_64.tar.gz
sudo install -m 0755 ps3pad-x86_64 /usr/local/bin/ps3pad

# Universal (works on both)
curl -LO https://github.com/jpenren/ps3pad/releases/latest/download/ps3pad-macos-universal.tar.gz
tar xzf ps3pad-macos-universal.tar.gz
sudo install -m 0755 ps3pad-universal /usr/local/bin/ps3pad
```

Run `ps3pad list` to verify the install. The binaries are not notarized; if
macOS blocks them, allow the app in **System Settings → Privacy & Security** or
remove the quarantine attribute:

```bash
sudo xattr -d com.apple.quarantine /usr/local/bin/ps3pad 2>/dev/null || true
```

## Building

```bash
# Linux (default target)
cargo build --workspace --release

# Cross-check Windows / macOS from Linux
cargo check --target x86_64-pc-windows-msvc --workspace
cargo check --target x86_64-apple-darwin --workspace
cargo check --target aarch64-apple-darwin --workspace
```

### Windows driver (KMDF/VHF)

Located in `drivers/windows/`:

1. Open `drivers/windows/ps3padvhf.vcxproj` in Visual Studio 2022 with WDK.
2. Build the driver (`x64` / `Release`).
3. Install with `devcon` or `pnputil` (requires test-signing or attestation):
   ```cmd
   devcon install ps3padvhf.inf "Root\Ps3padVhf"
   ```
4. The driver creates `\DosDevices\Ps3padVhf` for the Rust virtual-pad backend.

See `drivers/windows/BUILD.md` for detailed instructions.

### macOS

No special build steps. The Rust code links against IOKit and CoreFoundation system frameworks. Run the binary directly; the virtual HID device appears on a background CFRunLoop thread.

## Linux permissions (udev)

Required for both prebuilt and source builds; the Linux quick-install steps
above already install these rules.

```bash
sudo install -m 0644 drivers/linux/60-ps3pad.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
```

After this, `ps3pad list` / `dump` / `gui` work without `sudo`.

## Usage

```bash
# List detected controllers
ps3pad list

# Dump live reports (decoded)
ps3pad dump

# Raw hex dump
ps3pad dump --raw

# Set player LED (1-4, 0 = off)
ps3pad led 2

# Store host Bluetooth MAC for pairing (USB only)
ps3pad pair aa:bb:cc:dd:ee:ff

# Test virtual gamepad without a controller
ps3pad uhid-test --seconds 10

# Bridge physical → virtual gamepad (daemon)
ps3pad daemon --socket /tmp/ps3pad.sock

# Control daemon via Unix socket
echo '{"type":"status"}' | socat - UNIX-CONNECT:/tmp/ps3pad.sock

# Graphical tester
ps3pad gui
```

## GUI Screenshot

![ps3pad GUI](docs/gui-screenshot.png)

*The graphical tester (`ps3pad gui`) shows live controller state: button grid, analog stick positions, pressure bars for all 12 analog buttons, battery level with charging state, accelerometer/tilt/gyro readouts, LED player indicators, rumble sliders, virtual-pad toggle, and a scrolling report log.*

## Architecture

See `docs/ARCHITECTURE.md` for the crate layout, transport abstractions, and virtual-HID event flow.

## Protocol

See `docs/PROTOCOL.md` for the DualShock 3 report layout, byte offsets, and output report format.

## Reverse Engineering

See `docs/REVERSE_ENGINEERING.md` for sources, provenance, and methodology.

## License

MIT OR Apache-2.0