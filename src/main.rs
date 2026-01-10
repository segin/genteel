pub mod cpu;
pub mod apu;
pub mod vdp;
pub mod memory;
pub mod io;
pub mod z80;
pub mod debugger;
pub mod input;
pub mod frontend;

use std::cell::RefCell;
use std::rc::Rc;
use cpu::Cpu;
use z80::Z80;
use memory::bus::Bus;
use memory::{SharedBus, Z80Bus};
use input::InputManager;
use frontend::Frontend;

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
        let data = std::fs::read(path)?;
        self.bus.borrow_mut().load_rom(&data);
        
        // Reset again to load initial PC/SP from ROM vectors
        self.cpu.reset();
        self.z80.reset();
        
        Ok(())
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

    /// Run with SDL2 window (interactive play mode)
    pub fn run_with_frontend(&mut self) -> Result<(), String> {
        let mut frontend = Frontend::new("Genteel - Sega Genesis Emulator", 3)?;
        
        println!("Controls: Arrow keys=D-pad, Z=A, X=B, C=C, Enter=Start");
        println!("Press Escape to quit.");
        
        while frontend.poll_events() {
            // Get live input from frontend
            let input = frontend.get_input();
            self.step_frame_with_input(input.p1, input.p2);
            
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
    println!("  --help           Show this help");
    println!();
    println!("Controls (play mode):");
    println!("  Arrow keys       D-pad");
    println!("  Z/X/C            A/B/C buttons");
    println!("  Enter            Start");
    println!("  Escape           Quit");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    let mut rom_path: Option<String> = None;
    let mut script_path: Option<String> = None;
    let mut headless_frames: Option<u32> = None;
    
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
    if let Some(frames) = headless_frames {
        emulator.run(frames);
    } else {
        // Interactive mode with SDL2 window
        if let Err(e) = emulator.run_with_frontend() {
            eprintln!("Frontend error: {}", e);
        }
    }
}

