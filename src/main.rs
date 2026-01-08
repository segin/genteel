pub mod cpu;
pub mod apu;
pub mod vdp;
pub mod memory;
pub mod io;
pub mod z80;
pub mod debugger;

use std::cell::RefCell;
use std::rc::Rc;
use cpu::Cpu;
use z80::Z80;
use memory::bus::Bus;
use memory::{SharedBus, Z80Bus};

pub struct Emulator {
    pub cpu: Cpu,
    pub z80: Z80,
    pub bus: Rc<RefCell<Bus>>,
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

    pub fn step_frame(&mut self) {
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
    }


    pub fn run(&mut self) {
        println!("Emulator running...");
        // This is a very basic loop, will be expanded later
        // In a real app, this would be the main event loop
        for _frame_count in 0..60 { 
            self.step_frame();
        }
        println!("Emulator finished.");
    }
}

fn main() {
    let mut emulator = Emulator::new();
    if let Some(rom_path) = std::env::args().nth(1) {
        println!("Loading ROM: {}", rom_path);
        if let Err(e) = emulator.load_rom(&rom_path) {
            eprintln!("Failed to load ROM: {}", e);
            return;
        }
    } else {
        println!("No ROM provided. Usage: cargo run <rom_path>");
        println!("Running in empty mode...");
    }
    
    emulator.run();
}
