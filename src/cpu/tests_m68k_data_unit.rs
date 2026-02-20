//! Unit tests for M68k Data Movement Instructions
//!
//! Focused on `exec_move` and other data movement logic in isolation.

use crate::cpu::decoder::{AddressingMode, Size};
use crate::cpu::flags;
use crate::cpu::ops::data::exec_move;
#[cfg(test)]
use crate::cpu::Cpu;
use crate::memory::{Memory, MemoryInterface};

fn create_cpu() -> (Cpu, Memory) {
    let mut memory = Memory::new(0x10000);
    let mut cpu = Cpu::new(&mut memory);
    cpu.pc = 0x1000;
    cpu.a[7] = 0x8000;
    cpu.sr = 0x2700; // Supervisor mode
    (cpu, memory)
}

#[test]
fn test_move_data_register_byte() {
    let (mut cpu, mut memory) = create_cpu();
    cpu.d[0] = 0x12345678;
    cpu.d[1] = 0xAAAAAAAA;

    // MOVE.B D0, D1
    exec_move(
        &mut cpu,
        Size::Byte,
        AddressingMode::DataRegister(0),
        AddressingMode::DataRegister(1),
        &mut memory,
    );

    assert_eq!(cpu.d[1], 0xAAAAAA78); // Only low byte changed
    assert!(!cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::OVERFLOW));
    assert!(!cpu.get_flag(flags::CARRY));
}

#[test]
fn test_move_data_register_word() {
    let (mut cpu, mut memory) = create_cpu();
    cpu.d[0] = 0x12345678;
    cpu.d[1] = 0xAAAAAAAA;

    // MOVE.W D0, D1
    exec_move(
        &mut cpu,
        Size::Word,
        AddressingMode::DataRegister(0),
        AddressingMode::DataRegister(1),
        &mut memory,
    );

    assert_eq!(cpu.d[1], 0xAAAA5678); // Low word changed
    assert!(!cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::ZERO));
}

#[test]
fn test_move_data_register_long() {
    let (mut cpu, mut memory) = create_cpu();
    cpu.d[0] = 0x12345678;
    cpu.d[1] = 0xAAAAAAAA;

    // MOVE.L D0, D1
    exec_move(
        &mut cpu,
        Size::Long,
        AddressingMode::DataRegister(0),
        AddressingMode::DataRegister(1),
        &mut memory,
    );

    assert_eq!(cpu.d[1], 0x12345678); // All bits changed
    assert!(!cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::ZERO));
}

#[test]
fn test_move_sets_negative_flag() {
    let (mut cpu, mut memory) = create_cpu();
    cpu.d[0] = 0xFF; // -1 byte

    exec_move(
        &mut cpu,
        Size::Byte,
        AddressingMode::DataRegister(0),
        AddressingMode::DataRegister(1),
        &mut memory,
    );

    assert!(cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::ZERO));
}

#[test]
fn test_move_sets_zero_flag() {
    let (mut cpu, mut memory) = create_cpu();
    cpu.d[0] = 0;

    exec_move(
        &mut cpu,
        Size::Byte,
        AddressingMode::DataRegister(0),
        AddressingMode::DataRegister(1),
        &mut memory,
    );

    assert!(!cpu.get_flag(flags::NEGATIVE));
    assert!(cpu.get_flag(flags::ZERO));
}

#[test]
fn test_move_clears_vc_flags() {
    let (mut cpu, mut memory) = create_cpu();
    cpu.set_flag(flags::OVERFLOW, true);
    cpu.set_flag(flags::CARRY, true);
    cpu.d[0] = 0;

    exec_move(
        &mut cpu,
        Size::Byte,
        AddressingMode::DataRegister(0),
        AddressingMode::DataRegister(1),
        &mut memory,
    );

    assert!(!cpu.get_flag(flags::OVERFLOW));
    assert!(!cpu.get_flag(flags::CARRY));
}

#[test]
fn test_move_memory_to_register() {
    let (mut cpu, mut memory) = create_cpu();
    memory.write_word(0x2000, 0x1234);

    // MOVE.W (A0), D0
    cpu.a[0] = 0x2000;

    exec_move(
        &mut cpu,
        Size::Word,
        AddressingMode::AddressIndirect(0),
        AddressingMode::DataRegister(0),
        &mut memory,
    );

    assert_eq!(cpu.d[0] & 0xFFFF, 0x1234);
}

#[test]
fn test_move_register_to_memory() {
    let (mut cpu, mut memory) = create_cpu();
    cpu.d[0] = 0x12345678;
    cpu.a[0] = 0x3000;

    // MOVE.L D0, (A0)
    exec_move(
        &mut cpu,
        Size::Long,
        AddressingMode::DataRegister(0),
        AddressingMode::AddressIndirect(0),
        &mut memory,
    );

    assert_eq!(memory.read_long(0x3000), 0x12345678);
}

#[test]
fn test_move_post_increment() {
    let (mut cpu, mut memory) = create_cpu();
    cpu.d[0] = 0x11223344;
    cpu.a[0] = 0x4000;

    // MOVE.W D0, (A0)+
    exec_move(
        &mut cpu,
        Size::Word,
        AddressingMode::DataRegister(0),
        AddressingMode::AddressPostIncrement(0),
        &mut memory,
    );

    assert_eq!(memory.read_word(0x4000), 0x3344);
    assert_eq!(cpu.a[0], 0x4002);
}

#[test]
fn test_move_pre_decrement() {
    let (mut cpu, mut memory) = create_cpu();
    cpu.d[0] = 0x11223344;
    cpu.a[0] = 0x5002;

    // MOVE.W D0, -(A0)
    exec_move(
        &mut cpu,
        Size::Word,
        AddressingMode::DataRegister(0),
        AddressingMode::AddressPreDecrement(0),
        &mut memory,
    );

    assert_eq!(memory.read_word(0x5000), 0x3344);
    assert_eq!(cpu.a[0], 0x5000);
}

#[test]
fn test_move_immediate() {
    let (mut cpu, mut memory) = create_cpu();
    // Immediate data at PC: 0x1234
    memory.write_word(cpu.pc, 0x1234);

    // MOVE.W #$1234, D0
    exec_move(
        &mut cpu,
        Size::Word,
        AddressingMode::Immediate,
        AddressingMode::DataRegister(0),
        &mut memory,
    );

    assert_eq!(cpu.d[0] & 0xFFFF, 0x1234);
    // PC should advance by 2 (size of immediate data)
    // Note: create_cpu sets PC to 0x1000
    assert_eq!(cpu.pc, 0x1002);
}
