#![no_main]

//! M68k control-flow fuzzer.
//!
//! Runs a short arbitrary program with every exception vector pointing at a
//! STOP handler, exercising TRAP/CHK/TRAPV, privilege violations, address
//! errors and JSR/RTS/LINK/UNLK stack handling. The CPU must not panic.

use genteel::cpu::Cpu;
use genteel::memory::{Memory, MemoryInterface};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 20 {
        return;
    }

    let mut memory = Memory::new(0x100000);

    let program_len = data.len().min(0x100);
    for (i, &b) in data[..program_len].iter().enumerate() {
        memory.write_byte(0x1000 + i as u32, b);
    }

    /* Every vector points at a handler that stops the CPU. */
    for vector in 0..64 {
        memory.write_long(vector * 4, 0x2000);
    }
    memory.write_word(0x2000, 0x4E72); // STOP
    memory.write_word(0x2002, 0x2700); // #$2700

    memory.write_long(0x00, 0x8000); // reset SSP
    memory.write_long(0x04, 0x1000); // reset PC

    let mut cpu = Cpu::new(&mut memory);
    cpu.reset(&mut memory);

    let initial_sp = cpu.a[7];
    for _ in 0..50 {
        if cpu.halted || cpu.stopped {
            break;
        }
        if cpu.pc >= 0x1000 + program_len as u32 {
            break;
        }

        let old_pc = cpu.pc;
        let cycles = cpu.step_instruction(&mut memory);
        assert!(cycles > 0, "an instruction must take time");

        if cpu.pc == old_pc {
            break;
        }
        let sp_diff = (initial_sp as i64 - cpu.a[7] as i64).abs();
        if sp_diff > 0x1000 {
            break;
        }
    }
});
