#![no_main]

//! Z80 prefix-sequence fuzzer.
//!
//! Builds chains of DD/FD/ED/CB prefixes in front of a simple opcode:
//! - DD/FD redirect HL to IX/IY, and only the last of a run applies;
//! - DD/FD before ED is overridden by ED;
//! - DD CB / FD CB select the indexed bit operations.
//!
//! Execution must never panic.

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

const PREFIXES: [u8; 4] = [0xDD, 0xFD, 0xED, 0xCB];

/// NOP, INC B, DEC B, INC A, DEC A, XOR A, LD A,B.
const BASE_OPCODES: [u8; 7] = [0x00, 0x04, 0x05, 0x3C, 0x3D, 0xAF, 0x78];

fuzz_target!(|data: &[u8]| {
    if data.len() < 6 {
        return;
    }

    let num_prefixes = (data[0] % 4) as usize;
    let mut program = Vec::with_capacity(16);
    for &b in &data[1..=num_prefixes] {
        program.push(PREFIXES[(b % 4) as usize]);
    }
    program.push(BASE_OPCODES[data[num_prefixes + 1] as usize % BASE_OPCODES.len()]);
    /* Operand bytes for displacement and immediate forms. */
    for i in 0..4 {
        program.push(data.get(num_prefixes + 2 + i).copied().unwrap_or(0));
    }

    let mut memory = Memory::new(0x10000);
    memory.data[..program.len()].copy_from_slice(&program);

    let mut cpu = Z80::new(memory, OpenBusIo);
    cpu.sp = 0x8000;
    cpu.set_hl(0x4000);
    cpu.ix = 0x5000;
    cpu.iy = 0x6000;
    cpu.a = data[0];
    cpu.b = data[1];

    for _ in 0..10 {
        if cpu.halted || cpu.pc as usize >= program.len() {
            break;
        }
        cpu.step();
    }
});
