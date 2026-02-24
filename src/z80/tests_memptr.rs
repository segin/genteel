use super::*;
use crate::z80::test_utils::create_z80;

#[test]
fn test_memptr_ld_bc_nn() {
    let (mut z80, mut bus) = create_z80(&[0x01, 0x34, 0x12]); // LD BC, 0x1234
    z80.step(&mut bus);
    // For LD rp, nn, MEMPTR is NOT updated.
    // Wait, some sources say it is?
    // Let's check common Z80 implementations.
}

#[test]
fn test_memptr_ex_sp_hl() {
    let (mut z80, mut bus) = create_z80(&[0xE3]); // EX (SP), HL
    z80.sp = 0xFFFE;
    z80.set_hl(0x1234);
    bus.memory.write_byte(0xFFFE, 0x78);
    bus.memory.write_byte(0xFFFF, 0x56);

    z80.step(&mut bus);

    // After EX (SP), HL, MEMPTR should be the new value of HL (from SP)
    assert_eq!(z80.hl(), 0x5678);
    assert_eq!(z80.memptr, 0x5678);
}
