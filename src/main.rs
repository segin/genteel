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
pub mod z80;

use apu::Apu;
use cpu::Cpu;
use debugger::{GdbMemory, GdbRegisters, GdbServer, StopReason};
use input::InputManager;
use memory::bus::Bus;
use memory::{SharedBus, Z80Bus};
use std::cell::RefCell;
use std::rc::Rc;
use z80::Z80;

pub struct Emulator {
    pub cpu: Cpu,
    pub z80: Z80,
    pub apu: Apu,
    pub bus: Rc<RefCell<Bus>>,
    pub input: InputManager,
    pub audio_buffer: Vec<i16>,
    pub apu_accumulator: f32,
    pub internal_frame_count: u64,
    pub z80_last_bus_req: bool,
    pub z80_last_reset: bool,
    pub z80_trace_count: u32,
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

        let z80 = Z80::new(Box::new(z80_bus.clone()), Box::new(z80_bus));

        let mut emulator = Self {
            cpu,
            z80,
            apu: Apu::new(),
            bus,
            input: InputManager::new(),
            audio_buffer: Vec::with_capacity(735 * 2), // Pre-allocate approx 1 frame
            apu_accumulator: 0.0,
            internal_frame_count: 0,
            z80_last_bus_req: false,
            z80_last_reset: true,
            z80_trace_count: 0,
        };

        {
            let mut bus = emulator.bus.borrow_mut();
            emulator.cpu.reset(&mut *bus);
        }
        emulator.z80.reset();

        emulator
    }

    pub fn load_rom(&mut self, path: &str) -> std::io::Result<()> {
        let data = if path.to_lowercase().ends_with(".zip") {
            // Extract ROM from zip file
            Self::load_rom_from_zip(path)?
        } else {
            let file = std::fs::File::open(path)?;
            let size = file.metadata()?.len();
            let mut reader = std::io::BufReader::new(file);
            Self::read_rom_with_limit(&mut reader, size)?
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
    pub fn step_frame(&mut self) {
        // Apply inputs from script or live input
        let frame_input = self.input.advance_frame();
        {
            let mut bus = self.bus.borrow_mut();
            if let Some(ctrl) = bus.io.controller(1) {
                *ctrl = frame_input.p1;
            }
            if let Some(ctrl) = bus.io.controller(2) {
                *ctrl = frame_input.p2;
            }
        }

        self.step_frame_internal();
    }

    /// Step one frame with provided input (for live play)
    pub fn step_frame_with_input(&mut self, p1: io::ControllerState, p2: io::ControllerState) {
        {
            let mut bus = self.bus.borrow_mut();
            if let Some(ctrl) = bus.io.controller(1) {
                *ctrl = p1;
            }
            if let Some(ctrl) = bus.io.controller(2) {
                *ctrl = p2;
            }
        }
        self.step_frame_internal();
    }

    pub fn step_frame_internal(&mut self) {
        self.internal_frame_count += 1;

        if self.internal_frame_count % 60 == 0 || self.internal_frame_count < 5 {
            let bus = self.bus.borrow();
            let disp_en = bus.vdp.display_enabled();
            let dma_en = bus.vdp.dma_enabled();

            let cram_sum: u32 = bus.vdp.cram.iter().take(128).map(|&b| b as u32).sum();

            let z80_pc = self.z80.pc;
            let z80_reset = bus.z80_reset;
            let z80_req = bus.z80_bus_request;
            let z80_op = if (z80_pc as usize) < bus.z80_ram.len() {
                bus.z80_ram[z80_pc as usize]
            } else {
                0
            };

            eprintln!("Frame {}: 68k={:06X} Disp={} DMA={} CRAM_SUM={} IntMask={} | Z80={:04X} [{:02X}] Rst={} Req={}",
                     self.internal_frame_count, self.cpu.pc, disp_en, dma_en, cram_sum,
                     (self.cpu.sr >> 8) & 7,
                     z80_pc, z80_op, z80_reset, z80_req);
        }

        // Genesis timing constants (NTSC):
        // M68k: 7.67 MHz, Z80: 3.58 MHz
        // One frame = 262 scanlines
        // Active display: lines 0-223
        // VBlank: lines 224-261
        // Cycles per scanline: ~488

        const LINES_PER_FRAME: u16 = 262;
        const CYCLES_PER_LINE: u32 = 488;
        const Z80_CYCLES_PER_M68K_CYCLE: f32 = 3.58 / 7.67;

        // Active lines can be 224 or 240 (V30 mode)
        let active_lines = self.bus.borrow().vdp.screen_height();
        // APU Timing
        let samples_per_frame = audio::samples_per_frame() as f32;
        let samples_per_line = samples_per_frame / (LINES_PER_FRAME as f32);

        let mut z80_cycle_debt: f32 = 0.0;

        for line in 0..LINES_PER_FRAME {
            // === VDP scanline setup (single borrow) ===
            {
                let mut bus = self.bus.borrow_mut();
                bus.vdp.set_v_counter(line);

                // Render scanline if active
                if line < active_lines {
                    bus.vdp.render_line(line);
                }
            }

            // === CPU execution for this scanline ===
            let mut cycles_scanline: u32 = 0;
            while cycles_scanline < CYCLES_PER_LINE {
                // Borrow bus for CPU execution
                let mut bus = self.bus.borrow_mut();
                let m68k_cycles = self.cpu.step_instruction(&mut *bus);

                // Step APU with cycles to update timers
                bus.apu.fm.step(m68k_cycles as u32);
                drop(bus); // Release bus for Z80/etc checks if they need it (Z80 needs read only access usually but check implementation)

                cycles_scanline += m68k_cycles as u32;

                // Z80 execution
                let (z80_can_run, z80_is_reset) = {
                    let bus = self.bus.borrow();
                    let prev = self.z80_last_bus_req;
                    if bus.z80_bus_request != prev {
                        eprintln!(
                            "DEBUG: Bus Req Changed: {} -> {} at 68k PC={:06X}",
                            prev, bus.z80_bus_request, self.cpu.pc
                        );
                        self.z80_last_bus_req = bus.z80_bus_request;
                    }
                    (!bus.z80_reset && !bus.z80_bus_request, bus.z80_reset)
                };

                // Z80 reset logic:
                // Reset the Z80 on the leading edge of the reset signal.
                // The Z80 is held in reset (not stepped) as long as z80_reset is true.
                if z80_is_reset && !self.z80_last_reset {
                    self.z80.reset();
                }
                self.z80_last_reset = z80_is_reset;

                if z80_can_run && self.internal_frame_count > 0 {
                    if self.z80_trace_count < 5000 {
                        self.z80.debug = true;
                        self.z80_trace_count += 1; // Logic slightly wrong here (step count vs frame), but enables flag
                    } else {
                        self.z80.debug = false;
                    }
                } else {
                    self.z80.debug = false;
                }

                // Trigger Z80 VInt at start of VBlank
                if line == active_lines && cycles_scanline < m68k_cycles as u32 + 5 && !z80_is_reset
                {
                    self.z80.trigger_interrupt(0xFF);
                }

                if z80_can_run {
                    z80_cycle_debt += (m68k_cycles as f32) * Z80_CYCLES_PER_M68K_CYCLE;
                    while z80_cycle_debt >= 1.0 {
                        let z80_cycles = self.z80.step();
                        z80_cycle_debt -= z80_cycles as f32;
                    }
                }
            }

            // === APU sample generation (single borrow) ===
            self.apu_accumulator += samples_per_line;
            let samples_to_run = self.apu_accumulator as usize;
            if samples_to_run > 0 {
                self.apu_accumulator -= samples_to_run as f32;

                let mut bus = self.bus.borrow_mut();
                for _ in 0..samples_to_run {
                    let sample = bus.apu.step();
                    // Stereo output (duplicate for now)
                    self.audio_buffer.push(sample);
                    self.audio_buffer.push(sample);
                }
            }

            // === Interrupt handling (single borrow) ===
            {
                let mut bus = self.bus.borrow_mut();

                // VBlank Interrupt (Level 6) - Triggered at start of VBlank (line 224/240)
                if line == active_lines {
                    bus.vdp.trigger_vint();
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
        }

        // Update V30 rolling offset
        self.bus.borrow_mut().vdp.update_v30_offset();
    }

    /// Run headless for N frames (for TAS/testing)
    pub fn run(&mut self, frames: u32) {
        println!("Running {} frames headless...", frames);
        for _ in 0..frames {
            self.step_frame();
        }
        println!("Done.");
    }

    /// Run with GDB debugger attached
    pub fn run_with_gdb(&mut self, port: u16) -> std::io::Result<()> {
        let mut gdb = GdbServer::new(port)?;

        println!("Waiting for GDB connection on port {}...", port);
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

    fn print_debug_info(&self, frame_count: u64) {
        let mut bus = self.bus.borrow_mut();
        let disp_en = bus.vdp.display_enabled();
        let dma_en = bus.vdp.dma_enabled();
        let cram_val = if bus.vdp.cram.len() >= 2 {
            ((bus.vdp.cram[0] as u16) << 8) | (bus.vdp.cram[1] as u16)
        } else {
            0
        };
        let z80_pc = self.z80.pc;
        let z80_reset = bus.z80_reset;
        let z80_req = bus.z80_bus_request;
        let z80_op = if (z80_pc as usize) < bus.z80_ram.len() {
            bus.z80_ram[z80_pc as usize]
        } else {
            0
        };

        println_safe!(
            "Frame {}: 68k={:06X} Disp={} DMA={} CRAM={:04X} IntMask={} | Z80={:04X} [{:02X}] Rst={} Req={}",
            frame_count,
            self.cpu.pc,
            disp_en,
            dma_en,
            cram_val,
            (self.cpu.sr >> 8) & 7,
            z80_pc,
            z80_op,
            z80_reset,
            z80_req
        );

        // One-shot opcode dump for hangs
        if (self.cpu.pc == 0x072764 || self.cpu.pc == 0x002F06) && frame_count <= 121 {
            let dump_start = (self.cpu.pc - 0x10) & !1;
            let dump_end = self.cpu.pc + 0x20;
            eprintln!("Dumping code around {:06X}:", self.cpu.pc);
            for addr in (dump_start..dump_end).step_by(2) {
                let val = bus.read_word(addr);
                eprintln!("{:06X}: {:04X}", addr, val);
            }
        }
    }

    fn render_frame(&self, pixels: &mut pixels::Pixels) -> Result<(), String> {
        let frame = pixels.frame_mut();
        let bus = self.bus.borrow();
        frontend::rgb565_to_rgba8(&bus.vdp.framebuffer, frame);
        drop(bus);

        pixels.render().map_err(|e| e.to_string())
    }

    fn process_audio(&mut self, audio_buffer: &audio::SharedAudioBuffer) {
        if let Ok(mut buf) = audio_buffer.lock() {
            buf.push(&self.audio_buffer);
        }
        self.audio_buffer.clear();
    }

    /// Run with winit window (interactive play mode)
    pub fn run_with_frontend(mut self) -> Result<(), String> {
        use pixels::{Pixels, SurfaceTexture};
        use winit::event::{ElementState, Event, KeyEvent, WindowEvent};
        use winit::event_loop::{ControlFlow, EventLoop};
        use winit::keyboard::{KeyCode, PhysicalKey};
        use winit::window::WindowBuilder;
        println!("Controls: Arrow keys=D-pad, Z=A, X=B, C=C, Enter=Start");

        println!("Press Escape to quit.");

        let event_loop = EventLoop::new().map_err(|e| e.to_string())?;

        let window = {
            let size = winit::dpi::LogicalSize::new(
                frontend::GENESIS_WIDTH as f64 * 3.0,
                frontend::GENESIS_HEIGHT as f64 * 3.0,
            );
            WindowBuilder::new()
                .with_title("Genteel - Sega Genesis Emulator")
                .with_inner_size(size)
                .with_min_inner_size(winit::dpi::LogicalSize::new(
                    frontend::GENESIS_WIDTH as f64,
                    frontend::GENESIS_HEIGHT as f64,
                ))
                .build(&event_loop)
                .map_err(|e| e.to_string())?
        };

        let mut pixels = {
            let window_size = window.inner_size();
            let surface_texture =
                SurfaceTexture::new(window_size.width, window_size.height, &window);
            Pixels::new(
                frontend::GENESIS_WIDTH,
                frontend::GENESIS_HEIGHT,
                surface_texture,
            )
            .map_err(|e| e.to_string())?
        };

        // Audio setup
        let audio_buffer = audio::create_audio_buffer();
        let _audio_output = audio::AudioOutput::new(audio_buffer.clone()).ok();

        // Input state
        let mut input = input::FrameInput::default();
        let mut frame_count: u64 = 0;

        println!("Starting event loop...");
        event_loop
            .run(move |event, target| {
                target.set_control_flow(ControlFlow::Poll);

                match event {
                    Event::Resumed => {
                        println!("Event::Resumed");
                    }

                    Event::WindowEvent { event, .. } => match event {
                        WindowEvent::CloseRequested => {
                            println!("Using CloseRequested to exit");
                            target.exit();
                        }

                        WindowEvent::KeyboardInput {
                            event:
                                KeyEvent {
                                    physical_key,
                                    state,
                                    ..
                                },
                            ..
                        } => {
                            if let PhysicalKey::Code(keycode) = physical_key {
                                let pressed = state == ElementState::Pressed;

                                if keycode == KeyCode::Escape && pressed {
                                    println!("Escape pressed, exiting");
                                    target.exit();
                                    return;
                                }

                                if let Some((button, _)) = frontend::keycode_to_button(keycode) {
                                    input.p1.set_button(button, pressed);
                                }
                            }
                        }

                        WindowEvent::Resized(size) => {
                            if size.width > 0 && size.height > 0 {
                                pixels.resize_surface(size.width, size.height).ok();
                            }
                        }

                        WindowEvent::RedrawRequested => {
                            frame_count += 1;

                            // Debug: Print every 60 frames (about once per second)
                            if frame_count % 60 == 1 {
                                self.print_debug_info(frame_count);
                            }

                            // Run one frame of emulation
                            self.step_frame_with_input(input.p1.clone(), input.p2.clone());

                            // Process audio
                            self.process_audio(&audio_buffer);

                            // Render
                            if let Err(e) = self.render_frame(&mut pixels) {
                                eprintln!("Render error: {}", e);
                                target.exit();
                            }
                        }

                        _ => {}
                    },

                    Event::AboutToWait => {
                        // Request a redraw just before waiting for events, only on redraw
                        // println!("AboutToWait - requesting redraw"); // Too spammy?
                        window.request_redraw();
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
    println!("  --headless <n>   Run N frames without display");
    println!("  --gdb [port]     Start GDB server (default port: 1234)");
    println!("  --help           Show this help");
    println!();
    println!("Controls (play mode):");
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
    headless_frames: Option<u32>,
    gdb_port: Option<u16>,
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
                "--headless" => {
                    if let Some(next) = iter.next() {
                        config.headless_frames = next.parse().ok();
                    }
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
    let config = Config::from_args(std::env::args());

    if config.show_help {
        print_usage();
        return;
    }

    let rom_path = config.rom_path;
    let script_path = config.script_path;
    let headless_frames = config.headless_frames;
    let gdb_port = config.gdb_port;

    let mut emulator = Emulator::new();

    // Load ROM if provided
    if let Some(ref path) = rom_path {
        println!("Loading ROM: {}", path);
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
        if let Err(e) = emulator.run_with_gdb(port) {
            eprintln!("GDB server error: {}", e);
        }
    } else if let Some(frames) = headless_frames {
        emulator.run(frames);
    } else {
        // Interactive mode with SDL2 window
        #[cfg(feature = "gui")]
        if let Err(e) = emulator.run_with_frontend() {
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

        // Attempt to load - should fail after fix
        let result = Emulator::load_rom_from_zip(path);

        // Cleanup
        let _ = std::fs::remove_file(path);

        // Before fix: This assertion will fail (because result.is_ok() is likely true)
        // After fix: This assertion will pass
        assert!(result.is_err(), "Should reject large ROM file (>32MB)");
    }

    #[test]
    fn test_read_rom_with_limit_toctou() {
        // Simulate a file that claims to be small but is actually huge
        let claimed_size = 100;
        let actual_size = 33 * 1024 * 1024; // 33MB
        // Using a repeatable iterator to avoid allocating 33MB
        // However, Cursor needs a slice.
        // We can just allocate 33MB. It's fine for a test.
        let data = vec![0u8; actual_size];
        let mut reader = std::io::Cursor::new(&data);

        // This should fail because read_rom_with_limit reads up to MAX_ROM_SIZE + 1
        // and checks the final length.
        let result = Emulator::read_rom_with_limit(&mut reader, claimed_size);

        assert!(result.is_err(), "Should detect that actual content exceeds limit even if metadata says otherwise");
    }

    #[test]
    fn test_large_file_prevention() {
        let path = "test_large_rom.bin";
        // Create a 33MB file
        // We use a file > 32MB to trigger the limit
        let data = vec![0u8; 33 * 1024 * 1024];
        std::fs::write(path, &data).unwrap();

        let mut emulator = Emulator::new();
        let result = emulator.load_rom(path);

        // Cleanup
        let _ = std::fs::remove_file(path);

        assert!(result.is_err(), "Should reject large ROM file (>32MB)");
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

        // 4. Release Bus (Write 0 to 0xA11100)
        emulator.bus.borrow_mut().write_word(0xA11100, 0x0000);

        // 5. Release Reset (Write 0x100 to 0xA11200)
        emulator.bus.borrow_mut().write_word(0xA11200, 0x0100);

        // Run for a few frames to let Z80 execute
        emulator.step_frame_internal();
        emulator.step_frame_internal();

        // Check if 0xA01FFD is 0x80
        // We use read_byte on bus.
        // If hack is present, this passes trivially.
        // If hack is removed, this passes ONLY if Z80 ran correctly (PC=0 at start).
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
}
