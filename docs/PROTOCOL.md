# Protocol — DualShock 3 / SIXAXIS (USB & Bluetooth)

## Identifiers

- Vendor ID: `0x054c` (Sony)
- Product ID: `0x0268` (DualShock 3 / SIXAXIS)
- Interface: HID, 1 interface, 2 endpoints (IN/OUT) + control
- Report ID: `0x01` (all input/output reports)
- Input report length: **49 bytes**
- Output report length: **49 bytes** (48 payload + 1 report ID)

## Input report (device → host)

Byte offsets are 0-based including the report ID at offset 0.

| Offset | Size | Name | Description |
|--------|------|------|-------------|
| 0 | 1 | `report_id` | Always `0x01` |
| 1 | 1 | `padding` | Always `0x00` (USB); `0xff` = idle packet (Bluetooth, ignore) |
| 2 | 1 | `buttons_lo` | Digital buttons (see below) |
| 3 | 1 | `buttons_hi` | Digital buttons (see below) |
| 4 | 1 | `buttons_ps` | Bit 0 = PS button |
| 5 | 1 | `padding` | `0x00` |
| 6 | 1 | `lx` | Left stick X (0=left, 128=center, 255=right) |
| 7 | 1 | `ly` | Left stick Y (0=up, 128=center, 255=down) |
| 8 | 1 | `rx` | Right stick X (0=left, 128=center, 255=right) |
| 9 | 1 | `ry` | Right stick Y (0=up, 128=center, 255=down) |
| 10–13 | 4 | `padding` | `0x00` |
| 14 | 1 | `pressure_up` | D-pad up analog |
| 15 | 1 | `pressure_right` | D-pad right analog |
| 16 | 1 | `pressure_down` | D-pad down analog |
| 17 | 1 | `pressure_left` | D-pad left analog |
| 18 | 1 | `pressure_l2` | L2 trigger analog |
| 19 | 1 | `pressure_r2` | R2 trigger analog |
| 20 | 1 | `pressure_l1` | L1 analog |
| 21 | 1 | `pressure_r1` | R1 analog |
| 22 | 1 | `pressure_triangle` | △ analog |
| 23 | 1 | `pressure_circle` | ○ analog |
| 24 | 1 | `pressure_cross` | × analog |
| 25 | 1 | `pressure_square` | □ analog |
| 26–29 | 4 | `padding` | `0x00` |
| 30 | 1 | `battery` | Battery state (see below) |
| 31–40 | 10 | `padding` | `0x00` (some firmware: gyro temp at 31) |
| 41–42 | 2 | `accel_x` | Accelerometer X, **big-endian**, unsigned |
| 43–44 | 2 | `accel_z` | Accelerometer Z, **big-endian**, unsigned |
| 45–46 | 2 | `accel_y` | Accelerometer Y, **big-endian**, unsigned |
| 47–48 | 2 | `gyro` | Single-axis gyro (yaw), **big-endian**, signed |

### Button bitmasks

**Byte 2 (`buttons_lo`)**

| Bit | Button | Constant |
|-----|--------|----------|
| 0 | Select | `Buttons::SELECT` (0x0000_0001) |
| 1 | L3 (left stick press) | `Buttons::L3` (0x0000_0002) |
| 2 | R3 (right stick press) | `Buttons::R3` (0x0000_0004) |
| 3 | Start | `Buttons::START` (0x0000_0008) |
| 4 | Up (D-pad) | `Buttons::UP` (0x0000_0010) |
| 5 | Right (D-pad) | `Buttons::RIGHT` (0x0000_0020) |
| 6 | Down (D-pad) | `Buttons::DOWN` (0x0000_0040) |
| 7 | Left (D-pad) | `Buttons::LEFT` (0x0000_0080) |

**Byte 3 (`buttons_hi`)**

| Bit | Button | Constant |
|-----|--------|----------|
| 2 | L1 | `Buttons::L1` (0x0000_0100) |
| 3 | R1 | `Buttons::R1` (0x0000_0200) |
| 4 | Triangle (△) | `Buttons::TRIANGLE` (0x0000_0400) |
| 5 | Circle (○) | `Buttons::CIRCLE` (0x0000_0800) |
| 6 | Cross (×) | `Buttons::CROSS` (0x0000_1000) |
| 7 | Square (□) | `Buttons::SQUARE` (0x0000_2000) |

**Byte 4 (`buttons_ps`)**

| Bit | Button | Constant |
|-----|--------|----------|
| 0 | PS (Home) | `Buttons::PS` (0x0001_0000) |

**Digital triggers** (derived from pressure ≥ 27): L2, R2.

### Battery byte (offset 30)

| Value | Meaning | Percent | State |
|-------|---------|---------|-------|
| 0–5 | Discrete levels | 0 / 1 / 25 / 50 / 75 / 100 | Discharging |
| `0xee` (238) | Charging | 100 | Charging |
| `0xef` (239) | Full | 100 | Full |
| other | Unknown | 0 | Unknown |

### Accelerometer

- 10-bit sensor, left-aligned in 16 bits (values ≈ 0–1023, zero-g ≈ 511).
- Kernel/SDL convention: zero-g = **511**, sensitivity = **113 counts/g**.
- Axis signs (controller flat on back → +Z up):
  - X: `+X` = right → `(raw - 511) / 113`
  - Y: `+Y` = forward → `(511 - raw) / 113` (inverted)
  - Z: `+Z` = up → `(511 - raw) / 113` (inverted)
- Byte order: **big-endian** (MSByte first at offsets 41, 43, 45).

### Gyro

- Single axis (yaw), 16-bit signed, big-endian at offsets 47–48.
- Scale: empirical, not officially documented. Raw counts shown directly.

## Output report (host → device)

49 bytes, report ID `0x01` at offset 0.

| Offset | Name | Description |
|--------|------|-------------|
| 0 | `report_id` | `0x01` |
| 1 | `rumble_right_dur` | Right motor duration (0–255) |
| 2 | `rumble_right_on` | Right motor enable (0/1) |
| 3 | `rumble_left_dur` | Left motor duration (0–255) |
| 4 | `rumble_left_force` | Left motor amplitude (0–255) |
| 5–9 | `padding` | `0x00` |
| 10 | `led_bitmap` | LED control (see below) |
| 11–30 | `led_timing[20]` | 5 groups × 4 bytes (on/off/rise/fall), reverse LED order |
| 31–48 | `padding` | `0x00` |

### LED bitmap (offset 10)

| Value | LEDs |
|-------|------|
| `0x02` | Player 1 |
| `0x04` | Player 2 |
| `0x08` | Player 3 |
| `0x10` | Player 4 |
| `0x20` | All off |
| `0x00` | No change (keep current) |

Combine bits to light multiple LEDs. `0x20` overrides others (all off).

### LED timing groups (offsets 11–30)

20 bytes = 5 groups × 4 bytes. Groups correspond to LEDs **4, 3, 2, 1, PS** (reverse order).

Each group: `[on_time, off_time, rise_time, fall_time]` in units of **100 μs** (0–255).

Example (player 2 slow blink): `led_bitmap = 0x04`, group for LED 2 at offsets 15–18 = `[0x20, 0x20, 0x00, 0x00]` → ~3.2 Hz.

## Feature reports (control endpoint)

### 0xf2 — Calibration / Device info (USB only)

- `GET_REPORT`, length 17.
- Offset 4–9: **Controller Bluetooth MAC**, big-endian (MAC[5] at offset 4, MAC[0] at offset 9).
- Used to learn the controller's own MAC for Bluetooth pairing.

### 0xf5 — Pairing / Operational mode

- `GET_REPORT` (USB), length 8 → returns `[0xf5, 0x01, host_mac[5..0]]`.
- `SET_REPORT` (Bluetooth), payload `[0xf4, 0x42, 0x03, 0x00, 0x00]` (5 bytes) → enters operational mode.
- Writing host MAC (8 bytes `[0xf5, 0x01, mac5..mac0]`) stores the host address for Bluetooth pairing.

### 0xf4 — Bluetooth operational handshake

- `SET_REPORT`, payload `[0xf4, 0x42, 0x03, 0x00, 0x00]`.
- Alternative USB: `SET_REPORT` `[0xf4, 0x42, 0x0c, 0x00, 0x00]`.

## Initialization sequences

### USB

1. `GET_FEATURE` 0xf2, len 17 → read controller MAC (optional).
2. `GET_FEATURE` 0xf5, len 8.
3. `INTERRUPT_WRITE` 1 byte (first byte of 0xf5 response) — some clones need this.
4. Controller now streams input reports.

### Bluetooth

1. `SET_FEATURE` 0xf4, payload `[0x42, 0x03, 0x00, 0x00]`.
2. Controller streams input reports.
3. Pairing: `SET_FEATURE` 0xf5, payload `[0xf5, 0x01, host_mac5..host_mac0]`.

## Virtual HID gamepad (ps3pad created)

- VID: `0x1209` (Open Source Hardware)
- PID: `0x0001`
- Report ID: 1 (input), 2 (output)
- Input: 16 buttons, 6 axes (8-bit), 1 hat (4-bit)
- Output (ID 2): 8 bytes vendor-defined (mirrors DS3 LED/rumble layout)

## References

- Linux kernel: `drivers/hid/hid-sony.c` (`sixaxis_raw_event`, `sixaxis_set_operational_usb/bt`)
- SDL: `src/joystick/hidapi/SDL_hidapi_ps3.c` (`HIDAPI_DriverPS3_HandleStatePacket`)
- USB Host Shield 2.0: `PS3USB.cpp`
- DsHidMini: `Shim/DS3/DS3Protocol.cpp`
- All above used for **protocol facts only**; no GPL code copied. SDL (zlib), DsHidMini (BSD-3-Clause), USB Host Shield (GPL-2.0 but only referenced for offsets).