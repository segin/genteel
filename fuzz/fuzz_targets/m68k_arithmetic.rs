#![no_main]

//! M68k single-instruction fuzzer.
//!
//! Runs one arbitrary opcode from arbitrary D0/D1/SR values, aimed at the
//! arithmetic paths (overflow, division by zero, BCD carries). The CPU must
//! not panic and must account for every cycle it reports.

use genteel::cpu::flags;
use genteel::cpu::Cpu;
use genteel::memory::{Memory, MemoryInterface};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 12 {
        return;
    }

    let d0 = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let d1 = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let initial_sr = u16::from_le_bytes([data[8], data[9]]);
    let opcode = u16::from_le_bytes([data[10], data[11]]);

    let mut memory = Memory::new(0x100000);
    memory.write_word(0x1000, opcode);
    memory.write_word(0x1002, 0x4E71); // NOP
    memory.write_long(0x00, 0x8000); // reset SSP
    memory.write_long(0x04, 0x1000); // reset PC

    let mut cpu = Cpu::new(&mut memory);
    cpu.reset(&mut memory);
    cpu.d[0] = d0;
    cpu.d[1] = d1;
    /* Stay in supervisor mode and keep the unimplemented SR bits clear. */
    cpu.set_sr((initial_sr & 0xA71F) | flags::SUPERVISOR);

    let before = cpu.cycles;
    let cycles = cpu.step_instruction(&mut memory);
    assert!(cycles > 0, "an instruction must take time");
    assert_eq!(cpu.cycles, before + cycles as u64, "cycle counter drift");
});
