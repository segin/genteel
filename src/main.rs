#![deny(warnings)]

/// Graceful println that ignores broken pipe errors (for `| head` usage)
macro_rules! println_safe {
    ($($arg:tt)*) => {{
        use std::io::Write;
        let _ = writeln!(std::io::stdout(), $($arg)*);
    }};
}

pub mod cpu;
pub mod apu;
pub mod vdp;
pub mod memory;
pub mod io;
pub mod z80;
pub mod debugger;
pub mod input;
pub mod frontend;
pub mod audio;

use std::cell::RefCell;
use std::rc::Rc;
use cpu::Cpu;
use z80::Z80;
use memory::bus::Bus;
use memory::{SharedBus, Z80Bus};
use apu::Apu;
use input::InputManager;
// use frontend::Frontend;
use debugger::{GdbServer, GdbRegisters, GdbMemory, StopReason};

pub struct Emulator {
    pub cpu: Cpu,
    pub z80: Z80,
    pub apu: Apu,
    pub bus: Rc<RefCell<Bus>>,
    pub input: InputManager,
    pub audio_buffer: Vec<i16>,
    pub apu_accumulator: f32,
}

impl Emulator {
    pub fn new() -> Self {
        let bus = Rc::new(RefCell::new(Bus::new()));
        
        // M68k uses SharedBus wrapper for main Genesis bus access
        let cpu = Cpu::new(Box::new(SharedBus::new(bus.clone())));
        
        // Z80 uses Z80Bus which routes to sound chips and banked 68k memory
        let z80_bus = Z80Bus::new(SharedBus::new(bus.clone()));
        let z80 = Z80::new(Box::new(z80_bus));

        let mut emulator = Self {
            cpu,
            z80,
            apu: Apu::new(),
            bus,
            input: InputManager::new(),
            audio_buffer: Vec::with_capacity(735 * 2), // Pre-allocate approx 1 frame
            apu_accumulator: 0.0,
        };
        
        emulator.cpu.reset();
        emulator.z80.reset();
        
        emulator
    }

    pub fn load_rom(&mut self, path: &str) -> std::io::Result<()> {
        let data = if path.to_lowercase().ends_with(".zip") {
            // Extract ROM from zip file
            Self::load_rom_from_zip(path)?
        } else {
            std::fs::read(path)?
        };
        
        self.bus.borrow_mut().load_rom(&data);
        
        // Reset again to load initial PC/SP from ROM vectors
        self.cpu.reset();
        self.z80.reset();
        
        Ok(())
    }
    
    /// Load ROM from a zip file (finds first .bin, .md, .gen, or .smd file)
    fn load_rom_from_zip(path: &str) -> std::io::Result<Vec<u8>> {
        use std::io::Read;
        
        let file = std::fs::File::open(path)?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        
        // ROM file extensions to look for
        let rom_extensions = [".bin", ".md", ".gen", ".smd", ".32x"];
        
        // Find first ROM file in archive
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            
            let name = entry.name().to_lowercase();
            if rom_extensions.iter().any(|ext| name.ends_with(ext)) {
                let mut data = Vec::new();
                entry.read_to_end(&mut data)?;
                println!("Extracted ROM: {} ({} bytes)", entry.name(), data.len());
                return Ok(data);
            }
        }
        
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No ROM file found in zip archive"
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

    fn step_frame_internal(&mut self) {
        // Genesis timing constants (NTSC):
        // M68k: 7.67 MHz, Z80: 3.58 MHz
        // One frame = 262 scanlines
        // Active display: lines 0-223
        // VBlank: lines 224-261
        // Cycles per scanline: ~488
        
        const LINES_PER_FRAME: u16 = 262;
        const ACTIVE_LINES: u16 = 224;
        const CYCLES_PER_LINE: u32 = 488;
        const Z80_CYCLES_PER_M68K_CYCLE: f32 = 3.58 / 7.67;
        
        // APU Timing
        let samples_per_frame = audio::samples_per_frame() as f32;
        let samples_per_line = samples_per_frame / (LINES_PER_FRAME as f32);
        
        let mut z80_cycles_run: f32 = 0.0;
        
        for line in 0..LINES_PER_FRAME {
            // Update V-counter in VDP
            self.bus.borrow_mut().vdp.set_v_counter(line);
            
            // Render scanline if active
            if line < ACTIVE_LINES {
                self.bus.borrow_mut().vdp.render_line(line);
            }
            
            // Run CPU for this scanline
            let mut cycles_scanline = 0;
            while cycles_scanline < CYCLES_PER_LINE {



                let cycles = self.cpu.step_instruction();
                cycles_scanline += cycles as u32;
                
                // Z80 execution
                let bus = self.bus.borrow();
                let z80_can_run = !bus.z80_reset && !bus.z80_bus_request;
                drop(bus);
                
                if z80_can_run {
                    z80_cycles_run += (cycles as f32) * Z80_CYCLES_PER_M68K_CYCLE;
                    while z80_cycles_run >= 1.0 {
                        self.z80.step();
                        z80_cycles_run -= 1.0;
                    }
                }
            }
            
            // Run APU for this scanline (Interleaved)
            self.apu_accumulator += samples_per_line;
            let samples_to_run = self.apu_accumulator as usize;
            if samples_to_run > 0 {
                self.apu_accumulator -= samples_to_run as f32;
                
                // Borrow bus only for APU step
                let mut bus = self.bus.borrow_mut();
                for _ in 0..samples_to_run {
                    let sample = bus.apu.step();
                    // Stereo output (duplicate for now)
                    self.audio_buffer.push(sample);
                    self.audio_buffer.push(sample);
                }
            }
            
            // Handle Interrupts
            let bus = self.bus.borrow();
            
            // VBlank Interrupt (Level 6) - Triggered at start of VBlank (line 224)
            if line == ACTIVE_LINES {
                // vdp.status |= 0x0008; // VBlank flag is handled by read_status using v_counter
                // Check if VInterrupt enabled (Reg 1, bit 5)
                if (bus.vdp.mode2() & 0x20) != 0 {
                    self.cpu.request_interrupt(6);
                }
            }
            
            // HBlank Interrupt (Level 4) - Triggered at end of active lines
            if line < ACTIVE_LINES {
                // Check if HInterrupt enabled (Reg 0, bit 4)
                if (bus.vdp.mode1() & 0x10) != 0 {
                    // Start of HBlank?
                    // HCounter logic would trigger here
                     // For now, simplify: trigger if enabled and line counter expiration logic matches
                     // But strictly, HInt happens every line if enabled? Or based on HInt Counter?
                     // Reg 10 is HInt Counter.
                     
                    let h_counter = bus.vdp.hint_counter();
                    if h_counter == 0 {
                        // Reload counter
                        // TODO: Implement proper HInt counter reloading from Reg 10
                        self.cpu.request_interrupt(4);
                    } else {
                        // Decrement? VDP should handle this state.
                        // For MVP, trigger HInt every line if enabled to unblock games use it for timing
                        // self.cpu.request_interrupt(4);
                    }
                    
                    // Force HInt for now if enabled, many games need it for raster effects
                    self.cpu.request_interrupt(4); 
                }
            }
        }
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
        println!("Connect with: m68k-elf-gdb -ex \"target remote :{}\" <elf_file>", port);
        
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
                let mut mem_access = BusGdbMemory { bus: self.bus.clone() };
                
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
                self.cpu.step_instruction();
                
                // Check for breakpoint
                if gdb.is_breakpoint(self.cpu.pc) {
                    gdb.stop_reason = StopReason::Breakpoint;
                    gdb.send_packet(&format!("S{:02x}", StopReason::Breakpoint.signal())).ok();
                    running = false;
                } else if stepping {
                    gdb.stop_reason = StopReason::Step;
                    gdb.send_packet(&format!("S{:02x}", StopReason::Step.signal())).ok();
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

    /// Run with winit window (interactive play mode)
    /// Run with winit window (interactive play mode)
    pub fn run_with_frontend(mut self) -> Result<(), String> {
        use winit::event::{Event, WindowEvent, ElementState, KeyEvent};
        use winit::event_loop::{ControlFlow, EventLoop};
        use winit::keyboard::{KeyCode, PhysicalKey};
        use winit::window::WindowBuilder;
        use pixels::{Pixels, SurfaceTexture};
        // use std::sync::Arc;
        
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
            let surface_texture = SurfaceTexture::new(
                window_size.width, 
                window_size.height, 
                &window
            );
            Pixels::new(
                frontend::GENESIS_WIDTH, 
                frontend::GENESIS_HEIGHT, 
                surface_texture
            ).map_err(|e| e.to_string())?
        };
        
        // Audio setup
        let audio_buffer = audio::create_audio_buffer();
        let samples_per_frame = audio::samples_per_frame();
        let mut audio_samples = vec![0i16; samples_per_frame * 2];
        let _audio_output = audio::AudioOutput::new(audio_buffer.clone()).ok();
        
        // Input state
        let mut input = input::FrameInput::default();
        let mut frame_count: u64 = 0;
        
        println!("Starting event loop...");
        event_loop.run(move |event, target| {
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
                    
                    WindowEvent::KeyboardInput { event: KeyEvent { physical_key, state, .. }, .. } => {
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
                            let mut bus = self.bus.borrow_mut();
                            let disp_en = bus.vdp.display_enabled();
                            let dma_en = bus.vdp.dma_enabled();
                            let cram_val = if bus.vdp.cram.len() >= 2 {
                                (bus.vdp.cram[0] as u16) | ((bus.vdp.cram[1] as u16) << 8)
                            } else { 0 };
                            let z80_pc = self.z80.pc;
                            let z80_reset = bus.z80_reset;
                            let z80_req = bus.z80_bus_request;
                            let z80_op = if (z80_pc as usize) < bus.z80_ram.len() { bus.z80_ram[z80_pc as usize] } else { 0 };

                            println_safe!("Frame {}: 68k={:06X} Disp={} DMA={} CRAM={:04X} IntMask={} | Z80={:04X} [{:02X}] Rst={} Req={}", 
                                frame_count, self.cpu.pc, disp_en, dma_en, cram_val, 
                                (self.cpu.sr >> 8) & 7,
                                z80_pc, z80_op, z80_reset, z80_req);
                                
                            // One-shot opcode dump for hangs
                            if (self.cpu.pc == 0x072764 || self.cpu.pc == 0x002F06) && frame_count <= 121 {
                                let dump_start = (self.cpu.pc - 0x10) & !1;
                                let dump_end = self.cpu.pc + 0x20;
                                println_safe!("Dumping code around {:06X}:", self.cpu.pc);
                                for addr in (dump_start..dump_end).step_by(2) {
                                    let val = bus.read_word(addr);
                                    println_safe!("{:06X}: {:04X}", addr, val);
                                }
                            }
                        }
                        
                        // Run one frame of emulation
                        self.step_frame_with_input(input.p1.clone(), input.p2.clone());
                        
                        // Generate audio - already done in step_frame
                        // Just move samples from accumulator to output buffer
                        audio_samples.clear(); // Ensure clean start (though push appends)
                        // Actually, audio_samples is a Vec usually managed by caller or capacity?
                        // Let's check context.
                        // audio_samples was passed to generate_samples.
                        // Here we just extend audio_samples from self.audio_buffer
                        audio_samples.extend_from_slice(&self.audio_buffer);
                        self.audio_buffer.clear();
                        
                        // Push audio to buffer
                        if let Ok(mut buf) = audio_buffer.lock() {
                            buf.push(&audio_samples);
                        }
                        
                        // Render
                        let frame = pixels.frame_mut();
                        let bus = self.bus.borrow();
                        frontend::rgb565_to_rgba8(&bus.vdp.framebuffer, frame);
                        drop(bus);
                        
                        if let Err(e) = pixels.render() {
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
        }).map_err(|e| e.to_string())
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
        self.bus.borrow_mut().write_byte(addr, value);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    let mut rom_path: Option<String> = None;
    let mut script_path: Option<String> = None;
    let mut headless_frames: Option<u32> = None;
    let mut gdb_port: Option<u16> = None;
    
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                print_usage();
                return;
            }
            "--script" => {
                i += 1;
                if i < args.len() {
                    script_path = Some(args[i].clone());
                }
            }
            "--headless" => {
                i += 1;
                if i < args.len() {
                    headless_frames = args[i].parse().ok();
                }
            }
            "--gdb" => {
                let mut port = debugger::DEFAULT_PORT;
                // Check if next arg is a number
                if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                    if let Ok(p) = args[i + 1].parse() {
                        port = p;
                        i += 1;
                    }
                }
                gdb_port = Some(port);
            }
            arg if !arg.starts_with('-') => {
                if let Some(ref mut path) = rom_path {
                    path.push(' ');
                    path.push_str(arg);
                } else {
                    rom_path = Some(arg.to_string());
                }
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
            }
        }
        i += 1;
    }
    
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
        if let Err(e) = emulator.run_with_frontend() {
            eprintln!("Frontend error: {}", e);
        }
    }
}

