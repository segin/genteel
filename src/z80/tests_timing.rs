#![allow(unused_imports)]
//! Z80 CPU Cycle Timing Tests
//!
//! Verifies exact T-state counts for all instructions.
//! Accurate timing is critical for Genesis audio synchronization.

use super::*;
use crate::z80::test_utils::create_z80;

// ============ Main opcodes (no prefix) ============

#[test]
fn timing_nop() {
    let (mut c, mut bus) = create_z80(&[0x00]);
    assert_eq!(c.step(&mut bus), 4);
}
#[test]
fn timing_ld_bc_nn() {
    let (mut c, mut bus) = create_z80(&[0x01, 0x00, 0x00]);
    assert_eq!(c.step(&mut bus), 10);
}
#[test]
fn timing_ld_bc_a() {
    let (mut c, mut bus) = create_z80(&[0x02]);
    c.set_bc(0x100);
    assert_eq!(c.step(&mut bus), 7);
}
#[test]
fn timing_inc_bc() {
    let (mut c, mut bus) = create_z80(&[0x03]);
    assert_eq!(c.step(&mut bus), 6);
}
#[test]
fn timing_inc_b() {
    let (mut c, mut bus) = create_z80(&[0x04]);
    assert_eq!(c.step(&mut bus), 4);
}
#[test]
fn timing_dec_b() {
    let (mut c, mut bus) = create_z80(&[0x05]);
    assert_eq!(c.step(&mut bus), 4);
}
#[test]
fn timing_ld_b_n() {
    let (mut c, mut bus) = create_z80(&[0x06, 0x00]);
    assert_eq!(c.step(&mut bus), 7);
}
#[test]
fn timing_rlca() {
    let (mut c, mut bus) = create_z80(&[0x07]);
    assert_eq!(c.step(&mut bus), 4);
}
#[test]
fn timing_ex_af_af() {
    let (mut c, mut bus) = create_z80(&[0x08]);
    assert_eq!(c.step(&mut bus), 4);
}
#[test]
fn timing_add_hl_bc() {
    let (mut c, mut bus) = create_z80(&[0x09]);
    assert_eq!(c.step(&mut bus), 11);
}
#[test]
fn timing_ld_a_bc() {
    let (mut c, mut bus) = create_z80(&[0x0A]);
    c.set_bc(0x100);
    assert_eq!(c.step(&mut bus), 7);
}
#[test]
fn timing_dec_bc() {
    let (mut c, mut bus) = create_z80(&[0x0B]);
    assert_eq!(c.step(&mut bus), 6);
}
#[test]
fn timing_rrca() {
    let (mut c, mut bus) = create_z80(&[0x0F]);
    assert_eq!(c.step(&mut bus), 4);
}

#[test]
fn timing_djnz_taken() {
    let (mut c, mut bus) = create_z80(&[0x10, 0x00]);
    c.b = 2;
    assert_eq!(c.step(&mut bus), 13);
}
#[test]
fn timing_djnz_not_taken() {
    let (mut c, mut bus) = create_z80(&[0x10, 0x00]);
    c.b = 1;
    assert_eq!(c.step(&mut bus), 8);
}
#[test]
fn timing_ld_de_nn() {
    let (mut c, mut bus) = create_z80(&[0x11, 0x00, 0x00]);
    assert_eq!(c.step(&mut bus), 10);
}
#[test]
fn timing_rla() {
    let (mut c, mut bus) = create_z80(&[0x17]);
    assert_eq!(c.step(&mut bus), 4);
}
#[test]
fn timing_jr() {
    let (mut c, mut bus) = create_z80(&[0x18, 0x00]);
    assert_eq!(c.step(&mut bus), 12);
}
#[test]
fn timing_rra() {
    let (mut c, mut bus) = create_z80(&[0x1F]);
    assert_eq!(c.step(&mut bus), 4);
}

#[test]
fn timing_jr_nz_taken() {
    let (mut c, mut bus) = create_z80(&[0x20, 0x00]);
    c.f = 0;
    assert_eq!(c.step(&mut bus), 12);
}
#[test]
fn timing_jr_nz_not_taken() {
    let (mut c, mut bus) = create_z80(&[0x20, 0x00]);
    c.set_flag(flags::ZERO, true);
    assert_eq!(c.step(&mut bus), 7);
}
#[test]
fn timing_jr_z_taken() {
    let (mut c, mut bus) = create_z80(&[0x28, 0x00]);
    c.set_flag(flags::ZERO, true);
    assert_eq!(c.step(&mut bus), 12);
}
#[test]
fn timing_jr_z_not_taken() {
    let (mut c, mut bus) = create_z80(&[0x28, 0x00]);
    c.f = 0;
    assert_eq!(c.step(&mut bus), 7);
}
#[test]
fn timing_jr_nc_taken() {
    let (mut c, mut bus) = create_z80(&[0x30, 0x00]);
    c.f = 0;
    assert_eq!(c.step(&mut bus), 12);
}
#[test]
fn timing_jr_c_taken() {
    let (mut c, mut bus) = create_z80(&[0x38, 0x00]);
    c.set_flag(flags::CARRY, true);
    assert_eq!(c.step(&mut bus), 12);
}

#[test]
fn timing_ld_hl_nn() {
    let (mut c, mut bus) = create_z80(&[0x21, 0x00, 0x00]);
    assert_eq!(c.step(&mut bus), 10);
}
#[test]
fn timing_ld_nn_hl() {
    let (mut c, mut bus) = create_z80(&[0x22, 0x00, 0x10]);
    assert_eq!(c.step(&mut bus), 16);
}
#[test]
fn timing_inc_hl() {
    let (mut c, mut bus) = create_z80(&[0x23]);
    assert_eq!(c.step(&mut bus), 6);
}
#[test]
fn timing_daa() {
    let (mut c, mut bus) = create_z80(&[0x27]);
    assert_eq!(c.step(&mut bus), 4);
}
#[test]
fn timing_ld_hl_nn_ind() {
    let (mut c, mut bus) = create_z80(&[0x2A, 0x00, 0x10]);
    assert_eq!(c.step(&mut bus), 16);
}
#[test]
fn timing_cpl() {
    let (mut c, mut bus) = create_z80(&[0x2F]);
    assert_eq!(c.step(&mut bus), 4);
}

#[test]
fn timing_ld_sp_nn() {
    let (mut c, mut bus) = create_z80(&[0x31, 0x00, 0x00]);
    assert_eq!(c.step(&mut bus), 10);
}
#[test]
fn timing_ld_nn_a() {
    let (mut c, mut bus) = create_z80(&[0x32, 0x00, 0x10]);
    assert_eq!(c.step(&mut bus), 13);
}
#[test]
fn timing_inc_sp() {
    let (mut c, mut bus) = create_z80(&[0x33]);
    assert_eq!(c.step(&mut bus), 6);
}
#[test]
fn timing_inc_hl_ind() {
    let (mut c, mut bus) = create_z80(&[0x34]);
    c.set_hl(0x100);
    assert_eq!(c.step(&mut bus), 11);
}
#[test]
fn timing_dec_hl_ind() {
    let (mut c, mut bus) = create_z80(&[0x35]);
    c.set_hl(0x100);
    assert_eq!(c.step(&mut bus), 11);
}
#[test]
fn timing_ld_hl_n() {
    let (mut c, mut bus) = create_z80(&[0x36, 0x00]);
    c.set_hl(0x100);
    assert_eq!(c.step(&mut bus), 10);
}
#[test]
fn timing_scf() {
    let (mut c, mut bus) = create_z80(&[0x37]);
    assert_eq!(c.step(&mut bus), 4);
}
#[test]
fn timing_ld_a_nn() {
    let (mut c, mut bus) = create_z80(&[0x3A, 0x00, 0x10]);
    assert_eq!(c.step(&mut bus), 13);
}
#[test]
fn timing_ccf() {
    let (mut c, mut bus) = create_z80(&[0x3F]);
    assert_eq!(c.step(&mut bus), 4);
}

// LD r, r' (4 cycles) and LD r, (HL) (7 cycles)
#[test]
fn timing_ld_b_b() {
    let (mut c, mut bus) = create_z80(&[0x40]);
    assert_eq!(c.step(&mut bus), 4);
}
#[test]
fn timing_ld_b_hl() {
    let (mut c, mut bus) = create_z80(&[0x46]);
    c.set_hl(0x100);
    assert_eq!(c.step(&mut bus), 7);
}
#[test]
fn timing_ld_hl_b() {
    let (mut c, mut bus) = create_z80(&[0x70]);
    c.set_hl(0x100);
    assert_eq!(c.step(&mut bus), 7);
}
#[test]
fn timing_halt() {
    let (mut c, mut bus) = create_z80(&[0x76]);
    assert_eq!(c.step(&mut bus), 4);
}

// ALU r (4 cycles) and ALU (HL) (7 cycles)
#[test]
fn timing_add_a_b() {
    let (mut c, mut bus) = create_z80(&[0x80]);
    assert_eq!(c.step(&mut bus), 4);
}
#[test]
fn timing_add_a_hl() {
    let (mut c, mut bus) = create_z80(&[0x86]);
    c.set_hl(0x100);
    assert_eq!(c.step(&mut bus), 7);
}
#[test]
fn timing_sub_b() {
    let (mut c, mut bus) = create_z80(&[0x90]);
    assert_eq!(c.step(&mut bus), 4);
}
#[test]
fn timing_and_b() {
    let (mut c, mut bus) = create_z80(&[0xA0]);
    assert_eq!(c.step(&mut bus), 4);
}
#[test]
fn timing_xor_b() {
    let (mut c, mut bus) = create_z80(&[0xA8]);
    assert_eq!(c.step(&mut bus), 4);
}
#[test]
fn timing_or_b() {
    let (mut c, mut bus) = create_z80(&[0xB0]);
    assert_eq!(c.step(&mut bus), 4);
}
#[test]
fn timing_cp_b() {
    let (mut c, mut bus) = create_z80(&[0xB8]);
    assert_eq!(c.step(&mut bus), 4);
}

// Control flow
#[test]
fn timing_ret_nz_taken() {
    let (mut c, mut bus) = create_z80(&[0xC0]);
    c.sp = 0x100;
    bus.memory.write_byte(0x100 as u32, 0x00);
    bus.memory.write_byte(0x101 as u32, 0x10);
    c.f = 0;
    assert_eq!(c.step(&mut bus), 11);
}
#[test]
fn timing_ret_nz_not_taken() {
    let (mut c, mut bus) = create_z80(&[0xC0]);
    c.set_flag(flags::ZERO, true);
    assert_eq!(c.step(&mut bus), 5);
}
#[test]
fn timing_pop_bc() {
    let (mut c, mut bus) = create_z80(&[0xC1]);
    c.sp = 0x100;
    assert_eq!(c.step(&mut bus), 10);
}
#[test]
fn timing_jp_nn() {
    let (mut c, mut bus) = create_z80(&[0xC3, 0x00, 0x10]);
    assert_eq!(c.step(&mut bus), 10);
}
#[test]
fn timing_jp_nz_taken() {
    let (mut c, mut bus) = create_z80(&[0xC2, 0x00, 0x10]);
    c.f = 0;
    assert_eq!(c.step(&mut bus), 10);
}
#[test]
fn timing_call_nz_taken() {
    let (mut c, mut bus) = create_z80(&[0xC4, 0x00, 0x10]);
    c.sp = 0x200;
    c.f = 0;
    assert_eq!(c.step(&mut bus), 17);
}
#[test]
fn timing_call_nz_not_taken() {
    let (mut c, mut bus) = create_z80(&[0xC4, 0x00, 0x10]);
    c.sp = 0x200;
    c.set_flag(flags::ZERO, true);
    assert_eq!(c.step(&mut bus), 10);
}
#[test]
fn timing_push_bc() {
    let (mut c, mut bus) = create_z80(&[0xC5]);
    c.sp = 0x200;
    assert_eq!(c.step(&mut bus), 11);
}
#[test]
fn timing_add_a_n() {
    let (mut c, mut bus) = create_z80(&[0xC6, 0x00]);
    assert_eq!(c.step(&mut bus), 7);
}
#[test]
fn timing_rst_00() {
    let (mut c, mut bus) = create_z80(&[0xC7]);
    c.sp = 0x200;
    assert_eq!(c.step(&mut bus), 11);
}
#[test]
fn timing_ret() {
    let (mut c, mut bus) = create_z80(&[0xC9]);
    c.sp = 0x100;
    assert_eq!(c.step(&mut bus), 10);
}
#[test]
fn timing_call_nn() {
    let (mut c, mut bus) = create_z80(&[0xCD, 0x00, 0x10]);
    c.sp = 0x200;
    assert_eq!(c.step(&mut bus), 17);
}

#[test]
fn timing_out_n_a() {
    let (mut c, mut bus) = create_z80(&[0xD3, 0x00]);
    assert_eq!(c.step(&mut bus), 11);
}
#[test]
fn timing_exx() {
    let (mut c, mut bus) = create_z80(&[0xD9]);
    assert_eq!(c.step(&mut bus), 4);
}
#[test]
fn timing_in_a_n() {
    let (mut c, mut bus) = create_z80(&[0xDB, 0x00]);
    assert_eq!(c.step(&mut bus), 11);
}

#[test]
fn timing_ex_sp_hl() {
    let (mut c, mut bus) = create_z80(&[0xE3]);
    c.sp = 0x100;
    assert_eq!(c.step(&mut bus), 19);
}
#[test]
fn timing_jp_hl() {
    let (mut c, mut bus) = create_z80(&[0xE9]);
    assert_eq!(c.step(&mut bus), 4);
}
#[test]
fn timing_ex_de_hl() {
    let (mut c, mut bus) = create_z80(&[0xEB]);
    assert_eq!(c.step(&mut bus), 4);
}

#[test]
fn timing_di() {
    let (mut c, mut bus) = create_z80(&[0xF3]);
    assert_eq!(c.step(&mut bus), 4);
}
#[test]
fn timing_ld_sp_hl() {
    let (mut c, mut bus) = create_z80(&[0xF9]);
    assert_eq!(c.step(&mut bus), 6);
}
#[test]
fn timing_ei() {
    let (mut c, mut bus) = create_z80(&[0xFB]);
    assert_eq!(c.step(&mut bus), 4);
}

// ============ CB prefix ============

#[test]
fn timing_cb_rlc_b() {
    let (mut c, mut bus) = create_z80(&[0xCB, 0x00]);
    assert_eq!(c.step(&mut bus), 8);
}
#[test]
fn timing_cb_rlc_hl() {
    let (mut c, mut bus) = create_z80(&[0xCB, 0x06]);
    c.set_hl(0x100);
    assert_eq!(c.step(&mut bus), 15);
}
#[test]
fn timing_cb_bit_0_b() {
    let (mut c, mut bus) = create_z80(&[0xCB, 0x40]);
    assert_eq!(c.step(&mut bus), 8);
}
#[test]
fn timing_cb_bit_0_hl() {
    let (mut c, mut bus) = create_z80(&[0xCB, 0x46]);
    c.set_hl(0x100);
    assert_eq!(c.step(&mut bus), 12);
}
#[test]
fn timing_cb_res_0_b() {
    let (mut c, mut bus) = create_z80(&[0xCB, 0x80]);
    assert_eq!(c.step(&mut bus), 8);
}
#[test]
fn timing_cb_res_0_hl() {
    let (mut c, mut bus) = create_z80(&[0xCB, 0x86]);
    c.set_hl(0x100);
    assert_eq!(c.step(&mut bus), 15);
}
#[test]
fn timing_cb_set_0_b() {
    let (mut c, mut bus) = create_z80(&[0xCB, 0xC0]);
    assert_eq!(c.step(&mut bus), 8);
}
#[test]
fn timing_cb_set_0_hl() {
    let (mut c, mut bus) = create_z80(&[0xCB, 0xC6]);
    c.set_hl(0x100);
    assert_eq!(c.step(&mut bus), 15);
}

// ============ ED prefix ============

#[test]
fn timing_ed_in_b_c() {
    let (mut c, mut bus) = create_z80(&[0xED, 0x40]);
    assert_eq!(c.step(&mut bus), 12);
}
#[test]
fn timing_ed_out_c_b() {
    let (mut c, mut bus) = create_z80(&[0xED, 0x41]);
    assert_eq!(c.step(&mut bus), 12);
}
#[test]
fn timing_ed_sbc_hl_bc() {
    let (mut c, mut bus) = create_z80(&[0xED, 0x42]);
    assert_eq!(c.step(&mut bus), 15);
}
#[test]
fn timing_ed_ld_nn_bc() {
    let (mut c, mut bus) = create_z80(&[0xED, 0x43, 0x00, 0x10]);
    assert_eq!(c.step(&mut bus), 20);
}
#[test]
fn timing_ed_neg() {
    let (mut c, mut bus) = create_z80(&[0xED, 0x44]);
    assert_eq!(c.step(&mut bus), 8);
}
#[test]
fn timing_ed_retn() {
    let (mut c, mut bus) = create_z80(&[0xED, 0x45]);
    c.sp = 0x100;
    assert_eq!(c.step(&mut bus), 14);
}
#[test]
fn timing_ed_im_0() {
    let (mut c, mut bus) = create_z80(&[0xED, 0x46]);
    assert_eq!(c.step(&mut bus), 8);
}
#[test]
fn timing_ed_ld_i_a() {
    let (mut c, mut bus) = create_z80(&[0xED, 0x47]);
    assert_eq!(c.step(&mut bus), 9);
}
#[test]
fn timing_ed_adc_hl_bc() {
    let (mut c, mut bus) = create_z80(&[0xED, 0x4A]);
    assert_eq!(c.step(&mut bus), 15);
}
#[test]
fn timing_ed_ld_bc_nn() {
    let (mut c, mut bus) = create_z80(&[0xED, 0x4B, 0x00, 0x10]);
    assert_eq!(c.step(&mut bus), 20);
}
#[test]
fn timing_ed_reti() {
    let (mut c, mut bus) = create_z80(&[0xED, 0x4D]);
    c.sp = 0x100;
    assert_eq!(c.step(&mut bus), 14);
}
#[test]
fn timing_ed_ld_r_a() {
    let (mut c, mut bus) = create_z80(&[0xED, 0x4F]);
    assert_eq!(c.step(&mut bus), 9);
}
#[test]
fn timing_ed_ld_a_i() {
    let (mut c, mut bus) = create_z80(&[0xED, 0x57]);
    assert_eq!(c.step(&mut bus), 9);
}
#[test]
fn timing_ed_ld_a_r() {
    let (mut c, mut bus) = create_z80(&[0xED, 0x5F]);
    assert_eq!(c.step(&mut bus), 9);
}
#[test]
fn timing_ed_rrd() {
    let (mut c, mut bus) = create_z80(&[0xED, 0x67]);
    c.set_hl(0x100);
    assert_eq!(c.step(&mut bus), 18);
}
#[test]
fn timing_ed_rld() {
    let (mut c, mut bus) = create_z80(&[0xED, 0x6F]);
    c.set_hl(0x100);
    assert_eq!(c.step(&mut bus), 18);
}

#[test]
fn timing_ed_ldi() {
    let (mut c, mut bus) = create_z80(&[0xED, 0xA0]);
    c.set_hl(0x100);
    c.set_de(0x200);
    c.set_bc(1);
    assert_eq!(c.step(&mut bus), 16);
}
#[test]
fn timing_ed_cpi() {
    let (mut c, mut bus) = create_z80(&[0xED, 0xA1]);
    c.set_hl(0x100);
    c.set_bc(1);
    assert_eq!(c.step(&mut bus), 16);
}
#[test]
fn timing_ed_ldd() {
    let (mut c, mut bus) = create_z80(&[0xED, 0xA8]);
    c.set_hl(0x100);
    c.set_de(0x200);
    c.set_bc(1);
    assert_eq!(c.step(&mut bus), 16);
}
#[test]
fn timing_ed_cpd() {
    let (mut c, mut bus) = create_z80(&[0xED, 0xA9]);
    c.set_hl(0x100);
    c.set_bc(1);
    assert_eq!(c.step(&mut bus), 16);
}

#[test]
fn timing_ed_ldir_cont() {
    let (mut c, mut bus) = create_z80(&[0xED, 0xB0]);
    c.set_hl(0x100);
    c.set_de(0x200);
    c.set_bc(2);
    assert_eq!(c.step(&mut bus), 21);
}
#[test]
fn timing_ed_ldir_done() {
    let (mut c, mut bus) = create_z80(&[0xED, 0xB0]);
    c.set_hl(0x100);
    c.set_de(0x200);
    c.set_bc(1);
    assert_eq!(c.step(&mut bus), 16);
}
#[test]
fn timing_ed_lddr_cont() {
    let (mut c, mut bus) = create_z80(&[0xED, 0xB8]);
    c.set_hl(0x100);
    c.set_de(0x200);
    c.set_bc(2);
    assert_eq!(c.step(&mut bus), 21);
}

// ============ DD prefix (IX) ============

#[test]
fn timing_dd_ld_ix_nn() {
    let (mut c, mut bus) = create_z80(&[0xDD, 0x21, 0x00, 0x00]);
    assert_eq!(c.step(&mut bus), 14);
}
#[test]
fn timing_dd_ld_nn_ix() {
    let (mut c, mut bus) = create_z80(&[0xDD, 0x22, 0x00, 0x10]);
    assert_eq!(c.step(&mut bus), 20);
}
#[test]
fn timing_dd_inc_ix() {
    let (mut c, mut bus) = create_z80(&[0xDD, 0x23]);
    assert_eq!(c.step(&mut bus), 10);
}
#[test]
fn timing_dd_ld_ix_nn_ind() {
    let (mut c, mut bus) = create_z80(&[0xDD, 0x2A, 0x00, 0x10]);
    assert_eq!(c.step(&mut bus), 20);
}
#[test]
fn timing_dd_dec_ix() {
    let (mut c, mut bus) = create_z80(&[0xDD, 0x2B]);
    assert_eq!(c.step(&mut bus), 10);
}
#[test]
fn timing_dd_inc_ix_d() {
    let (mut c, mut bus) = create_z80(&[0xDD, 0x34, 0x00]);
    c.ix = 0x100;
    assert_eq!(c.step(&mut bus), 23);
}
#[test]
fn timing_dd_dec_ix_d() {
    let (mut c, mut bus) = create_z80(&[0xDD, 0x35, 0x00]);
    c.ix = 0x100;
    assert_eq!(c.step(&mut bus), 23);
}
#[test]
fn timing_dd_ld_ix_d_n() {
    let (mut c, mut bus) = create_z80(&[0xDD, 0x36, 0x00, 0x00]);
    c.ix = 0x100;
    assert_eq!(c.step(&mut bus), 19);
}
#[test]
fn timing_dd_add_ix_bc() {
    let (mut c, mut bus) = create_z80(&[0xDD, 0x09]);
    assert_eq!(c.step(&mut bus), 15);
}
#[test]
fn timing_dd_ld_b_ix_d() {
    let (mut c, mut bus) = create_z80(&[0xDD, 0x46, 0x00]);
    c.ix = 0x100;
    assert_eq!(c.step(&mut bus), 19);
}
#[test]
fn timing_dd_ld_ix_d_b() {
    let (mut c, mut bus) = create_z80(&[0xDD, 0x70, 0x00]);
    c.ix = 0x100;
    assert_eq!(c.step(&mut bus), 19);
}
#[test]
fn timing_dd_add_a_ix_d() {
    let (mut c, mut bus) = create_z80(&[0xDD, 0x86, 0x00]);
    c.ix = 0x100;
    assert_eq!(c.step(&mut bus), 19);
}
#[test]
fn timing_dd_pop_ix() {
    let (mut c, mut bus) = create_z80(&[0xDD, 0xE1]);
    c.sp = 0x100;
    assert_eq!(c.step(&mut bus), 14);
}
#[test]
fn timing_dd_ex_sp_ix() {
    let (mut c, mut bus) = create_z80(&[0xDD, 0xE3]);
    c.sp = 0x100;
    assert_eq!(c.step(&mut bus), 23);
}
#[test]
fn timing_dd_push_ix() {
    let (mut c, mut bus) = create_z80(&[0xDD, 0xE5]);
    c.sp = 0x200;
    assert_eq!(c.step(&mut bus), 15);
}
#[test]
fn timing_dd_jp_ix() {
    let (mut c, mut bus) = create_z80(&[0xDD, 0xE9]);
    assert_eq!(c.step(&mut bus), 8);
}
#[test]
fn timing_dd_ld_sp_ix() {
    let (mut c, mut bus) = create_z80(&[0xDD, 0xF9]);
    assert_eq!(c.step(&mut bus), 10);
}

// ============ DD CB prefix ============

#[test]
fn timing_ddcb_rlc() {
    let (mut c, mut bus) = create_z80(&[0xDD, 0xCB, 0x00, 0x06]);
    c.ix = 0x100;
    assert_eq!(c.step(&mut bus), 23);
}
#[test]
fn timing_ddcb_bit() {
    let (mut c, mut bus) = create_z80(&[0xDD, 0xCB, 0x00, 0x46]);
    c.ix = 0x100;
    assert_eq!(c.step(&mut bus), 20);
}
#[test]
fn timing_ddcb_res() {
    let (mut c, mut bus) = create_z80(&[0xDD, 0xCB, 0x00, 0x86]);
    c.ix = 0x100;
    assert_eq!(c.step(&mut bus), 23);
}
#[test]
fn timing_ddcb_set() {
    let (mut c, mut bus) = create_z80(&[0xDD, 0xCB, 0x00, 0xC6]);
    c.ix = 0x100;
    assert_eq!(c.step(&mut bus), 23);
}

// ============ FD prefix (IY) ============

#[test]
fn timing_fd_ld_iy_nn() {
    let (mut c, mut bus) = create_z80(&[0xFD, 0x21, 0x00, 0x00]);
    assert_eq!(c.step(&mut bus), 14);
}
#[test]
fn timing_fd_ld_nn_iy() {
    let (mut c, mut bus) = create_z80(&[0xFD, 0x22, 0x00, 0x10]);
    assert_eq!(c.step(&mut bus), 20);
}
#[test]
fn timing_fd_inc_iy() {
    let (mut c, mut bus) = create_z80(&[0xFD, 0x23]);
    assert_eq!(c.step(&mut bus), 10);
}
#[test]
fn timing_fd_ld_iy_nn_ind() {
    let (mut c, mut bus) = create_z80(&[0xFD, 0x2A, 0x00, 0x10]);
    assert_eq!(c.step(&mut bus), 20);
}
#[test]
fn timing_fd_dec_iy() {
    let (mut c, mut bus) = create_z80(&[0xFD, 0x2B]);
    assert_eq!(c.step(&mut bus), 10);
}
#[test]
fn timing_fd_inc_iy_d() {
    let (mut c, mut bus) = create_z80(&[0xFD, 0x34, 0x00]);
    c.iy = 0x100;
    assert_eq!(c.step(&mut bus), 23);
}
#[test]
fn timing_fd_dec_iy_d() {
    let (mut c, mut bus) = create_z80(&[0xFD, 0x35, 0x00]);
    c.iy = 0x100;
    assert_eq!(c.step(&mut bus), 23);
}
#[test]
fn timing_fd_ld_iy_d_n() {
    let (mut c, mut bus) = create_z80(&[0xFD, 0x36, 0x00, 0x00]);
    c.iy = 0x100;
    assert_eq!(c.step(&mut bus), 19);
}
#[test]
fn timing_fd_add_iy_bc() {
    let (mut c, mut bus) = create_z80(&[0xFD, 0x09]);
    assert_eq!(c.step(&mut bus), 15);
}
#[test]
fn timing_fd_ld_b_iy_d() {
    let (mut c, mut bus) = create_z80(&[0xFD, 0x46, 0x00]);
    c.iy = 0x100;
    assert_eq!(c.step(&mut bus), 19);
}
#[test]
fn timing_fd_ld_iy_d_b() {
    let (mut c, mut bus) = create_z80(&[0xFD, 0x70, 0x00]);
    c.iy = 0x100;
    assert_eq!(c.step(&mut bus), 19);
}
#[test]
fn timing_fd_add_a_iy_d() {
    let (mut c, mut bus) = create_z80(&[0xFD, 0x86, 0x00]);
    c.iy = 0x100;
    assert_eq!(c.step(&mut bus), 19);
}
#[test]
fn timing_fd_pop_iy() {
    let (mut c, mut bus) = create_z80(&[0xFD, 0xE1]);
    c.sp = 0x100;
    assert_eq!(c.step(&mut bus), 14);
}
#[test]
fn timing_fd_ex_sp_iy() {
    let (mut c, mut bus) = create_z80(&[0xFD, 0xE3]);
    c.sp = 0x100;
    assert_eq!(c.step(&mut bus), 23);
}
#[test]
fn timing_fd_push_iy() {
    let (mut c, mut bus) = create_z80(&[0xFD, 0xE5]);
    c.sp = 0x200;
    assert_eq!(c.step(&mut bus), 15);
}
#[test]
fn timing_fd_jp_iy() {
    let (mut c, mut bus) = create_z80(&[0xFD, 0xE9]);
    assert_eq!(c.step(&mut bus), 8);
}
#[test]
fn timing_fd_ld_sp_iy() {
    let (mut c, mut bus) = create_z80(&[0xFD, 0xF9]);
    assert_eq!(c.step(&mut bus), 10);
}

// ============ FD CB prefix ============

#[test]
fn timing_fdcb_rlc() {
    let (mut c, mut bus) = create_z80(&[0xFD, 0xCB, 0x00, 0x06]);
    c.iy = 0x100;
    assert_eq!(c.step(&mut bus), 23);
}
#[test]
fn timing_fdcb_bit() {
    let (mut c, mut bus) = create_z80(&[0xFD, 0xCB, 0x00, 0x46]);
    c.iy = 0x100;
    assert_eq!(c.step(&mut bus), 20);
}
#[test]
fn timing_fdcb_res() {
    let (mut c, mut bus) = create_z80(&[0xFD, 0xCB, 0x00, 0x86]);
    c.iy = 0x100;
    assert_eq!(c.step(&mut bus), 23);
}
#[test]
fn timing_fdcb_set() {
    let (mut c, mut bus) = create_z80(&[0xFD, 0xCB, 0x00, 0xC6]);
    c.iy = 0x100;
    assert_eq!(c.step(&mut bus), 23);
}
