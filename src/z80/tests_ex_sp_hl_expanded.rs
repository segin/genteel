#![cfg(test)]

use super::*;
use crate::memory::Memory;
use crate::z80::test_utils::{CombinedBus, TestIo};
use proptest::prelude::*;

fn create_z80(program: &[u8]) -> (Z80, CombinedBus) {
    let mut memory = Memory::new(0x10000);
    for (i, &byte) in program.iter().enumerate() {
        memory.data[i] = byte;
    }
    (Z80::new(), CombinedBus::new(memory, TestIo::default()))
}

#[test]
fn test_ex_sp_hl_memptr_and_flags() {
    // EX (SP), HL: 0xE3
    let (mut cpu, mut bus) = create_z80(&[0xE3]);
    cpu.sp = 0x1000;
    cpu.set_hl(0x1234);
    // Write 0xABCD to memory in Little Endian (CD at 0x1000, AB at 0x1001)
    bus.memory.write_byte(0x1000, 0xCD);
    bus.memory.write_byte(0x1001, 0xAB);
    cpu.f = 0xFF; // Set all flags
    cpu.memptr = 0;

    cpu.step(&mut bus);

    // Verify swap
    assert_eq!(cpu.hl(), 0xABCD);
    // Verify memory contains 0x1234 in Little Endian (34 at 0x1000, 12 at 0x1001)
    assert_eq!(bus.memory.read_byte(0x1000), 0x34);
    assert_eq!(bus.memory.read_byte(0x1001), 0x12);

    // Verify flags preserved
    assert_eq!(cpu.f, 0xFF);

    // Verify MEMPTR updated to new HL value (value popped from stack)
    assert_eq!(cpu.memptr, 0xABCD);
}

#[test]
fn test_ex_sp_ix_memptr_and_flags() {
    // EX (SP), IX: 0xDD 0xE3
    let (mut cpu, mut bus) = create_z80(&[0xDD, 0xE3]);
    cpu.sp = 0x2000;
    cpu.ix = 0x5678;
    // Write 0x9ABC to memory in Little Endian (BC at 0x2000, 9A at 0x2001)
    bus.memory.write_byte(0x2000, 0xBC);
    bus.memory.write_byte(0x2001, 0x9A);
    cpu.f = 0x00; // Clear all flags
    cpu.memptr = 0;

    cpu.step(&mut bus);

    // Verify swap
    assert_eq!(cpu.ix, 0x9ABC);
    // Verify memory contains 0x5678 in Little Endian
    assert_eq!(bus.memory.read_byte(0x2000), 0x78);
    assert_eq!(bus.memory.read_byte(0x2001), 0x56);

    // Verify flags preserved
    assert_eq!(cpu.f, 0x00);

    // Verify MEMPTR updated to new IX value
    assert_eq!(cpu.memptr, 0x9ABC);
}

#[test]
fn test_ex_sp_iy_memptr_and_flags() {
    // EX (SP), IY: 0xFD 0xE3
    let (mut cpu, mut bus) = create_z80(&[0xFD, 0xE3]);
    cpu.sp = 0x3000;
    cpu.iy = 0xDEAD;
    // Write 0xBEEF to memory in Little Endian (EF at 0x3000, BE at 0x3001)
    bus.memory.write_byte(0x3000, 0xEF);
    bus.memory.write_byte(0x3001, 0xBE);
    cpu.f = 0xAA; // Partial flags
    cpu.memptr = 0xFFFF;

    cpu.step(&mut bus);

    // Verify swap
    assert_eq!(cpu.iy, 0xBEEF);
    // Verify memory contains 0xDEAD in Little Endian
    assert_eq!(bus.memory.read_byte(0x3000), 0xAD);
    assert_eq!(bus.memory.read_byte(0x3001), 0xDE);

    // Verify flags preserved
    assert_eq!(cpu.f, 0xAA);

    // Verify MEMPTR updated to new IY value
    assert_eq!(cpu.memptr, 0xBEEF);
}

proptest! {
    #[test]
    fn prop_ex_sp_hl_swaps_correctly(hl_val in 0u16..=0xFFFF, sp_val in 2u16..=0xFFFE, mem_val in 0u16..=0xFFFF) {
        let (mut cpu, mut bus) = create_z80(&[0xE3]);
        cpu.sp = sp_val;
        cpu.set_hl(hl_val);
        // Write Little Endian
        bus.memory.write_byte(sp_val as u32, mem_val as u8);
        bus.memory.write_byte((sp_val as u32).wrapping_add(1), (mem_val >> 8) as u8);

        cpu.step(&mut bus);

        prop_assert_eq!(cpu.hl(), mem_val);
        // Check memory explicitly byte by byte for Little Endian
        prop_assert_eq!(bus.memory.read_byte(sp_val as u32), hl_val as u8);
        prop_assert_eq!(bus.memory.read_byte((sp_val as u32).wrapping_add(1)), (hl_val >> 8) as u8);
        prop_assert_eq!(cpu.memptr, mem_val);
    }

    #[test]
    fn prop_ex_sp_ix_swaps_correctly(ix_val in 0u16..=0xFFFF, sp_val in 2u16..=0xFFFE, mem_val in 0u16..=0xFFFF) {
        let (mut cpu, mut bus) = create_z80(&[0xDD, 0xE3]);
        cpu.sp = sp_val;
        cpu.ix = ix_val;
        // Write Little Endian
        bus.memory.write_byte(sp_val as u32, mem_val as u8);
        bus.memory.write_byte((sp_val as u32).wrapping_add(1), (mem_val >> 8) as u8);

        cpu.step(&mut bus);

        prop_assert_eq!(cpu.ix, mem_val);
        // Check memory
        prop_assert_eq!(bus.memory.read_byte(sp_val as u32), ix_val as u8);
        prop_assert_eq!(bus.memory.read_byte((sp_val as u32).wrapping_add(1)), (ix_val >> 8) as u8);
        prop_assert_eq!(cpu.memptr, mem_val);
    }

    #[test]
    fn prop_ex_sp_iy_swaps_correctly(iy_val in 0u16..=0xFFFF, sp_val in 2u16..=0xFFFE, mem_val in 0u16..=0xFFFF) {
        let (mut cpu, mut bus) = create_z80(&[0xFD, 0xE3]);
        cpu.sp = sp_val;
        cpu.iy = iy_val;
        // Write Little Endian
        bus.memory.write_byte(sp_val as u32, mem_val as u8);
        bus.memory.write_byte((sp_val as u32).wrapping_add(1), (mem_val >> 8) as u8);

        cpu.step(&mut bus);

        prop_assert_eq!(cpu.iy, mem_val);
        // Check memory
        prop_assert_eq!(bus.memory.read_byte(sp_val as u32), iy_val as u8);
        prop_assert_eq!(bus.memory.read_byte((sp_val as u32).wrapping_add(1)), (iy_val >> 8) as u8);
        prop_assert_eq!(cpu.memptr, mem_val);
    }
}
