#![no_main]

//! M68k random-program fuzzer.
//!
//! Executes arbitrary instruction streams from arbitrary data registers and
//! checks that the CPU never panics and that its cycle counter matches the
//! cycles it reports per step.

use genteel::cpu::Cpu;
use genteel::memory::{Memory, MemoryInterface};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 32 {
        return;
    }

    let mut memory = Memory::new(0x100000);

    let program = &data[16..];
    let copy_len = program.len().min(0x200);
    for (i, &b) in program[..copy_len].iter().enumerate() {
        memory.write_byte(0x1000 + i as u32, b);
    }

    for vector in 0..64 {
        memory.write_long(vector * 4, 0x3000);
    }
    memory.write_word(0x3000, 0x4E72); // STOP
    memory.write_word(0x3002, 0x2700);

    memory.write_long(0x00, 0x8000);
    memory.write_long(0x04, 0x1000);

    let mut cpu = Cpu::new(&mut memory);
    cpu.reset(&mut memory);

    for (i, chunk) in data[..16].chunks_exact(4).enumerate() {
        cpu.d[i] = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    cpu.a[0] = 0x4000;
    cpu.a[1] = 0x5000;
    cpu.a[2] = 0x6000;
    cpu.a[3] = 0x7000;

    let start = cpu.cycles;
    let mut total_cycles = 0u64;
    for _ in 0..100 {
        if cpu.halted {
            break;
        }
        if cpu.pc < 0x1000 || cpu.pc >= 0x1000 + copy_len as u32 {
            break;
        }

        let cycles = cpu.step_instruction(&mut memory);
        assert!(cycles > 0, "an instruction must take time");
        total_cycles += cycles as u64;
        assert_eq!(cpu.cycles - start, total_cycles, "cycle counter drift");

        if total_cycles > 10_000 {
            break;
        }
    }
});
