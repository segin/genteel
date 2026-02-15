use super::*;
use crate::memory::{Memory, MemoryInterface};

fn create_z80(program: &[u8]) -> Z80<Memory, crate::z80::test_utils::TestIo> {
    let mut memory = Memory::new(0x10000);
    for (i, &byte) in program.iter().enumerate() {
        memory.write_byte(i as u32, byte);
    }
    Z80::new(memory, crate::z80::test_utils::TestIo::default())
}

#[test]
fn test_memptr_ld_a_bc() {
    // Test case: LD BC, 0x2800; LD A, (BC); BIT 0, (HL)
    // LD A, (BC) should set MEMPTR to BC+1 = 0x2801.
    // BIT 0, (HL) uses MEMPTR high byte (0x28).
    // 0x28 = 0010 1000. X (bit 3) = 1, Y (bit 5) = 1.

    let program = [
        0x01, 0x00, 0x28, // LD BC, 0x2800
        0x0A, // LD A, (BC)
        0xCB, 0x46, // BIT 0, (HL)
    ];
    let mut z80 = create_z80(&program);

    // Setup memory at BC (0x2800)
    z80.write_byte(0x2800, 0x55);
    z80.set_hl(0x0000); // HL points to 0

    // Step 1: LD BC
    z80.step();
    // Step 2: LD A, (BC)
    z80.step();
    // Verify A
    assert_eq!(z80.a, 0x55);

    // Step 3: BIT 0, (HL)
    z80.step();

    // Verify X/Y flags
    assert_eq!(
        z80.get_flag(flags::X_FLAG),
        true,
        "X Flag (bit 3) should be set from MEMPTR high byte (0x28)"
    );
    assert_eq!(
        z80.get_flag(flags::Y_FLAG),
        true,
        "Y Flag (bit 5) should be set from MEMPTR high byte (0x28)"
    );
}

#[test]
fn test_memptr_ld_a_de() {
    // Test case: LD DE, 0x2800; LD A, (DE); BIT 0, (HL)
    let program = [
        0x11, 0x00, 0x28, // LD DE, 0x2800
        0x1A, // LD A, (DE)
        0xCB, 0x46, // BIT 0, (HL)
    ];
    let mut z80 = create_z80(&program);
    z80.write_byte(0x2800, 0x55);
    z80.set_hl(0x0000);

    z80.step(); // LD DE
    z80.step(); // LD A, (DE)
    assert_eq!(z80.a, 0x55);
    z80.step(); // BIT 0, (HL)

    assert_eq!(
        z80.get_flag(flags::X_FLAG),
        true,
        "X Flag should be set from MEMPTR high byte (0x28)"
    );
    assert_eq!(
        z80.get_flag(flags::Y_FLAG),
        true,
        "Y Flag should be set from MEMPTR high byte (0x28)"
    );
}

#[test]
fn test_memptr_ex_sp_hl() {
    // Test case: LD HL, 0x1234; LD SP, 0x2000; EX (SP), HL; BIT 0, (HL)
    // EX (SP), HL should set MEMPTR to value read from stack.
    // Stack at 0x2000 has 0xABCD.
    // MEMPTR = 0xABCD. High byte 0xAB = 1010 1011. X=1, Y=1.

    let program = [
        0x21, 0x34, 0x12, // LD HL, 0x1234
        0x31, 0x00, 0x20, // LD SP, 0x2000
        0xE3, // EX (SP), HL
        0xCB, 0x46, // BIT 0, (HL)
    ];
    let mut z80 = create_z80(&program);
    z80.write_word(0x2000, 0xABCD);

    z80.step(); // LD HL
    z80.step(); // LD SP
    z80.step(); // EX (SP), HL
    assert_eq!(z80.hl(), 0xABCD);
    z80.step(); // BIT 0, (HL)

    assert_eq!(
        z80.get_flag(flags::X_FLAG),
        true,
        "X Flag should be set from MEMPTR high byte (0xAB)"
    );
    assert_eq!(
        z80.get_flag(flags::Y_FLAG),
        true,
        "Y Flag should be set from MEMPTR high byte (0xAB)"
    );
}
