#![allow(unused_imports)]
//! Z80 Block Operation Tests
//!
//! Exhaustive property-based tests for block transfer and search operations.
//! Includes massive randomization of state to cover edge cases, overlaps, and wrapping.

use super::*;
use crate::memory::{IoInterface, Memory, MemoryInterface};

// Simple deterministic RNG to avoid dependencies
struct Rng {
    state: u32,
}
impl Rng {
    fn new(seed: u32) -> Self {
        Self { state: seed }
    }
    fn next(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(1664525).wrapping_add(1013904223);
        self.state
    }
    fn next_u16(&mut self) -> u16 {
        (self.next() >> 16) as u16
    }
    fn next_u8(&mut self) -> u8 {
        (self.next() >> 24) as u8
    }
    fn _range(&mut self, min: u16, max: u16) -> u16 {
        min + (self.next_u16() % (max - min))
    }
}

use crate::z80::test_utils::create_z80;

/// Snapshot memory contents into a Vec for reference comparison
fn snapshot_memory<M: MemoryInterface, I: crate::memory::IoInterface>(
    z80: &mut Z80<M, I>,
) -> Vec<u8> {
    let mut snapshot = Vec::with_capacity(0x10000);
    for addr in 0..0x10000u32 {
        snapshot.push(bus.memory.read_byte(addr));
    }
    snapshot
}

fn reference_ldir(mem: &mut [u8], mut hl: u16, mut de: u16, mut bc: u16) -> (u16, u16, u16) {
    if bc == 0 {
        return (hl, de, bc);
    } // Initial check? No, Z80 checks AFTER dec?
      // Actually LDIR checks BC!=0 at start of repeat loop.
      // But instruction executes: LD (DE),(HL); INC HL; INC DE; DEC BC.
      // Then checks BC. If BC!=0 repeat.
      // So if entry BC=0, it loops 65536 times?
      // Z80 User Manual: "If BC is 0, the instruction loops 64K times".
      // Wait, the instruction does operations then decrements.
      // If BC=0 on entry:
      // dec BC -> BC=0xFFFF.
      // loops until BC=0.

    let iterations = if bc == 0 { 0x10000 } else { bc as u32 };

    // Rust reference simulation needs to handle overlap byte-by-byte
    for _ in 0..iterations {
        let val = mem[hl as usize];
        mem[de as usize] = val;
        hl = hl.wrapping_add(1);
        de = de.wrapping_add(1);
        bc = bc.wrapping_sub(1);
    }
    (hl, de, bc)
}

fn reference_lddr(mem: &mut [u8], mut hl: u16, mut de: u16, mut bc: u16) -> (u16, u16, u16) {
    let iterations = if bc == 0 { 0x10000 } else { bc as u32 };
    for _ in 0..iterations {
        let val = mem[hl as usize];
        mem[de as usize] = val;
        hl = hl.wrapping_sub(1);
        de = de.wrapping_sub(1);
        bc = bc.wrapping_sub(1);
    }
    (hl, de, bc)
}

#[test]
fn test_ldir_exhaustive() {
    let mut rng = Rng::new(0x12345678);
    // Run 5,000 randomized tests (heavy weight)
    for i in 0..5000 {
        let src = rng.next_u16();
        let dst = rng.next_u16();
        // Weighted length distribution: heavy on small, some large, some 0
        let len_case = rng.next() % 10;
        let len = if len_case < 7 {
            (rng.next() % 32) as u16
        } else if len_case < 9 {
            (rng.next() % 1024) as u16
        } else {
            // 0 is handled as 65536, let's test specific BC=0 separately generally,
            // but include some random large ones
            rng.next_u16()
        };

        let bc = len as u16;

        // Setup Z80
        // We put the LDIR instruction at some safe place, e.g., 0x0000,
        // assuming src/dst don't overwrite it immediately.
        // To be safe, we execute until PC indicates completion.

        let (mut cpu, mut bus) = create_z80(&[]);
        // Put Opcode at 0x100 avoids conflict usually?
        // Let's randomize PC placement too? No, keep simple.
        let _code_base = 0x0000;
        bus.memory.write_byte(0 as u32, 0xED);
        bus.memory.write_byte(1 as u32, 0xB0); // LDIR
        cpu.pc = 0;

        cpu.set_hl(src);
        cpu.set_de(dst);
        cpu.set_bc(bc);

        // Fill memory with random junk
        // We can't fill 64k every time (too slow).
        // Just fill the affected source range.
        let mut ref_mem = snapshot_memory(&mut cpu);

        // Fill source area in both
        let real_len = if bc == 0 { 0x10000 } else { bc as usize };
        // Limit fill to avoid timeout on huge BC=0 tests in loop
        // If BC=0 (64k), we only fill a subset or accept 0s.
        // Let's rely on 'junk' fill:
        // Fill a window around src and dst
        for k in 0..256 {
            let val = rng.next_u8();
            let s_addr = src.wrapping_add(k) as usize;
            let d_addr = dst.wrapping_add(k) as usize;
            bus.memory.write_byte(s_addr as u32, val);
            ref_mem[s_addr] = val;
            bus.memory.write_byte(d_addr as u32, val ^ 0xFF); // Different initial dst
            ref_mem[d_addr] = val ^ 0xFF;
        }
        // Also fill end of range
        if real_len > 256 {
            let val = rng.next_u8();
            let s_addr = src.wrapping_add((real_len - 1) as u16) as usize;
            bus.memory.write_byte(s_addr as u32, val);
            ref_mem[s_addr] = val;
        }

        // Run Reference
        // Be careful with large BC in reference loop - it's fast in native code
        let (exp_hl, exp_de, exp_bc) = reference_ldir(&mut ref_mem, src, dst, bc);

        // Run Emulator
        // Step until PC moves past instruction
        // Safety Break
        let mut steps = 0;
        loop {
            // Check if we are about to execute LDIR
            // If PC == 0, we are at LDIR.
            // If instructions can be overwritten (self-modifying code), check that too?
            // If LDIR overwrites itself, behavior is undefined/complex.
            // We assume test cases generally don't overwrite 0x0000 unless random src/dst hits it.
            // If so, both ref and cpu behavior should arguably match or diverge.
            // For chaos test, let's accept divergence if code is overwritten.
            // But checking code integrity complicates things.
            // Let's check if code is intact.
            if bus.memory.read_byte(0 as u32) != 0xED || bus.memory.read_byte(1 as u32) != 0xB0 {
                // Code overwritten. Skip verification of this insane case.
                break;
            }

            cpu.step(&mut bus);
            steps += 1;
            if cpu.pc != 0 {
                break;
            } // Loop done
            if steps > 70000 {
                panic!("LDIR infinite loop or too long? BC={}", bc);
            }
        }

        // Validation
        if bus.memory.read_byte(0 as u32) == 0xED {
            // valid result check
            assert_eq!(cpu.hl(), exp_hl, "HL mismatch case #{}", i);
            assert_eq!(cpu.de(), exp_de, "DE mismatch case #{}", i);
            assert_eq!(cpu.bc(), exp_bc, "BC mismatch case #{}", i);

            // Check memory window
            // We can't check all 64k. Check random samples + boundaries.
            for _k in 0..50 {
                let offset = rng.next() as usize % 0x10000;
                assert_eq!(
                    bus.memory.read_byte(offset as u32),
                    ref_mem[offset],
                    "Mem mismatch at {} case #{} BC={}",
                    offset,
                    i,
                    bc
                );
            }
        }
    }
}

#[test]
fn test_lddr_exhaustive() {
    let mut rng = Rng::new(0x87654321);
    for i in 0..2000 {
        let src = rng.next_u16();
        let dst = rng.next_u16();
        let bc = (rng.next() % 0x100) as u16; // Limit size for speed, cover 256

        let (mut cpu, mut bus) = create_z80(&[0xED, 0xB8]); // LDDR at 0x0000
        cpu.set_hl(src);
        cpu.set_de(dst);
        cpu.set_bc(bc);

        let mut ref_mem = snapshot_memory(&mut cpu);

        // Fill some data
        for k in 0..bc {
            let addr = src.wrapping_sub(k);
            let val = rng.next_u8();
            bus.memory.write_byte(addr as usize as u32, val);
            ref_mem[addr as usize] = val;
        }

        let (exp_hl, exp_de, exp_bc) = reference_lddr(&mut ref_mem, src, dst, bc);

        // Run
        loop {
            if bus.memory.read_byte(0 as u32) != 0xED || bus.memory.read_byte(1 as u32) != 0xB8 {
                break;
            }
            cpu.step(&mut bus);
            if cpu.pc != 0 {
                break;
            }
        }

        // Validate
        if bus.memory.read_byte(0 as u32) == 0xED && bus.memory.read_byte(1 as u32) == 0xB8 {
            assert_eq!(cpu.hl(), exp_hl, "LDDR HL mismatch #{}", i);
            assert_eq!(cpu.de(), exp_de);
            assert_eq!(cpu.bc(), exp_bc);
            // Check Dest region
            if bc > 0 {
                let check_addr = dst;
                assert_eq!(
                    bus.memory.read_byte(check_addr as usize as u32),
                    ref_mem[check_addr as usize]
                );
                let check_addr_end = dst.wrapping_sub(bc - 1);
                assert_eq!(
                    bus.memory.read_byte(check_addr_end as usize as u32),
                    ref_mem[check_addr_end as usize]
                );
            }
        }
    }
}

#[test]
fn test_cpir_validation() {
    let mut rng = Rng::new(0xDEADBEEF);
    // 5000 small random scans
    for i in 0..5000 {
        let hl = rng.next_u16();
        let bc = (rng.next() % 256) as u16 + 1; // 1..256
        let target = rng.next_u8();

        let (mut cpu, mut bus) = create_z80(&[0xED, 0xB1]);
        cpu.set_hl(hl);
        cpu.set_bc(bc);
        cpu.a = target;

        // Logic:
        // Place target at random position? Or fill with noise.
        // Let's decide if we want to Find it or Not.
        let should_find = (rng.next() % 2) == 0;
        let found_idx = if should_find {
            rng.next() as u16 % bc
        } else {
            bc + 10 // Not in range
        };

        // Fill memory range hl..hl+bc
        for k in 0..bc {
            let addr = hl.wrapping_add(k);
            let val = if k == found_idx {
                target
            } else {
                target.wrapping_add(1)
            };
            bus.memory.write_byte(addr as usize as u32, val);
        }

        // Run
        loop {
            // Guard against self-modification
            if bus.memory.read_byte(0 as u32) != 0xED || bus.memory.read_byte(1 as u32) != 0xB1 {
                break;
            }
            cpu.step(&mut bus);
            if cpu.pc != 0 {
                break;
            }
        }

        if bus.memory.read_byte(0 as u32) == 0xED && bus.memory.read_byte(1 as u32) == 0xB1 {
            if should_find {
                assert!(cpu.get_flag(flags::ZERO), "Should fulfill find #{}", i);
                // Verify HL points to char AFTER found
                let exp_hl = hl.wrapping_add(found_idx).wrapping_add(1);
                assert_eq!(cpu.hl(), exp_hl);
                // BC should be decremented by (index + 1)
                let exp_bc = bc - (found_idx + 1);
                assert_eq!(cpu.bc(), exp_bc);
            } else {
                assert!(!cpu.get_flag(flags::ZERO), "Should NOT find #{}", i);
                assert_eq!(cpu.bc(), 0);
                let exp_hl = hl.wrapping_add(bc);
                assert_eq!(cpu.hl(), exp_hl);
            }
        }
    }
}

#[test]
fn test_inir_simulation() {
    // Tests register logic for INIR (mock IO)
    let (mut c, mut bus) = create_z80(&[0xED, 0xB2]); // INIR
    c.set_hl(0x2000);
    c.set_bc(0x0500); // B=5, C=0 implies port 0

    // As per stub, IO READ returns 0xFF.
    // So 5 bytes of 0xFF should be written to 2000..2004
    // B should become 0.

    // Loop
    loop {
        c.step(&mut bus);
        if c.pc != 0 {
            break;
        }
    }

    assert_eq!(c.b, 0);
    assert_eq!(c.hl(), 0x2005);
    // Check flags
    assert!(c.get_flag(flags::ZERO)); // B=0

    // Check memory (assuming stub wrote 0xFF)
    for i in 0..5 {
        assert_eq!(bus.memory.read_byte(0x2000 + i as u32), 0xFF);
    }
}
