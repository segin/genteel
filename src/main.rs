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
use input::InputManager;
use frontend::Frontend;
use debugger::{GdbServer, GdbRegisters, GdbMemory, StopReason};

pub struct Emulator {
    pub cpu: Cpu,
    pub z80: Z80,
    pub bus: Rc<RefCell<Bus>>,
    pub input: InputManager,
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
            bus,
            input: InputManager::new(),
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
        // One frame = ~262 scanlines, ~488 M68k cycles per scanline
        // Total: ~128,000 M68k cycles per frame, ~60,000 Z80 cycles per frame
        
        const M68K_CYCLES_PER_FRAME: u32 = 128000;
        const Z80_CYCLES_PER_M68K_CYCLE: f32 = 3.58 / 7.67;  // ~0.467
        
        let mut m68k_cycles_run: u32 = 0;
        let mut z80_cycles_run: f32 = 0.0;
        
        while m68k_cycles_run < M68K_CYCLES_PER_FRAME {
            // Step M68k
            let cycles = self.cpu.step_instruction();
            m68k_cycles_run += cycles as u32;
            
            // Z80 runs if not held in reset and bus not requested
            let bus = self.bus.borrow();
            let z80_can_run = !bus.z80_reset && !bus.z80_bus_request;
            drop(bus);
            
            if z80_can_run {
                // Run Z80 proportionally
                z80_cycles_run += (cycles as f32) * Z80_CYCLES_PER_M68K_CYCLE;
                while z80_cycles_run >= 1.0 {
                    self.z80.step();
                    z80_cycles_run -= 1.0;
                }
            }
        }
        
        // Render VDP frame after CPU execution
        self.bus.borrow_mut().vdp.render_frame();
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

    /// Run with SDL2 window (interactive play mode)
    pub fn run_with_frontend(&mut self) -> Result<(), String> {
        let mut frontend = Frontend::new("Genteel - Sega Genesis Emulator", 3)?;
        
        println!("Controls: Arrow keys=D-pad, Z=A, X=B, C=C, Enter=Start");
        println!("Press Escape to quit.");
        
        // Audio sample buffer (reused each frame)
        let samples_per_frame = audio::samples_per_frame();
        let mut audio_samples = vec![0i16; samples_per_frame * 2]; // Stereo
        
        while frontend.poll_events() {
            // Get live input from frontend
            let input = frontend.get_input();
            self.step_frame_with_input(input.p1, input.p2);
            
            // Generate audio samples for this frame
            {
                let mut bus = self.bus.borrow_mut();
                bus.apu.generate_samples(&mut audio_samples, samples_per_frame);
            }
            
            // Push audio samples to buffer
            if let Ok(mut buf) = frontend.audio_buffer.lock() {
                buf.push(&audio_samples);
            }
            
            // Convert VDP framebuffer (RGB565) to RGB24 and render
            let bus = self.bus.borrow();
            let framebuffer = frontend::rgb565_to_rgb24(&bus.vdp.framebuffer);
            drop(bus);
            
            frontend.render_frame(&framebuffer)?;
        }
        
        Ok(())
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
                // Optional port argument
                if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                    i += 1;
                    gdb_port = args[i].parse().ok();
                } else {
                    gdb_port = Some(debugger::DEFAULT_PORT);
                }
            }
            arg if !arg.starts_with('-') => {
                rom_path = Some(arg.to_string());
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

