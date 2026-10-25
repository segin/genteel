//! Hardware-conformance tests: prefix handling, R refresh, MEMPTR, flags,
//! the Q register and interrupt-line behaviour. Expected values agree with
//! the SingleStepTests Z80 data set.

use super::*;
use crate::memory::MemoryInterface;
use crate::z80::test_utils::create_z80;
use proptest::prelude::*;

const XY: u8 = flags::X_FLAG | flags::Y_FLAG;

fn at(program: &[u8], org: u16) -> Z80<crate::memory::Memory, crate::z80::test_utils::TestIo> {
    let mut c = create_z80(&[]);
    for (i, &b) in program.iter().enumerate() {
        c.memory.write_byte(org.wrapping_add(i as u16) as u32, b);
    }
    c.pc = org;
    c
}

// ============ Indexed addressing ============

#[test]
fn index_displacement_wraps_upward() {
    // LD A,(IX+1) with IX=0x7FFF reads 0x8000
    let mut c = create_z80(&[0xDD, 0x7E, 0x01]);
    c.ix = 0x7FFF;
    c.memory.write_byte(0x8000, 0x5A);
    c.step();
    assert_eq!(c.a, 0x5A);
    assert_eq!(c.memptr, 0x8000);
}

#[test]
fn index_displacement_wraps_downward() {
    // LD A,(IY-1) with IY=0x8000 reads 0x7FFF
    let mut c = create_z80(&[0xFD, 0x7E, 0xFF]);
    c.iy = 0x8000;
    c.memory.write_byte(0x7FFF, 0xA5);
    c.step();
    assert_eq!(c.a, 0xA5);
}

#[test]
fn index_displacement_wraps_around_64k() {
    let mut c = create_z80(&[0xDD, 0x7E, 0x05]);
    c.ix = 0xFFFF;
    c.memory.write_byte(0x0004, 0x77);
    c.step();
    assert_eq!(c.a, 0x77);
}

proptest! {
    #[test]
    fn prop_index_address_is_modular(ix in any::<u16>(), d in any::<i8>()) {
        let mut c = at(&[0xDD, 0x7E, d as u8], 0x0000);
        c.ix = ix;
        let ea = ix.wrapping_add(d as i16 as u16);
        if ea > 2 {
            c.memory.write_byte(ea as u32, 0x3C);
            c.step();
            prop_assert_eq!(c.a, 0x3C);
            prop_assert_eq!(c.memptr, ea);
        }
    }
}

// ============ DD/FD prefixes on non-HL opcodes ============

#[test]
fn dd_prefix_on_ld_a_n_executes_unprefixed() {
    let mut c = create_z80(&[0xDD, 0x3E, 0x42, 0x00]);
    let t = c.step();
    assert_eq!(c.a, 0x42);
    assert_eq!(c.pc, 3);
    assert_eq!(t, 11);
}

#[test]
fn dd_prefix_on_ld_bc_nn_and_jp() {
    let mut c = create_z80(&[0xDD, 0x01, 0x34, 0x12, 0xFD, 0xC3, 0x00, 0x20]);
    assert_eq!(c.step(), 14);
    assert_eq!(c.bc(), 0x1234);
    assert_eq!(c.step(), 14);
    assert_eq!(c.pc, 0x2000);
}

#[test]
fn dd_prefix_on_call_and_jr_and_djnz() {
    let mut c = create_z80(&[0xDD, 0xCD, 0x00, 0x30]);
    c.sp = 0x8000;
    assert_eq!(c.step(), 21);
    assert_eq!(c.pc, 0x3000);
    assert_eq!(c.read_word(0x7FFE), 0x0004);

    let mut c = create_z80(&[0xDD, 0x18, 0x10]);
    assert_eq!(c.step(), 16);
    assert_eq!(c.pc, 0x13);

    let mut c = create_z80(&[0xFD, 0x10, 0x10]);
    c.b = 2;
    assert_eq!(c.step(), 17);
    assert_eq!(c.pc, 0x13);
}

#[test]
fn dd_prefix_on_ld_b_n() {
    let mut c = create_z80(&[0xDD, 0x06, 0x99]);
    assert_eq!(c.step(), 11);
    assert_eq!(c.b, 0x99);
    assert_eq!(c.ix, 0);
}

#[test]
fn consecutive_index_prefixes_use_the_last() {
    // DD FD 21 nn nn loads IY
    let mut c = create_z80(&[0xDD, 0xFD, 0x21, 0x34, 0x12]);
    let t = c.step();
    assert_eq!(c.iy, 0x1234);
    assert_eq!(c.ix, 0);
    assert_eq!(c.hl(), 0);
    assert_eq!(t, 18);
    assert_eq!(c.r & 0x7F, 3);

    // FD DD 21 nn nn loads IX
    let mut c = create_z80(&[0xFD, 0xDD, 0x21, 0x78, 0x56]);
    c.step();
    assert_eq!(c.ix, 0x5678);
    assert_eq!(c.iy, 0);
}

#[test]
fn index_prefix_before_ed_runs_the_ed_instruction() {
    // DD ED B0 = LDIR
    let mut c = create_z80(&[0xDD, 0xED, 0xB0]);
    c.set_hl(0x1000);
    c.set_de(0x2000);
    c.set_bc(1);
    c.b = 0;
    c.memory.write_byte(0x1000, 0xAB);
    let t = c.step();
    assert_eq!(c.memory.read_byte(0x2000), 0xAB);
    assert_eq!(c.bc(), 0);
    assert_eq!(c.pc, 3);
    assert_eq!(t, 20);
    assert_eq!(c.b, 0);
}

// ============ R register ============

fn r_delta(program: &[u8]) -> u8 {
    let mut c = create_z80(program);
    c.r = 0x80;
    c.sp = 0x8000;
    c.step();
    assert_eq!(c.r & 0x80, 0x80, "bit 7 of R must be stable");
    c.r & 0x7F
}

#[test]
fn r_increments_once_per_m1_cycle() {
    assert_eq!(r_delta(&[0x00]), 1); // NOP
    assert_eq!(r_delta(&[0x3E, 0x12]), 1); // LD A,n
    assert_eq!(r_delta(&[0xC3, 0x00, 0x10]), 1); // JP nn
    assert_eq!(r_delta(&[0xCD, 0x00, 0x10]), 1); // CALL nn
    assert_eq!(r_delta(&[0xCB, 0x00]), 2); // RLC B
    assert_eq!(r_delta(&[0xED, 0x44]), 2); // NEG
    assert_eq!(r_delta(&[0xDD, 0x21, 0x00, 0x10]), 2); // LD IX,nn
    assert_eq!(r_delta(&[0xDD, 0x36, 0x01, 0x55]), 2); // LD (IX+d),n
    assert_eq!(r_delta(&[0xDD, 0xCB, 0x01, 0x06]), 2); // RLC (IX+d)
    assert_eq!(r_delta(&[0xFD, 0xCB, 0x01, 0x46]), 2); // BIT 0,(IY+d)
}

#[test]
fn r_low_bits_wrap_without_touching_bit7() {
    let mut c = create_z80(&[0x00]);
    c.r = 0xFF;
    c.step();
    assert_eq!(c.r, 0x80);
    let mut c = create_z80(&[0x00]);
    c.r = 0x7F;
    c.step();
    assert_eq!(c.r, 0x00);
}

#[test]
fn halted_cpu_refreshes_r_and_counts_cycles() {
    let mut c = create_z80(&[0x76]);
    c.step();
    assert!(c.halted);
    assert_eq!(c.r, 1);
    let cycles = c.cycles;
    for _ in 0..5 {
        assert_eq!(c.step(), 4);
    }
    assert_eq!(c.r, 6);
    assert_eq!(c.cycles, cycles + 20);
    assert_eq!(c.pc, 1);
}

#[test]
fn accepting_interrupts_increments_r() {
    let mut c = create_z80(&[0x00]);
    c.iff1 = true;
    c.sp = 0x8000;
    c.im = 1;
    c.trigger_interrupt(0xFF);
    assert_eq!(c.r, 1);
    c.trigger_nmi();
    assert_eq!(c.r, 2);
}

// ============ MEMPTR ============

#[test]
fn memptr_jp_and_jp_cc_both_paths() {
    let mut c = create_z80(&[0xC3, 0x34, 0x12]);
    c.step();
    assert_eq!(c.memptr, 0x1234);
    // JP NZ,nn not taken still loads MEMPTR
    let mut c = create_z80(&[0xC2, 0x78, 0x56]);
    c.f = flags::ZERO;
    c.step();
    assert_eq!(c.pc, 3);
    assert_eq!(c.memptr, 0x5678);
}

#[test]
fn memptr_call_and_call_cc_both_paths() {
    let mut c = create_z80(&[0xCD, 0x34, 0x12]);
    c.sp = 0x8000;
    c.step();
    assert_eq!(c.memptr, 0x1234);
    let mut c = create_z80(&[0xC4, 0x78, 0x56]);
    c.f = flags::ZERO;
    c.step();
    assert_eq!(c.memptr, 0x5678);
}

#[test]
fn memptr_ret_family_and_rst() {
    for prog in [&[0xC9u8][..], &[0xC0], &[0xED, 0x45], &[0xED, 0x4D]] {
        let mut c = create_z80(prog);
        c.f = 0; // RET NZ taken
        c.sp = 0x8000;
        c.write_word(0x8000, 0x4321);
        c.step();
        assert_eq!(c.pc, 0x4321);
        assert_eq!(c.memptr, 0x4321, "program {prog:02X?}");
    }
    let mut c = create_z80(&[0xEF]);
    c.sp = 0x8000;
    c.step();
    assert_eq!(c.memptr, 0x0028);
}

#[test]
fn memptr_jr_and_djnz_taken() {
    let mut c = at(&[0x18, 0xFE], 0x1000);
    c.step();
    assert_eq!(c.memptr, 0x1000);
    let mut c = at(&[0x20, 0x05], 0x1000);
    c.f = 0;
    c.step();
    assert_eq!(c.memptr, 0x1007);
    let mut c = at(&[0x10, 0x05], 0x1000);
    c.b = 3;
    c.memptr = 0xAAAA;
    c.step();
    assert_eq!(c.memptr, 0x1007);
    // DJNZ not taken leaves MEMPTR alone
    let mut c = at(&[0x10, 0x05], 0x1000);
    c.b = 1;
    c.memptr = 0xAAAA;
    c.step();
    assert_eq!(c.memptr, 0xAAAA);
}

#[test]
fn memptr_port_io() {
    let mut c = create_z80(&[0xDB, 0xFF]); // IN A,(n)
    c.a = 0x12;
    c.step();
    assert_eq!(c.memptr, 0x1300);
    let mut c = create_z80(&[0xD3, 0xFF]); // OUT (n),A
    c.a = 0x12;
    c.step();
    assert_eq!(c.memptr, 0x1200);
    let mut c = create_z80(&[0xED, 0x78]); // IN A,(C)
    c.set_bc(0x12FF);
    c.step();
    assert_eq!(c.memptr, 0x1300);
    let mut c = create_z80(&[0xED, 0x79]); // OUT (C),A
    c.set_bc(0x1234);
    c.step();
    assert_eq!(c.memptr, 0x1235);
}

#[test]
fn memptr_adc_sbc_hl_rld_rrd() {
    for prog in [[0xED, 0x4A], [0xED, 0x42], [0xED, 0x6F], [0xED, 0x67]] {
        let mut c = create_z80(&prog);
        c.set_hl(0x2FFF);
        c.step();
        assert_eq!(c.memptr, 0x3000, "program {prog:02X?}");
    }
}

#[test]
fn memptr_ld_nn_index() {
    let mut c = create_z80(&[0xDD, 0x22, 0x00, 0x40]);
    c.ix = 0xBEEF;
    c.step();
    assert_eq!(c.memptr, 0x4001);
    assert_eq!(c.read_word(0x4000), 0xBEEF);
    let mut c = create_z80(&[0xFD, 0x2A, 0xFF, 0xFF]);
    c.step();
    assert_eq!(c.memptr, 0x0000);
}

#[test]
fn memptr_block_io_and_cpd() {
    // INI: BC before B decrement + 1; IND: -1
    let mut c = create_z80(&[0xED, 0xA2]);
    c.set_bc(0x0510);
    c.set_hl(0x4000);
    c.step();
    assert_eq!(c.memptr, 0x0511);
    let mut c = create_z80(&[0xED, 0xAA]);
    c.set_bc(0x0510);
    c.set_hl(0x4000);
    c.step();
    assert_eq!(c.memptr, 0x050F);
    // OUTI: BC after B decrement + 1; OUTD: -1
    let mut c = create_z80(&[0xED, 0xA3]);
    c.set_bc(0x0510);
    c.set_hl(0x4000);
    c.step();
    assert_eq!(c.memptr, 0x0411);
    let mut c = create_z80(&[0xED, 0xAB]);
    c.set_bc(0x0510);
    c.set_hl(0x4000);
    c.step();
    assert_eq!(c.memptr, 0x040F);
    // CPD decrements MEMPTR
    let mut c = create_z80(&[0xED, 0xA9]);
    c.memptr = 0x1000;
    c.set_bc(2);
    c.step();
    assert_eq!(c.memptr, 0x0FFF);
}

#[test]
fn outi_places_decremented_b_on_port_high_byte() {
    let mut c = create_z80(&[0xED, 0xA3]);
    c.set_bc(0x0510);
    c.set_hl(0x4000);
    c.memory.write_byte(0x4000, 0x99);
    c.step();
    assert_eq!(c.io.ports.get(&0x0410), Some(&0x99));
}

#[test]
fn bit_hl_xy_after_jp_comes_from_target() {
    // JP 0x2800 sets MEMPTR=0x2800, so BIT 0,(HL) leaks X/Y=0x28
    let mut c = create_z80(&[0xC3, 0x00, 0x28]);
    c.memory.write_byte(0x2800, 0xCB);
    c.memory.write_byte(0x2801, 0x46);
    c.step();
    c.step();
    assert_eq!(c.f & XY, 0x28);
}

// ============ BIT flags ============

proptest! {
    #[test]
    fn prop_bit_register_flags(val in any::<u8>(), bit in 0u8..8, f_in in any::<u8>()) {
        let mut c = create_z80(&[0xCB, 0x40 | (bit << 3)]); // BIT bit,B
        c.b = val;
        c.f = f_in;
        c.step();
        let set = val & (1 << bit) != 0;
        prop_assert_eq!(c.get_flag(flags::ZERO), !set);
        prop_assert_eq!(c.get_flag(flags::PARITY), !set);
        prop_assert_eq!(c.get_flag(flags::SIGN), bit == 7 && set);
        prop_assert!(c.get_flag(flags::HALF_CARRY));
        prop_assert!(!c.get_flag(flags::ADD_SUB));
        prop_assert_eq!(c.get_flag(flags::CARRY), f_in & flags::CARRY != 0);
        prop_assert_eq!(c.f & XY, val & XY);
    }

    #[test]
    fn prop_bit_indexed_xy_from_address(d in any::<i8>(), iy in 0x1000u16..0xF000, bit in 0u8..8) {
        let mut c = create_z80(&[0xFD, 0xCB, d as u8, 0x46 | (bit << 3)]);
        c.iy = iy;
        c.step();
        let ea = iy.wrapping_add(d as i16 as u16);
        prop_assert_eq!(c.f & XY, ((ea >> 8) as u8) & XY);
        prop_assert!(c.get_flag(flags::ZERO));
        prop_assert!(c.get_flag(flags::PARITY));
    }
}

#[test]
fn bit7_set_sets_sign() {
    let mut c = create_z80(&[0xCB, 0x7F]); // BIT 7,A
    c.a = 0x80;
    c.step();
    assert!(c.get_flag(flags::SIGN));
    assert!(!c.get_flag(flags::ZERO));
    assert!(!c.get_flag(flags::PARITY));
}

// ============ DAA ============

/// Reference DAA from "The Undocumented Z80 Documented" tables.
fn ref_daa(a: u8, f: u8) -> (u8, u8) {
    let n = f & flags::ADD_SUB != 0;
    let h = f & flags::HALF_CARRY != 0;
    let c = f & flags::CARRY != 0;
    let hi = a >> 4;
    let lo = a & 0x0F;
    let diff: u8 = match (c, h) {
        (false, false) if hi <= 9 && lo <= 9 => 0x00,
        (false, true) if hi <= 9 && lo <= 9 => 0x06,
        (false, _) if hi <= 8 && lo >= 10 => 0x06,
        (false, false) if hi >= 10 && lo <= 9 => 0x60,
        (true, false) if lo <= 9 => 0x60,
        (true, true) if lo <= 9 => 0x66,
        (true, _) => 0x66,
        (false, _) if hi >= 9 && lo >= 10 => 0x66,
        (false, true) if hi >= 10 && lo <= 9 => 0x66,
        _ => unreachable!(),
    };
    let c_out = c || (hi >= 9 && lo >= 10) || (hi >= 10 && lo <= 9);
    let h_out = if n { h && lo <= 5 } else { lo >= 10 };
    let r = if n {
        a.wrapping_sub(diff)
    } else {
        a.wrapping_add(diff)
    };
    let mut fo = r & (flags::SIGN | XY);
    if r == 0 {
        fo |= flags::ZERO;
    }
    if r.count_ones() % 2 == 0 {
        fo |= flags::PARITY;
    }
    if n {
        fo |= flags::ADD_SUB;
    }
    if h_out {
        fo |= flags::HALF_CARRY;
    }
    if c_out {
        fo |= flags::CARRY;
    }
    (r, fo)
}

#[test]
fn daa_exhaustive_against_reference_table() {
    for a in 0..=255u8 {
        for nhc in 0..8u8 {
            let mut f = 0;
            if nhc & 1 != 0 {
                f |= flags::CARRY;
            }
            if nhc & 2 != 0 {
                f |= flags::HALF_CARRY;
            }
            if nhc & 4 != 0 {
                f |= flags::ADD_SUB;
            }
            let mut c = create_z80(&[0x27]);
            c.a = a;
            c.f = f;
            c.step();
            let (ra, rf) = ref_daa(a, f);
            assert_eq!((c.a, c.f), (ra, rf), "DAA A={a:02X} F={f:02X}");
        }
    }
}

#[test]
fn daa_after_subtraction_uses_a_range() {
    let mut c = create_z80(&[0x27]);
    c.a = 0x9A;
    c.f = flags::ADD_SUB;
    c.step();
    assert_eq!(c.a, 0x34);
    assert!(c.get_flag(flags::CARRY));
}

// ============ CP / ADD IX X/Y ============

#[test]
fn cp_copies_xy_from_operand() {
    let mut c = create_z80(&[0xFE, 0x28]);
    c.a = 0;
    c.step();
    assert_eq!(c.f & XY, 0x28);
    let mut c = create_z80(&[0xB8]); // CP B
    c.a = 0x28;
    c.b = 0x00;
    c.step();
    assert_eq!(c.f & XY, 0x00);
    let mut c = create_z80(&[0xDD, 0xBE, 0x00]); // CP (IX+0)
    c.ix = 0x4000;
    c.memory.write_byte(0x4000, 0x08);
    c.a = 0xFF;
    c.step();
    assert_eq!(c.f & XY, 0x08);
}

proptest! {
    #[test]
    fn prop_cp_flags(a in any::<u8>(), v in any::<u8>()) {
        let mut c = create_z80(&[0xFE, v]);
        c.a = a;
        c.step();
        let r = a.wrapping_sub(v);
        prop_assert_eq!(c.a, a);
        prop_assert_eq!(c.f & XY, v & XY);
        prop_assert_eq!(c.get_flag(flags::ZERO), r == 0);
        prop_assert_eq!(c.get_flag(flags::SIGN), r & 0x80 != 0);
        prop_assert_eq!(c.get_flag(flags::CARRY), v > a);
        prop_assert_eq!(c.get_flag(flags::HALF_CARRY), (v & 0xF) > (a & 0xF));
        prop_assert!(c.get_flag(flags::ADD_SUB));
        prop_assert_eq!(c.get_flag(flags::PARITY), ((a ^ v) & (a ^ r) & 0x80) != 0);
    }

    #[test]
    fn prop_add_ix_xy_from_result_high_byte(ix in any::<u16>(), bc in any::<u16>(), f_in in any::<u8>()) {
        let mut c = create_z80(&[0xDD, 0x09]);
        c.ix = ix;
        c.set_bc(bc);
        c.f = f_in;
        c.step();
        let r = ix.wrapping_add(bc);
        prop_assert_eq!(c.ix, r);
        prop_assert_eq!(c.f & XY, ((r >> 8) as u8) & XY);
        prop_assert_eq!(c.f & (flags::SIGN | flags::ZERO | flags::PARITY), f_in & (flags::SIGN | flags::ZERO | flags::PARITY));
        prop_assert_eq!(c.get_flag(flags::CARRY), ix as u32 + bc as u32 > 0xFFFF);
        prop_assert_eq!(c.get_flag(flags::HALF_CARRY), (ix & 0xFFF) + (bc & 0xFFF) > 0xFFF);
        prop_assert_eq!(c.memptr, ix.wrapping_add(1));
    }
}

// ============ Block I/O flags ============

/// Reference flags for a non-repeating INI/IND/OUTI/OUTD.
fn ref_block_io(b_new: u8, val: u8, k: u8) -> u8 {
    let sum = val as u16 + k as u16;
    let mut f = b_new & (flags::SIGN | XY);
    if b_new == 0 {
        f |= flags::ZERO;
    }
    if val & 0x80 != 0 {
        f |= flags::ADD_SUB;
    }
    if sum > 0xFF {
        f |= flags::HALF_CARRY | flags::CARRY;
    }
    if ((((sum as u8) & 7) ^ b_new).count_ones()).is_multiple_of(2) {
        f |= flags::PARITY;
    }
    f
}

proptest! {
    #[test]
    fn prop_ini_ind_flags(b in any::<u8>(), cc in any::<u8>(), val in any::<u8>(), dec in any::<bool>()) {
        let mut c = create_z80(&[0xED, if dec { 0xAA } else { 0xA2 }]);
        c.b = b;
        c.c = cc;
        c.set_hl(0x4000);
        c.io.ports.insert(((b as u16) << 8) | cc as u16, val);
        c.step();
        let k = if dec { cc.wrapping_sub(1) } else { cc.wrapping_add(1) };
        prop_assert_eq!(c.memory.read_byte(0x4000), val);
        prop_assert_eq!(c.f, ref_block_io(b.wrapping_sub(1), val, k));
    }

    #[test]
    fn prop_outi_outd_flags(b in any::<u8>(), l in any::<u8>(), val in any::<u8>(), dec in any::<bool>()) {
        let mut c = create_z80(&[0xED, if dec { 0xAB } else { 0xA3 }]);
        c.b = b;
        c.h = 0x40;
        c.l = l;
        c.memory.write_byte(0x4000 | l as u32, val);
        c.step();
        let l_new = if dec { l.wrapping_sub(1) } else { l.wrapping_add(1) };
        prop_assert_eq!(c.f, ref_block_io(b.wrapping_sub(1), val, l_new));
    }
}

#[test]
fn inir_repeat_takes_xy_from_pc_and_adjusts_pv_h() {
    // INIR at 0x2800, B=2 -> B=1 and repeats; X/Y from PC high byte (0x28)
    let mut c = at(&[0xED, 0xB2], 0x2800);
    c.set_bc(0x0201);
    c.set_hl(0x4000);
    c.io.ports.insert(0x0201, 0xFF);
    let t = c.step();
    assert_eq!(t, 21);
    assert_eq!(c.pc, 0x2800);
    assert_eq!(c.memptr, 0x2801);
    assert_eq!(c.f & XY, 0x28);
    /* val=0xFF, k=0x02: carry; N set; B=1 -> H=(1&0xF)==0 false;
    P = parity((0x01^1)&7=0 -> even) ^ !parity((B-1)&7=0 -> even) = 1 ^ 0. */
    assert!(c.get_flag(flags::CARRY));
    assert!(!c.get_flag(flags::HALF_CARRY));
    assert!(c.get_flag(flags::ADD_SUB));
    assert!(c.get_flag(flags::PARITY));
}

#[test]
fn ldir_repeat_takes_xy_from_pc() {
    let mut c = at(&[0xED, 0xB0], 0x0800);
    c.set_bc(2);
    c.set_hl(0x4000);
    c.set_de(0x5000);
    c.a = 0;
    c.step();
    assert_eq!(c.f & XY, 0x08);
    assert_eq!(c.memptr, 0x0801);
}

// ============ SCF/CCF and Q ============

#[test]
fn scf_after_flag_writing_instruction_uses_a_only() {
    // XOR A sets Q=F; SCF then takes X/Y from A | (F^Q) = A
    let mut c = create_z80(&[0xAF, 0x3E, 0x00, 0x37]);
    c.step(); // XOR A: F = Z|P
    c.step(); // LD A,0: does not write flags, Q=0
    c.f |= XY; // flags hold X/Y that the last instruction did not write
    c.step(); // SCF: (A | (F ^ 0)) & 0x28 = 0x28
    assert_eq!(c.f & XY, XY);
    assert!(c.get_flag(flags::CARRY));
}

#[test]
fn scf_immediately_after_flag_write_ignores_old_f() {
    let mut c = create_z80(&[0xB7, 0x37]); // OR A ; SCF
    c.a = 0x28;
    c.step(); // F = 0x28 | parity etc, Q = F
    assert_eq!(c.q, c.f);
    c.a = 0x00; // A no longer has the bits
    c.step(); // (0 | (F ^ Q)) & 0x28 = 0
    assert_eq!(c.f & XY, 0);
}

#[test]
fn q_is_cleared_by_non_flag_instructions() {
    let mut c = create_z80(&[0xB7, 0x00]);
    c.step();
    assert_eq!(c.q, c.f);
    c.step();
    assert_eq!(c.q, 0);
}

proptest! {
    #[test]
    fn prop_ccf_flags(a in any::<u8>(), f_in in any::<u8>(), q in any::<u8>()) {
        let mut c = create_z80(&[0x3F]);
        c.a = a;
        c.f = f_in;
        c.q = q;
        c.step();
        let carry = f_in & flags::CARRY != 0;
        prop_assert_eq!(c.get_flag(flags::CARRY), !carry);
        prop_assert_eq!(c.get_flag(flags::HALF_CARRY), carry);
        prop_assert!(!c.get_flag(flags::ADD_SUB));
        prop_assert_eq!(c.f & XY, (a | (f_in ^ q)) & XY);
        prop_assert_eq!(c.f & (flags::SIGN | flags::ZERO | flags::PARITY), f_in & (flags::SIGN | flags::ZERO | flags::PARITY));
        prop_assert_eq!(c.q, c.f);
    }
}

// ============ Interrupts ============

#[test]
fn im2_acknowledge_takes_19_t_states_and_sets_memptr() {
    let mut c = create_z80(&[0x00]);
    c.im = 2;
    c.i = 0x40;
    c.iff1 = true;
    c.sp = 0x8000;
    c.write_word(0x40FE, 0x1234);
    let t = c.trigger_interrupt(0xFE);
    assert_eq!(t, 19);
    assert_eq!(c.pc, 0x1234);
    assert_eq!(c.memptr, 0x1234);
    assert_eq!(c.cycles, 19);
}

#[test]
fn reti_copies_iff2_to_iff1() {
    let mut c = create_z80(&[0xED, 0x4D]);
    c.sp = 0x8000;
    c.iff1 = false;
    c.iff2 = true;
    c.step();
    assert!(c.iff1);
}

#[test]
fn int_line_is_sampled_while_asserted() {
    // DI; NOP; EI; NOP — the line is asserted before EI and still held after
    let mut c = create_z80(&[0xF3, 0x00, 0xFB, 0x00, 0x00]);
    c.im = 1;
    c.sp = 0x8000;
    c.assert_int_for(228, 0xFF);
    c.step(); // DI
    c.step(); // NOP
    c.step(); // EI (shadow)
    assert_eq!(c.pc, 3);
    c.step(); // NOP executes in the EI shadow
    assert_eq!(c.pc, 4);
    let t = c.step(); // interrupt accepted now
    assert_eq!(t, 13);
    assert_eq!(c.pc, 0x0038);
    assert!(!c.iff1);
}

#[test]
fn int_line_released_before_ei_is_not_taken() {
    let mut c = create_z80(&[0xFB, 0x00, 0x00]);
    c.im = 1;
    c.assert_int_for(4, 0xFF);
    c.step(); // EI: 4 T-states drain the line
    c.step();
    c.step();
    assert_eq!(c.pc, 3);
}

#[test]
fn interrupt_wakes_halt_and_charges_cycles() {
    let mut c = create_z80(&[0x76]);
    c.im = 1;
    c.iff1 = true;
    c.sp = 0x8000;
    c.step();
    assert!(c.halted);
    let before = c.cycles;
    c.assert_int_for(228, 0xFF);
    assert_eq!(c.step(), 13);
    assert!(!c.halted);
    assert_eq!(c.read_word(0x7FFE), 0x0001);
    assert_eq!(c.cycles, before + 13);
}

#[test]
fn int_line_expires_while_cpu_is_not_running() {
    let mut c = create_z80(&[0x00]);
    c.im = 1;
    c.iff1 = true;
    c.assert_int_for(228, 0xFF);
    c.advance_int_line(300); // bus held by the 68000 for the whole pulse
    c.step();
    assert_eq!(c.pc, 1);
}

#[test]
fn invalid_interrupt_mode_is_rejected() {
    use crate::debugger::Debuggable;
    let mut c = create_z80(&[0x00]);
    c.im = 1;
    c.write_state(&serde_json::json!({ "im": 3 }));
    assert_eq!(c.im, 1);
    c.write_state(&serde_json::json!({ "im": 2 }));
    assert_eq!(c.im, 2);

    let mut state = serde_json::to_value(&c).unwrap();
    state["im"] = serde_json::json!(7);
    let restored: Result<Z80<DefaultMem, crate::z80::test_utils::TestIo>, _> =
        serde_json::from_value(state.clone());
    assert!(restored.is_err());
    state["im"] = serde_json::json!(2);
    let restored: Z80<DefaultMem, crate::z80::test_utils::TestIo> =
        serde_json::from_value(state).unwrap();
    assert_eq!(restored.im, 2);
}

/// Memory with a `Default` impl, needed to deserialize a CPU.
#[derive(Debug, Default)]
struct DefaultMem;

impl MemoryInterface for DefaultMem {
    fn read_byte(&mut self, _: u32) -> u8 {
        0
    }
    fn write_byte(&mut self, _: u32, _: u8) {}
    fn read_word(&mut self, _: u32) -> u16 {
        0
    }
    fn write_word(&mut self, _: u32, _: u16) {}
    fn read_long(&mut self, _: u32) -> u32 {
        0
    }
    fn write_long(&mut self, _: u32, _: u32) {}
}
