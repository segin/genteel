use genteel::cpu::Cpu;
use genteel::memory::{Memory, MemoryInterface};
use std::time::Instant;

fn create_test_cpu() -> (Cpu, Memory) {
    let mut memory = Memory::new(0x10000); // 64KB
                                           // Initial SP and PC
    memory.write_long(0, 0x1000); // SP
    memory.write_long(4, 0x100); // PC
    let cpu = Cpu::new(&mut memory);
    (cpu, memory)
}

fn main() {
    let (mut cpu, mut memory) = create_test_cpu();

    // Loop:
    // 0x100: ADDQ.L #1, D0        (0x5280)
    // 0x102: CMP.L #1000000, D0   (0xB0BC, 0x000F, 0x4240)
    // 0x108: BNE loop (-10)       (0x66F6)

    memory.write_word(0x100, 0x5280);

    memory.write_word(0x102, 0xB0BC);
    memory.write_long(0x104, 1_000_000);

    memory.write_word(0x108, 0x66F6);

    // Reset D0
    cpu.d[0] = 0;
    cpu.pc = 0x100;

    let start = Instant::now();

    let instructions_to_run = 3_000_000;

    for _ in 0..instructions_to_run {
        cpu.step_instruction(&mut memory);
    }

    let duration = start.elapsed();
    println!(
        "Benchmark: {} instructions took {:?}",
        instructions_to_run, duration
    );
    println!(
        "Instructions per second: {:.2} MIPS",
        (instructions_to_run as f64 / duration.as_secs_f64()) / 1_000_000.0
    );

    assert_eq!(cpu.d[0], 1_000_000);
}
