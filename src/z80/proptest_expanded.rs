#![allow(unused_imports)]
//! Expanded property-based tests for Z80 CPU - Massive test coverage

use super::test_utils::{TestIo, CombinedBus};
use super::*;
use crate::memory::Memory;
use crate::memory::{IoInterface, MemoryInterface, Z80Interface};
use proptest::prelude::*;

fn z80_prog(
    program: &[u8],
) -> (Z80, CombinedBus) {
    let mut m = Memory::new(0x10000);
    for (i, &b) in program.iter().enumerate() {
        m.data[i] = b;
    }
    (Z80::new(), CombinedBus::new(m, TestIo::default()))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    // ============ Register pair roundtrips ============
    #[test] fn prop_af(v in 0u16..=0xFFFF) { let (mut c, _) = z80_prog(&[]); c.set_af(v); prop_assert_eq!(c.af(), v); }
    #[test] fn prop_bc(v in 0u16..=0xFFFF) { let (mut c, _) = z80_prog(&[]); c.set_bc(v); prop_assert_eq!(c.bc(), v); }
    #[test] fn prop_de(v in 0u16..=0xFFFF) { let (mut c, _) = z80_prog(&[]); c.set_de(v); prop_assert_eq!(c.de(), v); }
    #[test] fn prop_hl(v in 0u16..=0xFFFF) { let (mut c, _) = z80_prog(&[]); c.set_hl(v); prop_assert_eq!(c.hl(), v); }

    // ============ LD rr, nn for all values ============
    #[test] fn prop_ld_bc_nn(lo in 0u8..=255, hi in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x01, lo, hi]); c.step(&mut bus); prop_assert_eq!(c.bc(), ((hi as u16) << 8) | lo as u16); prop_assert_eq!(c.pc, 3); }
    #[test] fn prop_ld_de_nn(lo in 0u8..=255, hi in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x11, lo, hi]); c.step(&mut bus); prop_assert_eq!(c.de(), ((hi as u16) << 8) | lo as u16); prop_assert_eq!(c.pc, 3); }
    #[test] fn prop_ld_hl_nn(lo in 0u8..=255, hi in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x21, lo, hi]); c.step(&mut bus); prop_assert_eq!(c.hl(), ((hi as u16) << 8) | lo as u16); prop_assert_eq!(c.pc, 3); }
    #[test] fn prop_ld_sp_nn(lo in 0u8..=255, hi in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x31, lo, hi]); c.step(&mut bus); prop_assert_eq!(c.sp, ((hi as u16) << 8) | lo as u16); prop_assert_eq!(c.pc, 3); }
    #[test] fn prop_ld_ix_nn(lo in 0u8..=255, hi in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0xDD, 0x21, lo, hi]); c.step(&mut bus); prop_assert_eq!(c.ix, ((hi as u16) << 8) | lo as u16); prop_assert_eq!(c.pc, 4); }
    #[test] fn prop_ld_iy_nn(lo in 0u8..=255, hi in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0xFD, 0x21, lo, hi]); c.step(&mut bus); prop_assert_eq!(c.iy, ((hi as u16) << 8) | lo as u16); prop_assert_eq!(c.pc, 4); }

    // ============ LD r, n for all values ============
    #[test] fn prop_ld_b_n(n in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x06, n]); c.step(&mut bus); prop_assert_eq!(c.b, n); prop_assert_eq!(c.pc, 2); }
    #[test] fn prop_ld_c_n(n in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x0E, n]); c.step(&mut bus); prop_assert_eq!(c.c, n); prop_assert_eq!(c.pc, 2); }
    #[test] fn prop_ld_d_n(n in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x16, n]); c.step(&mut bus); prop_assert_eq!(c.d, n); prop_assert_eq!(c.pc, 2); }
    #[test] fn prop_ld_e_n(n in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x1E, n]); c.step(&mut bus); prop_assert_eq!(c.e, n); prop_assert_eq!(c.pc, 2); }
    #[test] fn prop_ld_h_n(n in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x26, n]); c.step(&mut bus); prop_assert_eq!(c.h, n); prop_assert_eq!(c.pc, 2); }
    #[test] fn prop_ld_l_n(n in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x2E, n]); c.step(&mut bus); prop_assert_eq!(c.l, n); prop_assert_eq!(c.pc, 2); }
    #[test] fn prop_ld_a_n(n in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x3E, n]); c.step(&mut bus); prop_assert_eq!(c.a, n); prop_assert_eq!(c.pc, 2); }

    // ============ INC/DEC 16-bit ============
    #[test] fn prop_inc_bc(v in 0u16..=0xFFFF) { let (mut c, mut bus) = z80_prog(&[0x03]); c.set_bc(v); c.step(&mut bus); prop_assert_eq!(c.bc(), v.wrapping_add(1)); prop_assert_eq!(c.pc, 1); }
    #[test] fn prop_dec_bc(v in 0u16..=0xFFFF) { let (mut c, mut bus) = z80_prog(&[0x0B]); c.set_bc(v); c.step(&mut bus); prop_assert_eq!(c.bc(), v.wrapping_sub(1)); prop_assert_eq!(c.pc, 1); }
    #[test] fn prop_inc_de(v in 0u16..=0xFFFF) { let (mut c, mut bus) = z80_prog(&[0x13]); c.set_de(v); c.step(&mut bus); prop_assert_eq!(c.de(), v.wrapping_add(1)); prop_assert_eq!(c.pc, 1); }
    #[test] fn prop_dec_de(v in 0u16..=0xFFFF) { let (mut c, mut bus) = z80_prog(&[0x1B]); c.set_de(v); c.step(&mut bus); prop_assert_eq!(c.de(), v.wrapping_sub(1)); prop_assert_eq!(c.pc, 1); }
    #[test] fn prop_inc_hl(v in 0u16..=0xFFFF) { let (mut c, mut bus) = z80_prog(&[0x23]); c.set_hl(v); c.step(&mut bus); prop_assert_eq!(c.hl(), v.wrapping_add(1)); prop_assert_eq!(c.pc, 1); }
    #[test] fn prop_dec_hl(v in 0u16..=0xFFFF) { let (mut c, mut bus) = z80_prog(&[0x2B]); c.set_hl(v); c.step(&mut bus); prop_assert_eq!(c.hl(), v.wrapping_sub(1)); prop_assert_eq!(c.pc, 1); }
    #[test] fn prop_inc_sp(v in 0u16..=0xFFFF) { let (mut c, mut bus) = z80_prog(&[0x33]); c.sp = v; c.step(&mut bus); prop_assert_eq!(c.sp, v.wrapping_add(1)); prop_assert_eq!(c.pc, 1); }
    #[test] fn prop_dec_sp(v in 0u16..=0xFFFF) { let (mut c, mut bus) = z80_prog(&[0x3B]); c.sp = v; c.step(&mut bus); prop_assert_eq!(c.sp, v.wrapping_sub(1)); prop_assert_eq!(c.pc, 1); }
    #[test] fn prop_inc_ix(v in 0u16..=0xFFFF) { let (mut c, mut bus) = z80_prog(&[0xDD, 0x23]); c.ix = v; c.step(&mut bus); prop_assert_eq!(c.ix, v.wrapping_add(1)); prop_assert_eq!(c.pc, 2); }
    #[test] fn prop_dec_ix(v in 0u16..=0xFFFF) { let (mut c, mut bus) = z80_prog(&[0xDD, 0x2B]); c.ix = v; c.step(&mut bus); prop_assert_eq!(c.ix, v.wrapping_sub(1)); prop_assert_eq!(c.pc, 2); }
    #[test] fn prop_inc_iy(v in 0u16..=0xFFFF) { let (mut c, mut bus) = z80_prog(&[0xFD, 0x23]); c.iy = v; c.step(&mut bus); prop_assert_eq!(c.iy, v.wrapping_add(1)); prop_assert_eq!(c.pc, 2); }
    #[test] fn prop_dec_iy(v in 0u16..=0xFFFF) { let (mut c, mut bus) = z80_prog(&[0xFD, 0x2B]); c.iy = v; c.step(&mut bus); prop_assert_eq!(c.iy, v.wrapping_sub(1)); prop_assert_eq!(c.pc, 2); }

    // ============ INC/DEC 8-bit ============
    #[test] fn prop_inc_a(v in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x3C]); c.a = v; c.step(&mut bus); prop_assert_eq!(c.a, v.wrapping_add(1)); prop_assert_eq!(c.pc, 1); }
    #[test] fn prop_dec_a(v in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x3D]); c.a = v; c.step(&mut bus); prop_assert_eq!(c.a, v.wrapping_sub(1)); prop_assert_eq!(c.pc, 1); }
    #[test] fn prop_inc_b(v in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x04]); c.b = v; c.step(&mut bus); prop_assert_eq!(c.b, v.wrapping_add(1)); }
    #[test] fn prop_dec_b(v in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x05]); c.b = v; c.step(&mut bus); prop_assert_eq!(c.b, v.wrapping_sub(1)); }
    #[test] fn prop_inc_c(v in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x0C]); c.c = v; c.step(&mut bus); prop_assert_eq!(c.c, v.wrapping_add(1)); }
    #[test] fn prop_dec_c(v in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x0D]); c.c = v; c.step(&mut bus); prop_assert_eq!(c.c, v.wrapping_sub(1)); }
    #[test] fn prop_inc_d(v in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x14]); c.d = v; c.step(&mut bus); prop_assert_eq!(c.d, v.wrapping_add(1)); }
    #[test] fn prop_dec_d(v in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x15]); c.d = v; c.step(&mut bus); prop_assert_eq!(c.d, v.wrapping_sub(1)); }
    #[test] fn prop_inc_e(v in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x1C]); c.e = v; c.step(&mut bus); prop_assert_eq!(c.e, v.wrapping_add(1)); }
    #[test] fn prop_dec_e(v in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x1D]); c.e = v; c.step(&mut bus); prop_assert_eq!(c.e, v.wrapping_sub(1)); }
    #[test] fn prop_inc_h(v in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x24]); c.h = v; c.step(&mut bus); prop_assert_eq!(c.h, v.wrapping_add(1)); }
    #[test] fn prop_dec_h(v in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x25]); c.h = v; c.step(&mut bus); prop_assert_eq!(c.h, v.wrapping_sub(1)); }
    #[test] fn prop_inc_l(v in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x2C]); c.l = v; c.step(&mut bus); prop_assert_eq!(c.l, v.wrapping_add(1)); }
    #[test] fn prop_dec_l(v in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x2D]); c.l = v; c.step(&mut bus); prop_assert_eq!(c.l, v.wrapping_sub(1)); }

    // ============ ALU: ADD A, r ============
    #[test] fn prop_add_a_b(a in 0u8..=255, b in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x80]); c.a = a; c.b = b; c.step(&mut bus); prop_assert_eq!(c.a, a.wrapping_add(b)); prop_assert_eq!(c.get_flag(flags::CARRY), (a as u16 + b as u16) > 0xFF); prop_assert_eq!(c.pc, 1); }
    #[test] fn prop_add_a_c(a in 0u8..=255, v in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x81]); c.a = a; c.c = v; c.step(&mut bus); prop_assert_eq!(c.a, a.wrapping_add(v)); }
    #[test] fn prop_add_a_d(a in 0u8..=255, v in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x82]); c.a = a; c.d = v; c.step(&mut bus); prop_assert_eq!(c.a, a.wrapping_add(v)); }
    #[test] fn prop_add_a_e(a in 0u8..=255, v in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x83]); c.a = a; c.e = v; c.step(&mut bus); prop_assert_eq!(c.a, a.wrapping_add(v)); }
    #[test] fn prop_add_a_h(a in 0u8..=255, v in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x84]); c.a = a; c.h = v; c.step(&mut bus); prop_assert_eq!(c.a, a.wrapping_add(v)); }
    #[test] fn prop_add_a_l(a in 0u8..=255, v in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x85]); c.a = a; c.l = v; c.step(&mut bus); prop_assert_eq!(c.a, a.wrapping_add(v)); }
    #[test] fn prop_add_a_a(a in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x87]); c.a = a; c.step(&mut bus); prop_assert_eq!(c.a, a.wrapping_add(a)); }

    // ============ ALU: SUB r ============
    #[test] fn prop_sub_b(a in 0u8..=255, b in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x90]); c.a = a; c.b = b; c.step(&mut bus); prop_assert_eq!(c.a, a.wrapping_sub(b)); prop_assert!(c.get_flag(flags::ADD_SUB)); prop_assert_eq!(c.pc, 1); }
    #[test] fn prop_sub_c(a in 0u8..=255, v in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x91]); c.a = a; c.c = v; c.step(&mut bus); prop_assert_eq!(c.a, a.wrapping_sub(v)); }
    #[test] fn prop_sub_d(a in 0u8..=255, v in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x92]); c.a = a; c.d = v; c.step(&mut bus); prop_assert_eq!(c.a, a.wrapping_sub(v)); }
    #[test] fn prop_sub_e(a in 0u8..=255, v in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x93]); c.a = a; c.e = v; c.step(&mut bus); prop_assert_eq!(c.a, a.wrapping_sub(v)); }
    #[test] fn prop_sub_h(a in 0u8..=255, v in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x94]); c.a = a; c.h = v; c.step(&mut bus); prop_assert_eq!(c.a, a.wrapping_sub(v)); }
    #[test] fn prop_sub_l(a in 0u8..=255, v in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x95]); c.a = a; c.l = v; c.step(&mut bus); prop_assert_eq!(c.a, a.wrapping_sub(v)); }
    #[test] fn prop_sub_a(a in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x97]); c.a = a; c.step(&mut bus); prop_assert_eq!(c.a, 0); prop_assert!(c.get_flag(flags::ZERO)); }

    // ============ ALU: AND/OR/XOR ============
    #[test] fn prop_and_b(a in 0u8..=255, b in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0xA0]); c.a = a; c.b = b; c.step(&mut bus); prop_assert_eq!(c.a, a & b); prop_assert!(!c.get_flag(flags::CARRY)); prop_assert!(c.get_flag(flags::HALF_CARRY)); }
    #[test] fn prop_or_b(a in 0u8..=255, b in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0xB0]); c.a = a; c.b = b; c.step(&mut bus); prop_assert_eq!(c.a, a | b); prop_assert!(!c.get_flag(flags::CARRY)); }
    #[test] fn prop_xor_b(a in 0u8..=255, b in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0xA8]); c.a = a; c.b = b; c.step(&mut bus); prop_assert_eq!(c.a, a ^ b); prop_assert!(!c.get_flag(flags::CARRY)); }
    #[test] fn prop_xor_a_clears(a in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0xAF]); c.a = a; c.step(&mut bus); prop_assert_eq!(c.a, 0); prop_assert!(c.get_flag(flags::ZERO)); }

    // ============ ALU: CP (compare doesn't modify A) ============
    #[test] fn prop_cp_b(a in 0u8..=255, b in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0xB8]); c.a = a; c.b = b; c.step(&mut bus); prop_assert_eq!(c.a, a); prop_assert_eq!(c.get_flag(flags::ZERO), a == b); prop_assert_eq!(c.pc, 1); }

    // ============ ALU: ADC/SBC with carry ============
    #[test] fn prop_adc_b_nc(a in 0u8..=255, b in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x88]); c.a = a; c.b = b; c.f = 0; c.step(&mut bus); prop_assert_eq!(c.a, a.wrapping_add(b)); }
    #[test] fn prop_adc_b_c(a in 0u8..=255, b in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x88]); c.a = a; c.b = b; c.set_flag(flags::CARRY, true); c.step(&mut bus); prop_assert_eq!(c.a, a.wrapping_add(b).wrapping_add(1)); }
    #[test] fn prop_sbc_b_nc(a in 0u8..=255, b in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x98]); c.a = a; c.b = b; c.f = 0; c.step(&mut bus); prop_assert_eq!(c.a, a.wrapping_sub(b)); }
    #[test] fn prop_sbc_b_c(a in 0u8..=255, b in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x98]); c.a = a; c.b = b; c.set_flag(flags::CARRY, true); c.step(&mut bus); prop_assert_eq!(c.a, a.wrapping_sub(b).wrapping_sub(1)); }

    // ============ ALU immediate ============
    #[test] fn prop_add_a_n(a in 0u8..=255, n in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0xC6, n]); c.a = a; c.step(&mut bus); prop_assert_eq!(c.a, a.wrapping_add(n)); prop_assert_eq!(c.pc, 2); }
    #[test] fn prop_sub_n(a in 0u8..=255, n in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0xD6, n]); c.a = a; c.step(&mut bus); prop_assert_eq!(c.a, a.wrapping_sub(n)); prop_assert_eq!(c.pc, 2); }
    #[test] fn prop_and_n(a in 0u8..=255, n in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0xE6, n]); c.a = a; c.step(&mut bus); prop_assert_eq!(c.a, a & n); prop_assert_eq!(c.pc, 2); }
    #[test] fn prop_or_n(a in 0u8..=255, n in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0xF6, n]); c.a = a; c.step(&mut bus); prop_assert_eq!(c.a, a | n); prop_assert_eq!(c.pc, 2); }
    #[test] fn prop_xor_n(a in 0u8..=255, n in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0xEE, n]); c.a = a; c.step(&mut bus); prop_assert_eq!(c.a, a ^ n); prop_assert_eq!(c.pc, 2); }
    #[test] fn prop_cp_n(a in 0u8..=255, n in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0xFE, n]); c.a = a; c.step(&mut bus); prop_assert_eq!(c.a, a); prop_assert_eq!(c.get_flag(flags::ZERO), a == n); prop_assert_eq!(c.pc, 2); }

    // ============ Rotates ============
    #[test] fn prop_rlca(a in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x07]); c.a = a; c.step(&mut bus); let carry = (a & 0x80) != 0; prop_assert_eq!(c.a, (a << 1) | if carry { 1 } else { 0 }); prop_assert_eq!(c.get_flag(flags::CARRY), carry); prop_assert_eq!(c.pc, 1); }
    #[test] fn prop_rrca(a in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x0F]); c.a = a; c.step(&mut bus); let carry = (a & 0x01) != 0; prop_assert_eq!(c.a, (a >> 1) | if carry { 0x80 } else { 0 }); prop_assert_eq!(c.get_flag(flags::CARRY), carry); prop_assert_eq!(c.pc, 1); }

    // ============ CB: RLC/RRC/RL/RR/SLA/SRA/SRL ============
    #[test] fn prop_cb_rlc_a(a in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0xCB, 0x07]); c.a = a; c.step(&mut bus); let carry = (a & 0x80) != 0; prop_assert_eq!(c.a, (a << 1) | if carry { 1 } else { 0 }); prop_assert_eq!(c.get_flag(flags::CARRY), carry); prop_assert_eq!(c.pc, 2); }
    #[test] fn prop_cb_rrc_a(a in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0xCB, 0x0F]); c.a = a; c.step(&mut bus); let carry = (a & 0x01) != 0; prop_assert_eq!(c.a, (a >> 1) | if carry { 0x80 } else { 0 }); prop_assert_eq!(c.get_flag(flags::CARRY), carry); prop_assert_eq!(c.pc, 2); }
    #[test] fn prop_cb_sla_a(a in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0xCB, 0x27]); c.a = a; c.step(&mut bus); prop_assert_eq!(c.a, a << 1); prop_assert_eq!(c.get_flag(flags::CARRY), (a & 0x80) != 0); prop_assert_eq!(c.pc, 2); }
    #[test] fn prop_cb_srl_a(a in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0xCB, 0x3F]); c.a = a; c.step(&mut bus); prop_assert_eq!(c.a, a >> 1); prop_assert_eq!(c.get_flag(flags::CARRY), (a & 0x01) != 0); prop_assert_eq!(c.pc, 2); }

    // ============ CB: BIT ============
    #[test] fn prop_cb_bit_0(a in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0xCB, 0x47]); c.a = a; c.step(&mut bus); prop_assert_eq!(c.get_flag(flags::ZERO), (a & 0x01) == 0); prop_assert_eq!(c.a, a); prop_assert_eq!(c.pc, 2); }
    #[test] fn prop_cb_bit_7(a in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0xCB, 0x7F]); c.a = a; c.step(&mut bus); prop_assert_eq!(c.get_flag(flags::ZERO), (a & 0x80) == 0); prop_assert_eq!(c.a, a); }

    // ============ CB: SET/RES ============
    #[test] fn prop_cb_set_0(a in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0xCB, 0xC7]); c.a = a; c.step(&mut bus); prop_assert_eq!(c.a, a | 0x01); prop_assert_eq!(c.pc, 2); }
    #[test] fn prop_cb_set_7(a in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0xCB, 0xFF]); c.a = a; c.step(&mut bus); prop_assert_eq!(c.a, a | 0x80); }
    #[test] fn prop_cb_res_0(a in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0xCB, 0x87]); c.a = a; c.step(&mut bus); prop_assert_eq!(c.a, a & 0xFE); prop_assert_eq!(c.pc, 2); }
    #[test] fn prop_cb_res_7(a in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0xCB, 0xBF]); c.a = a; c.step(&mut bus); prop_assert_eq!(c.a, a & 0x7F); }
    #[test] fn prop_cb_set_all(bit in 0u8..8, a in 0u8..=255) { let op = 0xC7 | (bit << 3); let (mut c, mut bus) = z80_prog(&[0xCB, op]); c.a = a; c.step(&mut bus); prop_assert_eq!(c.a, a | (1 << bit)); }
    #[test] fn prop_cb_res_all(bit in 0u8..8, a in 0u8..=255) { let op = 0x87 | (bit << 3); let (mut c, mut bus) = z80_prog(&[0xCB, op]); c.a = a; c.step(&mut bus); prop_assert_eq!(c.a, a & !(1 << bit)); }
    #[test] fn prop_cb_bit_all(bit in 0u8..8, a in 0u8..=255) { let op = 0x47 | (bit << 3); let (mut c, mut bus) = z80_prog(&[0xCB, op]); c.a = a; c.step(&mut bus); prop_assert_eq!(c.get_flag(flags::ZERO), (a & (1 << bit)) == 0); }

    // ============ PUSH/POP roundtrip ============
    #[test] fn prop_push_pop_bc(v in 0u16..=0xFFFF) { let (mut c, mut bus) = z80_prog(&[0xC5, 0xC1]); c.sp = 0x8000; c.set_bc(v); c.step(&mut bus); c.set_bc(0); c.step(&mut bus); prop_assert_eq!(c.bc(), v); prop_assert_eq!(c.sp, 0x8000); }
    #[test] fn prop_push_pop_de(v in 0u16..=0xFFFF) { let (mut c, mut bus) = z80_prog(&[0xD5, 0xD1]); c.sp = 0x8000; c.set_de(v); c.step(&mut bus); c.set_de(0); c.step(&mut bus); prop_assert_eq!(c.de(), v); }
    #[test] fn prop_push_pop_hl(v in 0u16..=0xFFFF) { let (mut c, mut bus) = z80_prog(&[0xE5, 0xE1]); c.sp = 0x8000; c.set_hl(v); c.step(&mut bus); c.set_hl(0); c.step(&mut bus); prop_assert_eq!(c.hl(), v); }
    #[test] fn prop_push_pop_af(v in 0u16..=0xFFFF) { let (mut c, mut bus) = z80_prog(&[0xF5, 0xF1]); c.sp = 0x8000; c.set_af(v); c.step(&mut bus); c.set_af(0); c.step(&mut bus); prop_assert_eq!(c.af(), v); }
    #[test] fn prop_push_pop_ix(v in 0u16..=0xFFFF) { let (mut c, mut bus) = z80_prog(&[0xDD, 0xE5, 0xDD, 0xE1]); c.sp = 0x8000; c.ix = v; c.step(&mut bus); c.ix = 0; c.step(&mut bus); prop_assert_eq!(c.ix, v); }
    #[test] fn prop_push_pop_iy(v in 0u16..=0xFFFF) { let (mut c, mut bus) = z80_prog(&[0xFD, 0xE5, 0xFD, 0xE1]); c.sp = 0x8000; c.iy = v; c.step(&mut bus); c.iy = 0; c.step(&mut bus); prop_assert_eq!(c.iy, v); }

    // ============ NEG ============
    #[test] fn prop_neg(a in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0xED, 0x44]); c.a = a; c.step(&mut bus); prop_assert_eq!(c.a, 0u8.wrapping_sub(a)); prop_assert_eq!(c.pc, 2); }

    // ============ CPL ============
    #[test] fn prop_cpl(a in 0u8..=255) { let (mut c, mut bus) = z80_prog(&[0x2F]); c.a = a; c.step(&mut bus); prop_assert_eq!(c.a, !a); prop_assert_eq!(c.pc, 1); }

    // ============ ADD HL, rr ============
    #[test] fn prop_add_hl_bc(hl in 0u16..=0xFFFF, bc in 0u16..=0xFFFF) { let (mut c, mut bus) = z80_prog(&[0x09]); c.set_hl(hl); c.set_bc(bc); c.step(&mut bus); prop_assert_eq!(c.hl(), hl.wrapping_add(bc)); prop_assert_eq!(c.pc, 1); }
    #[test] fn prop_add_hl_de(hl in 0u16..=0xFFFF, de in 0u16..=0xFFFF) { let (mut c, mut bus) = z80_prog(&[0x19]); c.set_hl(hl); c.set_de(de); c.step(&mut bus); prop_assert_eq!(c.hl(), hl.wrapping_add(de)); }
    #[test] fn prop_add_hl_hl(hl in 0u16..=0xFFFF) { let (mut c, mut bus) = z80_prog(&[0x29]); c.set_hl(hl); c.step(&mut bus); prop_assert_eq!(c.hl(), hl.wrapping_add(hl)); }
    #[test] fn prop_add_hl_sp(hl in 0u16..=0xFFFF, sp in 0u16..=0xFFFF) { let (mut c, mut bus) = z80_prog(&[0x39]); c.set_hl(hl); c.sp = sp; c.step(&mut bus); prop_assert_eq!(c.hl(), hl.wrapping_add(sp)); }

    // ============ R-Register invariants ============
    #[test] fn prop_r_inc_preserves_bit7(r_val in 0u8..=255) {
        let (mut c, mut bus) = z80_prog(&[0x00]); // NOP
        c.r = r_val;
        c.step(&mut bus);
        prop_assert_eq!(c.r & 0x80, r_val & 0x80); // Bit 7 MUST be same
        prop_assert_eq!(c.r & 0x7F, (r_val.wrapping_add(1)) & 0x7F); // Lower 7 bits MUST increment and wrap
    }

    // ============ NOP does nothing ============
    #[test] fn prop_nop(a in 0u8..=255, b in 0u8..=255, bc in 0u16..=0xFFFF, sp in 0u16..=0xFFFF) { let (mut c, mut bus) = z80_prog(&[0x00]); c.a = a; c.b = b; c.set_bc(bc); c.sp = sp; c.step(&mut bus); prop_assert_eq!(c.a, a); prop_assert_eq!(c.bc(), bc); prop_assert_eq!(c.sp, sp); prop_assert_eq!(c.pc, 1); }
}
