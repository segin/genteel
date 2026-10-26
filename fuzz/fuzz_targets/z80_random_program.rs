#![no_main]

//! Structured Z80 fuzzer: the first bytes seed A and BC, the rest is the
//! program. Index and pointer registers start in a safe RAM area.

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

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }

    let initial_a = data[0];
    let initial_bc = u16::from_le_bytes([data[1], data[2]]);

    let mut memory = Memory::new(0x10000);
    let program = &data[3..];
    let copy_len = program.len().min(0x4000);
    memory.data[..copy_len].copy_from_slice(&program[..copy_len]);

    let mut z80 = Z80::new(memory, OpenBusIo);
    z80.a = initial_a;
    z80.set_bc(initial_bc);
    z80.sp = 0x8000;
    z80.set_hl(0x2000);
    z80.set_de(0x3000);
    z80.ix = 0x4000;
    z80.iy = 0x5000;

    for _ in 0..1000 {
        if z80.halted || z80.pc as usize >= copy_len {
            break;
        }
        z80.step();

        /* The 16-bit pair views must always agree with the 8-bit halves. */
        assert_eq!(z80.bc(), u16::from_be_bytes([z80.b, z80.c]));
        assert_eq!(z80.de(), u16::from_be_bytes([z80.d, z80.e]));
        assert_eq!(z80.hl(), u16::from_be_bytes([z80.h, z80.l]));
    }
});
