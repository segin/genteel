#![no_main]

//! Genesis bus fuzzer: arbitrary byte/word/long reads and writes anywhere in
//! the 32-bit address space must never panic.

use genteel::memory::bus::Bus;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|ops: Vec<(u8, u32, u32)>| {
    let mut bus = Bus::new();
    bus.load_rom(&[0; 4096]);

    for (op_type, addr, val) in ops {
        match op_type % 6 {
            0 => {
                bus.read_byte(addr);
            }
            1 => bus.write_byte(addr, val as u8),
            2 => {
                bus.read_word(addr);
            }
            3 => bus.write_word(addr, val as u16),
            4 => {
                bus.read_long(addr);
            }
            _ => bus.write_long(addr, val),
        }
    }
});
