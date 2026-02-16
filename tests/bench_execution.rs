use genteel::cpu::Cpu;
use genteel::memory::{Memory, MemoryInterface};
use std::time::Instant;

#[test]
fn bench_execution_loop() {
    let mut memory = Memory::new(0x10000);
    // Program:
    // 0x100: MOVEQ #0, D0       (7000)
    // 0x102: ADDQ.L #1, D0      (5280)
    // 0x104: CMP.L #1000000, D0 (B0BC 000F 4240)
    // 0x10A: BNE 0x102          (66F6) - Displacement -10 (from 0x10C) = 0x102
    // 0x10C: RTS                (4E75)

    memory.write_word(0x100, 0x7000);
    memory.write_word(0x102, 0x5280);
    memory.write_word(0x104, 0xB0BC);
    memory.write_long(0x106, 1_000_000);
    memory.write_word(0x10A, 0x66F6);
    memory.write_word(0x10C, 0x4E75);

    // Initial PC
    memory.write_long(4, 0x100);
    // Initial SP
    memory.write_long(0, 0x1000);

    let mut cpu = Cpu::new(&mut memory);

    let start = Instant::now();

    let mut instructions = 0;
    while cpu.pc != 0x10C {
        cpu.step_instruction(&mut memory);
        instructions += 1;
        if instructions > 4_000_000 { // 1,000,000 * 3 + 1
            break;
        }
    }

    let duration = start.elapsed();
    println!("Execution took: {:?}", duration);
    println!("Instructions: {}", instructions);
    let seconds = duration.as_secs_f64();
    if seconds > 0.0 {
         println!("MIPS: {:.2}", (instructions as f64 / seconds) / 1_000_000.0);
    }
}
