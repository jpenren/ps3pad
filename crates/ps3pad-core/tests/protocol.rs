use ps3pad_core::{
    generic_gamepad_descriptor, host_mac_from_report, host_mac_report, parse, Battery,
    BatteryState, Button, Connection, InputReport, OutputReport, ParseError, Stick,
    VirtualPadState, INPUT_REPORT_LEN, OUTPUT_REPORT_LEN,
};

fn base_report() -> [u8; INPUT_REPORT_LEN] {
    let mut report = [0u8; INPUT_REPORT_LEN];
    report[0] = 0x01;
    report[6] = 0x80;
    report[7] = 0x80;
    report[8] = 0x80;
    report[9] = 0x80;
    report[30] = 5;
    // Resting accelerometer: Z at +1 g.
    report[43] = 0x02;
    report[44] = 0x3e; // 574 = 511 + 63
    report
}

#[test]
fn parses_buttons_sticks_pressure_and_motion() {
    let mut report = base_report();
    report[2] = 0x01 | 0x02 | 0x10 | 0x80; // select, L3, up, left
    report[3] = 0x04 | 0x40; // L1, cross
    report[4] = 0x01; // PS
    report[6] = 0x00;
    report[7] = 0xff;
    report[8] = 0x40;
    report[9] = 0xc0;
    report[14] = 0xaa; // d-pad up pressure
    report[18] = 0xff; // L2
    report[19] = 0x80; // R2
    report[20] = 0x30; // L1 pressure
    report[22] = 0x70; // triangle
    report[24] = 0xf0; // cross
    report[30] = 0xee; // charging
    report[41] = 0x01;
    report[42] = 0xf0; // X = 496
    report[45] = 0x02;
    report[46] = 0x30; // Y = 560

    let input = InputReport::parse(&report, Connection::Usb).unwrap();

    assert!(input.buttons.contains(Button::Select.bit()));
    assert!(input.buttons.contains(Button::L3.bit()));
    assert!(input.buttons.contains(Button::Up.bit()));
    assert!(input.buttons.contains(Button::Left.bit()));
    assert!(input.buttons.contains(Button::L1.bit()));
    assert!(input.buttons.contains(Button::Cross.bit()));
    assert!(input.buttons.contains(Button::Ps.bit()));
    assert!(input.buttons.contains(Button::L2.bit()));
    assert!(input.buttons.contains(Button::R2.bit()));
    assert!(!input.buttons.contains(Button::Down.bit()));

    assert_eq!(
        input.dpad,
        ps3pad_core::DPad {
            up: true,
            right: false,
            down: false,
            left: true
        }
    );
    assert_eq!(input.dpad.hat(), 7);

    assert_eq!(input.left_stick, Stick { x: 0, y: 255 });
    assert_eq!(input.right_stick, Stick { x: 0x40, y: 0xc0 });
    let (x, y) = input.left_stick.normalized();
    assert!((x + 1.0).abs() < 0.01);
    assert!(y < -0.99);

    assert_eq!(input.pressure.up, 0xaa);
    assert_eq!(input.pressure.l2, 0xff);
    assert_eq!(input.pressure.r2, 0x80);
    assert_eq!(input.pressure.get(Button::Triangle), Some(0x70));
    assert_eq!(input.pressure.get(Button::Ps), None);

    assert_eq!(
        input.battery,
        Battery {
            raw: 0xee,
            percent: 100,
            state: BatteryState::Charging
        }
    );

    assert_eq!(input.accel.x, 496);
    assert_eq!(input.accel.y, 560);
    assert_eq!(input.accel.z, 574);
    let g = input.accel.to_g(&Default::default());
    assert!((g[0] + 0.1327).abs() < 0.001);
    assert!((g[1] + 0.4336).abs() < 0.001);
    assert!((g[2] + 0.5575).abs() < 0.001);
}

#[test]
fn rejects_malformed_reports() {
    let short = [0u8; 10];
    assert_eq!(
        parse(&short, Connection::Usb),
        Err(ParseError::WrongLength {
            expected: 49,
            actual: 10
        })
    );

    let mut wrong_id = base_report();
    wrong_id[0] = 0x02;
    assert_eq!(
        parse(&wrong_id, Connection::Usb),
        Err(ParseError::NotInputReport(0x02))
    );

    let mut invalid_bt = base_report();
    invalid_bt[1] = 0xff;
    assert_eq!(
        parse(&invalid_bt, Connection::Bluetooth),
        Err(ParseError::InvalidBluetoothReport)
    );
}

#[test]
fn battery_levels_and_states() {
    assert_eq!(Battery::from_raw(0).percent, 0);
    assert_eq!(Battery::from_raw(1).percent, 1);
    assert_eq!(Battery::from_raw(2).percent, 25);
    assert_eq!(Battery::from_raw(3).percent, 50);
    assert_eq!(Battery::from_raw(4).percent, 75);
    assert_eq!(Battery::from_raw(5).percent, 100);
    assert_eq!(Battery::from_raw(0xef).state, BatteryState::Full);
    assert_eq!(Battery::from_raw(0x42).state, BatteryState::Unknown);
}

#[test]
fn output_report_has_expected_layout() {
    let report = OutputReport {
        rumble_left: 0x7f,
        rumble_right: true,
        ..OutputReport::default().with_player_led(3)
    }
    .to_bytes();

    assert_eq!(report.len(), OUTPUT_REPORT_LEN);
    assert_eq!(report[0], 0x01);
    assert_eq!(report[2], 0xff);
    assert_eq!(report[3], 1);
    assert_eq!(report[5], 0x7f);
    assert_eq!(report[10], 0x08); // LED 3 -> bitmap bit 3

    let all_off = OutputReport::default().to_bytes();
    assert_eq!(all_off[10], 0x20);
}

#[test]
fn full_led_mask_and_blink_patterns() {
    let output = OutputReport {
        leds: 0x0f,
        blink: [
            Some(ps3pad_core::output::LedBlink::new(500, 1_000)),
            None,
            None,
            None,
        ],
        ..OutputReport::default()
    };
    let report = output.to_bytes();

    assert_eq!(report[10], 0x1e);
    // LED 1 uses the last timing group (base 26), because groups are reversed.
    assert_eq!(report[26], 0xff);
    assert_eq!(report[29], 10); // off: 1000 ms = 10 deciseconds
    assert_eq!(report[30], 5); // on: 500 ms = 5 deciseconds
}

#[test]
fn host_mac_round_trips() {
    let mac = [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff];
    let report = host_mac_report(mac);
    assert_eq!(report, [0xf5, 0x01, 0xff, 0xee, 0xdd, 0xcc, 0xbb, 0xaa]);
    assert_eq!(host_mac_from_report(&report), Some(mac));
}

#[test]
fn host_mac_report_round_trip_uses_full_address() {
    let mac = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
    let report = host_mac_report(mac);
    assert_eq!(host_mac_from_report(&report), Some(mac));
}

#[test]
fn maps_to_virtual_gamepad() {
    let mut report = base_report();
    report[2] = 0x01 | 0x20; // select + right
    report[3] = 0x10 | 0x80; // triangle + square
    report[18] = 0xff;
    report[19] = 0x10;
    report[24] = 0x00;

    let input = InputReport::parse(&report, Connection::Bluetooth).unwrap();
    let state = VirtualPadState::from_input(&input);
    let bytes = state.to_report();

    assert_eq!(bytes.len(), ps3pad_core::GENERIC_GAMEPAD_INPUT_LEN);
    assert_eq!(bytes[0], 1);
    assert_eq!(bytes[1] & 0b0000_1100, 0b0000_1100); // square + triangle
    assert_eq!(bytes[9], 2); // right
    assert_eq!(bytes[5], 0xff); // L2

    let neutral = VirtualPadState::default().to_report();
    assert_eq!(neutral, [1, 0, 0, 128, 128, 0, 128, 128, 0, 8]);
}

#[test]
fn hid_descriptor_declares_expected_report_sizes() {
    let descriptor = generic_gamepad_descriptor();
    assert_eq!(descriptor[0], 0x05);
    assert_eq!(*descriptor.last().unwrap(), 0xc0);

    let mut report_size = 0u32;
    let mut report_count = 0u32;
    let mut report_id = 0u8;
    let mut input_bits = 0u32;
    let mut output_bits = 0u32;

    let mut i = 0;
    while i < descriptor.len() {
        let prefix = descriptor[i];
        i += 1;
        if prefix == 0xfe {
            // Long item: size, tag, then data.
            let size = descriptor[i] as usize;
            i += 2 + size;
            continue;
        }
        let size_code = prefix & 0b11;
        let size = match size_code {
            0 => 0,
            1 => 1,
            2 => 2,
            _ => 4,
        };
        let item_type = (prefix >> 2) & 0b11;
        let tag = prefix >> 4;
        let data = &descriptor[i..i + size];
        i += size;
        let value = data
            .iter()
            .enumerate()
            .fold(0u32, |acc, (n, byte)| acc | ((*byte as u32) << (8 * n)));

        if item_type == 1 {
            match tag {
                7 => report_size = value,  // Report Size
                9 => report_count = value, // Report Count
                8 => report_id = value as u8,
                _ => {}
            }
        } else if item_type == 0 {
            match tag {
                8 if report_id == 1 => input_bits += report_size * report_count,
                9 if report_id == 1 => output_bits += report_size * report_count,
                _ => {}
            }
        }
    }

    assert_eq!(input_bits, 72, "16 buttons + 6 axes + hat + padding");
    assert_eq!(output_bits, 64, "8 vendor output bytes");
}

#[test]
fn neutral_virtual_pad_has_null_hat() {
    let report = VirtualPadState::default().to_report();
    assert_eq!(report[9], 8);
    assert_eq!(report[3], 128);
    assert_eq!(report[4], 128);
}
