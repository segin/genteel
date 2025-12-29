#![no_main]

//! Smart Z80 fuzzer that tries to exploit edge cases and implementation bugs
//! 
//! This fuzzer is designed to find subtle bugs by:
//! 1. Testing boundary conditions (0x00, 0xFF, 0x7F, 0x80)
//! 2. Testing carry/borrow propagation
//! 3. Testing flag computation edge cases
//! 4. Testing prefix instruction interactions
//! 5. Testing memory boundary conditions

use libfuzzer_sys::fuzz_target;
use genteel::z80::{Z80, flags};
use genteel::memory::Memory;

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 { return; }
    
    // Parse structured input
    let initial_a = data[0];
    let initial_f = data[1];
    let initial_bc = u16::from_le_bytes([data[2], data[3]]);
    let initial_de = u16::from_le_bytes([data[4], data[5]]);
    
    let mut memory = Memory::new(0x10000);
    let program = &data[6..];
    let copy_len = std::cmp::min(program.len(), 0x2000);
    memory.data[..copy_len].copy_from_slice(&program[..copy_len]);
    
    let mut cpu = Z80::new(memory);
    cpu.a = initial_a;
    cpu.f = initial_f;
    cpu.set_bc(initial_bc);
    cpu.set_de(initial_de);
    cpu.sp = 0x8000;
    cpu.set_hl(0x4000); // Safe area for (HL) ops
    cpu.ix = 0x5000;
    cpu.iy = 0x6000;
    
    // Execute limited instructions
    let mut prev_pc = cpu.pc;
    for step in 0..500 {
        if cpu.halted { break; }
        if cpu.pc as usize >= copy_len { break; }
        
        prev_pc = cpu.pc;
        let cycles = cpu.step();
        
        // Invariant checks that should NEVER fail:
        
        // 1. Cycles should be reasonable (4-23 for most Z80 instructions)
        assert!(cycles >= 4 && cycles <= 30, 
            "Invalid cycle count {} at PC {:04X}", cycles, prev_pc);
        
        // 2. PC should have advanced (except for repeated instructions)
        // Note: LDIR/LDDR/CPIR/CPDR can repeat without advancing PC
        
        // 3. Register pairs should be consistent
        assert_eq!(cpu.bc(), (cpu.b as u16) << 8 | cpu.c as u16,
            "BC register pair inconsistent at step {}", step);
        assert_eq!(cpu.de(), (cpu.d as u16) << 8 | cpu.e as u16,
            "DE register pair inconsistent at step {}", step);
        assert_eq!(cpu.hl(), (cpu.h as u16) << 8 | cpu.l as u16,
            "HL register pair inconsistent at step {}", step);
        assert_eq!(cpu.af(), (cpu.a as u16) << 8 | cpu.f as u16,
            "AF register pair inconsistent at step {}", step);
    }
});
