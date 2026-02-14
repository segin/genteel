//! Property-based tests for Z80 CPU using proptest

use super::*;
use crate::memory::Memory;
use proptest::prelude::*;

fn create_z80_with_program(program: &[u8]) -> Z80 {
    let mut memory = Memory::new(0x10000);
    for (i, &byte) in program.iter().enumerate() {
        memory.data[i] = byte;
    }
    Z80::new(
        Box::new(memory),
        Box::new(crate::z80::test_utils::TestIo::default()),
    )
}

proptest! {
    // ==================== Register Pair Invariants ====================

    #[test]
    fn prop_bc_roundtrip(val in 0u16..=0xFFFF) {
        let mut z80 = create_z80_with_program(&[]);
        z80.set_bc(val);
        prop_assert_eq!(z80.bc(), val);
        prop_assert_eq!(z80.b, (val >> 8) as u8);
        prop_assert_eq!(z80.c, val as u8);
    }

    #[test]
    fn prop_de_roundtrip(val in 0u16..=0xFFFF) {
        let mut z80 = create_z80_with_program(&[]);
        z80.set_de(val);
        prop_assert_eq!(z80.de(), val);
    }

    #[test]
    fn prop_hl_roundtrip(val in 0u16..=0xFFFF) {
        let mut z80 = create_z80_with_program(&[]);
        z80.set_hl(val);
        prop_assert_eq!(z80.hl(), val);
    }

    #[test]
    fn prop_af_roundtrip(val in 0u16..=0xFFFF) {
        let mut z80 = create_z80_with_program(&[]);
        z80.set_af(val);
        prop_assert_eq!(z80.af(), val);
    }

    // ==================== LD rr, nn Properties ====================

    #[test]
    fn prop_ld_bc_nn(low in 0u8..=255, high in 0u8..=255) {
        let mut z80 = create_z80_with_program(&[0x01, low, high]);
        z80.step();
        let expected = ((high as u16) << 8) | (low as u16);
        prop_assert_eq!(z80.bc(), expected);
        prop_assert_eq!(z80.pc, 3);
    }

    #[test]
    fn prop_ld_de_nn(low in 0u8..=255, high in 0u8..=255) {
        let mut z80 = create_z80_with_program(&[0x11, low, high]);
        z80.step();
        let expected = ((high as u16) << 8) | (low as u16);
        prop_assert_eq!(z80.de(), expected);
    }

    #[test]
    fn prop_ld_hl_nn(low in 0u8..=255, high in 0u8..=255) {
        let mut z80 = create_z80_with_program(&[0x21, low, high]);
        z80.step();
        let expected = ((high as u16) << 8) | (low as u16);
        prop_assert_eq!(z80.hl(), expected);
    }

    #[test]
    fn prop_ld_sp_nn(low in 0u8..=255, high in 0u8..=255) {
        let mut z80 = create_z80_with_program(&[0x31, low, high]);
        z80.step();
        let expected = ((high as u16) << 8) | (low as u16);
        prop_assert_eq!(z80.sp, expected);
    }

    // ==================== INC/DEC Properties ====================

    #[test]
    fn prop_inc_bc_adds_one(val in 0u16..=0xFFFF) {
        let mut z80 = create_z80_with_program(&[0x03]);
        z80.set_bc(val);
        z80.step();
        prop_assert_eq!(z80.bc(), val.wrapping_add(1));
    }

    #[test]
    fn prop_dec_bc_subtracts_one(val in 0u16..=0xFFFF) {
        let mut z80 = create_z80_with_program(&[0x0B]);
        z80.set_bc(val);
        z80.step();
        prop_assert_eq!(z80.bc(), val.wrapping_sub(1));
    }

    #[test]
    fn prop_inc_de_adds_one(val in 0u16..=0xFFFF) {
        let mut z80 = create_z80_with_program(&[0x13]);
        z80.set_de(val);
        z80.step();
        prop_assert_eq!(z80.de(), val.wrapping_add(1));
    }

    #[test]
    fn prop_inc_hl_adds_one(val in 0u16..=0xFFFF) {
        let mut z80 = create_z80_with_program(&[0x23]);
        z80.set_hl(val);
        z80.step();
        prop_assert_eq!(z80.hl(), val.wrapping_add(1));
    }

    #[test]
    fn prop_inc_sp_adds_one(val in 0u16..=0xFFFF) {
        let mut z80 = create_z80_with_program(&[0x33]);
        z80.sp = val;
        z80.step();
        prop_assert_eq!(z80.sp, val.wrapping_add(1));
    }

    // ==================== 8-bit INC/DEC Properties ====================

    #[test]
    fn prop_inc_a(val in 0u8..=255) {
        let mut z80 = create_z80_with_program(&[0x3C]);
        z80.a = val;
        z80.step();
        prop_assert_eq!(z80.a, val.wrapping_add(1));
        // Check zero flag
        prop_assert_eq!(z80.get_flag(flags::ZERO), z80.a == 0);
    }

    #[test]
    fn prop_dec_a(val in 0u8..=255) {
        let mut z80 = create_z80_with_program(&[0x3D]);
        z80.a = val;
        z80.step();
        prop_assert_eq!(z80.a, val.wrapping_sub(1));
    }

    #[test]
    fn prop_inc_b(val in 0u8..=255) {
        let mut z80 = create_z80_with_program(&[0x04]);
        z80.b = val;
        z80.step();
        prop_assert_eq!(z80.b, val.wrapping_add(1));
    }

    // ==================== ALU Properties ====================

    #[test]
    fn prop_add_a_b(a in 0u8..=255, b in 0u8..=255) {
        let mut z80 = create_z80_with_program(&[0x80]);
        z80.a = a;
        z80.b = b;
        z80.step();
        prop_assert_eq!(z80.a, a.wrapping_add(b));
        // Carry if overflow
        prop_assert_eq!(z80.get_flag(flags::CARRY), (a as u16 + b as u16) > 0xFF);
    }

    #[test]
    fn prop_sub_a_b(a in 0u8..=255, b in 0u8..=255) {
        let mut z80 = create_z80_with_program(&[0x90]);
        z80.a = a;
        z80.b = b;
        z80.step();
        prop_assert_eq!(z80.a, a.wrapping_sub(b));
        prop_assert!(z80.get_flag(flags::ADD_SUB)); // N flag set for subtraction
    }

    #[test]
    fn prop_and_a_b(a in 0u8..=255, b in 0u8..=255) {
        let mut z80 = create_z80_with_program(&[0xA0]);
        z80.a = a;
        z80.b = b;
        z80.step();
        prop_assert_eq!(z80.a, a & b);
        prop_assert!(!z80.get_flag(flags::CARRY));
        prop_assert!(z80.get_flag(flags::HALF_CARRY));
    }

    #[test]
    fn prop_or_a_b(a in 0u8..=255, b in 0u8..=255) {
        let mut z80 = create_z80_with_program(&[0xB0]);
        z80.a = a;
        z80.b = b;
        z80.step();
        prop_assert_eq!(z80.a, a | b);
    }

    #[test]
    fn prop_xor_a_b(a in 0u8..=255, b in 0u8..=255) {
        let mut z80 = create_z80_with_program(&[0xA8]);
        z80.a = a;
        z80.b = b;
        z80.step();
        prop_assert_eq!(z80.a, a ^ b);
    }

    #[test]
    fn prop_cp_does_not_modify_a(a in 0u8..=255, b in 0u8..=255) {
        let mut z80 = create_z80_with_program(&[0xB8]);
        z80.a = a;
        z80.b = b;
        z80.step();
        prop_assert_eq!(z80.a, a);
    }

    // ==================== NOP Properties ====================

    #[test]
    fn prop_nop_only_advances_pc(a in 0u8..=255, b in 0u8..=255, c in 0u8..=255) {
        let mut z80 = create_z80_with_program(&[0x00]);
        z80.a = a;
        z80.b = b;
        z80.c = c;
        let old_sp = z80.sp;
        z80.step();
        prop_assert_eq!(z80.pc, 1);
        prop_assert_eq!(z80.a, a);
        prop_assert_eq!(z80.b, b);
        prop_assert_eq!(z80.c, c);
        prop_assert_eq!(z80.sp, old_sp);
    }

    // ==================== LD r, r' Properties ====================

    #[test]
    fn prop_ld_b_c_copies(val in 0u8..=255) {
        let mut z80 = create_z80_with_program(&[0x41]);
        z80.c = val;
        z80.step();
        prop_assert_eq!(z80.b, val);
    }

    #[test]
    fn prop_ld_a_b_copies(val in 0u8..=255) {
        let mut z80 = create_z80_with_program(&[0x78]);
        z80.b = val;
        z80.step();
        prop_assert_eq!(z80.a, val);
    }

    // ==================== Rotate Properties ====================

    #[test]
    fn prop_rlca_carry_is_bit_7(val in 0u8..=255) {
        let mut z80 = create_z80_with_program(&[0x07]);
        z80.a = val;
        z80.step();
        prop_assert_eq!(z80.get_flag(flags::CARRY), (val & 0x80) != 0);
    }

    #[test]
    fn prop_rrca_carry_is_bit_0(val in 0u8..=255) {
        let mut z80 = create_z80_with_program(&[0x0F]);
        z80.a = val;
        z80.step();
        prop_assert_eq!(z80.get_flag(flags::CARRY), (val & 0x01) != 0);
    }

    // ==================== Push/Pop Invariants ====================

    #[test]
    fn prop_push_pop_bc_roundtrip(val in 0u16..=0xFFFF) {
        let mut z80 = create_z80_with_program(&[0xC5, 0xC1]); // PUSH BC, POP BC
        z80.sp = 0x8000;
        z80.set_bc(val);
        z80.step(); // PUSH
        z80.set_bc(0x0000);
        z80.step(); // POP
        prop_assert_eq!(z80.bc(), val);
        prop_assert_eq!(z80.sp, 0x8000);
    }

    // ==================== CB Prefix Properties ====================

    #[test]
    fn prop_cb_set_bit(bit in 0u8..8, val in 0u8..=255) {
        let opcode = 0xC0 | (bit << 3); // SET bit, A
        let mut z80 = create_z80_with_program(&[0xCB, opcode | 0x07]);
        z80.a = val;
        z80.step();
        prop_assert_eq!(z80.a, val | (1 << bit));
    }

    #[test]
    fn prop_cb_res_bit(bit in 0u8..8, val in 0u8..=255) {
        let opcode = 0x80 | (bit << 3); // RES bit, A
        let mut z80 = create_z80_with_program(&[0xCB, opcode | 0x07]);
        z80.a = val;
        z80.step();
        prop_assert_eq!(z80.a, val & !(1 << bit));
    }

    #[test]
    fn prop_cb_bit_test(bit in 0u8..8, val in 0u8..=255) {
        let opcode = 0x40 | (bit << 3); // BIT bit, A
        let mut z80 = create_z80_with_program(&[0xCB, opcode | 0x07]);
        z80.a = val;
        z80.step();
        let bit_set = (val >> bit) & 1 != 0;
        prop_assert_eq!(z80.get_flag(flags::ZERO), !bit_set);
    }

    // ==================== IX/IY Properties ====================

    #[test]
    fn prop_ld_ix_nn(low in 0u8..=255, high in 0u8..=255) {
        let mut z80 = create_z80_with_program(&[0xDD, 0x21, low, high]);
        z80.step();
        let expected = ((high as u16) << 8) | (low as u16);
        prop_assert_eq!(z80.ix, expected);
    }

    #[test]
    fn prop_ld_iy_nn(low in 0u8..=255, high in 0u8..=255) {
        let mut z80 = create_z80_with_program(&[0xFD, 0x21, low, high]);
        z80.step();
        let expected = ((high as u16) << 8) | (low as u16);
        prop_assert_eq!(z80.iy, expected);
    }

    #[test]
    fn prop_inc_ix(val in 0u16..=0xFFFF) {
        let mut z80 = create_z80_with_program(&[0xDD, 0x23]);
        z80.ix = val;
        z80.step();
        prop_assert_eq!(z80.ix, val.wrapping_add(1));
    }

    #[test]
    fn prop_dec_iy(val in 0u16..=0xFFFF) {
        let mut z80 = create_z80_with_program(&[0xFD, 0x2B]);
        z80.iy = val;
        z80.step();
        prop_assert_eq!(z80.iy, val.wrapping_sub(1));
    }
}
