use std::collections::VecDeque;
use std::time::{Duration, Instant};

use anyhow::Result;
use eframe::egui;
use ps3pad_core::{AccelCalibration, Button, InputReport, OutputReport, Stick, VirtualPadState};
use ps3pad_platform::{Controller, DeviceInfo, InitInfo, VirtualPad, VirtualPadEvent};

use crate::cli::GuiArgs;
use crate::util::{format_mac, select_device};

const BG: egui::Color32 = egui::Color32::from_rgb(20, 22, 27);
const PANEL: egui::Color32 = egui::Color32::from_rgb(30, 33, 41);
const PANEL_ACTIVE: egui::Color32 = egui::Color32::from_rgb(44, 48, 58);
const ACCENT: egui::Color32 = egui::Color32::from_rgb(90, 160, 255);
const GOOD: egui::Color32 = egui::Color32::from_rgb(90, 214, 140);
const WARN: egui::Color32 = egui::Color32::from_rgb(255, 190, 90);
const BAD: egui::Color32 = egui::Color32::from_rgb(255, 105, 105);
const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(145, 152, 165);

pub fn run(args: GuiArgs) -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 800.0])
            .with_min_inner_size([900.0, 640.0])
            .with_title("PS3Pad — DualShock 3 tester"),
        ..Default::default()
    };

    eframe::run_native(
        "PS3Pad",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, args)))),
    )
    .map_err(|error| anyhow::anyhow!("GUI failed: {error}"))
}

struct App {
    devices: Vec<DeviceInfo>,
    selected: usize,
    controller: Option<Controller>,
    info: Option<DeviceInfo>,
    init: Option<InitInfo>,
    latest: Option<InputReport>,
    first_report_at: Option<Instant>,
    last_report_at: Option<Instant>,
    reports: u64,
    error: Option<String>,
    output: OutputReport,
    sent_output: OutputReport,
    player: u8,
    rumble_left: u8,
    rumble_right: bool,
    virtual_enabled: bool,
    pad: Option<VirtualPad>,
    pad_status: String,
    log: VecDeque<String>,
    show_raw: bool,
    calib: AccelCalibration,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, args: GuiArgs) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let mut app = Self {
            devices: Vec::new(),
            selected: 0,
            controller: None,
            info: None,
            init: None,
            latest: None,
            first_report_at: None,
            last_report_at: None,
            reports: 0,
            error: None,
            output: OutputReport::default().with_player_led(1),
            sent_output: OutputReport::default().with_player_led(1),
            player: 1,
            rumble_left: 0,
            rumble_right: false,
            virtual_enabled: false,
            pad: None,
            pad_status: "not created".into(),
            log: VecDeque::new(),
            show_raw: false,
            calib: AccelCalibration::default(),
        };
        app.refresh_devices();
        if let Some(selector) = args.device.as_deref() {
            if let Ok(info) = select_device(Some(selector)) {
                if let Some(index) = app
                    .devices
                    .iter()
                    .position(|device| device.path == info.path)
                {
                    app.selected = index;
                }
            }
        }
        app.connect();
        if args.virtual_pad {
            app.start_virtual();
        }
        app
    }

    fn refresh_devices(&mut self) {
        match Controller::discover() {
            Ok(devices) => {
                self.devices = devices;
                if self.selected >= self.devices.len() {
                    self.selected = 0;
                }
            }
            Err(error) => {
                self.devices.clear();
                self.error = Some(format!("cannot enumerate controllers: {error}"));
            }
        }
    }

    fn connect(&mut self) {
        self.disconnect();
        let Some(info) = self.devices.get(self.selected).cloned() else {
            self.error = Some("no controller selected".into());
            return;
        };
        match Controller::open(&info) {
            Ok(mut controller) => match controller.initialize() {
                Ok(init) => {
                    if let Err(error) = controller.send_output(&self.output) {
                        self.error = Some(format!("cannot set LED: {error}"));
                    }
                    self.controller = Some(controller);
                    self.init = Some(init);
                    self.info = Some(info);
                    self.error = None;
                    self.reports = 0;
                    self.first_report_at = None;
                    self.last_report_at = None;
                    self.latest = None;
                    self.sent_output = self.output;
                    self.push_log("connected".to_string());
                }
                Err(error) => {
                    self.error = Some(format!("initialisation failed: {error}"));
                }
            },
            Err(error) => self.error = Some(format!("cannot open controller: {error}")),
        }
    }

    fn disconnect(&mut self) {
        self.controller = None;
        self.info = None;
        self.init = None;
        self.latest = None;
    }

    fn start_virtual(&mut self) {
        if self.pad.is_some() {
            self.virtual_enabled = true;
            return;
        }
        match VirtualPad::create_default() {
            Ok(pad) => {
                self.pad = Some(pad);
                self.virtual_enabled = true;
                self.pad_status = format!(
                    "{} ({:04x}:{:04x})",
                    ps3pad_core::GENERIC_GAMEPAD_NAME,
                    ps3pad_core::VIRTUAL_VENDOR_ID,
                    ps3pad_core::VIRTUAL_PRODUCT_ID
                );
                self.push_log("virtual gamepad created".into());
            }
            Err(error) => {
                self.virtual_enabled = false;
                self.pad_status = format!("failed: {error}");
                self.error = Some(format!("cannot create virtual gamepad: {error}"));
            }
        }
    }

    fn stop_virtual(&mut self) {
        self.virtual_enabled = false;
        self.pad = None;
        self.pad_status = "not created".into();
        self.push_log("virtual gamepad removed".into());
    }

    fn push_log(&mut self, line: String) {
        self.log.push_back(line);
        while self.log.len() > 250 {
            self.log.pop_front();
        }
    }

    fn pump(&mut self) {
        let mut incoming = Vec::new();
        let mut lost = None;
        if let Some(controller) = self.controller.as_mut() {
            for _ in 0..16 {
                match controller.read(Duration::ZERO) {
                    Ok(Some(report)) => incoming.push(report),
                    Ok(None) => break,
                    Err(ps3pad_platform::PlatformError::Protocol(
                        ps3pad_core::ParseError::InvalidBluetoothReport,
                    )) => {}
                    Err(ps3pad_platform::PlatformError::Protocol(error)) => {
                        self.error = Some(format!("bad report: {error}"));
                    }
                    Err(error) => {
                        lost = Some(error.to_string());
                        break;
                    }
                }
            }
        }
        for report in incoming {
            self.handle_report(report);
        }
        if let Some(error) = lost {
            self.error = Some(format!("controller lost: {error}"));
            self.disconnect();
        }

        let mut host_outputs = Vec::new();
        let mut messages = Vec::new();
        let mut pad_error = None;
        let mut pad_destroyed = false;
        if let Some(pad) = self.pad.as_mut() {
            for _ in 0..16 {
                match pad.poll(Duration::ZERO) {
                    Ok(Some(event)) => {
                        let output_state = event.output_state();
                        match event {
                            VirtualPadEvent::GetReport {
                                id,
                                number,
                                report_type,
                            } => {
                                let _ = pad.reply_get_report(id, number, report_type, &[]);
                            }
                            VirtualPadEvent::SetReport { id, .. } => {
                                let _ = pad.reply_set_report(id, 0);
                            }
                            VirtualPadEvent::Output { .. } => {
                                if let Some(state) = output_state {
                                    host_outputs.push(state);
                                }
                            }
                            VirtualPadEvent::Start { dev_flags } => {
                                messages.push(format!("virtual device started ({dev_flags:#x})"))
                            }
                            VirtualPadEvent::Open => {
                                messages.push("host opened virtual device".into())
                            }
                            VirtualPadEvent::Close => {
                                messages.push("host closed virtual device".into())
                            }
                            VirtualPadEvent::Stop => {
                                messages.push("host stopped virtual device".into())
                            }
                            VirtualPadEvent::Destroy => {
                                messages.push("virtual device destroyed".into());
                                pad_destroyed = true;
                                break;
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(error) => {
                        pad_error = Some(error.to_string());
                        break;
                    }
                }
            }
        }
        for message in messages {
            self.push_log(message);
        }
        for host in host_outputs {
            self.apply_host_output(host);
        }
        if let Some(error) = pad_error {
            self.pad_status = format!("error: {error}");
            self.stop_virtual();
        } else if pad_destroyed {
            self.stop_virtual();
        }

        if self.output != self.sent_output {
            if let Some(controller) = self.controller.as_mut() {
                match controller.send_output(&self.output) {
                    Ok(()) => self.sent_output = self.output,
                    Err(error) => self.error = Some(format!("output report failed: {error}")),
                }
            }
        }
    }

    fn apply_host_output(&mut self, host: OutputReport) {
        self.output.rumble_left = host.rumble_left;
        self.output.rumble_right = host.rumble_right;
        self.rumble_left = host.rumble_left;
        self.rumble_right = host.rumble_right;
        if host.leds != 0 {
            self.output.leds = host.leds;
            self.player = host.leds.count_ones().clamp(1, 4) as u8;
        }
    }

    fn handle_report(&mut self, report: InputReport) {
        if self.virtual_enabled {
            if let Some(pad) = self.pad.as_mut() {
                let state = VirtualPadState::from_input(&report);
                if let Err(error) = pad.send_input(&state.to_report()) {
                    self.pad_status = format!("send failed: {error}");
                }
            }
        }
        self.reports += 1;
        let now = Instant::now();
        self.first_report_at.get_or_insert(now);
        self.last_report_at = Some(now);
        if self.log.len() < 250 || self.reports % 4 == 0 {
            self.push_log(format!(
                "{:>6}  {:<32} L2 {:>3}  R2 {:>3}  {}%",
                self.reports,
                short_buttons(&report),
                report.pressure.l2,
                report.pressure.r2,
                report.battery.percent
            ));
        }
        self.latest = Some(report);
    }

    fn set_player(&mut self, player: u8) {
        self.player = player;
        self.output.set_player_led(player);
    }

    fn draw_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("PS3Pad");
            ui.label(
                egui::RichText::new("DualShock 3 / SIXAXIS")
                    .color(TEXT_DIM)
                    .size(14.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Refresh").clicked() {
                    self.refresh_devices();
                }
                let connected = self.controller.is_some();
                ui.label(
                    egui::RichText::new(if connected {
                        "● connected"
                    } else {
                        "○ disconnected"
                    })
                    .color(if connected { GOOD } else { BAD }),
                );
            });
        });

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let names: Vec<String> = self
                .devices
                .iter()
                .enumerate()
                .map(|(index, device)| {
                    format!(
                        "[{index}] {} — {} ({})",
                        device.path,
                        device.name,
                        device.connection.label()
                    )
                })
                .collect();
            let selected_text = names
                .get(self.selected)
                .cloned()
                .unwrap_or_else(|| "no controller".to_string());
            egui::ComboBox::from_id_salt("device")
                .selected_text(selected_text)
                .width(560.0)
                .show_ui(ui, |ui| {
                    for (index, name) in names.iter().enumerate() {
                        ui.selectable_value(&mut self.selected, index, name);
                    }
                });
            if ui.button("Connect").clicked() {
                self.connect();
            }
            if ui.button("Disconnect").clicked() {
                self.disconnect();
            }
            let mut virtual_enabled = self.virtual_enabled;
            if ui
                .checkbox(&mut virtual_enabled, "Virtual gamepad")
                .changed()
            {
                if virtual_enabled {
                    self.start_virtual();
                } else {
                    self.stop_virtual();
                }
            }
        });

        if let Some(error) = self.error.clone() {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(format!("⚠ {error}")).color(BAD));
        }
    }

    fn draw_controller(&mut self, ui: &mut egui::Ui) {
        let Some(report) = self.latest.clone() else {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new("Waiting for input reports…")
                        .size(18.0)
                        .color(TEXT_DIM),
                );
            });
            return;
        };

        ui.columns(2, |columns| {
            self.draw_buttons(&mut columns[0], &report);
            self.draw_sticks(&mut columns[1], &report);
        });

        ui.add_space(6.0);
        ui.columns(3, |columns| {
            self.draw_triggers(&mut columns[0], &report);
            self.draw_battery(&mut columns[1], &report);
            self.draw_motion(&mut columns[2], &report);
        });
    }

    fn draw_buttons(&self, ui: &mut egui::Ui, report: &InputReport) {
        panel(ui, "Buttons", |ui| {
            ui.horizontal(|ui| {
                dpad_widget(ui, &report.dpad);
                ui.add_space(18.0);
                face_buttons(ui, &report.buttons);
            });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                for button in [
                    Button::L1,
                    Button::R1,
                    Button::L2,
                    Button::R2,
                    Button::L3,
                    Button::R3,
                    Button::Select,
                    Button::Start,
                    Button::Ps,
                ] {
                    let pressed = report.buttons.contains(button.bit());
                    let text = match button {
                        Button::L2 => format!("L2 {}", report.pressure.l2),
                        Button::R2 => format!("R2 {}", report.pressure.r2),
                        _ => button.name().to_string(),
                    };
                    chip(ui, &text, pressed);
                }
            });
        });
    }

    fn draw_sticks(&self, ui: &mut egui::Ui, report: &InputReport) {
        panel(ui, "Sticks", |ui| {
            ui.horizontal(|ui| {
                stick_widget(ui, report.left_stick, "Left", ACCENT);
                ui.add_space(12.0);
                stick_widget(ui, report.right_stick, "Right", GOOD);
            });
            let (lx, ly) = report.left_stick.normalized();
            let (rx, ry) = report.right_stick.normalized();
            ui.label(
                egui::RichText::new(format!(
                    "L {:+.2}, {:+.2}    R {:+.2}, {:+.2}",
                    lx, ly, rx, ry
                ))
                .monospace()
                .color(TEXT_DIM),
            );
        });
    }

    fn draw_triggers(&self, ui: &mut egui::Ui, report: &InputReport) {
        panel(ui, "Pressure", |ui| {
            for (label, value, color) in [
                ("L2", report.pressure.l2, ACCENT),
                ("R2", report.pressure.r2, GOOD),
                ("L1", report.pressure.l1, ACCENT),
                ("R1", report.pressure.r1, GOOD),
            ] {
                ui.horizontal(|ui| {
                    ui.add_sized([24.0, 18.0], egui::Label::new(label));
                    ui.add(
                        egui::ProgressBar::new(value as f32 / 255.0)
                            .desired_width(110.0)
                            .fill(color)
                            .text(format!("{value:>3}")),
                    );
                });
            }
            let max = report.pressure.max();
            ui.label(
                egui::RichText::new(format!("max {max}/255"))
                    .monospace()
                    .color(TEXT_DIM),
            );
        });
    }

    fn draw_battery(&self, ui: &mut egui::Ui, report: &InputReport) {
        panel(ui, "Battery", |ui| {
            let color = if report.battery.is_charging() {
                GOOD
            } else if report.battery.percent <= 25 {
                WARN
            } else {
                ACCENT
            };
            ui.add(
                egui::ProgressBar::new(report.battery.percent as f32 / 100.0)
                    .desired_width(170.0)
                    .fill(color)
                    .text(format!("{}%", report.battery.percent)),
            );
            ui.label(
                egui::RichText::new(format!(
                    "{:?}  raw 0x{:02x}  {}",
                    report.battery.state,
                    report.battery.raw,
                    report.connection.label()
                ))
                .color(TEXT_DIM),
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!(
                    "reports {}   {} Hz",
                    self.reports,
                    rate(self.first_report_at, self.reports)
                ))
                .monospace()
                .color(TEXT_DIM),
            );
        });
    }

    fn draw_motion(&self, ui: &mut egui::Ui, report: &InputReport) {
        panel(ui, "Motion", |ui| {
            let g = report.accel.to_g(&self.calib);
            let tilt = report.accel.tilt(&self.calib);
            motion_widget(ui, g);
            ui.label(
                egui::RichText::new(format!("x {:+.2}g  y {:+.2}g  z {:+.2}g", g[0], g[1], g[2]))
                    .monospace()
                    .color(TEXT_DIM),
            );
            ui.label(
                egui::RichText::new(format!(
                    "pitch {:+.1}°  roll {:+.1}°  gyro {}",
                    tilt.pitch, tilt.roll, report.gyro
                ))
                .monospace()
                .color(TEXT_DIM),
            );
        });
    }

    fn draw_output(&mut self, ui: &mut egui::Ui) {
        panel(ui, "Controller output", |ui| {
            ui.horizontal(|ui| {
                ui.label("Player LED");
                for player in 1..=4u8 {
                    if ui
                        .selectable_label(self.player == player, format!("{player}"))
                        .clicked()
                    {
                        self.set_player(player);
                    }
                }
                if ui.button("All off").clicked() {
                    self.player = 0;
                    self.output.leds = 0;
                }
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.rumble_right, "Small motor");
                let mut value = self.rumble_left;
                if ui
                    .add(
                        egui::Slider::new(&mut value, 0..=255)
                            .text("Large motor")
                            .clamping(egui::SliderClamping::Always),
                    )
                    .changed()
                {
                    self.rumble_left = value;
                }
            });
            self.output.rumble_left = self.rumble_left;
            self.output.rumble_right = self.rumble_right;

            ui.horizontal(|ui| {
                ui.label(format!("virtual pad: {}", self.pad_status));
                if let Some(info) = self.info.as_ref() {
                    if let Some(mac) = self.init.as_ref().and_then(|init| init.device_mac) {
                        ui.label(
                            egui::RichText::new(format!(
                                "  {}  MAC {}",
                                info.path,
                                format_mac(mac)
                            ))
                            .monospace()
                            .color(TEXT_DIM),
                        );
                    }
                }
            });
        });
    }

    fn draw_report_log(&mut self, ui: &mut egui::Ui) {
        panel(ui, "Report log", |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.show_raw, "Show raw bytes");
                if let Some(report) = &self.latest {
                    if self.show_raw {
                        ui.label(egui::RichText::new(report.raw_hex()).monospace().size(11.0));
                    }
                }
            });
            egui::ScrollArea::vertical()
                .max_height(150.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for line in &self.log {
                        ui.monospace(egui::RichText::new(line).size(11.0).color(TEXT_DIM));
                    }
                });
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.pump();

        egui::Frame::central_panel(ui.style())
            .fill(BG)
            .inner_margin(12.0)
            .show(ui, |ui| {
                self.draw_header(ui);
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.draw_controller(ui);
                    ui.add_space(8.0);
                    self.draw_output(ui);
                    ui.add_space(8.0);
                    self.draw_report_log(ui);
                });
            });

        ui.ctx().request_repaint_after(Duration::from_millis(8));
    }
}

fn panel(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style())
        .fill(PANEL)
        .corner_radius(8.0)
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                egui::RichText::new(title)
                    .size(13.0)
                    .strong()
                    .color(TEXT_DIM),
            );
            ui.add_space(4.0);
            add(ui);
        });
}

fn chip(ui: &mut egui::Ui, label: &str, pressed: bool) {
    let fill = if pressed { GOOD } else { PANEL_ACTIVE };
    let color = if pressed {
        egui::Color32::BLACK
    } else {
        egui::Color32::from_rgb(190, 196, 208)
    };
    egui::Frame::group(ui.style())
        .fill(fill)
        .corner_radius(5.0)
        .inner_margin(egui::Margin::symmetric(7, 4))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(label).color(color).strong().size(12.0));
        });
}

fn dpad_widget(ui: &mut egui::Ui, dpad: &ps3pad_core::DPad) {
    let (response, painter) = ui.allocate_painter(egui::Vec2::splat(132.0), egui::Sense::hover());
    let rect = response.rect;
    let cell = rect.width() / 3.0;
    painter.rect_filled(rect, 8.0, egui::Color32::from_rgb(23, 25, 31));

    let states = [
        (1, 0, dpad.up, "▲"),
        (2, 1, dpad.right, "▶"),
        (1, 2, dpad.down, "▼"),
        (0, 1, dpad.left, "◀"),
    ];
    for (column, row, active, symbol) in states {
        let min = rect.min + egui::vec2(column as f32 * cell + 2.0, row as f32 * cell + 2.0);
        let cell_rect = egui::Rect::from_min_size(min, egui::Vec2::splat(cell - 4.0));
        painter.rect_filled(cell_rect, 6.0, if active { ACCENT } else { PANEL_ACTIVE });
        painter.text(
            cell_rect.center(),
            egui::Align2::CENTER_CENTER,
            symbol,
            egui::FontId::proportional(16.0),
            if active {
                egui::Color32::BLACK
            } else {
                TEXT_DIM
            },
        );
    }
    painter.text(
        rect.center() + egui::vec2(cell, cell),
        egui::Align2::CENTER_CENTER,
        "D-pad",
        egui::FontId::proportional(11.0),
        TEXT_DIM,
    );
}

fn face_buttons(ui: &mut egui::Ui, buttons: &ps3pad_core::Buttons) {
    let (response, painter) =
        ui.allocate_painter(egui::Vec2::new(132.0, 132.0), egui::Sense::hover());
    let rect = response.rect;
    let center = rect.center();
    let radius = 15.0;
    let distance = 34.0;

    let items = [
        (
            0.0,
            -distance,
            Button::Triangle,
            "△",
            egui::Color32::from_rgb(140, 220, 255),
        ),
        (
            distance,
            0.0,
            Button::Circle,
            "○",
            egui::Color32::from_rgb(255, 140, 150),
        ),
        (
            0.0,
            distance,
            Button::Cross,
            "✕",
            egui::Color32::from_rgb(150, 190, 255),
        ),
        (
            -distance,
            0.0,
            Button::Square,
            "□",
            egui::Color32::from_rgb(230, 160, 245),
        ),
    ];
    for (dx, dy, button, symbol, color) in items {
        let pressed = buttons.contains(button.bit());
        let position = center + egui::vec2(dx, dy);
        painter.circle_filled(position, radius, if pressed { color } else { PANEL_ACTIVE });
        painter.circle_stroke(
            position,
            radius,
            egui::Stroke::new(1.0, color.gamma_multiply(0.7)),
        );
        painter.text(
            position,
            egui::Align2::CENTER_CENTER,
            symbol,
            egui::FontId::proportional(17.0),
            if pressed { egui::Color32::BLACK } else { color },
        );
    }
}

fn stick_widget(ui: &mut egui::Ui, stick: Stick, label: &str, color: egui::Color32) {
    let (response, painter) = ui.allocate_painter(egui::Vec2::splat(136.0), egui::Sense::hover());
    let rect = response.rect;
    let center = rect.center();
    let radius = rect.width() * 0.42;
    painter.circle_filled(center, radius, egui::Color32::from_rgb(23, 25, 31));
    painter.circle_stroke(center, radius, egui::Stroke::new(1.0, PANEL_ACTIVE));
    painter.line_segment(
        [
            center - egui::vec2(radius, 0.0),
            center + egui::vec2(radius, 0.0),
        ],
        egui::Stroke::new(1.0, PANEL_ACTIVE),
    );
    painter.line_segment(
        [
            center - egui::vec2(0.0, radius),
            center + egui::vec2(0.0, radius),
        ],
        egui::Stroke::new(1.0, PANEL_ACTIVE),
    );

    let (x, y) = stick.normalized();
    let position = center + egui::vec2(x * radius, -y * radius);
    painter.line_segment(
        [center, position],
        egui::Stroke::new(1.5, color.gamma_multiply(0.6)),
    );
    painter.circle_filled(position, 10.0, color);
    painter.text(
        rect.center() + egui::vec2(0.0, radius + 10.0),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(12.0),
        TEXT_DIM,
    );
}

fn motion_widget(ui: &mut egui::Ui, g: [f32; 3]) {
    let (response, painter) =
        ui.allocate_painter(egui::Vec2::new(150.0, 76.0), egui::Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 8.0, egui::Color32::from_rgb(23, 25, 31));
    let center = rect.center();
    let radius = 30.0;
    painter.circle_stroke(center, radius, egui::Stroke::new(1.0, PANEL_ACTIVE));
    let position = center
        + egui::vec2(
            g[0].clamp(-1.5, 1.5) * radius / 1.5,
            -g[1].clamp(-1.5, 1.5) * radius / 1.5,
        );
    painter.line_segment([center, position], egui::Stroke::new(1.5, ACCENT));
    let z_color = if g[2].abs() > 0.2 { GOOD } else { PANEL_ACTIVE };
    painter.circle_filled(position, 7.0, z_color);
    painter.text(
        rect.right_top() + egui::vec2(-8.0, 8.0),
        egui::Align2::RIGHT_TOP,
        format!("z {:+.1}", g[2]),
        egui::FontId::monospace(11.0),
        TEXT_DIM,
    );
}

fn short_buttons(report: &InputReport) -> String {
    if report.buttons.is_empty() {
        return "-".to_string();
    }
    report
        .buttons
        .iter()
        .map(|button| button.symbol())
        .collect::<Vec<_>>()
        .join(" ")
}

fn rate(first: Option<Instant>, reports: u64) -> u32 {
    let Some(first) = first else { return 0 };
    let seconds = first.elapsed().as_secs_f32();
    if seconds < 0.5 || reports == 0 {
        return 0;
    }
    (reports as f32 / seconds).round() as u32
}
