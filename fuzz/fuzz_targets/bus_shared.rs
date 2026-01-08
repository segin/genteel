#![no_main]
use libfuzzer_sys::fuzz_target;
use genteel::memory::{bus::Bus, SharedBus, MemoryInterface};
use std::rc::Rc;
use std::cell::RefCell;

fuzz_target!(|ops: Vec<(u8, u32, u32)>| {
    let bus = Rc::new(RefCell::new(Bus::new()));
    let mut shared = SharedBus::new(bus.clone());
    
    // Load some dummy ROM
    bus.borrow_mut().load_rom(&vec![0; 4096]);

    for (op_type, addr, val) in ops {
        // Mask address to 24-bit for Mega Drive
        let addr = addr & 0xFFFFFF;
        
        match op_type % 6 {
            0 => { shared.read_byte(addr); },
            1 => { shared.write_byte(addr, val as u8); },
            2 => { shared.read_word(addr); },
            3 => { shared.write_word(addr, val as u16); },
            4 => { shared.read_long(addr); },
            5 => { shared.write_long(addr, val); },
            _ => unreachable!(),
        }
    }
});
