pub mod cpu;
pub mod apu;
pub mod vdp;
pub mod memory;
pub mod io;
pub mod debugger;

use cpu::Cpu;
use memory::Memory;
// use apu::Apu; // Not yet implemented
// use vdp::Vdp; // Not yet implemented
// use io::Io;   // Not yet implemented

const RAM_SIZE: usize = 0x400000; // 4MB for testing

pub struct Emulator {
    cpu: Cpu,
    // apu: Apu,
    // vdp: Vdp,
    // io: Io,
}

impl Emulator {
    pub fn new() -> Self {
        let memory = Memory::new(RAM_SIZE); // Create memory
        let cpu = Cpu::new(memory);         // Pass memory to CPU

        Self {
            cpu,
            // apu: Apu::new(),
            // vdp: Vdp::new(),
            // io: Io::new(),
        }
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
        for _ in 0..100 { // Step CPU 100 instructions as a placeholder
            self.cpu.step_instruction();
        }

        println!("Stepped one frame.");
    }

    pub fn run(&mut self) {
        println!("Emulator running...");
        // This is a very basic loop, will be expanded later
        for _frame_count in 0..10 { // Run for 10 frames as a placeholder
            self.step_frame();
            // TODO: Render video, play audio, handle input
        }
        println!("Emulator finished.");
    }
}

fn main() {
    let mut emulator = Emulator::new();
    emulator.run();
}
