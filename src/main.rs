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
use memory::SharedBus;
// use apu::Apu; // Not yet implemented
// use vdp::Vdp; // Not yet implemented
// use io::Io;   // Not yet implemented

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
        // Z80 has dedicated 8KB sound RAM 
        let z80 = Z80::new(memory::Memory::new(0x2000));

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
        // This is a placeholder. A real step_frame would involve:
        // 1. Running CPU for a certain number of cycles
        // 2. Updating VDP
        // 3. Updating APU
        // 4. Handling interrupts
        // 5. Handling I/O

        // For now, let's just step the CPU a few times.
        // A single frame has 70937.5 CPU cycles, approx.
        // For testing, we run fewer.
        for _ in 0..100 { 
            self.cpu.step_instruction();
            // Z80 runs at ~ half clock speed of 68k, but for now just step it too
            // self.z80.step(); // Z80 step returns T-states
        }

        // println!("Stepped one frame.");
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
