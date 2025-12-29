#![no_main]

//! Flag computation fuzzer - focuses on finding bugs in flag handling
//!
//! The Z80 has complex flag behavior that's easy to get wrong:
//! - Half-carry (H flag) for BCD operations
//! - Overflow (P/V flag) for signed arithmetic
//! - Undocumented flags (bits 3 and 5)
//! - Different behavior for different instruction types

use libfuzzer_sys::fuzz_target;
use genteel::z80::{Z80, flags};
use genteel::memory::Memory;

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 { return; }
    
    let a_val = data[0];
    let b_val = data[1];
    let opcode_class = data[2] % 8; // Which ALU operation
    let carry_in = (data[3] & 1) != 0;
    
    // Test ADD, ADC, SUB, SBC, AND, XOR, OR, CP
    let opcodes = [0x80, 0x88, 0x90, 0x98, 0xA0, 0xA8, 0xB0, 0xB8];
    let opcode = opcodes[opcode_class as usize];
    
    let mut memory = Memory::new(0x10000);
    memory.data[0] = opcode;
    
    let mut cpu = Z80::new(memory);
    cpu.a = a_val;
    cpu.b = b_val;
    if carry_in {
        cpu.set_flag(flags::CARRY, true);
    }
    
    cpu.step();
    
    // Verify basic flag invariants:
    
    // 1. Zero flag should match result being zero
    let result = cpu.a;
    if opcode != 0xB8 { // CP doesn't modify A
        assert_eq!(cpu.get_flag(flags::ZERO), result == 0,
            "Zero flag mismatch: A={:02X}, Z={}", result, cpu.get_flag(flags::ZERO));
    }
    
    // 2. Sign flag should match bit 7 of result
    if opcode != 0xB8 {
        assert_eq!(cpu.get_flag(flags::SIGN), (result & 0x80) != 0,
            "Sign flag mismatch: A={:02X}, S={}", result, cpu.get_flag(flags::SIGN));
    }
    
    // 3. N flag: set for SUB/SBC/CP, clear for ADD/ADC/AND/XOR/OR
    let expect_n = opcode_class == 2 || opcode_class == 3 || opcode_class == 7;
    assert_eq!(cpu.get_flag(flags::ADD_SUB), expect_n,
        "N flag mismatch for opcode {:02X}", opcode);
    
    // 4. AND always sets H, clears C
    if opcode_class == 4 {
        assert!(cpu.get_flag(flags::HALF_CARRY), "AND should set H flag");
        assert!(!cpu.get_flag(flags::CARRY), "AND should clear C flag");
    }
    
    // 5. OR/XOR clear H and C
    if opcode_class == 5 || opcode_class == 6 {
        assert!(!cpu.get_flag(flags::HALF_CARRY), "OR/XOR should clear H flag");
        assert!(!cpu.get_flag(flags::CARRY), "OR/XOR should clear C flag");
    }
});
