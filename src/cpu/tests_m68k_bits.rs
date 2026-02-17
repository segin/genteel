//! M68k Bit and Data Movement Tests
//!
//! Tests for bit manipulation (BTST, BSET, BCLR, BCHG) and data movement
//! (MOVE, MOVEA, MOVEQ, MOVEM, MOVEP, EXG, SWAP, EXT, CLR).

#![cfg(test)]

use crate::cpu::flags;
use crate::cpu::Cpu;
use crate::memory::{Memory, MemoryInterface};

fn create_cpu() -> (Cpu, Memory) {
    let mut memory = Memory::new(0x100000);
    let mut cpu = Cpu::new(&mut memory);
    cpu.pc = 0x1000;
    cpu.a[7] = 0x8000;
    cpu.sr |= flags::SUPERVISOR;
    (cpu, memory)
}

fn write_op(memory: &mut Memory, opcodes: &[u16]) {
    let mut addr = 0x1000u32;
    for &op in opcodes {
        memory.write_word(addr, op);
        addr += 2;
    }
}

// ============================================================================
// BTST Tests
// ============================================================================

#[test]
fn test_btst_register_bit_0() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x0300]); // BTST D1, D0 (bit number in D1, test D0)
    cpu.d[0] = 0x01; // Bit 0 set
    cpu.d[1] = 0; // Test bit 0
    cpu.step_instruction(&mut memory);
    assert!(!cpu.get_flag(flags::ZERO)); // Bit 0 is set
}

#[test]
fn test_btst_register_bit_clear() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x0300]); // BTST D1, D0
    cpu.d[0] = 0x02; // Bit 1 set, bit 0 clear
    cpu.d[1] = 0; // Test bit 0
    cpu.step_instruction(&mut memory);
    assert!(cpu.get_flag(flags::ZERO)); // Bit 0 is clear
}

#[test]
fn test_btst_immediate() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x0800, 0x0007]); // BTST #7, D0
    cpu.d[0] = 0x80; // Bit 7 set
    cpu.step_instruction(&mut memory);
    assert!(!cpu.get_flag(flags::ZERO));
}

#[test]
fn test_btst_all_32_bits() {
    for bit in 0..32u8 {
        let (mut cpu, mut memory) = create_cpu();
        write_op(&mut memory, &[0x0800, bit as u16]); // BTST #bit, D0
        cpu.d[0] = 1u32 << bit;
        cpu.pc = 0x1000;
        cpu.step_instruction(&mut memory);
        assert!(!cpu.get_flag(flags::ZERO), "Bit {} should be set", bit);
    }
}

#[test]
fn test_btst_memory_immediate() {
    let (mut cpu, mut memory) = create_cpu();
    // BTST #bit, (A0)
    // Opcode: 0000 100 0 00 010 000 (0x0810)
    write_op(&mut memory, &[0x0810, 0x0007]);
    cpu.a[0] = 0x2000;
    memory.write_byte(0x2000, 0x80); // Bit 7 set
    cpu.step_instruction(&mut memory);
    assert!(!cpu.get_flag(flags::ZERO));
}

#[test]
fn test_btst_memory_register() {
    let (mut cpu, mut memory) = create_cpu();
    // BTST D0, (A0)
    // Opcode: 0000 000 1 00 010 000 (0x0110)
    write_op(&mut memory, &[0x0110]);
    cpu.a[0] = 0x2000;
    memory.write_byte(0x2000, 0x01); // Bit 0 set
    cpu.d[0] = 0; // Test bit 0
    cpu.step_instruction(&mut memory);
    assert!(!cpu.get_flag(flags::ZERO));
}

#[test]
fn test_btst_memory_modulo_behavior() {
    let (mut cpu, mut memory) = create_cpu();
    // BTST D0, (A0)
    // Opcode: 0000 000 1 00 010 000 (0x0110)
    write_op(&mut memory, &[0x0110]);
    cpu.a[0] = 0x2000;
    memory.write_byte(0x2000, 0x01); // Bit 0 set

    // Test bit 8 (should be 8 % 8 = 0, which is set)
    cpu.d[0] = 8;
    cpu.step_instruction(&mut memory);
    assert!(!cpu.get_flag(flags::ZERO), "Bit 8 (mod 8 = 0) should test bit 0 which is set");

    // Reset PC for next instruction
    cpu.pc = 0x1000;
    // Test bit 1 (should be 0)
    cpu.d[0] = 1;
    cpu.step_instruction(&mut memory);
    assert!(cpu.get_flag(flags::ZERO), "Bit 1 should be clear");

    // Reset PC
    cpu.pc = 0x1000;
    // Test bit 9 (should be 9 % 8 = 1, which is clear)
    cpu.d[0] = 9;
    cpu.step_instruction(&mut memory);
    assert!(cpu.get_flag(flags::ZERO), "Bit 9 (mod 8 = 1) should test bit 1 which is clear");
}

// ============================================================================
// BSET Tests
// ============================================================================

#[test]
fn test_bset_register() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x01C0]); // BSET D0, D0 (uses D0 as bit number)
    cpu.d[0] = 4; // Set bit 4
    cpu.step_instruction(&mut memory);
    assert!(cpu.d[0] & 0x10 != 0); // Bit 4 is now set
}

#[test]
fn test_bset_immediate() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x08C0, 0x0003]); // BSET #3, D0
    cpu.d[0] = 0x00;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x08);
    assert!(cpu.get_flag(flags::ZERO)); // Was zero before
}

// ============================================================================
// BCLR Tests
// ============================================================================

#[test]
fn test_bclr_register() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x0380]); // BCLR D1, D0 (bit number in D1, clear in D0)
    cpu.d[0] = 0x10; // Bit 4 set
    cpu.d[1] = 4; // Clear bit 4
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0x10, 0); // Bit 4 cleared
    assert!(!cpu.get_flag(flags::ZERO)); // Was set before
}

#[test]
fn test_bclr_immediate() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x0880, 0x0003]); // BCLR #3, D0
    cpu.d[0] = 0xFF;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0xF7);
    assert!(!cpu.get_flag(flags::ZERO)); // Was set before
}

// ============================================================================
// BCHG Tests
// ============================================================================

#[test]
fn test_bchg_set_to_clear() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x0840, 0x0003]); // BCHG #3, D0
    cpu.d[0] = 0x08; // Bit 3 set
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x00);
    assert!(!cpu.get_flag(flags::ZERO)); // Was set before toggle
}

#[test]
fn test_bchg_clear_to_set() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x0840, 0x0003]); // BCHG #3, D0
    cpu.d[0] = 0x00;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x08);
    assert!(cpu.get_flag(flags::ZERO)); // Was clear before toggle
}

// ============================================================================
// MOVE Tests
// ============================================================================

#[test]
fn test_move_b_d0_d1() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x1200]); // MOVE.B D0, D1
    cpu.d[0] = 0x42;
    cpu.d[1] = 0xFF00FF00;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1], 0xFF00FF42); // Only low byte changed
}

#[test]
fn test_move_w_d0_d1() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x3200]); // MOVE.W D0, D1
    cpu.d[0] = 0x1234;
    cpu.d[1] = 0xFFFF0000;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1], 0xFFFF1234);
}

#[test]
fn test_move_l_d0_d1() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x2200]); // MOVE.L D0, D1
    cpu.d[0] = 0x12345678;
    cpu.d[1] = 0xFFFFFFFF;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1], 0x12345678);
}

#[test]
fn test_move_sets_flags() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x3200]); // MOVE.W D0, D1
    cpu.d[0] = 0x8000; // Negative
    cpu.step_instruction(&mut memory);
    assert!(cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::ZERO));
}

#[test]
fn test_move_zero_flag() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x3200]); // MOVE.W D0, D1
    cpu.d[0] = 0x0000;
    cpu.step_instruction(&mut memory);
    assert!(cpu.get_flag(flags::ZERO));
}

// ============================================================================
// MOVEA Tests
// ============================================================================

#[test]
fn test_movea_w() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x3040]); // MOVEA.W D0, A0
    cpu.d[0] = 0x8000; // Negative word
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.a[0], 0xFFFF8000); // Sign extended
}

#[test]
fn test_movea_l() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x2040]); // MOVEA.L D0, A0
    cpu.d[0] = 0x12345678;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.a[0], 0x12345678);
}

// ============================================================================
// MOVEQ Tests
// ============================================================================

#[test]
fn test_moveq_positive() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x7042]); // MOVEQ #$42, D0
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0x42);
}

#[test]
fn test_moveq_negative() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x70FF]); // MOVEQ #-1, D0
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0xFFFFFFFF); // Sign extended
    assert!(cpu.get_flag(flags::NEGATIVE));
}

#[test]
fn test_moveq_zero() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x7000]); // MOVEQ #0, D0
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0);
    assert!(cpu.get_flag(flags::ZERO));
}

// ============================================================================
// EXG Tests
// ============================================================================

#[test]
fn test_exg_data_data() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xC141]); // EXG D0, D1 (data-data, mode 01000)
    cpu.d[0] = 0x11111111;
    cpu.d[1] = 0x22222222;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0x22222222);
    assert_eq!(cpu.d[1], 0x11111111);
}

#[test]
fn test_exg_addr_addr() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xC149]); // EXG A0, A1 (addr-addr, mode 01001)
    cpu.a[0] = 0x11111111;
    cpu.a[1] = 0x22222222;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.a[0], 0x22222222);
    assert_eq!(cpu.a[1], 0x11111111);
}

#[test]
fn test_exg_data_addr() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xC188]); // EXG D0, A0 (data-addr, mode 10001)
    cpu.d[0] = 0x11111111;
    cpu.a[0] = 0x22222222;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0x22222222);
    assert_eq!(cpu.a[0], 0x11111111);
}

// ============================================================================
// SWAP Tests
// ============================================================================

#[test]
fn test_swap_d0() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4840]); // SWAP D0
    cpu.d[0] = 0x12345678;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0x56781234);
}

#[test]
fn test_swap_flags() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4840]); // SWAP D0
    cpu.d[0] = 0x00008000; // Result will be 0x80000000
    cpu.step_instruction(&mut memory);
    assert!(cpu.get_flag(flags::NEGATIVE));
}

// ============================================================================
// EXT Tests
// ============================================================================

#[test]
fn test_ext_w_positive() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4880]); // EXT.W D0
    cpu.d[0] = 0xFF7F; // Low byte is 0x7F (positive)
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFFFF, 0x007F);
}

#[test]
fn test_ext_w_negative() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4880]); // EXT.W D0
    cpu.d[0] = 0x0080; // Low byte is 0x80 (negative)
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFFFF, 0xFF80);
}

#[test]
fn test_ext_l_positive() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x48C0]); // EXT.L D0
    cpu.d[0] = 0x00007FFF;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0x00007FFF);
}

#[test]
fn test_ext_l_negative() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x48C0]); // EXT.L D0
    cpu.d[0] = 0x00008000;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0xFFFF8000);
}

// ============================================================================
// CLR Tests
// ============================================================================

#[test]
fn test_clr_b() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4200]); // CLR.B D0
    cpu.d[0] = 0xFFFFFFFF;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0xFFFFFF00);
    assert!(cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::NEGATIVE));
}

#[test]
fn test_clr_w() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4240]); // CLR.W D0
    cpu.d[0] = 0xFFFFFFFF;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0xFFFF0000);
}

#[test]
fn test_clr_l() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4280]); // CLR.L D0
    cpu.d[0] = 0xFFFFFFFF;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0x00000000);
}

// ============================================================================
// MOVEM Tests
// ============================================================================

#[test]
fn test_movem_to_memory() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVEM.L D0-D3, (A0)
    write_op(&mut memory, &[0x48D0, 0x000F]);
    cpu.a[0] = 0x2000;
    cpu.d[0] = 0x11111111;
    cpu.d[1] = 0x22222222;
    cpu.d[2] = 0x33333333;
    cpu.d[3] = 0x44444444;
    cpu.step_instruction(&mut memory);
    assert_eq!(memory.read_long(0x2000), 0x11111111);
    assert_eq!(memory.read_long(0x2004), 0x22222222);
    assert_eq!(memory.read_long(0x2008), 0x33333333);
    assert_eq!(memory.read_long(0x200C), 0x44444444);
}

#[test]
fn test_movem_from_memory() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVEM.L (A0), D0-D3
    write_op(&mut memory, &[0x4CD0, 0x000F]);
    cpu.a[0] = 0x2000;
    memory.write_long(0x2000, 0xAAAAAAAA);
    memory.write_long(0x2004, 0xBBBBBBBB);
    memory.write_long(0x2008, 0xCCCCCCCC);
    memory.write_long(0x200C, 0xDDDDDDDD);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0xAAAAAAAA);
    assert_eq!(cpu.d[1], 0xBBBBBBBB);
    assert_eq!(cpu.d[2], 0xCCCCCCCC);
    assert_eq!(cpu.d[3], 0xDDDDDDDD);
}

// ============================================================================
// LEA Tests
// ============================================================================

#[test]
fn test_lea_absolute() {
    let (mut cpu, mut memory) = create_cpu();
    // LEA $1234.W, A0
    write_op(&mut memory, &[0x41F8, 0x1234]);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.a[0], 0x1234);
}

#[test]
fn test_lea_displacement() {
    let (mut cpu, mut memory) = create_cpu();
    // LEA (d16, A1), A0
    write_op(&mut memory, &[0x41E9, 0x0100]);
    cpu.a[1] = 0x2000;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.a[0], 0x2100);
}

// ============================================================================
// PEA Tests
// ============================================================================

#[test]
fn test_pea() {
    let (mut cpu, mut memory) = create_cpu();
    // PEA $1234.W
    write_op(&mut memory, &[0x4878, 0x1234]);
    cpu.a[7] = 0x8000;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.a[7], 0x7FFC);
    assert_eq!(memory.read_long(0x7FFC), 0x1234);
}
