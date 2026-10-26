use super::*;
use crate::z80::test_utils::create_z80;

#[test]
fn test_memptr_ld_bc_nn() {
    let mut z80 = create_z80(&[0x01, 0x34, 0x12]); // LD BC, 0x1234
    z80.memptr = 0xBEEF;
    z80.step();
    assert_eq!(z80.bc(), 0x1234);
    /* LD rp,nn reads an immediate, not memory through an address, so
    MEMPTR is left alone (unlike LD rp,(nn)). */
    assert_eq!(z80.memptr, 0xBEEF);
}

#[test]
fn test_memptr_ex_sp_hl() {
    let mut z80 = create_z80(&[0xE3]); // EX (SP), HL
    z80.sp = 0xFFFE;
    z80.set_hl(0x1234);
    z80.memory.write_byte(0xFFFE, 0x78);
    z80.memory.write_byte(0xFFFF, 0x56);

    z80.step();

    // After EX (SP), HL, MEMPTR should be the new value of HL (from SP)
    assert_eq!(z80.hl(), 0x5678);
    assert_eq!(z80.memptr, 0x5678);
}
