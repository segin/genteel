use genteel::cpu::Cpu;
use genteel::memory::{Memory, MemoryInterface};
use std::time::Instant;

fn main() {
    // 4MB memory to allow aliasing test
    let mut memory = Memory::new(0x400000);

    // Initial SP and PC
    memory.write_long(0, 0x10000); // SP
    memory.write_long(4, 0x001000); // PC - Start at 0x1000

    let mut cpu = Cpu::new(&mut memory);

    // JMP 0x021000
    // Opcode: 0x4EF9 (JMP Absolute Long)
    // Address: 0x00021000
    // Maps to cache index: (0x1000 >> 1) & 0xFFFF = 0x0800
    memory.write_word(0x001000, 0x4EF9);
    memory.write_long(0x001002, 0x021000);

    // JMP 0x001000
    // Opcode: 0x4EF9 (JMP Absolute Long)
    // Address: 0x00001000
    // Maps to cache index: (0x21000 >> 1) & 0xFFFF = 0x10800 & 0xFFFF = 0x0800
    // This causes collision in 64K cache, but not in 4MB cache.
    memory.write_word(0x021000, 0x4EF9);
    memory.write_long(0x021002, 0x001000);

    let start = Instant::now();
    let steps = 10_000_000;

    for _ in 0..steps {
        cpu.step_instruction(&mut memory);
    }

    let duration = start.elapsed();
    println!("Cache Benchmark: {} steps took {:?}", steps, duration);
    println!("Steps per second: {:.2} M", steps as f64 / duration.as_secs_f64() / 1_000_000.0);
}
