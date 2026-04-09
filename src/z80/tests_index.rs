use super::*;
use crate::z80::test_utils::create_z80;

#[test]
fn test_index_add_16() {
    // DD 09 - ADD IX, BC
    let mut cpu = create_z80(&[0xDD, 0x09]);
    cpu.ix = 0x1000;
    cpu.set_bc(0x2345);
    cpu.step();
    assert_eq!(cpu.ix, 0x3345);

    // FD 19 - ADD IY, DE
    let mut cpu = create_z80(&[0xFD, 0x19]);
    cpu.iy = 0x1000;
    cpu.set_de(0x2345);
    cpu.step();
    assert_eq!(cpu.iy, 0x3345);
}

#[test]
fn test_index_load_store_16() {
    // DD 21 nn nn - LD IX, nn
    let mut cpu = create_z80(&[0xDD, 0x21, 0x34, 0x12]);
    cpu.step();
    assert_eq!(cpu.ix, 0x1234);

    // FD 22 nn nn - LD (nn), IY
    let mut cpu = create_z80(&[0xFD, 0x22, 0x00, 0x20]);
    cpu.iy = 0xABCD;
    cpu.step();
    assert_eq!(cpu.read_word(0x2000), 0xABCD);

    // DD 2A nn nn - LD IX, (nn)
    let mut cpu = create_z80(&[0xDD, 0x2A, 0x00, 0x20]);
    cpu.write_word(0x2000, 0x5678);
    cpu.step();
    assert_eq!(cpu.ix, 0x5678);
}

#[test]
fn test_index_inc_dec_16() {
    // DD 23 - INC IX
    let mut cpu = create_z80(&[0xDD, 0x23]);
    cpu.ix = 0x1000;
    cpu.step();
    assert_eq!(cpu.ix, 0x1001);

    // FD 2B - DEC IY
    let mut cpu = create_z80(&[0xFD, 0x2B]);
    cpu.iy = 0x1000;
    cpu.step();
    assert_eq!(cpu.iy, 0x0FFF);
}

#[test]
fn test_index_8bit_halves() {
    // DD 24 - INC IXh
    let mut cpu = create_z80(&[0xDD, 0x24]);
    cpu.set_ixh(0x10);
    cpu.step();
    assert_eq!(cpu.ixh(), 0x11);

    // FD 25 - DEC IYh
    let mut cpu = create_z80(&[0xFD, 0x25]);
    cpu.set_iyh(0x10);
    cpu.step();
    assert_eq!(cpu.iyh(), 0x0F);

    // DD 2E n - LD IXl, n
    let mut cpu = create_z80(&[0xDD, 0x2E, 0x42]);
    cpu.step();
    assert_eq!(cpu.ixl(), 0x42);
}

#[test]
fn test_index_mem_8bit() {
    // DD 34 d - INC (IX+d)
    let mut cpu = create_z80(&[0xDD, 0x34, 0x05]);
    cpu.ix = 0x1000;
    cpu.write_byte(0x1005, 0x10);
    cpu.step();
    assert_eq!(cpu.read_byte(0x1005), 0x11);

    // FD 35 d - DEC (IY+d)
    let mut cpu = create_z80(&[0xFD, 0x35, 0xFE]); // -2
    cpu.iy = 0x1002;
    cpu.write_byte(0x1000, 0x10);
    cpu.step();
    assert_eq!(cpu.read_byte(0x1000), 0x0F);

    // DD 36 d n - LD (IX+d), n
    let mut cpu = create_z80(&[0xDD, 0x36, 0x05, 0x42]);
    cpu.ix = 0x1000;
    cpu.step();
    assert_eq!(cpu.read_byte(0x1005), 0x42);
}

#[test]
fn test_index_alu_mem() {
    // DD 86 d - ADD A, (IX+d)
    let mut cpu = create_z80(&[0xDD, 0x86, 0x05]);
    cpu.a = 0x10;
    cpu.ix = 0x1000;
    cpu.write_byte(0x1005, 0x20);
    cpu.step();
    assert_eq!(cpu.a, 0x30);

    // FD 8E d - ADC A, (IY+d)
    let mut cpu = create_z80(&[0xFD, 0x8E, 0x05]);
    cpu.a = 0x10;
    cpu.iy = 0x1000;
    cpu.set_flag(flags::CARRY, true);
    cpu.write_byte(0x1005, 0x20);
    cpu.step();
    assert_eq!(cpu.a, 0x31);
}

#[test]
fn test_index_load_r_mem() {
    // DD 46 d - LD B, (IX+d)
    let mut cpu = create_z80(&[0xDD, 0x46, 0x05]);
    cpu.ix = 0x1000;
    cpu.write_byte(0x1005, 0x42);
    cpu.step();
    assert_eq!(cpu.b, 0x42);
}

#[test]
fn test_index_load_mem_r() {
    // DD 70 d - LD (IX+d), B
    let mut cpu = create_z80(&[0xDD, 0x70, 0x05]);
    cpu.ix = 0x1000;
    cpu.b = 0x42;
    cpu.step();
    assert_eq!(cpu.read_byte(0x1005), 0x42);
}

#[test]
fn test_index_stack_control() {
    // DD E5 - PUSH IX
    let mut cpu = create_z80(&[0xDD, 0xE5]);
    cpu.ix = 0x1234;
    cpu.sp = 0x2000;
    cpu.step();
    assert_eq!(cpu.sp, 0x1FFE);
    assert_eq!(cpu.read_word(0x1FFE), 0x1234);

    // FD E1 - POP IY
    let mut cpu = create_z80(&[0xFD, 0xE1]);
    cpu.sp = 0x1FFE;
    cpu.write_word(0x1FFE, 0x5678);
    cpu.step();
    assert_eq!(cpu.iy, 0x5678);
    assert_eq!(cpu.sp, 0x2000);

    // DD E9 - JP (IX)
    let mut cpu = create_z80(&[0xDD, 0xE9]);
    cpu.ix = 0x1234;
    cpu.step();
    assert_eq!(cpu.pc, 0x1234);

    // FD F9 - LD SP, IY
    let mut cpu = create_z80(&[0xFD, 0xF9]);
    cpu.iy = 0x5678;
    cpu.step();
    assert_eq!(cpu.sp, 0x5678);
}
