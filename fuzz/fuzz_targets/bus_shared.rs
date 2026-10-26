#![no_main]

//! Shared-bus fuzzer: the same as `memory_ops`, but through `SharedBus` (the
//! `Rc<RefCell<Bus>>` wrapper the CPUs use) with 24-bit addresses.

use genteel::memory::{bus::Bus, MemoryInterface, SharedBus};
use libfuzzer_sys::fuzz_target;
use std::cell::RefCell;
use std::rc::Rc;

fuzz_target!(|ops: Vec<(u8, u32, u32)>| {
    let bus = Rc::new(RefCell::new(Bus::new()));
    bus.borrow_mut().load_rom(&[0; 4096]);
    let mut shared = SharedBus::new(bus.clone());

    for (op_type, addr, val) in ops {
        let addr = addr & 0xFF_FFFF;
        match op_type % 6 {
            0 => {
                shared.read_byte(addr);
            }
            1 => shared.write_byte(addr, val as u8),
            2 => {
                shared.read_word(addr);
            }
            3 => shared.write_word(addr, val as u16),
            4 => {
                shared.read_long(addr);
            }
            _ => shared.write_long(addr, val),
        }
    }
});
