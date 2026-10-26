#![no_main]

//! Z80 edge-case fuzzer.
//!
//! Seeds A, F, BC and DE from the input (so boundary values such as 0x00,
//! 0x7F, 0x80 and 0xFF, and every flag combination, are reached) and runs
//! the rest as a program, checking per-step invariants:
//! - every step takes a plausible number of T-states (a run of redundant
//!   DD/FD prefixes is folded into one step, up to 17 prefixes);
//! - the 16-bit register pairs always agree with their 8-bit halves.

use genteel::memory::{IoInterface, Memory};
use genteel::z80::Z80;
use libfuzzer_sys::fuzz_target;

/// I/O space with nothing attached: reads float high, writes are dropped.
#[derive(Debug)]
struct OpenBusIo;

impl IoInterface for OpenBusIo {
    fn read_port(&mut self, _port: u16) -> u8 {
        0xFF
    }
    fn write_port(&mut self, _port: u16, _value: u8) {}
}

/// Longest single step: 16 folded prefixes, then a 23 T-state DD CB op.
const MAX_STEP_T_STATES: u8 = 16 * 4 + 23;

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 {
        return;
    }

    let initial_a = data[0];
    let initial_f = data[1];
    let initial_bc = u16::from_le_bytes([data[2], data[3]]);
    let initial_de = u16::from_le_bytes([data[4], data[5]]);

    let mut memory = Memory::new(0x10000);
    let program = &data[6..];
    let copy_len = program.len().min(0x2000);
    memory.data[..copy_len].copy_from_slice(&program[..copy_len]);

    let mut cpu = Z80::new(memory, OpenBusIo);
    cpu.a = initial_a;
    cpu.f = initial_f;
    cpu.set_bc(initial_bc);
    cpu.set_de(initial_de);
    cpu.sp = 0x8000;
    cpu.set_hl(0x4000);
    cpu.ix = 0x5000;
    cpu.iy = 0x6000;

    for step in 0..500 {
        if cpu.halted || cpu.pc as usize >= copy_len {
            break;
        }

        let pc = cpu.pc;
        let cycles = cpu.step();
        assert!(
            (4..=MAX_STEP_T_STATES).contains(&cycles),
            "implausible cycle count {} at PC {:04X}",
            cycles,
            pc
        );

        assert_eq!(
            cpu.bc(),
            u16::from_be_bytes([cpu.b, cpu.c]),
            "BC at step {step}"
        );
        assert_eq!(
            cpu.de(),
            u16::from_be_bytes([cpu.d, cpu.e]),
            "DE at step {step}"
        );
        assert_eq!(
            cpu.hl(),
            u16::from_be_bytes([cpu.h, cpu.l]),
            "HL at step {step}"
        );
        assert_eq!(
            cpu.af(),
            u16::from_be_bytes([cpu.a, cpu.f]),
            "AF at step {step}"
        );
    }
});
