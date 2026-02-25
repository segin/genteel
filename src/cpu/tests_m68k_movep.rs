//! Tests for MOVEP instruction

use crate::cpu::decoder::Size;
use crate::cpu::ops::data::exec_movep;
#[cfg(test)]
use crate::cpu::Cpu;
use crate::memory::{Memory, MemoryInterface};

fn create_cpu() -> (Cpu, Memory) {
    let mut memory = Memory::new(0x10000);
    let mut cpu = Cpu::new(&mut memory);
    cpu.pc = 0x1000;
    cpu.a[7] = 0x8000;
    (cpu, memory)
}

#[test]
fn test_movep_word_mem_to_reg() {
    let (mut cpu, mut memory) = create_cpu();

    // MOVEP.W d16(A0), D0
    // A0 = 0x2000
    // d16 = 4
    // Addr = 0x2004
    // Read from 0x2004 (Hi) and 0x2006 (Lo)

    cpu.a[0] = 0x2000;
    cpu.d[0] = 0xFFFFFFFF; // Pre-fill with garbage

    // Set displacement in instruction stream at PC
    memory.write_word(cpu.pc, 0x0004);

    // Set memory values
    memory.write_byte(0x2004, 0xAA);
    memory.write_byte(0x2006, 0xBB);

    // Execute: size=Word, reg=0 (D0), an=0 (A0), reg_to_mem=false
    let cycles = exec_movep(&mut cpu, Size::Word, 0, 0, false, &mut memory);

    assert_eq!(cycles, 16);
    assert_eq!(cpu.pc, 0x1002); // PC should advance by 2 (displacement word)

    // D0 should be 0xFFFF AABB (upper word unaffected, lower word loaded)
    assert_eq!(cpu.d[0], 0xFFFFAABB);
}

#[test]
fn test_movep_word_reg_to_mem() {
    let (mut cpu, mut memory) = create_cpu();

    // MOVEP.W D0, d16(A0)
    // A0 = 0x3000
    // d16 = 2
    // Addr = 0x3002
    // Write to 0x3002 (Hi) and 0x3004 (Lo)

    cpu.a[0] = 0x3000;
    cpu.d[0] = 0x12345678;

    // Set displacement
    memory.write_word(cpu.pc, 0x0002);

    // Clear memory area
    memory.write_byte(0x3002, 0x00);
    memory.write_byte(0x3004, 0x00);

    // Execute: size=Word, reg=0 (D0), an=0 (A0), reg_to_mem=true
    let cycles = exec_movep(&mut cpu, Size::Word, 0, 0, true, &mut memory);

    assert_eq!(cycles, 16);
    assert_eq!(cpu.pc, 0x1002);

    // Memory should have 0x56 at 0x3002 and 0x78 at 0x3004
    assert_eq!(memory.read_byte(0x3002), 0x56);
    assert_eq!(memory.read_byte(0x3004), 0x78);
    // Ensure bytes in between are untouched
    assert_eq!(memory.read_byte(0x3003), 0x00);
}

#[test]
fn test_movep_long_mem_to_reg() {
    let (mut cpu, mut memory) = create_cpu();

    // MOVEP.L d16(A1), D1
    // A1 = 0x4000
    // d16 = 0
    // Addr = 0x4000
    // Read from 0x4000, 0x4002, 0x4004, 0x4006

    cpu.a[1] = 0x4000;
    cpu.d[1] = 0x00000000;

    // Set displacement
    memory.write_word(cpu.pc, 0x0000);

    // Set memory values
    memory.write_byte(0x4000, 0x11);
    memory.write_byte(0x4002, 0x22);
    memory.write_byte(0x4004, 0x33);
    memory.write_byte(0x4006, 0x44);

    // Execute: size=Long, reg=1 (D1), an=1 (A1), reg_to_mem=false
    let cycles = exec_movep(&mut cpu, Size::Long, 1, 1, false, &mut memory);

    assert_eq!(cycles, 24);
    assert_eq!(cpu.pc, 0x1002);

    assert_eq!(cpu.d[1], 0x11223344);
}

#[test]
fn test_movep_long_reg_to_mem() {
    let (mut cpu, mut memory) = create_cpu();

    // MOVEP.L D2, d16(A2)
    // A2 = 0x5000
    // d16 = 10
    // Addr = 0x500A
    // Write to 0x500A, 0x500C, 0x500E, 0x5010

    cpu.a[2] = 0x5000;
    cpu.d[2] = 0xAABBCCDD;

    // Set displacement
    memory.write_word(cpu.pc, 0x000A);

    // Execute: size=Long, reg=2 (D2), an=2 (A2), reg_to_mem=true
    let cycles = exec_movep(&mut cpu, Size::Long, 2, 2, true, &mut memory);

    assert_eq!(cycles, 24);
    assert_eq!(cpu.pc, 0x1002);

    assert_eq!(memory.read_byte(0x500A), 0xAA);
    assert_eq!(memory.read_byte(0x500C), 0xBB);
    assert_eq!(memory.read_byte(0x500E), 0xCC);
    assert_eq!(memory.read_byte(0x5010), 0xDD);

    // Verify gaps are untouched (assuming 0 from init)
    assert_eq!(memory.read_byte(0x500B), 0x00);
    assert_eq!(memory.read_byte(0x500D), 0x00);
    assert_eq!(memory.read_byte(0x500F), 0x00);
}

#[test]
fn test_movep_negative_displacement() {
    let (mut cpu, mut memory) = create_cpu();

    // MOVEP.W d16(A0), D0
    // A0 = 0x6000
    // d16 = -2 (0xFFFE)
    // Addr = 0x5FFE
    // Read from 0x5FFE, 0x6000

    cpu.a[0] = 0x6000;
    cpu.d[0] = 0x00000000;

    // Set displacement
    memory.write_word(cpu.pc, 0xFFFE); // -2

    memory.write_byte(0x5FFE, 0x88);
    memory.write_byte(0x6000, 0x99);

    // Execute: size=Word, reg=0 (D0), an=0 (A0), reg_to_mem=false
    let cycles = exec_movep(&mut cpu, Size::Word, 0, 0, false, &mut memory);

    assert_eq!(cycles, 16);

    // D0 should be 0x00008899 (assuming upper word 0)
    assert_eq!(cpu.d[0], 0x00008899);
}
