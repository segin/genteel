use crate::cpu::Cpu;
use crate::memory::{Memory, MemoryInterface};
use std::time::Instant;

#[test]
fn bench_step_nop_loop() {
    let mut memory = Memory::new(0x10000);
    // Write NOPs (0x4E71)
    // Fill memory with NOPs from 0x100 to 0x10000
    for i in (0x100..0x10000).step_by(2) {
        memory.write_word(i, 0x4E71);
    }
    // Write RTS at the end just in case, though we loop by count
    memory.write_word(0xFFFE, 0x4E75);

    let mut cpu = Cpu::new(&mut memory);
    cpu.pc = 0x100;

    // Warmup
    for _ in 0..1000 {
        cpu.step_instruction(&mut memory);
        if cpu.pc >= 0xFFFE {
            cpu.pc = 0x100;
        }
    }

    cpu.pc = 0x100;
    let iterations = 10_000_000;
    let start = Instant::now();

    for _ in 0..iterations {
        cpu.step_instruction(&mut memory);
        // Reset PC to avoid running out of memory bounds
        if cpu.pc >= 0xFFFE {
            cpu.pc = 0x100;
        }
    }

    let duration = start.elapsed();
    println!("Step 10M NOPs took: {:?}", duration);
    let nanos_per_instr = duration.as_nanos() / iterations as u128;
    println!("Average time per instruction: {} ns", nanos_per_instr);
}
