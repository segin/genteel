#![deny(warnings)]
use crate::frontend::{self, DebugInfo, InputMapping};
use crate::input::InputScript;
use crate::vdp::render::RenderOps;
use crate::{audio, Emulator, SLOT_EXTS};
use genteel::VERSION;
use egui::epaint::ahash::HashMap;
use egui_wgpu::wgpu;
use pixels::{Pixels, SurfaceTexture};
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use winit::event::{ElementState, Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::WindowBuilder;

pub const SLOT_NAMES: [&str; 10] = [
    "Slot 0", "Slot 1", "Slot 2", "Slot 3", "Slot 4", "Slot 5", "Slot 6", "Slot 7", "Slot 8",
    "Slot 9",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaneTab {
    PlaneA,
    PlaneB,
}

#[derive(Serialize, Deserialize)]
pub struct GuiState {
    pub windows: HashMap<String, bool>,
    pub recent_roms: Vec<PathBuf>,
    pub input_mapping: InputMapping,
    pub integer_scaling: bool,
    pub auto_save_load: bool,
    pub force_red: bool,
    pub scroll_plane_tab: PlaneTab,
    #[serde(skip)]
    pub pick_rom_requested: bool,
    #[serde(skip)]
    pub reset_requested: bool,
    #[serde(skip)]
    pub close_requested: bool,
    #[serde(skip)]
    pub save_requested: Option<u8>,
    #[serde(skip)]
    pub load_requested: Option<u8>,
    #[serde(skip)]
    pub delete_state_requested: Option<u8>,
    #[serde(skip)]
    pub show_about: bool,
}

impl Default for GuiState {
    fn default() -> Self {
        let mut windows = HashMap::default();
        windows.insert("Performance & Debug".to_string(), true);
        windows.insert("Settings".to_string(), false);
        windows.insert("Palette Viewer".to_string(), false);
        windows.insert("Tile Viewer".to_string(), false);
        windows.insert("Sprite Viewer".to_string(), false);
        windows.insert("Scroll Plane Viewer".to_string(), false);
        windows.insert("VDP Memory Hex".to_string(), false);
        windows.insert("Memory Viewer".to_string(), false);
        windows.insert("Sound Chip Visualizer".to_string(), false);
        windows.insert("State Browser".to_string(), false);

        Self {
            windows,
            recent_roms: Vec::new(),
            input_mapping: InputMapping::Original,
            integer_scaling: false,
            auto_save_load: true,
            force_red: false,
            scroll_plane_tab: PlaneTab::PlaneA,
            pick_rom_requested: false,
            reset_requested: false,
            close_requested: false,
            save_requested: None,
            load_requested: None,
            delete_state_requested: None,
            show_about: false,
        }
    }
}

impl GuiState {
    pub fn load() -> Self {
        if let Ok(data) = std::fs::read_to_string("gui_config.json") {
            if let Ok(state) = serde_json::from_str(&data) {
                return state;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        if let Ok(data) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write("gui_config.json", data);
        }
    }

    pub fn is_window_open(&self, name: &str) -> bool {
        *self.windows.get(name).unwrap_or(&false)
    }

    pub fn set_window_open(&mut self, name: &str, open: bool) {
        self.windows.insert(name.to_string(), open);
        self.save();
    }

    pub fn add_recent(&mut self, path: PathBuf) {
        self.recent_roms.retain(|p| p != &path);
        self.recent_roms.insert(0, path);
        if self.recent_roms.len() > 10 {
            self.recent_roms.pop();
        }
        self.save();
    }
}

pub struct Framework {
    pub egui_ctx: egui::Context,
    pub egui_state: egui_winit::State,
    pub screen_descriptor: egui_wgpu::ScreenDescriptor,
    pub renderer: egui_wgpu::Renderer,
    pub gui_state: GuiState,
    pub pending_rom_path: Arc<Mutex<Option<PathBuf>>>,
    pub tile_texture: Option<egui::TextureHandle>,
    pub plane_a_texture: Option<egui::TextureHandle>,
    pub plane_b_texture: Option<egui::TextureHandle>,
    #[cfg(feature = "gilrs")]
    pub gilrs: Option<gilrs::Gilrs>,
}

impl Framework {
    pub fn new(
        event_loop: &EventLoop<()>,
        width: u32,
        height: u32,
        scale_factor: f32,
        pixels: &Pixels,
        input_mapping: InputMapping,
    ) -> Self {
        let egui_ctx = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::viewport::ViewportId::ROOT,
            event_loop,
            Some(scale_factor),
            None,
        );
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: scale_factor,
        };
        let renderer = egui_wgpu::Renderer::new(pixels.device(), pixels.render_texture_format(), None, 1);
        let mut gui_state = GuiState::load();
        gui_state.input_mapping = input_mapping;

        Self {
            egui_ctx,
            egui_state,
            screen_descriptor,
            renderer,
            gui_state,
            pending_rom_path: Arc::new(Mutex::new(None)),
            tile_texture: None,
            plane_a_texture: None,
            plane_b_texture: None,
            #[cfg(feature = "gilrs")]
            gilrs: gilrs::Gilrs::new().ok(),
        }
    }

    pub fn handle_event(&mut self, window: &winit::window::Window, event: &WindowEvent) {
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
                    if ui.button("Settings...").clicked() {
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
                        ui.label(format!("Version: {}", VERSION));
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

        // Central panel for the game view and status bar
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::TRANSPARENT))
            .show(&self.egui_ctx, |ui: &mut egui::Ui| {
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui: &mut egui::Ui| {
                    ui.group(|ui: &mut egui::Ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui: &mut egui::Ui| {
                            if debug_info.has_rom {
                                let filename = debug_info.current_rom_path.as_ref()
                                    .and_then(|p| p.file_name())
                                    .and_then(|f| f.to_str())
                                    .unwrap_or("Unknown");
                                ui.label(egui::RichText::new(format!("🎮 {}", filename)).strong());
                            } else {
                                ui.label(egui::RichText::new("📂 No ROM Loaded").weak());
                            }
                            
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui: &mut egui::Ui| {
                                let dt = self.egui_ctx.input(|i| i.stable_dt);
                                let fps = if dt > 0.0 { 1.0 / dt } else { 0.0 };
                                ui.label(format!("{:.0} FPS", fps));
                                ui.separator();
                                ui.label(if debug_info.display_enabled { "VDP: ON" } else { "VDP: OFF" });
                            });
                        });
                    });
                });
            });

        if self.gui_state.is_window_open("Performance & Debug") {
            let mut open = true;
            egui::Window::new("Performance & Debug")
                .open(&mut open)
                .resizable(true)
                .default_width(300.0)
                .show(&self.egui_ctx, |ui| {
                    ui.collapsing("🚀 Overview", |ui| {
                        egui::Grid::new("perf_overview").show(ui, |ui| {
                            let dt = self.egui_ctx.input(|i| i.stable_dt);
                            let fps = if dt > 0.0 { 1.0 / dt } else { 0.0 };
                            ui.label("Frontend FPS:");
                            ui.label(format!("{:.1}", fps));
                            ui.end_row();
                            ui.label("Frame Time:");
                            ui.label(format!("{:.2}ms", dt * 1000.0));
                            ui.end_row();
                            ui.label("Internal Frames:");
                            ui.label(format!("{}", debug_info.frame_count));
                            ui.end_row();
                        });
                    });

                    ui.collapsing("💻 CPU State", |ui| {
                        egui::Grid::new("cpu_overview").show(ui, |ui| {
                            ui.label("M68k PC:");
                            ui.label(egui::RichText::new(format!("{:06X}", debug_info.m68k_pc)).monospace());
                            ui.end_row();
                            ui.label("Z80 PC:");
                            ui.label(egui::RichText::new(format!("{:04X}", debug_info.z80_pc)).monospace());
                            ui.end_row();
                        });
                    });

                    ui.collapsing("📺 VDP Status", |ui| {
                        egui::Grid::new("vdp_overview").show(ui, |ui| {
                            ui.label("Display:");
                            ui.label(if debug_info.display_enabled {
                                egui::RichText::new("ENABLED").color(egui::Color32::GREEN)
                            } else {
                                egui::RichText::new("DISABLED").color(egui::Color32::RED)
                            });
                            ui.end_row();
                            ui.label("Status:");
                            ui.label(egui::RichText::new(format!("{:04X}", debug_info.vdp_status)).monospace());
                            ui.end_row();
                            ui.label("BG Index:");
                            ui.label(format!("{}", debug_info.bg_color_index));
                            ui.end_row();
                        });
                        
                        if ui.checkbox(&mut self.gui_state.force_red, "Force Red BG (Debug)").changed() {
                            self.gui_state.save();
                        }
                    });

                    #[cfg(feature = "gilrs")]
                    {
                        ui.collapsing("🎮 Input Devices", |ui| {
                            if let Some(gilrs) = &self.gilrs {
                                for (id, gamepad) in gilrs.gamepads() {
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new(format!("{}", id)).strong());
                                        ui.label(gamepad.name());
                                    });
                                }
                            } else {
                                ui.label("Gamepad support unavailable");
                            }
                        });
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
                .resizable(true)
                .default_width(250.0)
                .show(&self.egui_ctx, |ui| {
                    ui.collapsing("🎬 Video", |ui| {
                        if ui.checkbox(&mut self.gui_state.integer_scaling, "Integer Pixel Scaling").changed() {
                            self.gui_state.save();
                        }
                    });

                    ui.collapsing("⌨ Input", |ui| {
                        ui.label("Keyboard Layout:");
                        ui.horizontal(|ui| {
                            if ui.selectable_value(&mut self.gui_state.input_mapping, InputMapping::Original, "Original").changed() {
                                self.gui_state.save();
                            }
                            if ui.selectable_value(&mut self.gui_state.input_mapping, InputMapping::Ergonomic, "Ergonomic").changed() {
                                self.gui_state.save();
                            }
                        });
                    });

                    ui.collapsing("⚙ System", |ui| {
                        if ui.checkbox(&mut self.gui_state.auto_save_load, "Auto-Save/Load State").changed() {
                            self.gui_state.save();
                        }
                    });
                });
            if !open {
                self.gui_state.set_window_open("Settings", false);
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
                    let (plane_w, plane_h) = crate::vdp::Vdp::decode_plane_size(size_bits);

                    ui.label(format!("Plane Size: {}x{}", plane_w, plane_h));

                    ui.horizontal(|ui| {
                        if ui
                            .selectable_value(
                                &mut self.gui_state.scroll_plane_tab,
                                PlaneTab::PlaneA,
                                "Plane A",
                            )
                            .changed()
                        {
                            self.gui_state.save();
                        }
                        if ui
                            .selectable_value(
                                &mut self.gui_state.scroll_plane_tab,
                                PlaneTab::PlaneB,
                                "Plane B",
                            )
                            .changed()
                        {
                            self.gui_state.save();
                        }
                    });

                    let plane_a_base = ((debug_info.vdp_registers[2] as usize) & 0x38) << 10;
                    let plane_b_base = ((debug_info.vdp_registers[4] as usize) & 0x07) << 13;

                    let render_plane = |ui: &mut egui::Ui,
                                        base: usize,
                                        texture_opt: &mut Option<egui::TextureHandle>,
                                        name: &str| {
                        let mut pixels = vec![0u8; plane_w as usize * plane_h as usize * 4];
                        for tile_y in 0..(plane_h / 8) {
                            for tile_x in 0..(plane_w / 8) {
                                let addr = base + (tile_y as usize * (plane_w as usize / 8) + tile_x as usize) * 2;
                                let word = ((debug_info.vram[addr] as u16) << 8) | debug_info.vram[addr + 1] as u16;
                                let tile_idx = word & 0x07FF;
                                let pal = (word >> 13) & 0x03;
                                let hflip = (word & 0x0800) != 0;
                                let vflip = (word & 0x1000) != 0;

                                for py in 0..8 {
                                    let row_addr = tile_idx as usize * 32 + (if vflip { 7 - py } else { py }) * 4;
                                    for px in 0..8 {
                                        let byte = debug_info.vram[row_addr + (px / 2)];
                                        let color_idx = if (px % 2 == 0) ^ hflip { byte >> 4 } else { byte & 0x0F };
                                        let color565 = debug_info.cram[pal as usize * 16 + color_idx as usize];
                                        let r = (((color565 >> 11) & 0x1F) << 3) as u8;
                                        let g = (((color565 >> 5) & 0x3F) << 2) as u8;
                                        let b = ((color565 & 0x1F) << 3) as u8;

                                        let pixel_idx = ((tile_y * 8 + py) as usize * plane_w as usize + (tile_x * 8 + px) as usize) * 4;
                                        pixels[pixel_idx] = r;
                                        pixels[pixel_idx + 1] = g;
                                        pixels[pixel_idx + 2] = b;
                                        pixels[pixel_idx + 3] = 255;
                                    }
                                }
                            }
                        }

                        let image = egui::ColorImage::from_rgba_unmultiplied(
                            [plane_w as usize, plane_h as usize],
                            &pixels,
                        );
                        let texture = texture_opt.get_or_insert_with(|| {
                            ui.ctx().load_texture(name, image.clone(), Default::default())
                        });
                        texture.set(image, Default::default());
                        egui::ScrollArea::both().show(ui, |ui| {
                            ui.image(&*texture);
                        });
                    };

                    match self.gui_state.scroll_plane_tab {
                        PlaneTab::PlaneA => render_plane(ui, plane_a_base, &mut self.plane_a_texture, "plane_a"),
                        PlaneTab::PlaneB => render_plane(ui, plane_b_base, &mut self.plane_b_texture, "plane_b"),
                    }
                });
            if !open {
                self.gui_state.set_window_open("Scroll Plane Viewer", false);
            }
        }

        if self.gui_state.is_window_open("VDP Memory Hex") {
            let mut open = true;
            egui::Window::new("VDP Memory Hex")
                .open(&mut open)
                .show(&self.egui_ctx, |ui| {
                    ui.collapsing("VRAM", |ui| {
                        egui::ScrollArea::vertical()
                            .id_source("vram_hex")
                            .show_rows(
                                ui,
                                ui.text_style_height(&egui::TextStyle::Monospace),
                                0x10000 / 16,
                                |ui, row_range| {
                                    egui::Grid::new("vram_grid").show(ui, |ui| {
                                        for row in row_range {
                                            let addr = row * 16;
                                            ui.label(
                                                egui::RichText::new(format!("{:04X}:", addr))
                                                    .monospace(),
                                            );
                                            for i in 0..16 {
                                                ui.label(
                                                    egui::RichText::new(format!(
                                                        "{:02X}",
                                                        debug_info.vram[addr + i]
                                                    ))
                                                    .monospace(),
                                                );
                                            }
                                            ui.end_row();
                                        }
                                    });
                                },
                            );
                    });
                    ui.collapsing("CRAM", |ui| {
                        egui::Grid::new("cram_grid").show(ui, |ui| {
                            for row in 0..4 {
                                let addr = row * 16;
                                ui.label(egui::RichText::new(format!("{:02X}:", addr)).monospace());
                                for i in 0..16 {
                                    let val = if (addr + i) % 2 == 0 {
                                        debug_info.cram_raw[(addr + i) / 2] >> 8
                                    } else {
                                        debug_info.cram_raw[(addr + i) / 2] & 0xFF
                                    } as u8;
                                    ui.label(
                                        egui::RichText::new(format!("{:02X}", val)).monospace(),
                                    );
                                }
                                ui.end_row();
                            }
                        });
                    });
                    ui.collapsing("VSRAM", |ui| {
                        egui::Grid::new("vsram_grid").show(ui, |ui| {
                            for row in 0..5 {
                                let addr = row * 16;
                                ui.label(egui::RichText::new(format!("{:02X}:", addr)).monospace());
                                for i in 0..16 {
                                    if addr + i < 80 {
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "{:02X}",
                                                debug_info.vsram[addr + i]
                                            ))
                                            .monospace(),
                                        );
                                    } else {
                                        ui.label("  ");
                                    }
                                }
                                ui.end_row();
                            }
                        });
                    });
                });
            if !open {
                self.gui_state.set_window_open("VDP Memory Hex", false);
            }
        }

        if self.gui_state.is_window_open("Memory Viewer") {
            let mut open = true;
            egui::Window::new("Memory Viewer")
                .open(&mut open)
                .show(&self.egui_ctx, |ui| {
                    ui.collapsing("Work RAM (M68k)", |ui| {
                        egui::ScrollArea::vertical()
                            .id_source("wram_hex")
                            .show_rows(
                                ui,
                                ui.text_style_height(&egui::TextStyle::Monospace),
                                0x10000 / 16,
                                |ui, row_range| {
                                    egui::Grid::new("wram_grid").show(ui, |ui| {
                                        let mut label_buffer = String::with_capacity(16);
                                        let mut hex_buffer = String::with_capacity(4);
                                        for row in row_range {
                                            let addr = row * 16;
                                            label_buffer.clear();
                                            let _ = write!(&mut label_buffer, "{:04X}:", addr);
                                            ui.label(
                                                egui::RichText::new(&label_buffer).monospace(),
                                            );
                                            for i in 0..16 {
                                                hex_buffer.clear();
                                                let _ = write!(
                                                    &mut hex_buffer,
                                                    "{:02X}",
                                                    debug_info.wram[addr + i]
                                                );
                                                ui.label(
                                                    egui::RichText::new(&hex_buffer).monospace(),
                                                );
                                            }
                                            ui.end_row();
                                        }
                                    });
                                },
                            );
                    });
                    ui.collapsing("Z80 RAM", |ui| {
                        egui::ScrollArea::vertical()
                            .id_source("z80_ram_hex")
                            .show_rows(
                                ui,
                                ui.text_style_height(&egui::TextStyle::Monospace),
                                0x2000 / 16,
                                |ui, row_range| {
                                    egui::Grid::new("z80_ram_grid").show(ui, |ui| {
                                        let mut label_buffer = String::with_capacity(16);
                                        let mut hex_buffer = String::with_capacity(4);
                                        for row in row_range {
                                            let addr = row * 16;
                                            label_buffer.clear();
                                            let _ = write!(&mut label_buffer, "{:04X}:", addr);
                                            ui.label(
                                                egui::RichText::new(&label_buffer).monospace(),
                                            );
                                            for i in 0..16 {
                                                hex_buffer.clear();
                                                let _ = write!(
                                                    &mut hex_buffer,
                                                    "{:02X}",
                                                    debug_info.z80_ram[addr + i]
                                                );
                                                ui.label(
                                                    egui::RichText::new(&hex_buffer).monospace(),
                                                );
                                            }
                                            ui.end_row();
                                        }
                                    });
                                },
                            );
                    });
                });
            if !open {
                self.gui_state.set_window_open("Memory Viewer", false);
            }
        }

        if self.gui_state.is_window_open("Sound Chip Visualizer") {
            let mut open = true;
            egui::Window::new("Sound Chip Visualizer")
                .open(&mut open)
                .show(&self.egui_ctx, |ui| {
                    ui.collapsing("SN76489 PSG", |ui| {
                        for i in 0..3 {
                            ui.horizontal(|ui| {
                                ui.label(format!("Tone {}:", i));
                                ui.label(format!("Freq: {:03X}", debug_info.psg_tone[i].frequency));
                                ui.label(format!("Vol: {:01X}", debug_info.psg_tone[i].volume));
                                let vol_norm = 1.0 - (debug_info.psg_tone[i].volume as f32 / 15.0);
                                ui.add(egui::ProgressBar::new(vol_norm).show_percentage());
                            });
                        }
                        ui.horizontal(|ui| {
                            ui.label("Noise:");
                            ui.label(format!("Type: {}", if debug_info.psg_noise.white { "White" } else { "Periodic" }));
                            ui.label(format!("Rate: {}", debug_info.psg_noise.rate));
                            let vol_norm = 1.0 - (debug_info.psg_noise.volume as f32 / 15.0);
                            ui.add(egui::ProgressBar::new(vol_norm).show_percentage());
                        });
                    });
                    ui.collapsing("YM2612 FM", |ui| {
                        ui.label("FM visualization coming soon...");
                    });
                });
            if !open {
                self.gui_state.set_window_open("Sound Chip Visualizer", false);
            }
        }

        if self.gui_state.is_window_open("State Browser") {
            let mut open = true;
            egui::Window::new("State Browser")
                .open(&mut open)
                .show(&self.egui_ctx, |ui| {
                    if let Some(rom_path) = &debug_info.current_rom_path {
                        for slot in 0..10 {
                            ui.horizontal(|ui| {
                                ui.label(SLOT_NAMES[slot as usize]);
                                let state_path = rom_path.with_extension(SLOT_EXTS[slot as usize]);
                                if state_path.exists() {
                                    if ui.button("Load").clicked() {
                                        self.gui_state.load_requested = Some(slot);
                                    }
                                    if ui.button("Overwrite").clicked() {
                                        self.gui_state.save_requested = Some(slot);
                                    }
                                    if ui.button("Delete").clicked() {
                                        self.gui_state.delete_state_requested = Some(slot);
                                    }
                                    let metadata = std::fs::metadata(&state_path);
                                    if let Ok(m) = metadata {
                                        if let Ok(t) = m.modified() {
                                            use chrono::{DateTime, Local};
                                            let dt: DateTime<Local> = t.into();
                                            ui.label(egui::RichText::new(dt.format("%Y-%m-%d %H:%M").to_string()).weak());
                                        }
                                    }
                                } else {
                                    if ui.button("Save New").clicked() {
                                        self.gui_state.save_requested = Some(slot);
                                    }
                                    ui.label(egui::RichText::new("Empty").weak());
                                }
                            });
                        }
                    } else {
                        ui.label("Load a ROM to use save states");
                    }
                });
            if !open {
                self.gui_state.set_window_open("State Browser", false);
            }
        }
    }

    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        render_target: &wgpu::TextureView,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        let full_output = self.egui_ctx.end_frame();
        let paint_jobs = self
            .egui_ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);
        // Update textures
        for (id, image_delta) in full_output.textures_delta.set {
            self.renderer
                .update_texture(device, queue, id, &image_delta);
        }
        // Prepare renderer
        self.renderer
            .update_buffers(device, queue, encoder, &paint_jobs, &self.screen_descriptor);
        // Render GUI
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: render_target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            self.renderer
                .render(&mut rpass, &paint_jobs, &self.screen_descriptor);
        }
        // Clean up textures
        for id in full_output.textures_delta.free {
            self.renderer.free_texture(&id);
        }
    }

    pub fn handle_exit(&mut self, emulator: &mut Emulator, record_path: &Option<String>) {
        if self.gui_state.auto_save_load {
            if let Some(path) = &emulator.current_rom_path {
                emulator.save_state_to_path(path.with_extension("auto"));
            }
        }
        self.gui_state.save();
        if let Some(path) = record_path {
            let script: InputScript = emulator.input.stop_recording();
            if let Err(e) = script.save(path) {
                eprintln!("Failed to save recorded script: {}", e);
            } else {
                println!("Recorded script saved to: {}", path);
            }
        }
    }
}

#[cfg(feature = "gui")]
pub fn run(mut emulator: Emulator, record_path: Option<String>) -> Result<(), String> {
    if emulator.input_mapping == InputMapping::Original {
        println!("Controls: Arrow keys=D-pad, Z=A, X=B, C=C, Enter=Start");
    } else {
        println!("Controls: WASD/Arrows=D-pad, J/Z=A, K/X=B, L/C=C, U=X, I=Y, O=Z, Enter=Start, Space=Mode");
    }
    println!("Press Escape to quit.");
    if let Some(path) = &record_path {
        println!("Recording inputs to: {}", path);
        emulator.input.start_recording();
    }
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let size = winit::dpi::LogicalSize::new(
        frontend::GENESIS_WIDTH as f64 * 3.0,
        frontend::GENESIS_HEIGHT as f64 * 3.0,
    );
    let window = WindowBuilder::new()
        .with_title("Genteel - Sega Genesis Emulator")
        .with_inner_size(size)
        .with_min_inner_size(winit::dpi::LogicalSize::new(
            frontend::GENESIS_WIDTH as f64,
            frontend::GENESIS_HEIGHT as f64,
        ))
        .build(&event_loop)
        .map_err(|e| e.to_string())?;
    // Leak the window to get a &'static Window, simplifying lifetime management
    let window: &'static winit::window::Window = Box::leak(Box::new(window));
    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, window);
        Pixels::new(320, 240, surface_texture).map_err(|e| e.to_string())?
    };
    // Initialize egui framework
    let mut framework = Framework::new(
        &event_loop,
        window.inner_size().width,
        window.inner_size().height,
        window.scale_factor() as f32,
        &pixels,
        emulator.input_mapping,
    );
    // Audio setup
    let audio_buffer = audio::create_audio_buffer();
    let audio_output = match audio::AudioOutput::new(audio_buffer.clone()) {
        Ok(output) => {
            emulator.bus.borrow_mut().sample_rate = output.sample_rate;
            Some(output)
        }
        Err(e) => {
            eprintln!("Warning: Failed to initialize audio: {}", e);
            None
        }
    };
    let _audio_output = audio_output;
    // Input and Timing state
    let mut input = crate::input::FrameInput::default();
    let mut last_frame_inst = std::time::Instant::now();
    let frame_duration = std::time::Duration::from_nanos(16_666_667); // 60.0 fps
    println!("Starting event loop...");
    event_loop
        .run(move |event, target| {
            match event {
                Event::WindowEvent { event, .. } => {
                    // Handle GUI events
                    framework.handle_event(window, &event);
                    match event {
                        WindowEvent::CloseRequested => {
                            println!("Using CloseRequested to exit");
                            framework.handle_exit(&mut emulator, &record_path);
                            target.exit();
                        }
                        WindowEvent::Resized(size) => {
                            if let Err(e) = pixels.resize_surface(size.width, size.height) {
                                eprintln!("Pixels resize error: {}", e);
                                target.exit();
                                return;
                            }
                            framework.resize(size.width, size.height);
                        }
                        WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                            framework.scale_factor(scale_factor as f32);
                        }
                        WindowEvent::KeyboardInput {
                            event: key_event, ..
                        } => {
                            // If egui wants focus, don't process game input
                            if framework.egui_ctx.wants_keyboard_input() {
                                return;
                            }
                            let pressed = key_event.state == ElementState::Pressed;
                            // 1. Try physical key first
                            let mut handled = false;
                            if let PhysicalKey::Code(keycode) = key_event.physical_key {
                                if keycode == KeyCode::Escape && pressed {
                                    println!("Escape pressed, exiting");
                                    framework.handle_exit(&mut emulator, &record_path);
                                    target.exit();
                                    return;
                                }
                                if let Some((button, _)) =
                                    frontend::keycode_to_button(keycode, emulator.input_mapping)
                                {
                                    input.p1.set_button(button, pressed);
                                    handled = true;
                                }
                            }
                            // 2. Try logical key if physical wasn't handled (layout independent)
                            if !handled {
                                if let Some((button, _)) =
                                    frontend::key_to_button(&key_event.logical_key, emulator.input_mapping)
                                {
                                    input.p1.set_button(button, pressed);
                                }
                            }
                        }
                        WindowEvent::RedrawRequested => {
                            // 1. Update Game State
                            emulator.step_frame(Some(&input));
                            
                            // Apply force_red debug hack
                            if framework.gui_state.force_red {
                                let mut bus = emulator.bus.borrow_mut();
                                // Genesis CRAM is 9-bit (BBB GGG RRR), stored as BE u16.
                                // Red is 0x000E.
                                bus.vdp.cram[0] = 0x00;
                                bus.vdp.cram[1] = 0x0E;
                            }

                            // 2. Handle GUI Requests
                            if framework.gui_state.pick_rom_requested {
                                if let Some(path) = FileDialog::new()
                                    .add_filter("Genesis ROMs", &["bin", "md", "gen", "smd", "zip", "32x"])
                                    .pick_file()
                                {
                                    let mut lock = framework.pending_rom_path.lock().unwrap();
                                    *lock = Some(path);
                                }
                                framework.gui_state.pick_rom_requested = false;
                            }

                            let pending_rom = {
                                let mut lock = framework.pending_rom_path.lock().unwrap();
                                lock.take()
                            };

                            if let Some(path) = pending_rom {
                                // Whitelist directory
                                if let Some(parent) = path.parent() {
                                    let _ = emulator.add_allowed_path(parent);
                                }
                                if let Err(e) = emulator.load_rom(path.to_str().unwrap()) {
                                    eprintln!("Failed to load ROM: {}", e);
                                } else {
                                    framework.gui_state.add_recent(path.clone());
                                    // Auto-load state if enabled
                                    if framework.gui_state.auto_save_load {
                                        let auto_path = path.with_extension("auto");
                                        if auto_path.exists() {
                                            emulator.load_state_from_path(auto_path);
                                        }
                                    }
                                }
                            }

                            if framework.gui_state.reset_requested {
                                emulator.hard_reset();
                                framework.gui_state.reset_requested = false;
                            }

                            if framework.gui_state.close_requested {
                                emulator.close_rom();
                                framework.gui_state.close_requested = false;
                            }

                            if let Some(slot) = framework.gui_state.save_requested {
                                emulator.save_state(slot);
                                framework.gui_state.save_requested = None;
                            }

                            if let Some(slot) = framework.gui_state.load_requested {
                                emulator.load_state(slot);
                                framework.gui_state.load_requested = None;
                            }

                            if let Some(slot) = framework.gui_state.delete_state_requested {
                                emulator.delete_state(slot);
                                framework.gui_state.delete_state_requested = None;
                            }

                            // 3. Render
                            let debug_info = {
                                let bus = emulator.bus.borrow();
                                let info = DebugInfo {
                                    frame_count: emulator.internal_frame_count,
                                    m68k_pc: emulator.cpu.pc,
                                    z80_pc: emulator.z80.pc as u16,
                                    display_enabled: bus.vdp.display_enabled(),
                                    vdp_status: bus.vdp.status,
                                    bg_color_index: (bus.vdp.registers[7] & 0x3F) as usize,
                                    cram: bus.vdp.get_cram_rgb565().to_vec(),
                                    cram_raw: bus.vdp.get_cram_raw().to_vec(),
                                    vram: bus.vdp.vram.to_vec(),
                                    vsram: bus.vdp.vsram.to_vec(),
                                    wram: bus.work_ram.to_vec(),
                                    z80_ram: bus.z80_ram.to_vec(),
                                    psg_tone: std::array::from_fn(|i| frontend::PsgToneInfo {
                                        frequency: emulator.apu.psg.tones[i].frequency,
                                        volume: emulator.apu.psg.tones[i].volume,
                                    }),
                                    psg_noise: frontend::PsgNoiseInfo {
                                        volume: emulator.apu.psg.noise.volume,
                                        white: emulator.apu.psg.noise.white,
                                        rate: emulator.apu.psg.noise.rate,
                                    },
                                    vdp_registers: bus.vdp.registers,
                                    m68k_disasm: Vec::new(),
                                    z80_disasm: Vec::new(),
                                    port1_type: bus.io.port1.controller_type,
                                    port2_type: bus.io.port2.controller_type,
                                    has_rom: !bus.rom.is_empty(),
                                    current_rom_path: emulator.current_rom_path.clone(),
                                };
                                frontend::rgb565_to_rgba8(&bus.vdp.framebuffer, pixels.frame_mut());
                                
                                // Transfer audio to thread-safe buffer
                                if let Ok(mut buf) = audio_buffer.lock() {
                                    buf.push(&emulator.audio_buffer);
                                }
                                emulator.audio_buffer.clear();
                                
                                info
                            };

                            // Update egui
                            framework.prepare(window, &debug_info);
                            if let Err(e) = pixels.render_with(|encoder, render_target, context| {
                                // Render the board
                                context.scaling_renderer.render(encoder, render_target);
                                // Render GUI
                                framework.render(
                                    encoder,
                                    render_target,
                                    &context.device,
                                    &context.queue,
                                );
                                Ok(())
                            }) {
                                eprintln!("Render error: {}", e);
                                target.exit();
                            }
                        }
                        _ => {}
                    }
                }
                Event::AboutToWait => {
                    let now = std::time::Instant::now();
                    let next_frame = last_frame_inst + frame_duration;
                    if now >= next_frame {
                        last_frame_inst = now;
                        window.request_redraw();
                    }
                    target.set_control_flow(winit::event_loop::ControlFlow::Poll);
                }
                _ => {}
            }
        })
        .map_err(|e| e.to_string())
}
