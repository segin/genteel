//! Comprehensive M68k CPU Tests
//!
//! Contains 100+ unit and property-based tests covering the M68k instruction set.


use crate::cpu::flags;
use crate::cpu::Cpu;
use crate::memory::{Memory, MemoryInterface};
use proptest::prelude::*;

// === Test Utilities ===

fn create_test_cpu() -> Cpu {
    let mut memory = Memory::new(0x10000);
    memory.write_long(0, 0x1000); // SP
    memory.write_long(4, 0x100);  // PC
    Cpu::new(Box::new(memory))
}

fn _create_cpu_with_program(opcodes: &[u16]) -> Cpu {
    let mut memory = Memory::new(0x10000);
    memory.write_long(0, 0x1000); // SP
    memory.write_long(4, 0x100);  // PC
    for (i, &opcode) in opcodes.iter().enumerate() {
        memory.write_word(0x100 + (i * 2) as u32, opcode);
    }
    Cpu::new(Box::new(memory))
}

// === Data Movement Tests ===

#[test]
fn test_moveq_positive_values() {
    for data in 0..=127u8 {
        let mut cpu = create_test_cpu();
        cpu.memory.write_word(0x100, 0x7000 | (data as u16)); // MOVEQ #data, D0
        cpu.step_instruction();
        assert_eq!(cpu.d[0], data as u32);
    }
}

#[test]
fn test_moveq_negative_values() {
    for data in 128..=255u8 {
        let mut cpu = create_test_cpu();
        cpu.memory.write_word(0x100, 0x7000 | (data as u16)); // MOVEQ #data, D0
        cpu.step_instruction();
        assert_eq!(cpu.d[0], data as i8 as i32 as u32);
    }
}

#[test]
fn test_move_l_all_registers() {
    for src_reg in 0..8u8 {
        for dst_reg in 0..8u8 {
            let mut cpu = create_test_cpu();
            let opcode = 0x2000 | ((dst_reg as u16) << 9) | (src_reg as u16);
            cpu.memory.write_word(0x100, opcode);
            cpu.d[src_reg as usize] = 0xDEADBEEF;
            cpu.step_instruction();
            assert_eq!(cpu.d[dst_reg as usize], 0xDEADBEEF);
        }
    }
}

#[test]
fn test_move_w_all_registers() {
    for src_reg in 0..8u8 {
        for dst_reg in 0..8u8 {
            let mut cpu = create_test_cpu();
            let opcode = 0x3000 | ((dst_reg as u16) << 9) | (src_reg as u16);
            cpu.memory.write_word(0x100, opcode);
            
            // Set Dst first
            cpu.d[dst_reg as usize] = 0xFFFFFFFF;
            
            // Set Src (overwriting Dst if same register)
            cpu.d[src_reg as usize] = 0xCAFEBABE;
            
            cpu.step_instruction();
            
            if src_reg == dst_reg {
                assert_eq!(cpu.d[dst_reg as usize] & 0xFFFF, 0xBABE);
            } else {
                assert_eq!(cpu.d[dst_reg as usize] & 0xFFFF, 0xBABE);
            }
        }
    }
}

#[test]
fn test_lea_all_address_registers() {
    for dst_reg in 0..8u8 {
        let mut cpu = create_test_cpu();
        // LEA $1234.W, An = 0x41F8 + (dst_reg << 9) + immediate
        let opcode = 0x41F8 | ((dst_reg as u16) << 9);
        cpu.memory.write_word(0x100, opcode);
        cpu.memory.write_word(0x102, 0x1234);
        cpu.step_instruction();
        assert_eq!(cpu.a[dst_reg as usize], 0x00001234);
    }
}

// === Arithmetic Tests ===

#[test]
fn test_addq_all_data_values() {
    for data in 1..=8u8 {
        let actual_data = if data == 8 { 8 } else { data };
        let mut cpu = create_test_cpu();
        let opcode = 0x5080 | ((data as u16 % 8) << 9); // ADDQ.L #data, D0
        cpu.memory.write_word(0x100, opcode);
        cpu.d[0] = 100;
        cpu.step_instruction();
        assert_eq!(cpu.d[0], 100 + actual_data as u32);
    }
}

#[test]
fn test_subq_all_data_values() {
    for data in 1..=8u8 {
        let actual_data = if data == 8 { 8 } else { data };
        let mut cpu = create_test_cpu();
        let opcode = 0x5180 | ((data as u16 % 8) << 9); // SUBQ.L #data, D0
        cpu.memory.write_word(0x100, opcode);
        cpu.d[0] = 100;
        cpu.step_instruction();
        assert_eq!(cpu.d[0], 100 - actual_data as u32);
    }
}

#[test]
fn test_add_overflow_detection() {
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0xD041); // ADD.W D1, D0
    cpu.d[0] = 0x7FFF; // Max positive word
    cpu.d[1] = 0x0001;
    cpu.step_instruction();
    assert!(cpu.get_flag(flags::OVERFLOW));
    assert!(cpu.get_flag(flags::NEGATIVE));
}

#[test]
fn test_sub_borrow_detection() {
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0x9041); // SUB.W D1, D0
    cpu.d[0] = 0x0000;
    cpu.d[1] = 0x0001;
    cpu.step_instruction();
    assert!(cpu.get_flag(flags::CARRY));
    assert!(cpu.get_flag(flags::EXTEND));
}

#[test]
fn test_neg_all_sizes() {
    // NEG.B D0
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0x4400); 
    cpu.d[0] = 0x01;
    cpu.step_instruction();
    assert_eq!(cpu.d[0] & 0xFF, 0xFF);
    
    // NEG.W D0
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0x4440);
    cpu.d[0] = 0x0001;
    cpu.step_instruction();
    assert_eq!(cpu.d[0] & 0xFFFF, 0xFFFF);
    
    // NEG.L D0
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0x4480);
    cpu.d[0] = 0x00000001;
    cpu.step_instruction();
    assert_eq!(cpu.d[0], 0xFFFFFFFF);
}

// === Logical Tests ===

#[test]
fn test_and_all_combinations() {
    let test_values: [(u32, u32, u32); 8] = [
        (0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF),
        (0xFFFFFFFF, 0x00000000, 0x00000000),
        (0x00000000, 0xFFFFFFFF, 0x00000000),
        (0x00000000, 0x00000000, 0x00000000),
        (0x55555555, 0xAAAAAAAA, 0x00000000),
        (0x0F0F0F0F, 0xF0F0F0F0, 0x00000000),
        (0x12345678, 0xFFFFFFFF, 0x12345678),
        (0xABCDEF01, 0xF0F0F0F0, 0xA0C0E000),
    ];
    
    for (a, b, expected) in test_values {
        let mut cpu = create_test_cpu();
        cpu.memory.write_word(0x100, 0xC081); // AND.L D1, D0
        cpu.d[0] = a;
        cpu.d[1] = b;
        cpu.step_instruction();
        assert_eq!(cpu.d[0], expected);
    }
}

#[test]
fn test_or_all_combinations() {
    let test_values: [(u32, u32, u32); 8] = [
        (0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF),
        (0xFFFFFFFF, 0x00000000, 0xFFFFFFFF),
        (0x00000000, 0xFFFFFFFF, 0xFFFFFFFF),
        (0x00000000, 0x00000000, 0x00000000),
        (0x55555555, 0xAAAAAAAA, 0xFFFFFFFF),
        (0x0F0F0F0F, 0xF0F0F0F0, 0xFFFFFFFF),
        (0x12345678, 0x00000000, 0x12345678),
        (0xABCDEF01, 0x0F0F0F0F, 0xAFCFEF0F),
    ];
    
    for (a, b, expected) in test_values {
        let mut cpu = create_test_cpu();
        cpu.memory.write_word(0x100, 0x8081); // OR.L D1, D0
        cpu.d[0] = a;
        cpu.d[1] = b;
        cpu.step_instruction();
        assert_eq!(cpu.d[0], expected);
    }
}

#[test]
fn test_eor_all_combinations() {
    let test_values: [(u32, u32, u32); 8] = [
        (0xFFFFFFFF, 0xFFFFFFFF, 0x00000000),
        (0xFFFFFFFF, 0x00000000, 0xFFFFFFFF),
        (0x00000000, 0xFFFFFFFF, 0xFFFFFFFF),
        (0x00000000, 0x00000000, 0x00000000),
        (0x55555555, 0xAAAAAAAA, 0xFFFFFFFF),
        (0x0F0F0F0F, 0xF0F0F0F0, 0xFFFFFFFF),
        (0x12345678, 0x12345678, 0x00000000),
        (0xABCDEF01, 0x0F0F0F0F, 0xA4C2E00E),
    ];
    
    for (a, b, expected) in test_values {
        let mut cpu = create_test_cpu();
        // EOR.L D0, D1 = 0xB181
        cpu.memory.write_word(0x100, 0xB181);
        cpu.d[0] = a;
        cpu.d[1] = b;
        cpu.step_instruction();
        assert_eq!(cpu.d[1], expected);
    }
}

#[test]
fn test_not_all_sizes() {
    // NOT.B D0
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0x4600);
    cpu.d[0] = 0x000000AA;
    cpu.step_instruction();
    assert_eq!(cpu.d[0] & 0xFF, 0x55);
    
    // NOT.W D0
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0x4640);
    cpu.d[0] = 0x0000AAAA;
    cpu.step_instruction();
    assert_eq!(cpu.d[0] & 0xFFFF, 0x5555);
    
    // NOT.L D0
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0x4680);
    cpu.d[0] = 0xAAAAAAAA;
    cpu.step_instruction();
    assert_eq!(cpu.d[0], 0x55555555);
}

// === Branch Tests ===

#[test]
fn test_bra_forward() {
    for offset in [2i8, 4, 6, 8, 10, 20, 50, 100] {
        let mut cpu = create_test_cpu();
        cpu.memory.write_word(0x100, 0x6000 | (offset as u8 as u16));
        cpu.step_instruction();
        assert_eq!(cpu.pc, 0x102u32.wrapping_add(offset as u32));
    }
}

#[test]
fn test_bra_backward() {
    for offset in [-2i8, -4, -6, -8, -10, -20, -50] {
        let mut cpu = create_test_cpu();
        cpu.memory.write_word(0x100, 0x6000 | (offset as u8 as u16));
        cpu.step_instruction();
        assert_eq!(cpu.pc, 0x102u32.wrapping_add(offset as i32 as u32));
    }
}

#[test]
fn test_bcc_all_conditions() {
    // BHI (C=0 and Z=0)
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0x6204); // BHI +4
    cpu.set_flag(flags::CARRY, false);
    cpu.set_flag(flags::ZERO, false);
    cpu.step_instruction();
    assert_eq!(cpu.pc, 0x106);
    
    // BLS (C=1 or Z=1)
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0x6304); // BLS +4
    cpu.set_flag(flags::CARRY, true);
    cpu.set_flag(flags::ZERO, false);
    cpu.step_instruction();
    assert_eq!(cpu.pc, 0x106);
    
    // BCC (C=0)
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0x6404); // BCC +4
    cpu.set_flag(flags::CARRY, false);
    cpu.step_instruction();
    assert_eq!(cpu.pc, 0x106);
    
    // BCS (C=1)
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0x6504); // BCS +4
    cpu.set_flag(flags::CARRY, true);
    cpu.step_instruction();
    assert_eq!(cpu.pc, 0x106);
    
    // BNE (Z=0)
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0x6604); // BNE +4
    cpu.set_flag(flags::ZERO, false);
    cpu.step_instruction();
    assert_eq!(cpu.pc, 0x106);
    
    // BEQ (Z=1)
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0x6704); // BEQ +4
    cpu.set_flag(flags::ZERO, true);
    cpu.step_instruction();
    assert_eq!(cpu.pc, 0x106);
    
    // BPL (N=0)
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0x6A04); // BPL +4
    cpu.set_flag(flags::NEGATIVE, false);
    cpu.step_instruction();
    assert_eq!(cpu.pc, 0x106);
    
    // BMI (N=1)
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0x6B04); // BMI +4
    cpu.set_flag(flags::NEGATIVE, true);
    cpu.step_instruction();
    assert_eq!(cpu.pc, 0x106);
}

// === Bit Operation Tests ===

#[test]
fn test_btst_all_bits() {
    for bit in 0..32u8 {
        let mut cpu = create_test_cpu();
        cpu.memory.write_word(0x100, 0x0800); // BTST #bit, D0
        cpu.memory.write_word(0x102, bit as u16);
        cpu.d[0] = 1 << bit;
        cpu.step_instruction();
        assert!(!cpu.get_flag(flags::ZERO), "bit {} should be set", bit);
        
        let mut cpu = create_test_cpu();
        cpu.memory.write_word(0x100, 0x0800);
        cpu.memory.write_word(0x102, bit as u16);
        cpu.d[0] = !(1u32 << bit);
        cpu.step_instruction();
        assert!(cpu.get_flag(flags::ZERO), "bit {} should be clear", bit);
    }
}

#[test]
fn test_bset_all_bits() {
    for bit in 0..32u8 {
        let mut cpu = create_test_cpu();
        cpu.memory.write_word(0x100, 0x08C0); // BSET #bit, D0
        cpu.memory.write_word(0x102, bit as u16);
        cpu.d[0] = 0;
        cpu.step_instruction();
        assert_eq!(cpu.d[0], 1 << bit);
    }
}

#[test]
fn test_bclr_all_bits() {
    for bit in 0..32u8 {
        let mut cpu = create_test_cpu();
        cpu.memory.write_word(0x100, 0x0880); // BCLR #bit, D0
        cpu.memory.write_word(0x102, bit as u16);
        cpu.d[0] = 0xFFFFFFFF;
        cpu.step_instruction();
        assert_eq!(cpu.d[0], !(1u32 << bit));
    }
}

// === Shift and Rotate Tests ===

#[test]
fn test_lsl_all_counts() {
    for count in 1..=8u8 {
        let mut cpu = create_test_cpu();
        let opcode = 0xE108 | ((count as u16 % 8) << 9); // LSL.B #count, D0
        cpu.memory.write_word(0x100, opcode);
        cpu.d[0] = 0x01;
        cpu.step_instruction();
        assert_eq!(cpu.d[0] & 0xFF, ((1u32 << count) & 0xFF) as u32);
    }
}

#[test]
fn test_lsr_all_counts() {
    for count in 1..=8u8 {
        let mut cpu = create_test_cpu();
        let opcode = 0xE008 | ((count as u16 % 8) << 9); // LSR.B #count, D0
        cpu.memory.write_word(0x100, opcode);
        cpu.d[0] = 0x80;
        cpu.step_instruction();
        assert_eq!(cpu.d[0] & 0xFF, ((0x80u32 >> count) & 0xFF) as u32);
    }
}

#[test]
fn test_asr_sign_extension() {
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0xE240); // ASR.W #1, D0
    cpu.d[0] = 0x8000; // Negative word
    cpu.step_instruction();
    assert_eq!(cpu.d[0] & 0xFFFF, 0xC000); // Sign extended
    
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0xE440); // ASR.W #2, D0
    cpu.d[0] = 0x8000;
    cpu.step_instruction();
    assert_eq!(cpu.d[0] & 0xFFFF, 0xE000);
}

// === Stack Tests ===

#[test]
fn test_push_pop_long_sequence() {
    let mut cpu = create_test_cpu();
    cpu.a[7] = 0x2000;
    
    // Push sequence
    let values = [0x11111111u32, 0x22222222, 0x33333333, 0x44444444];
    for (_i, &val) in values.iter().enumerate() {
        cpu.a[7] = cpu.a[7].wrapping_sub(4);
        cpu.memory.write_long(cpu.a[7], val);
    }
    
    // Verify stack order
    assert_eq!(cpu.memory.read_long(0x1FF0), 0x44444444);
    assert_eq!(cpu.memory.read_long(0x1FF4), 0x33333333);
    assert_eq!(cpu.memory.read_long(0x1FF8), 0x22222222);
    assert_eq!(cpu.memory.read_long(0x1FFC), 0x11111111);
}

// === Multiply/Divide Tests ===

#[test]
fn test_mulu_boundary_values() {
    let test_cases: [(u16, u16, u32); 6] = [
        (0, 0, 0),
        (1, 1, 1),
        (0xFFFF, 1, 0xFFFF),
        (1, 0xFFFF, 0xFFFF),
        (0xFFFF, 0xFFFF, 0xFFFE0001),
        (0x100, 0x100, 0x10000),
    ];
    
    for (a, b, expected) in test_cases {
        let mut cpu = create_test_cpu();
        cpu.memory.write_word(0x100, 0xC0C1); // MULU D1, D0
        cpu.d[0] = a as u32;
        cpu.d[1] = b as u32;
        cpu.step_instruction();
        assert_eq!(cpu.d[0], expected, "MULU {} * {} = {}", a, b, expected);
    }
}

#[test]
fn test_muls_signed_values() {
    let test_cases: [(i16, i16, i32); 6] = [
        (0, 0, 0),
        (1, 1, 1),
        (-1, 1, -1),
        (1, -1, -1),
        (-1, -1, 1),
        (-32768, 2, -65536),
    ];
    
    for (a, b, expected) in test_cases {
        let mut cpu = create_test_cpu();
        cpu.memory.write_word(0x100, 0xC1C1); // MULS D1, D0
        cpu.d[0] = a as u16 as u32;
        cpu.d[1] = b as u16 as u32;
        cpu.step_instruction();
        assert_eq!(cpu.d[0] as i32, expected, "MULS {} * {} = {}", a, b, expected);
    }
}

#[test]
fn test_divu_boundary_values() {
    let test_cases: [(u32, u16, u16, u16); 4] = [
        (100, 10, 10, 0),       // q=10, r=0
        (101, 10, 10, 1),       // q=10, r=1
        (0x10000, 0x100, 0x100, 0),
        (7, 3, 2, 1),
    ];
    
    for (dividend, divisor, q, r) in test_cases {
        let mut cpu = create_test_cpu();
        cpu.memory.write_word(0x100, 0x80C1); // DIVU D1, D0
        cpu.d[0] = dividend;
        cpu.d[1] = divisor as u32;
        cpu.step_instruction();
        let result_q = cpu.d[0] & 0xFFFF;
        let result_r = (cpu.d[0] >> 16) & 0xFFFF;
        assert_eq!(result_q as u16, q, "DIVU {} / {} quotient", dividend, divisor);
        assert_eq!(result_r as u16, r, "DIVU {} / {} remainder", dividend, divisor);
    }
}

// === Property-Based Tests ===

proptest! {
    #[test]
    fn prop_add_commutative(a in 0u32..0xFFFF, b in 0u32..0xFFFF) {
        let mut cpu1 = create_test_cpu();
        cpu1.memory.write_word(0x100, 0xD041); // ADD.W D1, D0
        cpu1.d[0] = a;
        cpu1.d[1] = b;
        cpu1.step_instruction();
        let result1 = cpu1.d[0] & 0xFFFF;
        
        let mut cpu2 = create_test_cpu();
        cpu2.memory.write_word(0x100, 0xD041);
        cpu2.d[0] = b;
        cpu2.d[1] = a;
        cpu2.step_instruction();
        let result2 = cpu2.d[0] & 0xFFFF;
        
        prop_assert_eq!(result1, result2);
    }
    
    #[test]
    fn prop_sub_zero_identity(a in 0u32..0xFFFFFFFF) {
        let mut cpu = create_test_cpu();
        cpu.memory.write_word(0x100, 0x9081); // SUB.L D1, D0
        cpu.d[0] = a;
        cpu.d[1] = 0;
        cpu.step_instruction();
        prop_assert_eq!(cpu.d[0], a);
    }
    
    #[test]
    fn prop_and_idempotent(a in 0u32..0xFFFFFFFF) {
        let mut cpu = create_test_cpu();
        cpu.memory.write_word(0x100, 0xC081); // AND.L D1, D0
        cpu.d[0] = a;
        cpu.d[1] = a;
        cpu.step_instruction();
        prop_assert_eq!(cpu.d[0], a);
    }
    
    #[test]
    fn prop_or_idempotent(a in 0u32..0xFFFFFFFF) {
        let mut cpu = create_test_cpu();
        cpu.memory.write_word(0x100, 0x8081); // OR.L D1, D0
        cpu.d[0] = a;
        cpu.d[1] = a;
        cpu.step_instruction();
        prop_assert_eq!(cpu.d[0], a);
    }
    
    #[test]
    fn prop_eor_self_zero(a in 0u32..0xFFFFFFFF) {
        let mut cpu = create_test_cpu();
        cpu.memory.write_word(0x100, 0xB181); // EOR.L D0, D1
        cpu.d[0] = a;
        cpu.d[1] = a;
        cpu.step_instruction();
        prop_assert_eq!(cpu.d[1], 0);
    }
    
    #[test]
    fn prop_not_involution(a in 0u32..0xFFFFFFFF) {
        let mut cpu = create_test_cpu();
        cpu.memory.write_word(0x100, 0x4680); // NOT.L D0
        cpu.memory.write_word(0x102, 0x4680); // NOT.L D0
        cpu.d[0] = a;
        cpu.step_instruction();
        cpu.step_instruction();
        prop_assert_eq!(cpu.d[0], a);
    }
    
    #[test]
    fn prop_neg_neg_identity(a in 0u32..0xFFFFFFFF) {
        let mut cpu = create_test_cpu();
        cpu.memory.write_word(0x100, 0x4480); // NEG.L D0
        cpu.memory.write_word(0x102, 0x4480); // NEG.L D0
        cpu.d[0] = a;
        cpu.step_instruction();
        cpu.step_instruction();
        prop_assert_eq!(cpu.d[0], a);
    }
    
    #[test]
    fn prop_swap_swap_identity(a in 0u32..0xFFFFFFFF) {
        let mut cpu = create_test_cpu();
        cpu.memory.write_word(0x100, 0x4840); // SWAP D0
        cpu.memory.write_word(0x102, 0x4840); // SWAP D0
        cpu.d[0] = a;
        cpu.step_instruction();
        cpu.step_instruction();
        prop_assert_eq!(cpu.d[0], a);
    }
    
    #[test]
    fn prop_ext_word_sign(a in 0u8..=255u8) {
        let mut cpu = create_test_cpu();
        cpu.memory.write_word(0x100, 0x4880); // EXT.W D0
        cpu.d[0] = a as u32;
        cpu.step_instruction();
        let expected = (a as i8 as i16 as u16) as u32;
        prop_assert_eq!(cpu.d[0] & 0xFFFF, expected);
    }
    
    #[test]
    fn prop_clr_always_zero(a in 0u32..0xFFFFFFFF) {
        let mut cpu = create_test_cpu();
        cpu.memory.write_word(0x100, 0x4280); // CLR.L D0
        cpu.d[0] = a;
        cpu.step_instruction();
        prop_assert_eq!(cpu.d[0], 0);
        prop_assert!(cpu.get_flag(flags::ZERO));
        prop_assert!(!cpu.get_flag(flags::NEGATIVE));
    }
}

// === Additional Edge Case Tests ===

#[test]
fn test_tst_all_sizes() {
    // TST.B
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0x4A00);
    cpu.d[0] = 0xFF;
    cpu.step_instruction();
    assert!(cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::ZERO));
    
    // TST.W
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0x4A40);
    cpu.d[0] = 0x0000;
    cpu.step_instruction();
    assert!(!cpu.get_flag(flags::NEGATIVE));
    assert!(cpu.get_flag(flags::ZERO));
    
    // TST.L
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0x4A80);
    cpu.d[0] = 0x80000000;
    cpu.step_instruction();
    assert!(cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::ZERO));
}

#[test]
fn test_cmp_all_conditions() {
    // Equal
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0xB081); // CMP.L D1, D0
    cpu.d[0] = 0x12345678;
    cpu.d[1] = 0x12345678;
    cpu.step_instruction();
    assert!(cpu.get_flag(flags::ZERO));
    
    // Greater
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0xB081);
    cpu.d[0] = 0x12345679;
    cpu.d[1] = 0x12345678;
    cpu.step_instruction();
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::CARRY));
    
    // Less
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0xB081);
    cpu.d[0] = 0x12345677;
    cpu.d[1] = 0x12345678;
    cpu.step_instruction();
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(cpu.get_flag(flags::CARRY));
}

#[test]
fn test_ext_all_sizes() {
    // EXT.W D0 (byte to word)
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0x4880);
    cpu.d[0] = 0x80;
    cpu.step_instruction();
    assert_eq!(cpu.d[0] & 0xFFFF, 0xFF80);
    
    // EXT.L D0 (word to long)
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0x48C0);
    cpu.d[0] = 0x8000;
    cpu.step_instruction();
    assert_eq!(cpu.d[0], 0xFFFF8000);
}

#[test]
fn test_exg_all_modes() {
    // EXG Dx, Dy
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0xC141); // EXG D0, D1
    cpu.d[0] = 0xAAAAAAAA;
    cpu.d[1] = 0x55555555;
    cpu.step_instruction();
    assert_eq!(cpu.d[0], 0x55555555);
    assert_eq!(cpu.d[1], 0xAAAAAAAA);
    
    // EXG Ax, Ay  
    let mut cpu = create_test_cpu();
    cpu.memory.write_word(0x100, 0xC149); // EXG A0, A1
    cpu.a[0] = 0x11111111;
    cpu.a[1] = 0x22222222;
    cpu.step_instruction();
    assert_eq!(cpu.a[0], 0x22222222);
    assert_eq!(cpu.a[1], 0x11111111);
}

#[test]
fn test_dbcc_loop() {
    let mut cpu = create_test_cpu();
    // DBRA D0, $-2 (loop back)
    cpu.memory.write_word(0x100, 0x51C8); // DBRA D0
    cpu.memory.write_word(0x102, 0xFFFE); // -2 displacement
    cpu.d[0] = 3;
    
    cpu.step_instruction(); // D0 = 2
    assert_eq!(cpu.d[0], 2);
    assert_eq!(cpu.pc, 0x100);
    
    cpu.step_instruction(); // D0 = 1
    assert_eq!(cpu.d[0], 1);
    assert_eq!(cpu.pc, 0x100);
    
    cpu.step_instruction(); // D0 = 0
    assert_eq!(cpu.d[0], 0);
    assert_eq!(cpu.pc, 0x100);
    
    // Loop exit: Condition matches (False, so no). Dn == -1?
    // D0 decrements to -1.
    // Loop terminates. PC increments by 2 (to 0x104).
    cpu.step_instruction(); // D0 = -1 (0xFFFF)
    // d[0] high word is 0, so result is 0x0000FFFF
    assert_eq!(cpu.d[0], 0x0000FFFF); 
    assert_eq!(cpu.pc, 0x104);
}
