use super::*;
use crate::memory::{Memory, MemoryInterface};
use crate::z80::test_utils::TestIo;

fn create_z80(program: &[u8]) -> crate::z80::test_utils::TestZ80 {
    let mut memory = Memory::new(0x10000);
    for (i, &byte) in program.iter().enumerate() {
        memory.data[i] = byte;
    }
    let cpu = Z80::new();
    crate::z80::test_utils::TestZ80::new(cpu, memory, TestIo::default())
}

#[test]
fn test_memptr_ld_bc_nn() {
    let mut z80 = create_z80(&[0x01, 0x34, 0x12]); // LD BC, 0x1234
    z80.step();
    // For LD rp, nn, MEMPTR is NOT updated.
    // Wait, some sources say it is?
    // Let's check common Z80 implementations.
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
