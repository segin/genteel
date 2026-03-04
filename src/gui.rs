use crate::audio;
use crate::frontend::{self, InputMapping};
use crate::input::InputScript;
use crate::Emulator;
#[cfg(feature = "gilrs")]
use gilrs::{Axis, Button, EventType, Gilrs};
#[cfg(feature = "gui")]
use pixels::{wgpu, Pixels, SurfaceTexture};
use std::collections::HashMap;
use std::fmt::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
#[cfg(feature = "gui")]
use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};


#[cfg(feature = "gilrs")]
pub fn init_gilrs_with_builder<F, R, E>(builder: F) -> Option<R>
where
    F: FnOnce() -> Result<R, E>,
    E: std::fmt::Display,
{
    match builder() {
        Ok(g) => Some(g),
        Err(e) => {
            eprintln!("Warning: Failed to initialize gilrs: {}", e);
            None
        }
    }
}

pub const SLOT_NAMES: [&str; 10] = [
    "Slot 0", "Slot 1", "Slot 2", "Slot 3", "Slot 4", "Slot 5", "Slot 6", "Slot 7", "Slot 8",
    "Slot 9",
];

pub const SLOT_EXTS: [&str; 10] = [
    "s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9",
];

#[cfg(feature = "gui")]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct WindowState {
    pub open: bool,
}

#[cfg(feature = "gui")]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct GuiState {
    pub windows: HashMap<String, WindowState>,
    pub input_mapping: InputMapping,
    pub integer_scaling: bool,
    pub force_red: bool,
    pub paused: bool,
    pub recent_roms: Vec<PathBuf>,
    pub auto_save_load: bool,
    #[serde(skip)]
    pub single_step: bool,
    #[serde(skip)]
    pub show_about: bool,
    #[serde(skip)]
    pub reset_requested: bool,
    #[serde(skip)]
    pub close_requested: bool,
    #[serde(skip)]
    pub pick_rom_requested: bool,
    #[serde(skip)]
    pub save_requested: Option<u8>,
    #[serde(skip)]
    pub load_requested: Option<u8>,
    #[serde(skip)]
    pub delete_state_requested: Option<u8>,
}

#[cfg(feature = "gui")]
impl GuiState {
    pub fn new(input_mapping: InputMapping) -> Self {
        let mut state = Self {
            windows: HashMap::new(),
            input_mapping,
            integer_scaling: true,
            force_red: false,
            paused: false,
            recent_roms: Vec::new(),
            auto_save_load: false,
            single_step: false,
            show_about: false,
            reset_requested: false,
            close_requested: false,
            pick_rom_requested: false,
            save_requested: None,
            load_requested: None,
            delete_state_requested: None,
        };
        state.register_default_windows();
        state
    }

    pub fn load_or_default(input_mapping: InputMapping) -> Self {
        if let Ok(content) = std::fs::read_to_string("gui_config.json") {
            if let Ok(mut state) = serde_json::from_str::<Self>(&content) {
                state.register_default_windows();
                return state;
            }
        }
        Self::new(input_mapping)
    }

    fn register_default_windows(&mut self) {
        let defaults = [
            "Settings",
            "Performance & Debug",
            "M68k Status",
            "Z80 Status",
            "Disassembly",
            "Execution Control",
            "Palette Viewer",
            "Tile Viewer",
            "Sprite Viewer",
            "Scroll Plane Viewer",
            "VDP Memory Hex",
            "Memory Viewer",
            "Sound Chip Visualizer",
            "Audio Channel Waveforms",
            "Controller Viewer",
            "Expansion Status",
            "State Browser",
        ];
        for &name in &defaults {
            if !self.windows.contains_key(name) {
                self.windows
                    .insert(name.to_string(), WindowState { open: false });
            }
        }
    }

    pub fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write("gui_config.json", json);
        }
    }

    pub fn is_window_open(&self, name: &str) -> bool {
        self.windows.get(name).map(|w| w.open).unwrap_or(false)
    }

    pub fn set_window_open(&mut self, name: &str, open: bool) {
        if let Some(window) = self.windows.get_mut(name) {
            window.open = open;
        } else {
            self.windows.insert(name.to_string(), WindowState { open });
        }
        self.save();
    }

    pub fn toggle_window(&mut self, name: &str) {
        let open = self.is_window_open(name);
        self.set_window_open(name, !open);
    }
}

#[cfg(feature = "gui")]
pub struct DebugInfo {
    pub m68k_pc: u32,
    pub m68k_d: [u32; 8],
    pub m68k_a: [u32; 8],
    pub m68k_sr: u16,
    pub m68k_usp: u32,
    pub m68k_ssp: u32,
    pub m68k_disasm: Vec<(u32, String)>,
    pub z80_pc: u16,
    pub z80_a: u8,
    pub z80_f: u8,
    pub z80_b: u8,
    pub z80_c: u8,
    pub z80_d: u8,
    pub z80_e: u8,
    pub z80_h: u8,
    pub z80_l: u8,
    pub z80_ix: u16,
    pub z80_iy: u16,
    pub z80_sp: u16,
    pub z80_i: u8,
    pub z80_r: u8,
    pub z80_memptr: u16,
    pub z80_iff1: bool,
    pub z80_im: u8,
    pub z80_disasm: Vec<(u16, String)>,
    pub frame_count: u64,
    pub vdp_status: u16,
    pub vdp_registers: [u8; 24],
    pub display_enabled: bool,
    pub bg_color_index: u8,
    pub cram: [u16; 64],
    pub cram_raw: [u16; 64],
    pub vram: [u8; 0x10000],
    pub vsram: [u8; 80],
    pub wram: [u8; 0x10000],
    pub z80_ram: [u8; 0x2000],
    pub ym2612_regs: [[u8; 256]; 2],
    pub psg_tone: [crate::apu::psg::ToneChannel; 3],
    pub psg_noise: crate::apu::psg::NoiseChannel,
    pub channel_waveforms: [[i16; 128]; 10],
    pub port1_state: crate::io::ControllerState,
    pub port1_type: crate::io::ControllerType,
    pub port2_state: crate::io::ControllerState,
    pub port2_type: crate::io::ControllerType,
    pub has_rom: bool,
    pub current_rom_path: Option<PathBuf>,
}


#[cfg(feature = "gui")]
pub struct Framework {
    pub egui_ctx: egui::Context,
    pub egui_state: egui_winit::State,
    pub screen_descriptor: egui_wgpu::ScreenDescriptor,
    pub renderer: egui_wgpu::Renderer,
    pub gui_state: GuiState,
    pub tile_texture: Option<egui::TextureHandle>,
    pub plane_a_texture: Option<egui::TextureHandle>,
    pub plane_b_texture: Option<egui::TextureHandle>,
    pub pending_rom_path: Arc<Mutex<Option<PathBuf>>>,
    #[cfg(feature = "gilrs")]
    pub gilrs: Option<Gilrs>,
}


#[cfg(feature = "gui")]
impl Framework {
    pub fn new(
        event_loop: &winit::event_loop::EventLoopWindowTarget<()>,
        width: u32,
        height: u32,
        scale_factor: f32,
        pixels: &pixels::Pixels,
        input_mapping: InputMapping,
    ) -> Self {
        let egui_ctx = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::viewport::ViewportId::ROOT,
            &event_loop,
            Some(scale_factor),
            None,
        );
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: scale_factor,
        };
        let renderer =
            egui_wgpu::Renderer::new(pixels.device(), pixels.render_texture_format(), None, 1);
        let gui_state = GuiState::load_or_default(input_mapping);
        Self {
            egui_ctx,
            egui_state,
            screen_descriptor,
            renderer,
            gui_state,
            tile_texture: None,
            plane_a_texture: None,
            plane_b_texture: None,
            pending_rom_path: Arc::new(Mutex::new(None)),
            #[cfg(feature = "gilrs")]
            gilrs: init_gilrs_with_builder(|| gilrs::Gilrs::new()),
        }
    }
    pub fn handle_event(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
    ) {
        let _ = self.egui_state.on_window_event(window, event);
    }
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.screen_descriptor.size_in_pixels = [width, height];
        }
    }
    pub fn scale_factor(&mut self, scale_factor: f32) {
        self.screen_descriptor.pixels_per_point = scale_factor;
    }

    pub fn pick_rom(&mut self) {
        let pending = self.pending_rom_path.clone();
        std::thread::spawn(move || {
            let file = rfd::FileDialog::new()
                .add_filter("Genesis ROMs", &["bin", "md", "gen", "zip"])
                .add_filter("All Files", &["*"])
                .pick_file();
            if let Some(path) = file {
                if let Ok(mut lock) = pending.lock() {
                    *lock = Some(path);
                } else {
                    eprintln!("Failed to acquire pending_rom_path lock");
                }
            }
        });
    }

    #[cfg(feature = "gilrs")]
    pub fn poll_gamepads(&mut self, state: &mut crate::io::ControllerState) {
        let Some(gilrs) = &mut self.gilrs else { return; };
        while let Some(gilrs::Event { event, .. }) = gilrs.next_event() {
            match event {
                EventType::ButtonPressed(button, _) => match button {
                    Button::DPadUp => state.up = true,
                    Button::DPadDown => state.down = true,
                    Button::DPadLeft => state.left = true,
                    Button::DPadRight => state.right = true,
                    Button::South => state.b = true,
                    Button::East => state.c = true,
                    Button::West => state.a = true,
                    Button::North => state.x = true,
                    Button::LeftTrigger => state.y = true,
                    Button::RightTrigger => state.z = true,
                    Button::Select => state.mode = true,
                    Button::Start => state.start = true,
                    _ => {}
                },
                EventType::ButtonReleased(button, _) => match button {
                    Button::DPadUp => state.up = false,
                    Button::DPadDown => state.down = false,
                    Button::DPadLeft => state.left = false,
                    Button::DPadRight => state.right = false,
                    Button::South => state.b = false,
                    Button::East => state.c = false,
                    Button::West => state.a = false,
                    Button::North => state.x = false,
                    Button::LeftTrigger => state.y = false,
                    Button::RightTrigger => state.z = false,
                    Button::Select => state.mode = false,
                    Button::Start => state.start = false,
                    _ => {}
                },
                EventType::AxisChanged(axis, value, _) => {
                    let threshold = 0.5;
                    match axis {
                        Axis::LeftStickX => {
                            if value > threshold {
                                state.right = true;
                                state.left = false;
                            } else if value < -threshold {
                                state.left = true;
                                state.right = false;
                            } else {
                                state.left = false;
                                state.right = false;
                            }
                        }
                        Axis::LeftStickY => {
                            if value > threshold {
                                state.up = true;
                                state.down = false;
                            } else if value < -threshold {
                                state.down = true;
                                state.up = false;
                            } else {
                                state.up = false;
                                state.down = false;
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    pub fn prepare(&mut self, window: &winit::window::Window, debug_info: &DebugInfo) {
        let raw_input = self.egui_state.take_egui_input(window);
        self.egui_ctx.begin_frame(raw_input);

        // Global shortcuts
        let ctrl = self.egui_ctx.input(|i| i.modifiers.command);
        if ctrl && self.egui_ctx.input(|i| i.key_pressed(egui::Key::O)) {
            self.gui_state.pick_rom_requested = true;
        }
        if ctrl && self.egui_ctx.input(|i| i.key_pressed(egui::Key::R)) && debug_info.has_rom {
            self.gui_state.reset_requested = true;
        }
        if self.egui_ctx.input(|i| i.key_pressed(egui::Key::F5)) && debug_info.has_rom {
            self.gui_state.save_requested = Some(0); // Default to slot 0
        }
        if self.egui_ctx.input(|i| i.key_pressed(egui::Key::F8)) && debug_info.has_rom {
            self.gui_state.load_requested = Some(0); // Default to slot 0
        }

        // Draw the GUI
        egui::TopBottomPanel::top("menubar_container").show(&self.egui_ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui
                        .add(egui::Button::new("Open...").shortcut_text("Ctrl+O"))
                        .clicked()
                    {
                        self.gui_state.pick_rom_requested = true;
                        ui.close_menu();
                    }
                    ui.menu_button("Open Recent", |ui| {
                        if self.gui_state.recent_roms.is_empty() {
                            ui.label("No recent ROMs");
                        } else {
                            let recent = self.gui_state.recent_roms.clone();
                            for path in recent {
                                let filename = path
                                    .file_name()
                                    .and_then(|f| f.to_str())
                                    .unwrap_or("Unknown");
                                if ui.button(filename).clicked() {
                                    let mut lock = self.pending_rom_path.lock().unwrap();
                                    *lock = Some(path);
                                    ui.close_menu();
                                }
                            }
                        }
                    });
                    ui.separator();
                    if ui
                        .add_enabled(
                            debug_info.has_rom,
                            egui::Button::new("Reset ROM").shortcut_text("Ctrl+R"),
                        )
                        .clicked()
                    {
                        self.gui_state.reset_requested = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(debug_info.has_rom, egui::Button::new("Close ROM"))
                        .clicked()
                    {
                        self.gui_state.close_requested = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.menu_button("Save State", |ui| {
                        for slot in 0..10 {
                            let btn = if slot == 0 {
                                egui::Button::new(SLOT_NAMES[slot as usize]).shortcut_text("F5")
                            } else {
                                egui::Button::new(SLOT_NAMES[slot as usize])
                            };
                            if ui.add_enabled(debug_info.has_rom, btn).clicked() {
                                self.gui_state.save_requested = Some(slot);
                                ui.close_menu();
                            }
                        }
                    });
                    ui.menu_button("Load State", |ui| {
                        for slot in 0..10 {
                            let btn = if slot == 0 {
                                egui::Button::new(SLOT_NAMES[slot as usize]).shortcut_text("F8")
                            } else {
                                egui::Button::new(SLOT_NAMES[slot as usize])
                            };
                            if ui.add_enabled(debug_info.has_rom, btn).clicked() {
                                self.gui_state.load_requested = Some(slot);
                                ui.close_menu();
                            }
                        }
                    });
                    if ui
                        .add_enabled(debug_info.has_rom, egui::Button::new("State Browser..."))
                        .clicked()
                    {
                        self.gui_state.set_window_open("State Browser", true);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        std::process::exit(0);
                    }
                });
                ui.menu_button("Settings", |ui| {
                    if ui.button("Video").clicked() {
                        self.gui_state.set_window_open("Settings", true);
                        ui.close_menu();
                    }
                    if ui.button("Input Mapping").clicked() {
                        self.gui_state.set_window_open("Settings", true);
                        ui.close_menu();
                    }
                });
                ui.menu_button("Debug", |ui| {
                    let mut names: Vec<String> = self.gui_state.windows.keys().cloned().collect();
                    names.sort(); // Keep menu consistent
                    for name in names {
                        if name == "Settings" {
                            continue;
                        } // Settings is in Settings menu
                        let mut open = self.gui_state.is_window_open(&name);
                        if ui.checkbox(&mut open, &name).changed() {
                            self.gui_state.set_window_open(&name, open);
                        }
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("About").clicked() {
                        self.gui_state.show_about = true;
                        ui.close_menu();
                    }
                });
            });
        });

        if self.gui_state.show_about {
            egui::Window::new("About Genteel")
                .open(&mut self.gui_state.show_about)
                .show(&self.egui_ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("Genteel");
                        ui.label(format!("Version: {}", genteel::VERSION));
                    });
                    ui.separator();

                    ui.label("An instrumentable Sega Mega Drive/Genesis emulator architected for the intersection of human and machine intelligence.");
                    ui.add_space(8.0);

                    ui.group(|ui| {
                        ui.label(egui::RichText::new("🛠 Comprehensive Debugging").strong());
                        ui.label("Integrated multi-window suite for real-time VDP, CPU, Memory, and Audio analysis with GDB support.");
                    });

                    ui.group(|ui| {
                        ui.label(egui::RichText::new("🧪 CI/CD Instrumentation").strong());
                        ui.label("Headless validation, deterministic execution, and massive automated test coverage for ROM verification.");
                    });

                    ui.group(|ui| {
                        ui.label(egui::RichText::new("🤖 AI-Driven Development").strong());
                        ui.label("Serialization-first design and instrumentable APIs tailored for AI agents and machine observation.");
                    });

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.hyperlink_to("GitHub Repository", "https://github.com/segin/genteel");
                        ui.label("•");
                        ui.label("MIT License");
                    });
                });
        }

        if self.gui_state.is_window_open("Performance & Debug") {
            let mut open = true;
            egui::Window::new("Performance & Debug")
                .open(&mut open)
                .show(&self.egui_ctx, |ui| {
                    let dt = self.egui_ctx.input(|i| i.stable_dt);
                    let fps = if dt > 0.0 { 1.0 / dt } else { 0.0 };
                    ui.label(format!("Frontend FPS: {:.1}", fps));
                    ui.label(format!("Frame Time: {:.2}ms", dt * 1000.0));
                    ui.separator();
                    ui.label(format!("Internal Frames: {}", debug_info.frame_count));
                    ui.label(format!("M68k PC: {:06X}", debug_info.m68k_pc));
                    ui.label(format!("Z80 PC: {:04X}", debug_info.z80_pc));
                    ui.separator();
                    ui.label(format!(
                        "VDP Display: {}",
                        if debug_info.display_enabled {
                            "ENABLED"
                        } else {
                            "DISABLED"
                        }
                    ));
                    ui.label(format!("VDP Status: {:04X}", debug_info.vdp_status));
                    ui.label(format!("BG Color Index: {}", debug_info.bg_color_index));
                    ui.label(format!("CRAM[0] (RGB565): {:04X}", debug_info.cram[0]));
                    if ui
                        .checkbox(&mut self.gui_state.force_red, "Force Red BG (Debug)")
                        .changed()
                    {
                        self.gui_state.save();
                    }

                    #[cfg(feature = "gilrs")]
                    {
                        ui.separator();
                        ui.heading("Connected Gamepads");
                        if let Some(gilrs) = &self.gilrs {
                            let mut gamepad_str = String::with_capacity(64);
                            for (id, gamepad) in gilrs.gamepads() {
                                gamepad_str.clear();
                                let _ = write!(&mut gamepad_str, "{}: {}", id, gamepad.name());
                                ui.label(&gamepad_str);
                            }
                        } else {
                            ui.label("Gamepad support unavailable");
                        }
                    }
                });
            if !open {
                self.gui_state.set_window_open("Performance & Debug", false);
            }
        }

        if self.gui_state.is_window_open("Settings") {
            let mut open = true;
            egui::Window::new("Settings")
                .open(&mut open)
                .show(&self.egui_ctx, |ui| {
                    ui.heading("Video");
                    if ui
                        .checkbox(&mut self.gui_state.integer_scaling, "Integer Pixel Scaling")
                        .changed()
                    {
                        self.gui_state.save();
                    }
                    ui.separator();
                    ui.heading("Input");
                    ui.label("Input Mapping:");
                    if ui
                        .radio_value(
                            &mut self.gui_state.input_mapping,
                            InputMapping::Original,
                            "Original",
                        )
                        .changed()
                    {
                        self.gui_state.save();
                    }
                    if ui
                        .radio_value(
                            &mut self.gui_state.input_mapping,
                            InputMapping::Ergonomic,
                            "Ergonomic",
                        )
                        .changed()
                    {
                        self.gui_state.save();
                    }
                    ui.separator();
                    ui.heading("System");
                    if ui
                        .checkbox(&mut self.gui_state.auto_save_load, "Auto-Save/Load State")
                        .changed()
                    {
                        self.gui_state.save();
                    }
                });
            if !open {
                self.gui_state.set_window_open("Settings", false);
            }
        }

        if self.gui_state.is_window_open("Execution Control") {
            let mut open = true;
            egui::Window::new("Execution Control")
                .open(&mut open)
                .show(&self.egui_ctx, |ui| {
                    ui.horizontal(|ui| {
                        if self.gui_state.paused {
                            if ui.button("▶ Resume").clicked() {
                                self.gui_state.paused = false;
                                self.gui_state.save();
                            }
                        } else {
                            if ui.button("⏸ Pause").clicked() {
                                self.gui_state.paused = true;
                                self.gui_state.save();
                            }
                        }
                        if ui.button("⏭ Single Step").clicked() {
                            self.gui_state.single_step = true;
                            self.gui_state.paused = true;
                        }
                    });
                });
            if !open {
                self.gui_state.set_window_open("Execution Control", false);
            }
        }

        if self.gui_state.is_window_open("M68k Status") {
            let mut open = true;
            egui::Window::new("M68k Status")
                .open(&mut open)
                .show(&self.egui_ctx, |ui| {
                    ui.label(format!("PC: {:06X}", debug_info.m68k_pc));
                    ui.label(format!("SR: {:04X}", debug_info.m68k_sr));
                    let sr = debug_info.m68k_sr;
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "Flags: [ {} {} {} {} {} ]",
                            if sr & 0x10 != 0 { "X" } else { "x" },
                            if sr & 0x08 != 0 { "N" } else { "n" },
                            if sr & 0x04 != 0 { "Z" } else { "z" },
                            if sr & 0x02 != 0 { "V" } else { "v" },
                            if sr & 0x01 != 0 { "C" } else { "c" },
                        ));
                    });
                    ui.separator();
                    ui.columns(2, |columns| {
                        for i in 0..8 {
                            columns[0].label(format!("D{}: {:08X}", i, debug_info.m68k_d[i]));
                            columns[1].label(format!("A{}: {:08X}", i, debug_info.m68k_a[i]));
                        }
                    });
                    ui.separator();
                    ui.label(format!("USP: {:08X}", debug_info.m68k_usp));
                    ui.label(format!("SSP: {:08X}", debug_info.m68k_ssp));
                });
            if !open {
                self.gui_state.set_window_open("M68k Status", false);
            }
        }

        if self.gui_state.is_window_open("Z80 Status") {
            let mut open = true;
            egui::Window::new("Z80 Status")
                .open(&mut open)
                .show(&self.egui_ctx, |ui| {
                    ui.label(format!("PC: {:04X}", debug_info.z80_pc));
                    ui.label(format!("SP: {:04X}", debug_info.z80_sp));
                    ui.label(format!("MEMPTR (WZ): {:04X}", debug_info.z80_memptr));
                    ui.separator();
                    let f = debug_info.z80_f;
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "Flags: [ {} {} {} {} {} {} {} {} ]",
                            if f & 0x80 != 0 { "S" } else { "s" },
                            if f & 0x40 != 0 { "Z" } else { "z" },
                            if f & 0x20 != 0 { "Y" } else { "y" },
                            if f & 0x10 != 0 { "H" } else { "h" },
                            if f & 0x08 != 0 { "X" } else { "x" },
                            if f & 0x04 != 0 { "P" } else { "p" },
                            if f & 0x02 != 0 { "N" } else { "n" },
                            if f & 0x01 != 0 { "C" } else { "c" },
                        ));
                    });
                    ui.separator();
                    ui.columns(2, |columns| {
                        columns[0].label(format!("A:  {:02X}", debug_info.z80_a));
                        columns[1].label(format!("F:  {:02X}", debug_info.z80_f));
                        columns[0].label(format!(
                            "BC: {:02X}{:02X}",
                            debug_info.z80_b, debug_info.z80_c
                        ));
                        columns[1].label(format!(
                            "DE: {:02X}{:02X}",
                            debug_info.z80_d, debug_info.z80_e
                        ));
                        columns[0].label(format!(
                            "HL: {:02X}{:02X}",
                            debug_info.z80_h, debug_info.z80_l
                        ));
                        columns[1].label(format!("IX: {:04X}", debug_info.z80_ix));
                        columns[0].label(format!("IY: {:04X}", debug_info.z80_iy));
                        columns[1].label(format!("I:  {:02X}", debug_info.z80_i));
                        columns[0].label(format!("R:  {:02X}", debug_info.z80_r));
                    });
                    ui.separator();
                    ui.label(format!("IM: {}", debug_info.z80_im));
                    ui.label(format!("IFF1: {}", debug_info.z80_iff1));
                });
            if !open {
                self.gui_state.set_window_open("Z80 Status", false);
            }
        }

        if self.gui_state.is_window_open("Disassembly") {
            let mut open = true;
            egui::Window::new("Disassembly")
                .open(&mut open)
                .show(&self.egui_ctx, |ui| {
                    ui.heading("M68k Disassembly");
                    egui::ScrollArea::vertical()
                        .id_source("m68k_disasm")
                        .show(ui, |ui| {
                            let mut label_buffer = String::with_capacity(64);
                            for (addr, text) in &debug_info.m68k_disasm {
                                label_buffer.clear();
                                let is_current = *addr == debug_info.m68k_pc;
                                if is_current {
                                    let _ = write!(&mut label_buffer, "-> {:06X}: {}", addr, text);
                                    ui.colored_label(egui::Color32::YELLOW, label_buffer.as_str());
                                } else {
                                    let _ = write!(&mut label_buffer, "   {:06X}: {}", addr, text);
                                    ui.label(label_buffer.as_str());
                                }
                            }
                        });
                    ui.separator();
                    ui.heading("Z80 Disassembly");
                    egui::ScrollArea::vertical()
                        .id_source("z80_disasm")
                        .show(ui, |ui| {
                            let mut label_buffer = String::with_capacity(64);
                            for (addr, text) in &debug_info.z80_disasm {
                                label_buffer.clear();
                                let is_current = *addr == debug_info.z80_pc;
                                if is_current {
                                    let _ = write!(&mut label_buffer, "-> {:04X}: {}", addr, text);
                                    ui.colored_label(egui::Color32::YELLOW, label_buffer.as_str());
                                } else {
                                    let _ = write!(&mut label_buffer, "   {:04X}: {}", addr, text);
                                    ui.label(label_buffer.as_str());
                                }
                            }
                        });
                });
            if !open {
                self.gui_state.set_window_open("Disassembly", false);
            }
        }

        if self.gui_state.is_window_open("Palette Viewer") {
            let mut open = true;
            egui::Window::new("Palette Viewer")
                .open(&mut open)
                .show(&self.egui_ctx, |ui| {
                    for palette in 0..4 {
                        ui.horizontal(|ui| {
                            ui.label(format!("Pal {}:", palette));
                            for i in 0..16 {
                                let idx = palette * 16 + i;
                                let color565 = debug_info.cram[idx];
                                let r = (((color565 >> 11) & 0x1F) << 3) as u8;
                                let g = (((color565 >> 5) & 0x3F) << 2) as u8;
                                let b = ((color565 & 0x1F) << 3) as u8;
                                let color = egui::Color32::from_rgb(r, g, b);

                                let (rect, _response) = ui.allocate_at_least(
                                    egui::vec2(16.0, 16.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().rect_filled(rect, 0.0, color);
                                if _response.hovered() {
                                    _response.on_hover_text(format!(
                                        "Index: {}\nRaw: {:04X}\nRGB565: {:04X}",
                                        idx, debug_info.cram_raw[idx], color565
                                    ));
                                }
                            }
                        });
                    }
                });
            if !open {
                self.gui_state.set_window_open("Palette Viewer", false);
            }
        }

        if self.gui_state.is_window_open("Tile Viewer") {
            let mut open = true;
            egui::Window::new("Tile Viewer")
                .open(&mut open)
                .show(&self.egui_ctx, |ui| {
                    // Render tiles to a buffer
                    let mut pixels = vec![0u8; 128 * 1024 * 4]; // RGBA
                    for tile_idx in 0..2048 {
                        let tile_x = (tile_idx % 16) * 8;
                        let tile_y = (tile_idx / 16) * 8;

                        for y in 0..8 {
                            let row_addr = tile_idx * 32 + y * 4;
                            for x in 0..8 {
                                let byte = debug_info.vram[row_addr + (x / 2)];
                                let color_idx = if x % 2 == 0 { byte >> 4 } else { byte & 0x0F };

                                // Use first palette (0-15)
                                let color565 = debug_info.cram[color_idx as usize];
                                let r = (((color565 >> 11) & 0x1F) << 3) as u8;
                                let g = (((color565 >> 5) & 0x3F) << 2) as u8;
                                let b = ((color565 & 0x1F) << 3) as u8;

                                let pixel_idx = ((tile_y + y) * 128 + (tile_x + x)) * 4;
                                pixels[pixel_idx] = r;
                                pixels[pixel_idx + 1] = g;
                                pixels[pixel_idx + 2] = b;
                                pixels[pixel_idx + 3] = 255;
                            }
                        }
                    }

                    let image = egui::ColorImage::from_rgba_unmultiplied([128, 1024], &pixels);
                    let texture = self.tile_texture.get_or_insert_with(|| {
                        ui.ctx()
                            .load_texture("tile_viewer", image.clone(), Default::default())
                    });
                    texture.set(image, Default::default());

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.image(&*texture);
                    });
                });
            if !open {
                self.gui_state.set_window_open("Tile Viewer", false);
            }
        }

        if self.gui_state.is_window_open("Sprite Viewer") {
            let mut open = true;
            egui::Window::new("Sprite Viewer")
                .open(&mut open)
                .show(&self.egui_ctx, |ui| {
                    let sat_base = ((debug_info.vdp_registers[5] as usize) & 0x7F) << 9;
                    let h40 = (debug_info.vdp_registers[12] & 0x81) == 0x81;
                    let max_sprites = if h40 { 80 } else { 64 };

                    let iter = crate::vdp::SpriteIterator {
                        vram: &debug_info.vram,
                        next_idx: 0,
                        count: 0,
                        max_sprites,
                        sat_base,
                    };

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        egui::Grid::new("sprite_grid").striped(true).show(ui, |ui| {
                            ui.label("Idx");
                            ui.label("Pos");
                            ui.label("Size");
                            ui.label("Tile");
                            ui.label("Pal");
                            ui.label("Pri");
                            ui.label("Flip");
                            ui.label("Link");
                            ui.end_row();

                            for attr in iter {
                                ui.label(format!("{}", attr.index));
                                ui.label(format!("{},{}", attr.h_pos, attr.v_pos));
                                ui.label(format!("{}x{}", attr.h_size, attr.v_size));
                                ui.label(format!("{:03X}", attr.base_tile));
                                ui.label(format!("{}", attr.palette));
                                ui.label(if attr.priority { "H" } else { "L" });
                                ui.label(format!(
                                    "{}{}",
                                    if attr.h_flip { "H" } else { "-" },
                                    if attr.v_flip { "V" } else { "-" }
                                ));
                                ui.label(format!("{}", attr.link));
                                ui.end_row();
                            }
                        });
                    });
                });
            if !open {
                self.gui_state.set_window_open("Sprite Viewer", false);
            }
        }

        if self.gui_state.is_window_open("Scroll Plane Viewer") {
            let mut open = true;
            egui::Window::new("Scroll Plane Viewer")
                .open(&mut open)
                .show(&self.egui_ctx, |ui| {
                    let size_bits = debug_info.vdp_registers[crate::vdp::REG_PLANE_SIZE];
                    let plane_w = match size_bits & 0x03 {
                        0x00 => 32,
                        0x01 => 64,
                        0x03 => 128,
                        _ => 32,
                    };
