#![deny(warnings)]
/// Graceful println that ignores broken pipe errors (for `| head` usage)
#[allow(unused_macros)]
macro_rules! println_safe {
    ($($arg:tt)*) => {{
        use std::io::Write;
        let _ = writeln!(std::io::stdout(), $($arg)*);
    }};
}
pub mod apu;
pub mod audio;
pub mod cpu;
pub mod debugger;
pub mod frontend;
pub mod input;
pub mod io;
pub mod memory;
pub mod vdp;
pub mod wav_writer;
pub mod z80;
use crate::vdp::RenderOps;
use apu::Apu;
use cpu::Cpu;
use debugger::{GdbMemory, GdbRegisters, GdbServer, StopReason};
use frontend::InputMapping;
use input::{InputManager, InputScript};
use memory::bus::Bus;
use memory::{SharedBus, Z80Bus};
#[cfg(feature = "gui")]
use pixels::wgpu;
use std::cell::RefCell;
use std::rc::Rc;
use z80::Z80;
#[cfg(feature = "gui")]
struct GuiState {
    show_settings: bool,
    show_debug: bool,
    input_mapping: InputMapping,
    integer_scaling: bool,
    force_red: bool,
}
#[cfg(feature = "gui")]
struct DebugInfo {
    m68k_pc: u32,
    z80_pc: u16,
    frame_count: u64,
    vdp_status: u16,
    display_enabled: bool,
    bg_color_index: u8,
    cram0: u16,
}

#[cfg(feature = "gui")]
struct Framework {
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    screen_descriptor: egui_wgpu::ScreenDescriptor,
    renderer: egui_wgpu::Renderer,
    gui_state: GuiState,
}
#[cfg(feature = "gui")]
impl Framework {
    fn new(
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
        let gui_state = GuiState {
            show_settings: false,
            show_debug: false,
            input_mapping,
            integer_scaling: true,
            force_red: false,
        };
        Self {
            egui_ctx,
            egui_state,
            screen_descriptor,
            renderer,
            gui_state,
        }
    }
    fn handle_event(&mut self, window: &winit::window::Window, event: &winit::event::WindowEvent) {
        let _ = self.egui_state.on_window_event(window, event);
    }
    fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.screen_descriptor.size_in_pixels = [width, height];
        }
    }
    fn scale_factor(&mut self, scale_factor: f32) {
        self.screen_descriptor.pixels_per_point = scale_factor;
    }
    fn prepare(&mut self, window: &winit::window::Window, debug_info: &DebugInfo) {
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
                        self.gui_state.show_settings = true;
                        ui.close_menu();
                    }
                    if ui.button("Input Mapping").clicked() {
                        self.gui_state.show_settings = true;
                        ui.close_menu();
                    }
                    ui.checkbox(&mut self.gui_state.show_debug, "Show Performance & Debug");
                });
            });
        });

        if self.gui_state.show_debug {
            egui::Window::new("Performance & Debug").show(&self.egui_ctx, |ui| {
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
                ui.label(format!("CRAM[0] (RGB565): {:04X}", debug_info.cram0));
                ui.checkbox(&mut self.gui_state.force_red, "Force Red BG (Debug)");
            });
        }

        if self.gui_state.show_settings {
            egui::Window::new("Settings").show(&self.egui_ctx, |ui| {
                ui.heading("Video");
                ui.checkbox(&mut self.gui_state.integer_scaling, "Integer Pixel Scaling");
                ui.separator();
                ui.heading("Input");
                ui.label("Input Mapping:");
                ui.radio_value(
                    &mut self.gui_state.input_mapping,
                    InputMapping::Original,
                    "Original",
                );
                ui.radio_value(
                    &mut self.gui_state.input_mapping,
                    InputMapping::Ergonomic,
                    "Ergonomic",
                );
                ui.separator();
                if ui.button("Close").clicked() {
                    self.gui_state.show_settings = false;
                }
            });
        }
    }
    fn render(
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
#[derive(Debug, Clone, Copy)]
struct Z80Change {
    instruction_cycles: u32,
    new_req: bool,
    new_rst: bool,
}
#[derive(Debug, Clone, Copy)]
struct CpuBatchResult {
    cycles: u32,
    z80_change: Option<Z80Change>,
}
pub struct Emulator {
    pub cpu: Cpu,
    pub z80: Z80<Z80Bus, Z80Bus>,
    pub apu: Apu,
    pub bus: Rc<RefCell<Bus>>,
    pub input: InputManager,
    pub audio_buffer: Vec<i16>,
    pub wav_writer: Option<wav_writer::FileWavWriter>,
    pub internal_frame_count: u64,
    pub z80_last_bus_req: bool,
    pub z80_last_reset: bool,
    pub z80_trace_count: u32,
    pub input_mapping: InputMapping,
    pub debug: bool,
    pub allowed_paths: Vec<std::path::PathBuf>,
}
impl Default for Emulator {
    fn default() -> Self {
        Self::new()
    }
}
impl Emulator {
    pub fn new() -> Self {
        let bus = Rc::new(RefCell::new(Bus::new()));
        // M68k uses SharedBus wrapper for main Genesis bus access
        let mut bus_ref = bus.borrow_mut();
        let cpu = Cpu::new(&mut *bus_ref);
        drop(bus_ref);
        // Z80 uses Z80Bus which routes to sound chips and banked 68k memory
        // It also handles Z80 I/O (which is unconnected on Genesis)
        let z80_bus = Z80Bus::new(SharedBus::new(bus.clone()));
        let z80 = Z80::new(z80_bus.clone(), z80_bus);
        let mut emulator = Self {
            cpu,
            z80,
            apu: Apu::new(),
            bus,
            input: InputManager::new(),
            audio_buffer: Vec::with_capacity(735 * 2), // Pre-allocate approx 1 frame
            wav_writer: None,
            internal_frame_count: 0,
            z80_last_bus_req: false,
            z80_last_reset: true,
            z80_trace_count: 0,
            input_mapping: InputMapping::default(),
            debug: false,
            allowed_paths: Vec::new(),
        };
        {
            let mut bus = emulator.bus.borrow_mut();
            emulator.cpu.reset(&mut *bus);
        }
        emulator.z80.reset();
        emulator
    }
    /// Add a path to the whitelist of allowed ROM directories.
    /// If no paths are added, `load_rom` will fail (secure by default).
    /// The path is canonicalized before addition.
    pub fn add_allowed_path<P: AsRef<std::path::Path>>(&mut self, path: P) -> std::io::Result<()> {
        let canonical = path.as_ref().canonicalize()?;
        self.allowed_paths.push(canonical);
        Ok(())
    }

    pub fn load_rom(&mut self, path: &str) -> std::io::Result<()> {
        // Security: Validate path against whitelist
        let path_obj = std::path::Path::new(path);
        let canonical_path = path_obj.canonicalize()?;

        if self.allowed_paths.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "ROM loading is restricted. No allowed paths configured.",
            ));
        }

        let allowed = self
            .allowed_paths
            .iter()
            .any(|allowed_base| canonical_path.starts_with(allowed_base));

        if !allowed {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("Access denied to ROM path: {:?}", canonical_path),
            ));
        }

        let data = if path.to_lowercase().ends_with(".zip") {
            // Extract ROM from zip file
            Self::load_rom_from_zip(path)?
        } else {
            let mut file = std::fs::File::open(path)?;
            let metadata = file.metadata()?;
            let size = metadata.len();
            Self::read_rom_with_limit(&mut file, size)?
        };
        let mut bus = self.bus.borrow_mut();
        bus.load_rom(&data);
        // Reset again to load initial PC/SP from ROM vectors
        self.cpu.reset(&mut *bus);
        self.z80.reset();
        Ok(())
    }
    fn read_rom_with_limit<R: std::io::Read>(
        reader: &mut R,
        size: u64,
    ) -> std::io::Result<Vec<u8>> {
        use std::io::Read;
        const MAX_ROM_SIZE: u64 = 32 * 1024 * 1024; // 32 MB
        if size > MAX_ROM_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("ROM size {} exceeds limit of {} bytes", size, MAX_ROM_SIZE),
            ));
        }
        // Check if size fits in usize (for 32-bit/16-bit systems)
        if size > usize::MAX as u64 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "ROM size too large for memory address space",
            ));
        }
        let mut data = Vec::with_capacity(size as usize);
        reader.take(MAX_ROM_SIZE + 1).read_to_end(&mut data)?;
        if data.len() as u64 > MAX_ROM_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Decompressed ROM size exceeds limit",
            ));
        }
        Ok(data)
    }
    /// Load ROM from a zip file (finds first .bin, .md, .gen, or .smd file)
    fn load_rom_from_zip(path: &str) -> std::io::Result<Vec<u8>> {
        let file = std::fs::File::open(path)?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        // ROM file extensions to look for
        let rom_extensions = [".bin", ".md", ".gen", ".smd", ".32x"];
        // Find first ROM file in archive
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            let name = entry.name().to_lowercase();
            if rom_extensions.iter().any(|ext| name.ends_with(ext)) {
                let size = entry.size();
                if size > 32 * 1024 * 1024 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("ROM size {} exceeds limit of 32MB", size),
                    ));
                }
                let data = Self::read_rom_with_limit(&mut entry, size)?;
                println!("Extracted ROM: {} ({} bytes)", entry.name(), data.len());
                return Ok(data);
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No ROM file found in zip archive",
        ))
    }
    /// Step one frame with current input state
    pub fn step_frame(&mut self, input: Option<input::FrameInput>) {
        // Apply inputs from script or live input
        let (p1, p2, command) = {
            let frame_input = match input {
                Some(i) => {
                    self.input.record(i.clone());
                    std::borrow::Cow::Owned(i)
                }
                None => self.input.advance_frame(),
            };
            (frame_input.p1, frame_input.p2, frame_input.command.clone())
        };

        {
            let mut bus = self.bus.borrow_mut();
            if let Some(ctrl) = bus.io.controller(1) {
                *ctrl = p1;
            }
            if let Some(ctrl) = bus.io.controller(2) {
                *ctrl = p2;
            }
        }

        // Handle commands (e.g., SCREENSHOT <path>)
        if let Some(cmd) = &command {
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            if !parts.is_empty() {
                match parts[0].to_uppercase().as_str() {
                    "SCREENSHOT" => {
                        if parts.len() > 1 {
                            let raw_path = parts[1];
                            // Security: Sanitize path to prevent arbitrary file writes
                            // Only allow saving to current directory by using only the file name component
                            let path = std::path::Path::new(raw_path)
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("screenshot.png");

                            if path != raw_path {
                                eprintln!(
                                    "Script Warning: Sanitized screenshot path '{}' to '{}'",
                                    raw_path, path
                                );
                            }

                            if let Err(e) = self.save_screenshot(path) {
                                eprintln!(
                                    "Script Error: Failed to save screenshot to {}: {}",
                                    path, e
                                );
                            } else {
                                println!("Script: Saved screenshot to {}", path);
                            }
                        }
                    }
                    _ => {
                        eprintln!("Script Warning: Unknown command '{}'", parts[0]);
                    }
                }
            }
        }

        self.step_frame_internal();
    }
    pub fn step_frame_internal(&mut self) {
        self.internal_frame_count += 1;
        if self.debug {
            #[cfg(feature = "gui")]
            self.log_debug(self.internal_frame_count);
        }
        // Genesis timing constants (NTSC):
        // M68k: 7.67 MHz, Z80: 3.58 MHz
        // One frame = 262 scanlines
        // Active display: lines 0-223
        // VBlank: lines 224-261
        // Cycles per scanline: ~488
        const LINES_PER_FRAME: u16 = 262;
        // Active lines can be 224 or 240 (V30 mode)
        let active_lines = self.bus.borrow().vdp.screen_height();
        // APU Timing
        let samples_per_frame = audio::samples_per_frame() as f32;
        let samples_per_line = samples_per_frame / (LINES_PER_FRAME as f32);
        let mut z80_cycle_debt: f32 = 0.0;
        for line in 0..LINES_PER_FRAME {
            self.step_scanline(line, active_lines, samples_per_line, &mut z80_cycle_debt);
        }
        self.generate_audio_samples(samples_per_line);
        // Update V30 rolling offset
        self.bus.borrow_mut().vdp.update_v30_offset();
    }
    fn step_scanline(
        &mut self,
        line: u16,
        active_lines: u16,
        _samples_per_line: f32,
        z80_cycle_debt: &mut f32,
    ) {
        self.vdp_scanline_setup(line, active_lines);
        self.run_cpu_loop(line, active_lines, z80_cycle_debt);
        self.handle_interrupts(line, active_lines);
    }
    fn vdp_scanline_setup(&mut self, line: u16, active_lines: u16) {
        let mut bus = self.bus.borrow_mut();
        bus.vdp.set_v_counter(line);

        // VBlank Management
        if line == active_lines {
            bus.vdp.set_vblank(true);
        } else if line == 0 {
            bus.vdp.set_vblank(false);
        }

        // Process scanline if within framebuffer bounds (320x240)
        if line < 240 {
            bus.vdp.render_line(line);
        }
    }
    fn sync_components(
        bus_rc: &SharedBus,
        m68k_cycles: u32,
        z80: &mut Z80<Z80Bus, Z80Bus>,
        z80_cycle_debt: &mut f32,
        trigger_vint: bool,
        internal_frame_count: u64,
        z80_last_bus_req: &mut bool,
        z80_last_reset: &mut bool,
        z80_trace_count: &mut u32,
        cpu_pc: u32,
        debug: bool,
    ) {
        // 1. Z80 State and Timing
        let (z80_can_run, z80_is_reset, cycles_per_sample) = {
            let bus = bus_rc.bus.borrow();
            let prev = *z80_last_bus_req;
            if debug && bus.z80_bus_request != prev {
                log::debug!(
                    "Bus Req Changed: {} -> {} at 68k PC={:06X}",
                    prev,
                    bus.z80_bus_request,
                    cpu_pc
                );
            }
            *z80_last_bus_req = bus.z80_bus_request;

            let z80_can_run = !bus.z80_reset && !bus.z80_bus_request;
            let z80_is_reset = bus.z80_reset;
            let cycles_per_sample = 7670453.0 / (bus.sample_rate as f32);
            (z80_can_run, z80_is_reset, cycles_per_sample)
        };

        // Handle Z80 Reset
        if z80_is_reset && !*z80_last_reset {
            z80.reset();
        }
        *z80_last_reset = z80_is_reset;

        // Trace Z80 if debugging
        if z80_can_run && internal_frame_count > 0 {
            z80.debug = debug && *z80_trace_count < 5000;
            if z80.debug {
                *z80_trace_count += 1;
            }
        } else {
            z80.debug = false;
        }

        // Trigger Z80 VInt if requested
        if trigger_vint && !z80_is_reset {
            z80.trigger_interrupt(0xFF);
        }

        // 2. Catch up Z80
        if z80_can_run {
            const Z80_CYCLES_PER_M68K_CYCLE: f32 = 3.58 / 7.67;
            *z80_cycle_debt += m68k_cycles as f32 * Z80_CYCLES_PER_M68K_CYCLE;
            while *z80_cycle_debt >= 1.0 {
                let cycles = z80.step();
                *z80_cycle_debt -= cycles as f32;
            }
        }

        // 3. Update APU and generate audio samples
        {
            let mut bus = bus_rc.bus.borrow_mut();
            bus.audio_accumulator += m68k_cycles as f32;

            while bus.audio_accumulator >= cycles_per_sample {
                let (l, r) = bus.apu.step(cycles_per_sample as u32);
                if bus.audio_buffer.len() < 32768 {
                    bus.audio_buffer.push(l);
                    bus.audio_buffer.push(r);
                }
                bus.audio_accumulator -= cycles_per_sample;
            }
        }
    }

    fn run_cpu_batch_static(
        cpu: &mut Cpu,
        bus_rc: &SharedBus,
        max_cycles: u32,
        z80: &mut Z80<Z80Bus, Z80Bus>,
        z80_cycle_debt: &mut f32,
        line: u16,
        active_lines: u16,
        internal_frame_count: u64,
        z80_last_bus_req: &mut bool,
        z80_last_reset: &mut bool,
        z80_trace_count: &mut u32,
        debug: bool,
    ) -> CpuBatchResult {
        let (initial_req, initial_rst) = {
            let bus = bus_rc.bus.borrow();
            (bus.z80_bus_request, bus.z80_reset)
        };
        let mut pending_cycles = 0;
        loop {
            if pending_cycles >= max_cycles {
                return CpuBatchResult {
                    cycles: pending_cycles,
                    z80_change: None,
                };
            }
            let m68k_cycles = {
                let mut bus = bus_rc.bus.borrow_mut();
                cpu.step_instruction(&mut *bus)
            };

            let trigger_vint = line == active_lines && pending_cycles < 10;
            Self::sync_components(
                bus_rc,
                m68k_cycles,
                z80,
                z80_cycle_debt,
                trigger_vint,
                internal_frame_count,
                z80_last_bus_req,
                z80_last_reset,
                z80_trace_count,
                cpu.pc,
                debug,
            );

            // Check for Z80 state change
            let (req, rst) = {
                let bus = bus_rc.bus.borrow();
                (bus.z80_bus_request, bus.z80_reset)
            };

            if req != initial_req || rst != initial_rst {
                return CpuBatchResult {
                    cycles: pending_cycles, // cycles accumulated *before* this instruction
                    z80_change: Some(Z80Change {
                        instruction_cycles: m68k_cycles,
                        new_req: req,
                        new_rst: rst,
                    }),
                };
            }
            pending_cycles += m68k_cycles;
        }
    }
    fn run_cpu_loop(&mut self, line: u16, active_lines: u16, z80_cycle_debt: &mut f32) {
        const CYCLES_PER_LINE: u32 = 488;
        const BATCH_SIZE: u32 = 128;
        let mut cycles_scanline: u32 = 0;
        let bus_rc = SharedBus::new(self.bus.clone());

        while cycles_scanline < CYCLES_PER_LINE {
            let remaining = CYCLES_PER_LINE - cycles_scanline;
            let limit = std::cmp::min(remaining, BATCH_SIZE);
            let result = Self::run_cpu_batch_static(
                &mut self.cpu,
                &bus_rc,
                limit,
                &mut self.z80,
                z80_cycle_debt,
                line,
                active_lines,
                self.internal_frame_count,
                &mut self.z80_last_bus_req,
                &mut self.z80_last_reset,
                &mut self.z80_trace_count,
                self.debug,
            );

            cycles_scanline += result.cycles;

            if let Some(change) = result.z80_change {
                {
                    let mut bus = self.bus.borrow_mut();
                    // Apply the new state for the instruction that caused the change
                    bus.z80_bus_request = change.new_req;
                    bus.z80_reset = change.new_rst;
                }

                let trigger_vint = line == active_lines && cycles_scanline < 10;
                // Sync components for the instruction that triggered the state change
                Self::sync_components(
                    &bus_rc,
                    change.instruction_cycles,
                    &mut self.z80,
                    z80_cycle_debt,
                    trigger_vint,
                    self.internal_frame_count,
                    &mut self.z80_last_bus_req,
                    &mut self.z80_last_reset,
                    &mut self.z80_trace_count,
                    self.cpu.pc,
                    self.debug,
                );
                cycles_scanline += change.instruction_cycles;
            }
        }
    }
    fn generate_audio_samples(&mut self, _samples_per_line: f32) {
        let mut bus = self.bus.borrow_mut();
        if bus.audio_buffer.is_empty() {
            return;
        }
        if let Some(writer) = &mut self.wav_writer {
            let _ = writer.write_samples(&bus.audio_buffer);
        }
        // Move samples to emulator buffer for frontend consumption
        if self.audio_buffer.len() < 32768 {
            self.audio_buffer.extend(bus.audio_buffer.iter());
        }
        bus.audio_buffer.clear();
    }
    fn handle_interrupts(&mut self, line: u16, active_lines: u16) {
        let mut bus = self.bus.borrow_mut();
        // VBlank Interrupt (Level 6) - Triggered at start of VBlank (line 224/240)
        if line == active_lines {
            // Check if VInterrupt enabled (Reg 1, bit 5)
            if (bus.vdp.mode2() & 0x20) != 0 {
                self.cpu.request_interrupt(6);
            }
        }
        // HBlank Interrupt (Level 4) - Proper counter logic
        if line < active_lines {
            // Check if HInterrupt enabled (Reg 0, bit 4)
            if (bus.vdp.mode1() & 0x10) != 0 {
                // Decrement line counter
                if bus.vdp.line_counter == 0 {
                    // Counter expired - trigger HInt and reload
                    self.cpu.request_interrupt(4);
                    bus.vdp.line_counter = bus.vdp.registers[10];
                } else {
                    bus.vdp.line_counter = bus.vdp.line_counter.saturating_sub(1);
                }
            }
        } else {
            // During VBlank, reload HInt counter every line
            bus.vdp.line_counter = bus.vdp.registers[10];
        }
    }
    /// Run headless for N frames (or until script ends if N is None)
    pub fn run(&mut self, frames: Option<u32>, screenshot_path: Option<String>, record_path: Option<String>) {
        match frames {
            Some(n) => println!("Running {} frames headless...", n),
            None => println!("Running headless until script ends..."),
        }

        if let Some(path) = &record_path {
            println!("Recording inputs to: {}", path);
            self.input.start_recording();
        }

        let start_time = std::time::Instant::now();
        let mut current = 0;
        loop {
            if let Some(n) = frames {
                if current >= n {
                    break;
                }
            } else if self.input.is_complete() {
                // Only stop if a script was actually loaded and it's done
                break;
            }

            self.step_frame(None);
            // Clear audio buffer in headless mode to prevent memory leak
            self.audio_buffer.clear();
            current += 1;

            // Log every 600 frames (approx 10 seconds)
            if self.debug && current % 600 == 0 {
                #[cfg(feature = "gui")]
                self.log_debug(current as u64);
            }
        }

        let elapsed = start_time.elapsed();
        if let Some(path) = screenshot_path {
            if let Err(e) = self.save_screenshot(&path) {
                eprintln!("Failed to save final screenshot: {}", e);
            } else {
                println!("Final screenshot saved to: {}", path);
            }
        }
<<<<<<< HEAD
        println!(
            "Done in {:?} ({:.2} fps).",
            elapsed,
            current as f64 / elapsed.as_secs_f64()
        );
=======

        if let Some(path) = record_path {
            let script: InputScript = self.input.stop_recording();
            if let Err(e) = script.save(&path) {
                eprintln!("Failed to save recorded script: {}", e);
            } else {
                println!("Recorded script saved to: {}", path);
            }
        }

        println!("Done in {:?} ({:.2} fps).", elapsed, current as f64 / elapsed.as_secs_f64());
>>>>>>> main
    }

    pub fn save_screenshot(&self, path: &str) -> Result<(), String> {
        let bus = self.bus.borrow();
        let fb = &bus.vdp.framebuffer;
        let mut rgb_data = Vec::with_capacity(fb.len() * 3);
        for &pixel in fb {
            let r5 = ((pixel >> 11) & 0x1F) as u8;
            let g6 = ((pixel >> 5) & 0x3F) as u8;
            let b5 = (pixel & 0x1F) as u8;
            rgb_data.push((r5 << 3) | (r5 >> 2));
            rgb_data.push((g6 << 2) | (g6 >> 4));
            rgb_data.push((b5 << 3) | (b5 >> 2));
        }
        image::save_buffer(path, &rgb_data, 320, 240, image::ExtendedColorType::Rgb8)
            .map_err(|e| e.to_string())
    }
    /// Run with GDB debugger attached
    pub fn run_with_gdb(&mut self, port: u16, password: Option<String>) -> std::io::Result<()> {
        let mut gdb = GdbServer::new(port, password.clone())?;
        println!("Waiting for GDB connection on port {}...", port);
        if let Some(pwd) = password {
            println!(
                "🔒 Password protected. After connecting, run: monitor auth {}",
                pwd
            );
        }
        println!(
            "Connect with: m68k-elf-gdb -ex \"target remote :{}\" <elf_file>",
            port
        );
        // Wait for connection
        while !gdb.accept() {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let mut stepping = false;
        let mut running = false;
        loop {
            // Check for GDB commands
            if let Some(cmd) = gdb.receive_packet() {
                // Build register state from CPU
                let mut regs = GdbRegisters {
                    d: self.cpu.d,
                    a: self.cpu.a,
                    sr: self.cpu.sr,
                    pc: self.cpu.pc,
                };
                // Create memory accessor
                let mut mem_access = BusGdbMemory {
                    bus: self.bus.clone(),
                };
                let response = gdb.process_command(&cmd, &mut regs, &mut mem_access);
                // Apply register changes back to CPU
                self.cpu.d = regs.d;
                self.cpu.a = regs.a;
                self.cpu.sr = regs.sr;
                self.cpu.pc = regs.pc;
                match response.as_str() {
                    "CONTINUE" => {
                        running = true;
                        stepping = false;
                    }
                    "STEP" => {
                        stepping = true;
                        running = true;
                    }
                    _ if !response.is_empty() => {
                        gdb.send_packet(&response).ok();
                    }
                    _ => {}
                }
            }
            // Execute if running
            if running {
                // Step one instruction
                let mut bus = self.bus.borrow_mut();
                self.cpu.step_instruction(&mut *bus);
                drop(bus);
                // Check for breakpoint
                if gdb.is_breakpoint(self.cpu.pc) {
                    gdb.stop_reason = StopReason::Breakpoint;
                    gdb.send_packet(&format!("S{:02x}", StopReason::Breakpoint.signal()))
                        .ok();
                    running = false;
                } else if stepping {
                    gdb.stop_reason = StopReason::Step;
                    gdb.send_packet(&format!("S{:02x}", StopReason::Step.signal()))
                        .ok();
                    running = false;
                }
            } else {
                // Not running, sleep a bit to avoid spinning
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            // Check if client disconnected
            if !gdb.is_connected() && !gdb.accept() {
                println!("GDB client disconnected");
                break;
            }
        }
        Ok(())
    }
    fn log_debug(&self, frame_count: u64) {
        let bus = self.bus.borrow();
        let disp_en = if bus.vdp.display_enabled() {
            "ON "
        } else {
            "OFF"
        };
        let dma_en = if bus.vdp.dma_enabled() { "ON " } else { "OFF" };
        let cram_val = if bus.vdp.cram.len() >= 2 {
            ((bus.vdp.cram[0] as u16) << 8) | (bus.vdp.cram[1] as u16)
        } else {
            0
        };
        let z80_pc = self.z80.pc;
        let z80_reset = if bus.z80_reset { "RST" } else { "RUN" };
        let z80_req = if bus.z80_bus_request { "BUS" } else { "---" };
        let z80_op = if (z80_pc as usize) < bus.z80_ram.len() {
            bus.z80_ram[z80_pc as usize]
        } else {
            0
        };
        println_safe!(
            "FRAME {:05} | 68k: PC={:06X} SR={:04X} | VDP: Disp={} DMA={} CRAM={:04X} | Z80: PC={:04X} OP={:02X} St={} Req={}",
            frame_count,
            self.cpu.pc,
            self.cpu.sr,
            disp_en,
            dma_en,
            cram_val,
            z80_pc,
            z80_op,
            z80_reset,
            z80_req
        );
    }
    #[cfg(feature = "gui")]
    fn process_audio(&mut self, audio_buffer: &audio::SharedAudioBuffer) {
        if let Ok(mut buf) = audio_buffer.lock() {
            buf.push(&self.audio_buffer);
        }
        self.audio_buffer.clear();
    }
    /// Run with winit window (interactive play mode)
    #[cfg(feature = "gui")]
    pub fn run_with_frontend(mut self, record_path: Option<String>) -> Result<(), String> {
        use pixels::{Pixels, SurfaceTexture};
        use winit::event::{ElementState, Event, WindowEvent};
        use winit::event_loop::EventLoop;
        use winit::keyboard::{KeyCode, PhysicalKey};
        use winit::window::WindowBuilder;
        if self.input_mapping == InputMapping::Original {
            println!("Controls: Arrow keys=D-pad, Z=A, X=B, C=C, Enter=Start");
        } else {
            println!("Controls: WASD/Arrows=D-pad, J/Z=A, K/X=B, L/C=C, U=X, I=Y, O=Z, Enter=Start, Space=Mode");
        }
        println!("Press Escape to quit.");
        if let Some(path) = &record_path {
            println!("Recording inputs to: {}", path);
            self.input.start_recording();
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
            let surface_texture =
                SurfaceTexture::new(window_size.width, window_size.height, window);
            Pixels::new(320, 240, surface_texture).map_err(|e| e.to_string())?
        };
        // Initialize egui framework
        let mut framework = Framework::new(
            &event_loop,
            window.inner_size().width,
            window.inner_size().height,
            window.scale_factor() as f32,
            &pixels,
            self.input_mapping,
        );
        // Audio setup
        let audio_buffer = audio::create_audio_buffer();
        let audio_output = match audio::AudioOutput::new(audio_buffer.clone()) {
            Ok(output) => {
                self.bus.borrow_mut().sample_rate = output.sample_rate;
                Some(output)
            }
            Err(e) => {
                eprintln!("Warning: Failed to initialize audio: {}", e);
                None
            }
        };
        let _audio_output = audio_output;
        // Input and Timing state
        let mut input = input::FrameInput::default();
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
                                if let Some(path) = &record_path {
                                    let script: InputScript = self.input.stop_recording();
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
                                        if let Some(path) = &record_path {
                                            let script: InputScript = self.input.stop_recording();
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
                                        frontend::keycode_to_button(keycode, self.input_mapping)
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
                                self.input_mapping = framework.gui_state.input_mapping;
                                let force_red = framework.gui_state.force_red;

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
                                if self.debug && frame_count % 60 == 1 {
                                    self.log_debug(frame_count);
                                }
                                // Run one frame of emulation
                                self.step_frame(Some(input.clone()));
                                // Process audio
                                self.process_audio(&audio_buffer);

                                // Collect debug info and render
                                let debug_info = {
                                    let mut bus = self.bus.borrow_mut();
                                    if force_red {
                                        bus.vdp.framebuffer.fill(0xF800); // Red in RGB565
                                    }
                                    let info = DebugInfo {
                                        m68k_pc: self.cpu.pc,
                                        z80_pc: self.z80.pc,
                                        frame_count: self.internal_frame_count,
                                        vdp_status: bus.vdp.peek_status(),
                                        display_enabled: bus.vdp.display_enabled(),
                                        bg_color_index: bus.vdp.registers[7],
                                        cram0: bus.vdp.cram_cache[0],
                                    };
                                    frontend::rgb565_to_rgba8(
                                        &bus.vdp.framebuffer,
                                        pixels.frame_mut(),
                                    );
                                    info
                                };

                                // Update egui
                                framework.prepare(window, &debug_info);
                                if let Err(e) =
                                    pixels.render_with(|encoder, render_target, context| {
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
                                    })
                                {
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
}
fn print_usage() {
    println!("Genteel - Sega Genesis/Mega Drive Emulator");
    println!();
    println!("Usage: genteel [OPTIONS] <ROM>");
    println!();
    println!("Options:");
    println!("  --script <path>  Load TAS input script");
    println!("  --record <path>  Record inputs to a script file");
    println!("  --headless <n>   Run N frames without display");
    println!("  --screenshot <path> Save screenshot after headless run");
    println!("  --gdb [port]     Start GDB server (default port: 1234)");
    println!("  --gdb-password <pwd> Set password for GDB server");
    println!("  --dump-audio <file> Dump audio output to WAV file");
    println!(
        "  --input-mapping <type> Set keyboard mapping (original|ergonomic, default: original)"
    );
    println!("  --debug          Enable verbose debug output");
    println!("  --help           Show this help");
    println!();
    println!("Controls (play mode - original layout):");
    println!("  Arrow keys       D-pad");
    println!("  Z/X/C            A/B/C buttons");
    println!("  Enter            Start");
    println!("  Escape           Quit");
}
/// GDB memory accessor for Bus
struct BusGdbMemory {
    bus: Rc<RefCell<Bus>>,
}
impl GdbMemory for BusGdbMemory {
    fn read_byte(&mut self, addr: u32) -> u8 {
        self.bus.borrow_mut().read_byte(addr)
    }
    fn write_byte(&mut self, addr: u32, value: u8) {
        // Assuming Z80 RAM is mapped within the first 0x2000 bytes of the Z80's address space
        // This is a heuristic based on the provided snippet's `addr < 0x2000` condition.
        // The GDB `write_byte` operates on the 68k's address space, so we need to
        // translate or ensure this is the correct place for Z80 writes.
        // If the intent was to log Z80Bus's internal write, that function is not in this file.
        // For now, applying the eprintln to the GDB memory write if it targets Z80 RAM.
        // This assumes the GDB server can write directly to Z80 RAM via the main bus.
        if addr < 0x2000 {
            // This condition is a guess based on the user's snippet
            eprintln!(
                "DEBUG: GDB Z80 RAM WRITE: addr=0x{:04X} val=0x{:02X}",
                addr, value
            );
        }
        self.bus.borrow_mut().write_byte(addr, value);
    }
}
#[derive(Debug, Default)]
struct Config {
    rom_path: Option<String>,
    script_path: Option<String>,
    headless: bool,
    headless_frames: Option<u32>,
    screenshot_path: Option<String>,
    gdb_port: Option<u16>,
    gdb_password: Option<String>,
    dump_audio_path: Option<String>,
    record_path: Option<String>,
    input_mapping: InputMapping,
    debug: bool,
    show_help: bool,
}
impl Config {
    fn from_args<I>(args: I) -> Self
    where
        I: IntoIterator<Item = String>,
    {
        let mut config = Config::default();
        let mut iter = args.into_iter().skip(1).peekable();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--help" | "-h" => {
                    config.show_help = true;
                }
                "--script" => {
                    config.script_path = iter.next();
                }
                "--record" => {
                    config.record_path = iter.next();
                }
                "--headless" => {
                    config.headless = true;
                    if let Some(next) = iter.peek() {
                        if !next.starts_with('-') {
                            if let Ok(n) = next.parse::<u32>() {
                                config.headless_frames = Some(n);
                                iter.next(); // consume
                            }
                        }
                    }
                }
                "--screenshot" => {
                    config.screenshot_path = iter.next();
                }
                "--gdb" => {
                    let mut port = debugger::DEFAULT_PORT;
                    if let Some(next) = iter.peek() {
                        if !next.starts_with('-') {
                            if let Ok(p) = next.parse() {
                                port = p;
                                iter.next(); // consume it
                            }
                        }
                    }
                    config.gdb_port = Some(port);
                }
                "--gdb-password" => {
                    config.gdb_password = iter.next();
                }
                "--dump-audio" => {
                    config.dump_audio_path = iter.next();
                }
                "--input-mapping" => {
                    if let Some(mapping_str) = iter.next() {
                        match mapping_str.to_lowercase().as_str() {
                            "ergonomic" | "modern" | "wasd" => {
                                config.input_mapping = InputMapping::Ergonomic;
                            }
                            _ => {
                                config.input_mapping = InputMapping::Original;
                            }
                        }
                    }
                }
                "--debug" => {
                    config.debug = true;
                }
                arg if !arg.starts_with('-') => {
                    if let Some(ref mut path) = config.rom_path {
                        path.push(' ');
                        path.push_str(arg);
                    } else {
                        config.rom_path = Some(arg.to_string());
                    }
                }
                _ => {
                    eprintln!("Unknown option: {}", arg);
                }
            }
        }
        config
    }
}
fn main() {
    env_logger::init();
    let config = Config::from_args(std::env::args());
    if config.show_help {
        print_usage();
        return;
    }
    let rom_path = config.rom_path;
    let script_path = config.script_path;
    let gdb_port = config.gdb_port;
    let dump_audio_path = config.dump_audio_path;
    let mut emulator = Emulator::new();
    emulator.input_mapping = config.input_mapping;
    emulator.debug = config.debug;
    if let Some(path) = dump_audio_path {
        println!("Dumping audio to: {}", path);
        match wav_writer::WavWriter::new(&path, audio::SAMPLE_RATE, 2) {
            Ok(writer) => emulator.wav_writer = Some(writer),
            Err(e) => eprintln!("Failed to create audio dump file: {}", e),
        }
    }
    // Load ROM if provided
    if let Some(ref path) = rom_path {
        println!("Loading ROM: {}", path);

        // Security: Whitelist the directory containing the ROM
        if let Ok(canonical) = std::path::Path::new(path).canonicalize() {
            if let Some(parent) = canonical.parent() {
                if let Err(e) = emulator.add_allowed_path(parent) {
                    eprintln!("Warning: Failed to whitelist ROM directory: {}", e);
                }
            }
        }

        if let Err(e) = emulator.load_rom(path) {
            eprintln!("Failed to load ROM: {}", e);
            return;
        }
    } else {
        print_usage();
        return;
    }
    // Load input script if provided
    if let Some(ref path) = script_path {
        println!("Loading input script: {}", path);
        if let Err(e) = emulator.input.load_script(path) {
            eprintln!("Failed to load script: {}", e);
            return;
        }
    }
    // Run in appropriate mode
    if let Some(port) = gdb_port {
        // Debug mode with GDB
        if let Err(e) = emulator.run_with_gdb(port, config.gdb_password) {
            eprintln!("GDB server error: {}", e);
        }
    } else if config.headless {
        emulator.run(config.headless_frames, config.screenshot_path, config.record_path);
    } else {
        // Interactive mode with SDL2 window
        #[cfg(feature = "gui")]
        if let Err(e) = emulator.run_with_frontend(config.record_path) {
            eprintln!("Frontend error: {}", e);
        }
        #[cfg(not(feature = "gui"))]
        {
            eprintln!("Interactive mode requires 'gui' feature.");
            eprintln!("Use --headless <N> or --gdb <PORT> instead.");
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn test_zip_bomb_prevention() {
        let path = "test_bomb.zip";
        let mut zip_data = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_data));
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            // Generate 33MB of dummy data
            zip.start_file("large.bin", options).unwrap();
            let chunk = vec![0u8; 1024 * 1024]; // 1MB
            for _ in 0..33 {
                zip.write_all(&chunk).unwrap();
            }
            zip.finish().unwrap();
        }
        std::fs::write(path, &zip_data).unwrap();
        // Attempt to load
        let result = Emulator::load_rom_from_zip(path);
        // Cleanup
        let _ = std::fs::remove_file(path);
        // Verify rejection
        assert!(result.is_err(), "Should reject large ROM file (>32MB)");
    }
    #[test]
    fn test_zip_bomb_mismatch() {
        // Create 33MB of dummy data (simulating decompressed stream)
        let size = 33 * 1024 * 1024;
        let data = vec![0u8; size];
        let mut reader = std::io::Cursor::new(data);
        // Report size as small (e.g., 1KB), simulating a zip bomb header lie
        let reported_size = 1024;
        // This should fail because it reads > 32MB despite reported size
        let result = Emulator::read_rom_with_limit(&mut reader, reported_size);
        assert!(
            result.is_err(),
            "Should reject read size exceeding limit even if reported size is small"
        );
    }
    #[test]
    fn test_z80_ready_flag_behavior() {
        let mut emulator = Emulator::new();
        // Z80 Program to write 0x80 to 0x1FFD
        // LD A, 0x80      (3E 80)
        // LD (0x1FFD), A  (32 FD 1F)
        // HALT            (76)
        let z80_code = vec![0x3E, 0x80, 0x32, 0xFD, 0x1F, 0x76];
        // Ensure Z80 RAM is clear initially (Emulator::new clears it)
        // First, let Z80 run to dirty the PC (simulate running garbage or previous code)
        // By default Z80 is in reset (from new()). We must release it to run.
        emulator.bus.borrow_mut().write_word(0xA11200, 0x0100); // Release Reset
        emulator.step_frame_internal();
        // Check PC > 0 (it should be running NOPs/garbage from 0)
        assert!(emulator.z80.pc > 0, "Z80 PC should have advanced");
        // Sonic 1-like sequence:
        // 1. Assert Z80 Reset (Write 0 to 0xA11200)
        emulator.bus.borrow_mut().write_word(0xA11200, 0x0000);
        // Crucial: Step frame to allow emulator to detect the reset assertion
        emulator.step_frame_internal();
        // At this point, Z80 PC should be 0 because reset was detected
        assert_eq!(emulator.z80.pc, 0, "Z80 PC should be 0 after reset");
        // 2. Request Bus (Write 0x100 to 0xA11100)
        emulator.bus.borrow_mut().write_word(0xA11100, 0x0100);
        // 3. Load Code to Z80 RAM (0xA00000)
        for (i, byte) in z80_code.iter().enumerate() {
            emulator
                .bus
                .borrow_mut()
                .write_byte(0xA00000 + i as u32, *byte);
        }
        // Verify Z80 RAM at target address is 0 before execution
        assert_eq!(
            emulator.bus.borrow_mut().read_byte(0xA01FFD),
            0x00,
            "Z80 RAM should be 0 before run"
        );
        // 4. Release Bus (Write 0 to 0xA11100)
        emulator.bus.borrow_mut().write_word(0xA11100, 0x0000);
        // 5. Release Reset (Write 0x100 to 0xA11200)
        emulator.bus.borrow_mut().write_word(0xA11200, 0x0100);
        // Run for a few frames to let Z80 execute
        emulator.step_frame_internal();
        emulator.step_frame_internal();
        // Request bus to read result (Z80 RAM is only accessible when Z80 is stopped)
        emulator.bus.borrow_mut().write_word(0xA11100, 0x0100);
        // Check if 0xA01FFD is 0x80
        // We use read_byte on bus.
        // We must request the bus first to read Z80 RAM
        emulator.bus.borrow_mut().write_word(0xA11100, 0x0100);
        let val = emulator.bus.borrow_mut().read_byte(0xA01FFD);
        assert_eq!(val, 0x80, "Z80 should have written 0x80 to 0xA01FFD");
    }
    #[test]
    fn test_config_parsing() {
        let args = vec!["genteel".to_string(), "rom.bin".to_string()];
        let config = Config::from_args(args);
        assert_eq!(config.rom_path, Some("rom.bin".to_string()));
        assert!(!config.show_help);
        let args = vec!["genteel".to_string(), "--help".to_string()];
        let config = Config::from_args(args);
        assert!(config.show_help);
        let args = vec![
            "genteel".to_string(),
            "--script".to_string(),
            "script.txt".to_string(),
            "rom.bin".to_string(),
        ];
        let config = Config::from_args(args);
        assert_eq!(config.script_path, Some("script.txt".to_string()));
        assert_eq!(config.rom_path, Some("rom.bin".to_string()));
        let args = vec![
            "genteel".to_string(),
            "--headless".to_string(),
            "60".to_string(),
            "rom.bin".to_string(),
        ];
        let config = Config::from_args(args);
        assert_eq!(config.headless_frames, Some(60));
        assert_eq!(config.rom_path, Some("rom.bin".to_string()));
        let args = vec![
            "genteel".to_string(),
            "--headless".to_string(),
            "1200".to_string(),
            "--screenshot".to_string(),
            "final.png".to_string(),
            "rom.bin".to_string(),
        ];
        let config = Config::from_args(args);
        assert_eq!(config.headless_frames, Some(1200));
        assert_eq!(config.screenshot_path, Some("final.png".to_string()));
        assert_eq!(config.rom_path, Some("rom.bin".to_string()));
        let args = vec![
            "genteel".to_string(),
            "--gdb".to_string(),
            "rom.bin".to_string(),
        ];
        let config = Config::from_args(args);
        assert_eq!(config.gdb_port, Some(crate::debugger::DEFAULT_PORT));
        assert_eq!(config.rom_path, Some("rom.bin".to_string()));
        let args = vec![
            "genteel".to_string(),
            "--gdb".to_string(),
            "1234".to_string(),
            "rom.bin".to_string(),
        ];
        let config = Config::from_args(args);
        assert_eq!(config.gdb_port, Some(1234));
        assert_eq!(config.rom_path, Some("rom.bin".to_string()));
        let args = vec![
            "genteel".to_string(),
            "rom".to_string(),
            "part2.bin".to_string(),
        ];
        let config = Config::from_args(args);
        assert_eq!(config.rom_path, Some("rom part2.bin".to_string()));
    }
    #[test]
    fn test_step_frame_basic() {
        let mut emulator = Emulator::new();
        emulator.step_frame(None);
        emulator.step_frame(None);
        assert_eq!(emulator.internal_frame_count, 2);
    }
    #[test]
    fn test_large_raw_rom_prevention() {
        let path = "large_rom.bin";
        // Create 33MB of dummy data
        let size = 33 * 1024 * 1024;
        let data = vec![0u8; size];
        std::fs::write(path, &data).unwrap();
        let mut emulator = Emulator::new();
        // Must whitelist the path first
        emulator.add_allowed_path(".").unwrap();
        let result = emulator.load_rom(path);
        // Cleanup
        let _ = std::fs::remove_file(path);
        // Verify rejection
        assert!(result.is_err(), "Should reject large ROM file (>32MB)");
    }

    #[test]
    fn test_path_traversal_protection() {
        let dummy_rom = "dummy_traversal.bin";
        std::fs::write(dummy_rom, b"dummy rom content").unwrap();

        let mut emulator = Emulator::new();

        // 1. Should fail without whitelist (Secure by Default)
        let result = emulator.load_rom(dummy_rom);
        assert!(result.is_err(), "Should fail without whitelisted path");
        assert_eq!(
            result.as_ref().err().unwrap().kind(),
            std::io::ErrorKind::PermissionDenied
        );

        // 2. Should fail if whitelist doesn't cover the file
        // Whitelist a different directory (e.g., system temp)
        let temp_dir = std::env::temp_dir();
        // Only if temp_dir exists and is different from CWD
        if let Ok(_) = emulator.add_allowed_path(&temp_dir) {
            let result = emulator.load_rom(dummy_rom);
            // Assuming dummy_rom is in CWD and not in temp_dir
            // If CWD == temp_dir, this test is weak but passes.
            // Usually CWD is project root.
            // We can check canonical paths to be sure.
            let cwd = std::env::current_dir().unwrap().canonicalize().unwrap();
            let temp = temp_dir.canonicalize().unwrap();
            if !cwd.starts_with(&temp) {
                assert!(result.is_err(), "Should fail if path not in whitelist");
            }
        }

        // 3. Should succeed if whitelisted
        emulator.add_allowed_path(".").unwrap();
        let result = emulator.load_rom(dummy_rom);
        assert!(result.is_ok(), "Should succeed if path is whitelisted");

        // Cleanup
        let _ = std::fs::remove_file(dummy_rom);
    }

    #[test]
    fn test_screenshot_path_sanitization() {
        let mut emulator = Emulator::new();
        let path = "/tmp/genteel_exploit.png";
        let sanitized_path = "genteel_exploit.png";

        // Ensure files don't exist
        if std::path::Path::new(path).exists() {
            let _ = std::fs::remove_file(path);
        }
        if std::path::Path::new(sanitized_path).exists() {
            let _ = std::fs::remove_file(sanitized_path);
        }

        // Construct input with command
        let mut input = crate::input::FrameInput::default();
        input.command = Some(format!("SCREENSHOT {}", path));

        emulator.step_frame(Some(input));

        // Check vulnerability is fixed
        if std::path::Path::new(path).exists() {
            let _ = std::fs::remove_file(path);
            panic!("Vulnerability still present: file created at {}", path);
        }

        // Check sanitized behavior
        if std::path::Path::new(sanitized_path).exists() {
            // Success: created at sanitized path
            let _ = std::fs::remove_file(sanitized_path);
        } else {
            panic!(
                "Sanitization failed: file not created at {}",
                sanitized_path
            );
        }
    }
}
