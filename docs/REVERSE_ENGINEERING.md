# Reverse Engineering Notes

## Sources consulted

| Source | License | Used for |
|--------|---------|----------|
| Linux kernel `drivers/hid/hid-sony.c` (torvalds/linux) | GPL-2.0 | Protocol facts only (report offsets, initialization sequences, LED/rumble layout, accelerometer calibration). **No code copied.** |
| SDL `src/joystick/hidapi/SDL_hidapi_ps3.c` | zlib | Protocol facts (report layout, big-endian accelerometer, scaling, Bluetooth idle packet). |
| USB Host Shield 2.0 `PS3USB.cpp` | GPL-2.0 | Protocol facts (pairing report format, output report structure). |
| DsHidMini `Shim/DS3/DS3Protocol.cpp` | BSD-3-Clause | Protocol facts (virtual HID mapping, feature report details, Windows VHF approach). |
| Microsoft docs: VHF (`vhf.h`, `VhfCreate`, `VhfReadReportSubmit`, `EvtVhfAsyncOperationWriteReport`) | Public API | Windows driver design. |
| Apple docs: `IOHIDUserDevice`, `IOHIDManager`, `IOHIDDevice` | Public API | macOS backend design. |

## Methodology

1. **Specification gathering**: Read the four open-source references above to build a combined protocol map (offsets, bitmasks, initialization handshakes, calibration constants).
2. **Cross-validation**: Verified each offset against at least two independent sources (e.g., Linux kernel + SDL for accelerometer; kernel + USB Host Shield for pairing).
3. **Live capture**: On Linux, used `/dev/hidraw*` to capture raw 49-byte reports while exercising the controller. Confirmed:
   - Report ID = 0x01
   - Button bits at 2, 3, 4
   - Sticks at 6–9
   - Pressures at 14–25
   - Battery at 30 (0xee charging, 0xef full)
   - Accelerometer big-endian at 41–46 (X, Z, Y order)
   - Gyro big-endian at 47–48
4. **Kernel observation**: Enumerated `/sys/class/hidraw/hidraw*/device/uevent` → confirmed `HID_ID=0003:0000054C:00000268` for USB, `0005` for Bluetooth. `hid_sony` driver binds and exposes both joystick (`event15`/`js0`) and accelerometer (`event5`).
5. **Virtual HID testing**: Used `/dev/uhid` with `CREATE2`/`INPUT2` to create a generic gamepad (VID 1209, PID 0001). Verified host opens/closes device, sends output reports (LED/rumble), and requests feature reports.
6. **Windows VHF research**: Microsoft documentation confirms VHF is **kernel-mode only** as of Windows 10 (UMDF/VHF was never shipped). Designed KMDF driver with `EvtVhfReadyForNextReadReport=NULL` (VHF handles input buffering) and `EvtVhfAsyncOperationWriteReport` for output. Control device with custom IOCTLs for create/input/destroy.
7. **macOS IOHID research**: `IOHIDUserDevice` (introduced macOS 11) allows user-space virtual HID without DriverKit. Requires scheduling on a `CFRunLoop` thread. Physical device enumeration via `IOHIDManager` with VID/PID matching. Stable reopen via `DevSrvsID:<registry_entry_id>`. Transport property distinguishes `"USB"` vs `"Bluetooth*"`.

## Provenance disclaimer

- **No GPL code is included** in this repository. All protocol constants, offsets, and algorithms are re-implemented from public documentation and reverse-engineered specifications.
- The `ps3pad-core` crate is original Rust code implementing the protocol.
- The `ps3pad-platform` Linux backend uses only public kernel UAPI (`hidraw.h`, `uhid.h`) via `libc` FFI.
- Windows driver (`drivers/windows/`) is original C code using WDK/KMDF/VHF APIs.
- macOS backend uses only public IOKit/CoreFoundation FFI.

## Signing / distribution caveats

- **Windows**: KMDF driver requires **test-signing** (`bcdedit /set testsigning on`) for development, or **attestation signing** via Microsoft Partner Center for production. `drivers/windows/BUILD.md` documents both.
- **macOS**: No special entitlements needed for `IOHIDUserDevice` (user-space). The binary must be code-signed (`codesign -s -`) to run on Apple Silicon; ad-hoc signing works for local testing.
- **Linux**: udev rule `60-ps3pad.rules` grants `TAG+="uaccess"` (systemd-logind ACL) for non-root access to `/dev/hidraw*` and `/dev/uhid`. Alternative: `GROUP="plugdev"`.

## Validation checklist

- [x] Linux: hidraw discovery → open → initialize → read 49-byte reports → parse all fields
- [x] Linux: uhid CREATE2 → INPUT2 → OUTPUT/GET_REPORT/SET_REPORT event handling
- [x] Linux: virtual pad appears as standard HID gamepad (16 btn, 6 axes, hat)
- [x] Linux: daemon bridges physical → virtual with rumble/LED forwarding
- [x] Linux: CLI commands (list/dump/doctor/led/pair/uhid-test) functional
- [x] Linux: egui GUI launches, renders live state
- [x] Cross-compile: x86_64-pc-windows-msvc, x86_64-apple-darwin, aarch64-apple-darwin
- [ ] Windows: KMDF driver builds, installs, creates `\DosDevices\Ps3padVhf`
- [ ] Windows: Rust virtual-pad backend talks to driver via IOCTLs
- [ ] Windows: physical HID via SetupAPI/HidD/overlapped I/O
- [ ] macOS: IOHIDManager enumeration, input callback, feature reports
- [ ] macOS: IOHIDUserDevice virtual pad on CFRunLoop thread
- [ ] Hardware test: Bluetooth pairing (store host MAC, reconnect)
- [ ] Hardware test: rumble/LED round-trip via virtual pad

## Open questions / known differences

- **Accelerometer sign conventions**: Linux kernel inverts Y and Z (`511 - raw`). SDL does the same. Our `AccelSample::to_g()` matches this.
- **Gyro scale**: Undocumented. Linux kernel does not expose it for DS3 (only DS4). We pass raw counts.
- **Pressure sensitivity**: 12 buttons report analog values (0–255). L2/R2 also digital at threshold 27.
- **Bluetooth idle packet**: Second byte = 0xff, rest zeroed. Must be ignored (kernel returns `-EINVAL`, SDL skips).
- **Third-party clones**: Some (Shanwan, Gasia) need the extra 1-byte interrupt write after 0xf5; others start rumbling if sent. We send it but ignore errors.
- **Report descriptor**: Official DS3 descriptor is 200+ bytes. We ship a minimal generic gamepad descriptor (VID 1209, PID 0001) for the virtual device; the physical device's descriptor is not used by our parser.

## Future work

- Implement Windows `src/windows/` backend (SetupAPI + KMDF control IOCTLs).
- Implement macOS `src/macos/` backend (IOHIDManager + IOHIDUserDevice).
- Add Bluetooth transport support in Linux (already works via hidraw; pairing via `pair` command).
- Add systemd service unit for daemon.
- Fuzz test parser with malformed reports.