use std::collections::HashMap;
use crate::audio;
use crate::frontend::{self, InputMapping};
use crate::input::InputScript;
use crate::Emulator;
#[cfg(feature = "gui")]
use pixels::{wgpu, Pixels, SurfaceTexture};
#[cfg(feature = "gui")]
use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

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
    #[serde(skip)]
    pub single_step: bool,
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
            single_step: false,
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
        ];
        for &name in &defaults {
            if !self.windows.contains_key(name) {
                self.windows.insert(name.to_string(), WindowState { open: false });
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
    pub display_enabled: bool,
    pub bg_color_index: u8,
    pub cram: [u16; 64],
    pub cram_raw: [u16; 64],
    pub vram: [u8; 0x10000],
}

#[cfg(feature = "gui")]
pub struct Framework {
    pub egui_ctx: egui::Context,
    pub egui_state: egui_winit::State,
    pub screen_descriptor: egui_wgpu::ScreenDescriptor,
    pub renderer: egui_wgpu::Renderer,
    pub gui_state: GuiState,
    pub tile_texture: Option<egui::TextureHandle>,
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
    pub fn prepare(&mut self, window: &winit::window::Window, debug_info: &DebugInfo) {
        let raw_input = self.egui_state.take_egui_input(window);
        self.egui_ctx.begin_frame(raw_input);
        // Draw the GUI
        egui::TopBottomPanel::top("menubar_container").show(&self.egui_ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
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
                        if name == "Settings" { continue; } // Settings is in Settings menu
                        let mut open = self.gui_state.is_window_open(&name);
                        if ui.checkbox(&mut open, &name).changed() {
                            self.gui_state.set_window_open(&name, open);
                        }
                    }
                });
            });
        });

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
                if ui.checkbox(&mut self.gui_state.force_red, "Force Red BG (Debug)").changed() {
                    self.gui_state.save();
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
                if ui.checkbox(&mut self.gui_state.integer_scaling, "Integer Pixel Scaling").changed() {
                    self.gui_state.save();
                }
                ui.separator();
                ui.heading("Input");
                ui.label("Input Mapping:");
                if ui.radio_value(
                    &mut self.gui_state.input_mapping,
                    InputMapping::Original,
                    "Original",
                ).changed() {
                    self.gui_state.save();
                }
                if ui.radio_value(
                    &mut self.gui_state.input_mapping,
                    InputMapping::Ergonomic,
                    "Ergonomic",
                ).changed() {
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
                    ui.label(format!("Flags: [ {} {} {} {} {} ]",
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
                    ui.label(format!("Flags: [ {} {} {} {} {} {} {} {} ]",
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
                    columns[0].label(format!("BC: {:02X}{:02X}", debug_info.z80_b, debug_info.z80_c));
                    columns[1].label(format!("DE: {:02X}{:02X}", debug_info.z80_d, debug_info.z80_e));
                    columns[0].label(format!("HL: {:02X}{:02X}", debug_info.z80_h, debug_info.z80_l));
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
                egui::ScrollArea::vertical().id_source("m68k_disasm").show(ui, |ui| {
                    for (addr, text) in &debug_info.m68k_disasm {
                        let is_current = *addr == debug_info.m68k_pc;
                        let label = format!("{:06X}: {}", addr, text);
                        if is_current {
                            ui.colored_label(egui::Color32::YELLOW, format!("-> {}", label));
                        } else {
                            ui.label(format!("   {}", label));
                        }
                    }
                });
                ui.separator();
                ui.heading("Z80 Disassembly");
                egui::ScrollArea::vertical().id_source("z80_disasm").show(ui, |ui| {
                    for (addr, text) in &debug_info.z80_disasm {
                        let is_current = *addr == debug_info.z80_pc;
                        let label = format!("{:04X}: {}", addr, text);
                        if is_current {
                            ui.colored_label(egui::Color32::YELLOW, format!("-> {}", label));
                        } else {
                            ui.label(format!("   {}", label));
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
                            
                            let (rect, _response) = ui.allocate_at_least(egui::vec2(16.0, 16.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, 0.0, color);
                            if _response.hovered() {
                                _response.on_hover_text(format!("Index: {}\nRaw: {:04X}\nRGB565: {:04X}", idx, debug_info.cram_raw[idx], color565));
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
                    ui.ctx().load_texture("tile_viewer", image.clone(), Default::default())
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
    let mut frame_count: u64 = 0;
    let mut last_frame_inst = std::time::Instant::now();
    let mut fps_timer = std::time::Instant::now();
    let mut fps_count = 0;
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
                            framework.gui_state.save();
                            if let Some(path) = &record_path {
                                let script: InputScript = emulator.input.stop_recording();
                                if let Err(e) = script.save(path) {
                                    eprintln!("Failed to save recorded script: {}", e);
                                } else {
                                    println!("Recorded script saved to: {}", path);
                                }
                            }
                            target.exit();
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
                                    framework.gui_state.save();
                                    if let Some(path) = &record_path {
                                        let script: InputScript = emulator.input.stop_recording();
                                        if let Err(e) = script.save(path) {
                                            eprintln!("Failed to save recorded script: {}", e);
                                        } else {
                                            println!("Recorded script saved to: {}", path);
                                        }
                                    }
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
                            // 2. Fallback to logical key
                            if !handled {
                                use winit::keyboard::Key;
                                if let Key::Named(named) = key_event.logical_key {
                                    let button = match named {
                                        winit::keyboard::NamedKey::ArrowUp => Some("up"),
                                        winit::keyboard::NamedKey::ArrowDown => Some("down"),
                                        winit::keyboard::NamedKey::ArrowLeft => Some("left"),
                                        winit::keyboard::NamedKey::ArrowRight => Some("right"),
                                        winit::keyboard::NamedKey::Enter => Some("start"),
                                        winit::keyboard::NamedKey::Space => Some("mode"),
                                        _ => None,
                                    };
                                    if let Some(btn) = button {
                                        input.p1.set_button(btn, pressed);
                                    }
                                }
                            }
                        }
                        WindowEvent::Resized(size) => {
                            if size.width > 0 && size.height > 0 {
                                pixels.resize_surface(size.width, size.height).ok();
                                framework.resize(size.width, size.height);
                            }
                        }
                        WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                            framework.scale_factor(scale_factor as f32);
                        }
                        WindowEvent::RedrawRequested => {
                            // Sync settings from GUI
                            emulator.input_mapping = framework.gui_state.input_mapping;
                            let force_red = framework.gui_state.force_red;
                            emulator.paused = framework.gui_state.paused;
                            emulator.single_step = framework.gui_state.single_step;
                            framework.gui_state.single_step = false; // Reset GUI state

                            frame_count += 1;
                            fps_count += 1;
                            // Update FPS in title bar every second
                            if fps_timer.elapsed() >= std::time::Duration::from_secs(1) {
                                window.set_title(&format!(
                                    "Genteel - Sega Genesis Emulator | FPS: {}",
                                    fps_count
                                ));
                                fps_count = 0;
                                fps_timer = std::time::Instant::now();
                            }
                            // Debug: Print every 60 frames
                            if emulator.debug && frame_count % 60 == 1 {
                                emulator.log_debug(frame_count);
                            }
                            // Run one frame of emulation
                            emulator.step_frame(Some(&input));
                            // Process audio
                            if let Ok(mut buf) = audio_buffer.lock() {
                                buf.push(&emulator.audio_buffer);
                            }
                            emulator.audio_buffer.clear();

                            // Collect debug info and render
                            let debug_info = {
                                let mut bus = emulator.bus.borrow_mut();
                                if force_red {
                                    bus.vdp.framebuffer.fill(0xF800); // Red in RGB565
                                }
                                let m68k_disasm = {
                                    let mut disasm = Vec::new();
                                    let mut addr = emulator.cpu.pc;
                                    for _ in 0..10 {
                                        let opcode = bus.read_word(addr);
                                        let instr = crate::cpu::decode(opcode);
                                        disasm.push((addr, format!("{:?}", instr)));
                                        // Rough estimate of instruction length
                                        // TODO: Use actual instruction length from decoder
                                        addr += 2; 
                                    }
                                    disasm
                                };
                                let z80_disasm = {
                                    let mut disasm = Vec::new();
                                    let mut addr = emulator.z80.pc;
                                    for _ in 0..10 {
                                        let byte = bus.read_byte(0xA00000 + addr as u32);
                                        disasm.push((addr, format!("{:02X}", byte)));
                                        addr += 1;
                                    }
                                    disasm
                                };

                                let mut cram_raw = [0u16; 64];
                                for i in 0..64 {
                                    cram_raw[i] = u16::from_be_bytes([
                                        bus.vdp.cram[i * 2],
                                        bus.vdp.cram[i * 2 + 1]
                                    ]);
                                }

                                let info = DebugInfo {
                                    m68k_pc: emulator.cpu.pc,
                                    m68k_d: emulator.cpu.d,
                                    m68k_a: emulator.cpu.a,
                                    m68k_sr: emulator.cpu.sr,
                                    m68k_usp: emulator.cpu.usp,
                                    m68k_ssp: emulator.cpu.ssp,
                                    m68k_disasm,
                                    z80_pc: emulator.z80.pc,
                                    z80_a: emulator.z80.a,
                                    z80_f: emulator.z80.f,
                                    z80_b: emulator.z80.b,
                                    z80_c: emulator.z80.c,
                                    z80_d: emulator.z80.d,
                                    z80_e: emulator.z80.e,
                                    z80_h: emulator.z80.h,
                                    z80_l: emulator.z80.l,
                                    z80_ix: emulator.z80.ix,
                                    z80_iy: emulator.z80.iy,
                                    z80_sp: emulator.z80.sp,
                                    z80_i: emulator.z80.i,
                                    z80_r: emulator.z80.r,
                                    z80_memptr: emulator.z80.memptr,
                                    z80_iff1: emulator.z80.iff1,
                                    z80_im: emulator.z80.im,
                                    z80_disasm,
                                    frame_count: emulator.internal_frame_count,
                                    vdp_status: bus.vdp.read_status(),
                                    display_enabled: bus.vdp.display_enabled(),
                                    bg_color_index: bus.vdp.registers[7],
                                    cram: bus.vdp.cram_cache,
                                    cram_raw,
                                    vram: bus.vdp.vram,
                                };
                                frontend::rgb565_to_rgba8(&bus.vdp.framebuffer, pixels.frame_mut());
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
