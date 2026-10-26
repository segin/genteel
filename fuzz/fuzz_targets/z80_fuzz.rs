#![no_main]

//! Z80 random-program fuzzer: executes arbitrary code from flat 64 KiB RAM
//! and checks that the CPU never panics.

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
    if data.is_empty() {
        return;
    }

    let mut memory = Memory::new(0x10000);
    let copy_len = data.len().min(0x8000);
    memory.data[..copy_len].copy_from_slice(&data[..copy_len]);

    let mut z80 = Z80::new(memory, OpenBusIo);
    z80.sp = 0xFF00;

    for _ in 0..10_000 {
        if z80.halted || z80.pc as usize >= copy_len {
            break;
        }
        let cycles = z80.step();
        assert!(cycles > 0, "an instruction must take time");
    }
});
