#![no_main]

//! Z80 memory-boundary fuzzer.
//!
//! Checks stack, indexed and block accesses at arbitrary addresses:
//! - PUSH/POP with SP anywhere;
//! - IX+d / IY+d with extreme displacements (16-bit wraparound);
//! - LDI/LDD with arbitrary source and destination;
//! - LD (nn),HL at arbitrary addresses.
//!
//! Cases whose data would overwrite the instruction under test are skipped.

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

/// Bytes at the start of RAM that hold the instruction under test.
const CODE_LEN: u16 = 4;

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 {
        return;
    }

    let word = |i: usize| u16::from_le_bytes([data[i], data[i + 1]]);
    let mut cpu = Z80::new(Memory::new(0x10000), OpenBusIo);

    match data[0] % 6 {
        0 => {
            cpu.memory.data[0] = 0xC5; // PUSH BC
            cpu.memory.data[1] = 0xC1; // POP BC
            cpu.sp = word(1);
            cpu.set_bc(word(3));

            /* The two pushed bytes land at SP-1 and SP-2. */
            if cpu.sp.wrapping_sub(2) >= CODE_LEN && cpu.sp >= 2 {
                let orig_bc = cpu.bc();
                cpu.step();
                cpu.set_bc(0);
                cpu.step();
                assert_eq!(cpu.bc(), orig_bc, "PUSH/POP BC failed at SP={:04X}", cpu.sp);
            }
        }
        1 | 2 => {
            let (prefix, value) = if data[0] % 6 == 1 {
                (0xDD, 0x42)
            } else {
                (0xFD, 0x55)
            };
            let d = data[1] as i8;
            cpu.memory.data[0] = prefix;
            cpu.memory.data[1] = 0x7E; // LD A,(IX/IY+d)
            cpu.memory.data[2] = d as u8;
            let base = word(3);
            if prefix == 0xDD {
                cpu.ix = base;
            } else {
                cpu.iy = base;
            }

            let addr = base.wrapping_add(d as i16 as u16);
            if addr >= CODE_LEN {
                cpu.memory.data[addr as usize] = value;
                cpu.step();
                assert_eq!(
                    cpu.a, value,
                    "LD A,({prefix:02X}+{d}) failed with base {base:04X}"
                );
            }
        }
        3 | 4 => {
            let ldi = data[0] % 6 == 3;
            cpu.memory.data[0] = 0xED;
            cpu.memory.data[1] = if ldi { 0xA0 } else { 0xA8 };
            let src = word(1);
            let dst = word(3);
            cpu.set_hl(src);
            cpu.set_de(dst);
            cpu.set_bc(word(5).max(1));

            if src >= CODE_LEN && dst >= CODE_LEN {
                let val = data[7];
                cpu.memory.data[src as usize] = val;
                cpu.step();

                assert_eq!(
                    cpu.memory.data[dst as usize], val,
                    "block copy {src:04X}->{dst:04X}"
                );
                let (hl, de) = if ldi {
                    (src.wrapping_add(1), dst.wrapping_add(1))
                } else {
                    (src.wrapping_sub(1), dst.wrapping_sub(1))
                };
                assert_eq!(cpu.hl(), hl, "HL after block copy");
                assert_eq!(cpu.de(), de, "DE after block copy");
            }
        }
        _ => {
            let addr = word(1);
            if (CODE_LEN..0xFFFF).contains(&addr) {
                cpu.memory.data[0] = 0x22; // LD (nn),HL
                cpu.memory.data[1] = addr as u8;
                cpu.memory.data[2] = (addr >> 8) as u8;
                cpu.set_hl(word(3));

                let orig_hl = cpu.hl();
                cpu.step();

                let stored = u16::from_le_bytes([
                    cpu.memory.data[addr as usize],
                    cpu.memory.data[addr as usize + 1],
                ]);
                assert_eq!(stored, orig_hl, "LD (nn),HL failed at {addr:04X}");
            }
        }
    }
});
