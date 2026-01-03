//! Extended M68k CPU Tests
//!
//! Comprehensive unit tests and property-based tests for M68k instructions.

#![cfg(test)]

use proptest::prelude::*;
use super::*;
use crate::memory::{Memory, MemoryInterface};

fn create_test_cpu() -> Cpu {
    let mut memory = Memory::new(0x10000);
    // Set up minimal vector table
    memory.write_long(0, 0x1000);  // SSP
    memory.write_long(4, 0x100);   // Reset vector PC
    Cpu::new(Box::new(memory))
}

// ============ CHK Tests ============

#[test]
fn test_chk_in_bounds() {
    let mut cpu = create_test_cpu();
    
    // CHK D1, D0 - opcode 4181
    cpu.memory.write_word(0x100, 0x4181);
    cpu.d[0] = 10;  // Value to check
    cpu.d[1] = 50;  // Upper bound
    
    cpu.step_instruction();
    
    // Should not trap - value in bounds [0..50]
    assert_eq!(cpu.pc, 0x102); // Normal execution continues
}

#[test]
fn test_chk_negative() {
    let mut cpu = create_test_cpu();
    
    // Set up CHK exception vector (vector 6 = address 0x18)
    cpu.memory.write_long(0x18, 0x4000);
    
    // CHK D1, D0
    cpu.memory.write_word(0x100, 0x4181);
    cpu.d[0] = 0xFFFF; // -1 as i16
    cpu.d[1] = 50;
    
    cpu.step_instruction();
    
    // Should trap to vector 6
    assert_eq!(cpu.pc, 0x4000);
    assert!(cpu.get_flag(flags::NEGATIVE)); // N set for negative
}

#[test]
fn test_chk_exceeds_bound() {
    let mut cpu = create_test_cpu();
    
    // Set up CHK exception vector
    cpu.memory.write_long(0x18, 0x5000);
    
    // CHK D1, D0
    cpu.memory.write_word(0x100, 0x4181);
    cpu.d[0] = 100;  // Exceeds bound
    cpu.d[1] = 50;   // Upper bound
    
    cpu.step_instruction();
    
    // Should trap
    assert_eq!(cpu.pc, 0x5000);
    assert!(!cpu.get_flag(flags::NEGATIVE)); // N clear for exceeds
}

// ============ TAS Tests ============

#[test]
fn test_tas_zero() {
    let mut cpu = create_test_cpu();
    
    // TAS (A0) - opcode 4AD0
    cpu.memory.write_word(0x100, 0x4AD0);
    cpu.a[0] = 0x2000;
    cpu.memory.write_byte(0x2000, 0x00);
    
    cpu.step_instruction();
    
    // Flags from original value (0)
    assert!(cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::NEGATIVE));
    
    // High bit now set
    assert_eq!(cpu.memory.read_byte(0x2000), 0x80);
}

#[test]
fn test_tas_negative() {
    let mut cpu = create_test_cpu();
    
    // TAS D0 - opcode 4AC0
    cpu.memory.write_word(0x100, 0x4AC0);
    cpu.d[0] = 0x80;
    
    cpu.step_instruction();
    
    // High bit already set
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(cpu.get_flag(flags::NEGATIVE));
    assert_eq!(cpu.d[0] & 0xFF, 0x80);
}

// ============ MOVEM Tests ============

#[test]
fn test_movem_to_memory() {
    let mut cpu = create_test_cpu();
    
    // MOVEM.L D0-D1, (A0)
    // Opcode: 0100 1000 1101 0000 = 0x48D0 (MOVEM.L regs to (A0))
    cpu.memory.write_word(0x100, 0x48D0);
    cpu.memory.write_word(0x102, 0x0003); // D0, D1 = bits 0,1
    
    cpu.a[0] = 0x2000;
    cpu.d[0] = 0x11111111;
    cpu.d[1] = 0x22222222;
    
    cpu.step_instruction();
    
    // Check memory
    assert_eq!(cpu.memory.read_long(0x2000), 0x11111111);
    assert_eq!(cpu.memory.read_long(0x2004), 0x22222222);
}

#[test]
fn test_movem_from_memory() {
    let mut cpu = create_test_cpu();
    
    // MOVEM.L (A0)+, D0-D1
    // Opcode: 4CD8, mask follows
    cpu.memory.write_word(0x100, 0x4CD8);
    cpu.memory.write_word(0x102, 0x0003); // D0, D1
    
    cpu.a[0] = 0x2000;
    cpu.memory.write_long(0x2000, 0xDEADBEEF);
    cpu.memory.write_long(0x2004, 0xCAFEBABE);
    
    cpu.step_instruction();
    
    assert_eq!(cpu.d[0], 0xDEADBEEF);
    assert_eq!(cpu.d[1], 0xCAFEBABE);
    assert_eq!(cpu.a[0], 0x2008); // Post-incremented
}

// ============ PEA Tests ============

#[test]
fn test_pea() {
    let mut cpu = create_test_cpu();
    
    // PEA (A0) - opcode 4850
    cpu.memory.write_word(0x100, 0x4850);
    cpu.a[0] = 0x12345678;
    cpu.a[7] = 0x3000;
    
    cpu.step_instruction();
    
    // Effective address pushed to stack
    assert_eq!(cpu.a[7], 0x2FFC);
    assert_eq!(cpu.memory.read_long(0x2FFC), 0x12345678);
}

// ============ SR Operation Tests ============

#[test]
fn test_move_to_sr() {
    let mut cpu = create_test_cpu();
    cpu.sr = 0x2700; // Supervisor mode
    
    // MOVE #$2300, SR - set new SR
    cpu.memory.write_word(0x100, 0x46FC);
    cpu.memory.write_word(0x102, 0x2300);
    
    cpu.step_instruction();
    
    assert_eq!(cpu.sr, 0x2300);
}

#[test]
fn test_move_to_sr_privilege_violation() {
    let mut cpu = create_test_cpu();
    cpu.sr = 0x0000; // User mode
    
    // Set up privilege violation vector (vector 8)
    cpu.memory.write_long(0x20, 0x6000);
    
    // MOVE #$2300, SR
    cpu.memory.write_word(0x100, 0x46FC);
    cpu.memory.write_word(0x102, 0x2300);
    
    cpu.step_instruction();
    
    // Should trap
    assert_eq!(cpu.pc, 0x6000);
    assert!((cpu.sr & 0x2000) != 0); // Now in supervisor
}

#[test]
fn test_move_to_ccr() {
    let mut cpu = create_test_cpu();
    cpu.sr = 0x2700;
    
    // MOVE #$1F, CCR
    cpu.memory.write_word(0x100, 0x44FC);
    cpu.memory.write_word(0x102, 0x001F);
    
    cpu.step_instruction();
    
    // Only CCR bits set, supervisor bits unchanged
    assert_eq!(cpu.sr, 0x271F);
}

#[test]
fn test_andi_to_ccr() {
    let mut cpu = create_test_cpu();
    cpu.sr = 0x271F; // All CCR bits set
    
    // ANDI #$10, CCR - Clear all except X
    cpu.memory.write_word(0x100, 0x023C);
    cpu.memory.write_word(0x102, 0x0010);
    
    cpu.step_instruction();
    
    assert_eq!(cpu.sr & 0x00FF, 0x0010);
}

// ============ Property Tests ============

proptest! {
    #[test]
    fn prop_add_sub_inverse(a in 0u16..0xFFFF, b in 0u16..0xFFFF) {
        let mut cpu = create_test_cpu();
        cpu.d[0] = a as u32;
        cpu.d[1] = b as u32;
        
        // ADD.W D1, D0
        cpu.memory.write_word(0x100, 0xD041);
        cpu.step_instruction();
        
        let sum = cpu.d[0] & 0xFFFF;
        
        // SUB.W D1, D0
        cpu.pc = 0x102;
        cpu.memory.write_word(0x102, 0x9041);
        cpu.step_instruction();
        
        // Should get back original
        prop_assert_eq!(cpu.d[0] & 0xFFFF, a as u32);
    }
    
    #[test]
    fn prop_neg_neg_identity(a in 0u16..0xFFFF) {
        let mut cpu = create_test_cpu();
        cpu.d[0] = a as u32;
        
        // NEG.W D0 twice should restore original
        cpu.memory.write_word(0x100, 0x4440);
        cpu.memory.write_word(0x102, 0x4440);
        
        cpu.step_instruction();
        cpu.step_instruction();
        
        prop_assert_eq!(cpu.d[0] & 0xFFFF, a as u32);
    }
    
    #[test]
    fn prop_not_not_identity(a in any::<u32>()) {
        let mut cpu = create_test_cpu();
        cpu.d[0] = a;
        
        // NOT.L D0 twice should restore original
        cpu.memory.write_word(0x100, 0x4680);
        cpu.memory.write_word(0x102, 0x4680);
        
        cpu.step_instruction();
        cpu.step_instruction();
        
        prop_assert_eq!(cpu.d[0], a);
    }
    
    #[test]
    fn prop_swap_swap_identity(a in any::<u32>()) {
        let mut cpu = create_test_cpu();
        cpu.d[0] = a;
        
        // SWAP D0 twice should restore original
        cpu.memory.write_word(0x100, 0x4840);
        cpu.memory.write_word(0x102, 0x4840);
        
        cpu.step_instruction();
        cpu.step_instruction();
        
        prop_assert_eq!(cpu.d[0], a);
    }
    
    #[test]
    fn prop_mul_preserves_low_word_with_one(a in 0u16..0xFFFF) {
        let mut cpu = create_test_cpu();
        cpu.d[0] = a as u32;
        cpu.d[1] = 1;
        
        // MULU D1, D0 (multiply by 1)
        cpu.memory.write_word(0x100, 0xC0C1);
        cpu.step_instruction();
        
        prop_assert_eq!(cpu.d[0], a as u32);
    }
}
