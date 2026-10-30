#![no_main]

//! EverDrive mailbox fuzzer: arbitrary register traffic and host input
//! against the command parser, with no SD card attached so nothing touches
//! the host file system.

use genteel::memory::everdrive_io::{EverdriveIo, MAILBOX_BASE};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|ops: Vec<(u8, u8, u16)>| {
    let mut io = EverdriveIo::new();
    for (op, byte, word) in ops {
        let addr = MAILBOX_BASE + u32::from(byte & 0xF);
        match op % 8 {
            0 | 1 => io.write_mailbox(byte),
            2 => {
                io.read_word(addr & !1);
            }
            3 => {
                io.read_byte(addr);
            }
            4 => io.write_byte(addr, byte),
            5 => io.write_word(addr & !1, word),
            6 => io.tick(u32::from(word), byte & 1 != 0),
            _ => {
                io.push_host_input(&word.to_be_bytes());
            }
        }
        assert!(io.rx_level() <= genteel::memory::everdrive_io::RX_FIFO_CAPACITY);
    }
    let _ = io.take_usb_output();
});
