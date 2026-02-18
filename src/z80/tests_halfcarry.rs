#![allow(unused_imports)]
//! Z80 Half-Carry Flag Tests
//!
//! The Half-Carry (H) flag indicates carry from bit 3 to bit 4.
//! It's critical for DAA and often implemented incorrectly.

use super::*;
use crate::z80::test_utils::create_z80;

// ============ ADD: Half-carry when low nibble overflows ============

#[test]
fn add_h_0f_01() {
    let mut c = create_z80(&[0x80]);
    c.a = 0x0F;
    c.b = 0x01;
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn add_h_0e_01() {
    let mut c = create_z80(&[0x80]);
    c.a = 0x0E;
    c.b = 0x01;
    c.step();
    assert!(!c.get_flag(flags::HALF_CARRY));
}
#[test]
fn add_h_0f_0f() {
    let mut c = create_z80(&[0x80]);
    c.a = 0x0F;
    c.b = 0x0F;
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn add_h_08_08() {
    let mut c = create_z80(&[0x80]);
    c.a = 0x08;
    c.b = 0x08;
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn add_h_07_08() {
    let mut c = create_z80(&[0x80]);
    c.a = 0x07;
    c.b = 0x08;
    c.step();
    assert!(!c.get_flag(flags::HALF_CARRY));
}
#[test]
fn add_h_00_00() {
    let mut c = create_z80(&[0x80]);
    c.a = 0x00;
    c.b = 0x00;
    c.step();
    assert!(!c.get_flag(flags::HALF_CARRY));
}
#[test]
fn add_h_f0_10() {
    let mut c = create_z80(&[0x80]);
    c.a = 0xF0;
    c.b = 0x10;
    c.step();
    assert!(!c.get_flag(flags::HALF_CARRY));
} // High nibble carry, not half
#[test]
fn add_h_1f_01() {
    let mut c = create_z80(&[0x80]);
    c.a = 0x1F;
    c.b = 0x01;
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}

// ============ ADC: Half-carry with carry input ============

#[test]
fn adc_h_0e_00_c() {
    let mut c = create_z80(&[0x88]);
    c.a = 0x0E;
    c.b = 0x00;
    c.set_flag(flags::CARRY, true);
    c.step();
    assert!(!c.get_flag(flags::HALF_CARRY));
}
#[test]
fn adc_h_0e_01_c() {
    let mut c = create_z80(&[0x88]);
    c.a = 0x0E;
    c.b = 0x01;
    c.set_flag(flags::CARRY, true);
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn adc_h_0f_00_c() {
    let mut c = create_z80(&[0x88]);
    c.a = 0x0F;
    c.b = 0x00;
    c.set_flag(flags::CARRY, true);
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}

// ============ SUB: Half-borrow when low nibble underflows ============

#[test]
fn sub_h_10_01() {
    let mut c = create_z80(&[0x90]);
    c.a = 0x10;
    c.b = 0x01;
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn sub_h_11_01() {
    let mut c = create_z80(&[0x90]);
    c.a = 0x11;
    c.b = 0x01;
    c.step();
    assert!(!c.get_flag(flags::HALF_CARRY));
}
#[test]
fn sub_h_00_01() {
    let mut c = create_z80(&[0x90]);
    c.a = 0x00;
    c.b = 0x01;
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn sub_h_20_0f() {
    let mut c = create_z80(&[0x90]);
    c.a = 0x20;
    c.b = 0x0F;
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn sub_h_2f_0f() {
    let mut c = create_z80(&[0x90]);
    c.a = 0x2F;
    c.b = 0x0F;
    c.step();
    assert!(!c.get_flag(flags::HALF_CARRY));
}

// ============ SBC: Half-borrow with carry input ============

#[test]
fn sbc_h_11_00_c() {
    let mut c = create_z80(&[0x98]);
    c.a = 0x11;
    c.b = 0x00;
    c.set_flag(flags::CARRY, true);
    c.step();
    assert!(!c.get_flag(flags::HALF_CARRY));
}
#[test]
fn sbc_h_10_00_c() {
    let mut c = create_z80(&[0x98]);
    c.a = 0x10;
    c.b = 0x00;
    c.set_flag(flags::CARRY, true);
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn sbc_h_11_01_c() {
    let mut c = create_z80(&[0x98]);
    c.a = 0x11;
    c.b = 0x01;
    c.set_flag(flags::CARRY, true);
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}

// ============ INC: Half-carry at 0xnF -> 0xn0 ============

#[test]
fn inc_h_0f() {
    let mut c = create_z80(&[0x3C]);
    c.a = 0x0F;
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn inc_h_1f() {
    let mut c = create_z80(&[0x3C]);
    c.a = 0x1F;
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn inc_h_ff() {
    let mut c = create_z80(&[0x3C]);
    c.a = 0xFF;
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn inc_h_0e() {
    let mut c = create_z80(&[0x3C]);
    c.a = 0x0E;
    c.step();
    assert!(!c.get_flag(flags::HALF_CARRY));
}
#[test]
fn inc_h_00() {
    let mut c = create_z80(&[0x3C]);
    c.a = 0x00;
    c.step();
    assert!(!c.get_flag(flags::HALF_CARRY));
}

// ============ DEC: Half-borrow at 0xn0 -> 0xnF ============

#[test]
fn dec_h_10() {
    let mut c = create_z80(&[0x3D]);
    c.a = 0x10;
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn dec_h_20() {
    let mut c = create_z80(&[0x3D]);
    c.a = 0x20;
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn dec_h_00() {
    let mut c = create_z80(&[0x3D]);
    c.a = 0x00;
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn dec_h_11() {
    let mut c = create_z80(&[0x3D]);
    c.a = 0x11;
    c.step();
    assert!(!c.get_flag(flags::HALF_CARRY));
}
#[test]
fn dec_h_01() {
    let mut c = create_z80(&[0x3D]);
    c.a = 0x01;
    c.step();
    assert!(!c.get_flag(flags::HALF_CARRY));
}

// ============ CP: Same as SUB for flags ============

#[test]
fn cp_h_10_01() {
    let mut c = create_z80(&[0xB8]);
    c.a = 0x10;
    c.b = 0x01;
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn cp_h_11_01() {
    let mut c = create_z80(&[0xB8]);
    c.a = 0x11;
    c.b = 0x01;
    c.step();
    assert!(!c.get_flag(flags::HALF_CARRY));
}

// ============ AND: Always sets H ============

#[test]
fn and_h_always_set_1() {
    let mut c = create_z80(&[0xA0]);
    c.a = 0xFF;
    c.b = 0xFF;
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn and_h_always_set_2() {
    let mut c = create_z80(&[0xA0]);
    c.a = 0x00;
    c.b = 0x00;
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn and_h_always_set_3() {
    let mut c = create_z80(&[0xA0]);
    c.a = 0xAA;
    c.b = 0x55;
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}

// ============ OR: Always clears H ============

#[test]
fn or_h_always_clear_1() {
    let mut c = create_z80(&[0xB0]);
    c.a = 0xFF;
    c.b = 0xFF;
    c.step();
    assert!(!c.get_flag(flags::HALF_CARRY));
}
#[test]
fn or_h_always_clear_2() {
    let mut c = create_z80(&[0xB0]);
    c.a = 0x00;
    c.b = 0x00;
    c.step();
    assert!(!c.get_flag(flags::HALF_CARRY));
}

// ============ XOR: Always clears H ============

#[test]
fn xor_h_always_clear_1() {
    let mut c = create_z80(&[0xA8]);
    c.a = 0xFF;
    c.b = 0xFF;
    c.step();
    assert!(!c.get_flag(flags::HALF_CARRY));
}
#[test]
fn xor_h_always_clear_2() {
    let mut c = create_z80(&[0xA8]);
    c.a = 0x00;
    c.b = 0x00;
    c.step();
    assert!(!c.get_flag(flags::HALF_CARRY));
}

// ============ ADD HL, rr: Half-carry from bit 11 ============

#[test]
fn add_hl_h_0fff_0001() {
    let mut c = create_z80(&[0x09]);
    c.set_hl(0x0FFF);
    c.set_bc(0x0001);
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn add_hl_h_0ffe_0001() {
    let mut c = create_z80(&[0x09]);
    c.set_hl(0x0FFE);
    c.set_bc(0x0001);
    c.step();
    assert!(!c.get_flag(flags::HALF_CARRY));
}
#[test]
fn add_hl_h_1fff_0001() {
    let mut c = create_z80(&[0x09]);
    c.set_hl(0x1FFF);
    c.set_bc(0x0001);
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn add_hl_h_0800_0800() {
    let mut c = create_z80(&[0x09]);
    c.set_hl(0x0800);
    c.set_bc(0x0800);
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn add_hl_h_0700_0800() {
    let mut c = create_z80(&[0x09]);
    c.set_hl(0x0700);
    c.set_bc(0x0800);
    c.step();
    assert!(!c.get_flag(flags::HALF_CARRY));
}

// ============ ADC/SBC HL, rr: 16-bit half-carry ============

#[test]
fn adc_hl_h_0fff_0000_c() {
    let mut c = create_z80(&[0xED, 0x4A]);
    c.set_hl(0x0FFF);
    c.set_bc(0x0000);
    c.set_flag(flags::CARRY, true);
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn sbc_hl_h_1000_0001() {
    let mut c = create_z80(&[0xED, 0x42]);
    c.set_hl(0x1000);
    c.set_bc(0x0001);
    c.f = 0;
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}
