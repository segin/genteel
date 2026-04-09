//! Unit tests for M68k Data Movement Instructions
//!
//! Focused on `exec_move` and other data movement logic in isolation.

use crate::cpu::decoder::{AddressingMode, Size};
use crate::cpu::flags;
use crate::cpu::ops::data::{exec_move, exec_movea, exec_moveq, exec_lea, exec_pea, exec_exg, exec_movep, exec_swap, exec_ext, exec_movem};
use crate::cpu::test_utils::create_cpu;
use crate::memory::MemoryInterface;

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

#[test]
fn test_exec_movea_word() {
    let (mut cpu, mut memory) = create_cpu();
    cpu.d[0] = 0xFFFF; // -1
    exec_movea(&mut cpu, Size::Word, AddressingMode::DataRegister(0), 1, &mut memory);
    assert_eq!(cpu.a[1], 0xFFFFFFFF); // Sign extended
}

#[test]
fn test_exec_movea_long() {
    let (mut cpu, mut memory) = create_cpu();
    cpu.d[0] = 0x87654321;
    exec_movea(&mut cpu, Size::Long, AddressingMode::DataRegister(0), 2, &mut memory);
    assert_eq!(cpu.a[2], 0x87654321);
}

#[test]
fn test_exec_moveq() {
    let (mut cpu, _memory) = create_cpu();
    // Test negative sign extension
    exec_moveq(&mut cpu, 0, 0xFF);
    assert_eq!(cpu.d[0], 0xFFFFFFFF);
    assert!(cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::ZERO));

    // Test positive sign extension
    exec_moveq(&mut cpu, 1, 0x7F);
    assert_eq!(cpu.d[1], 0x0000007F);
    assert!(!cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::ZERO));

    // Test zero
    exec_moveq(&mut cpu, 2, 0);
    assert_eq!(cpu.d[2], 0);
    assert!(!cpu.get_flag(flags::NEGATIVE));
    assert!(cpu.get_flag(flags::ZERO));
}

#[test]
fn test_exec_lea() {
    let (mut cpu, mut memory) = create_cpu();
    cpu.a[0] = 0x1000;
    // Calculate EA for Indirect (A0) which should give 0x1000
    exec_lea(&mut cpu, AddressingMode::AddressIndirect(0), 1, &mut memory);
    assert_eq!(cpu.a[1], 0x1000);
}

#[test]
fn test_exec_pea() {
    let (mut cpu, mut memory) = create_cpu();
    cpu.a[0] = 0x2000;
    cpu.a[7] = 0x8000;
    exec_pea(&mut cpu, AddressingMode::AddressIndirect(0), &mut memory);
    assert_eq!(cpu.a[7], 0x7FFC);
    assert_eq!(memory.read_long(0x7FFC), 0x2000);
}

#[test]
fn test_exec_exg() {
    let (mut cpu, _memory) = create_cpu();

    // Data registers
    cpu.d[0] = 0x11111111;
    cpu.d[1] = 0x22222222;
    exec_exg(&mut cpu, 0, 1, 0x08);
    assert_eq!(cpu.d[0], 0x22222222);
    assert_eq!(cpu.d[1], 0x11111111);

    // Address registers
    cpu.a[2] = 0x33333333;
    cpu.a[3] = 0x44444444;
    exec_exg(&mut cpu, 2, 3, 0x09);
    assert_eq!(cpu.a[2], 0x44444444);
    assert_eq!(cpu.a[3], 0x33333333);

    // Data and Address
    cpu.d[4] = 0x55555555;
    cpu.a[5] = 0x66666666;
    exec_exg(&mut cpu, 4, 5, 0x11);
    assert_eq!(cpu.d[4], 0x66666666);
    assert_eq!(cpu.a[5], 0x55555555);
}

#[test]
fn test_exec_movep_register_to_memory() {
    let (mut cpu, mut memory) = create_cpu();
    cpu.d[0] = 0x12345678;
    cpu.a[0] = 0x1000;
    cpu.pc = 0x2000;
    memory.write_word(0x2000, 0x0004); // displacement

    // Register to Memory, Size::Long
    exec_movep(&mut cpu, Size::Long, 0, 0, true, &mut memory);

    // 0x1004, 0x1006, 0x1008, 0x100A should contain 0x12, 0x34, 0x56, 0x78
    assert_eq!(memory.read_byte(0x1004), 0x12);
    assert_eq!(memory.read_byte(0x1006), 0x34);
    assert_eq!(memory.read_byte(0x1008), 0x56);
    assert_eq!(memory.read_byte(0x100A), 0x78);

    // Try Word
    cpu.pc = 0x2002;
    memory.write_word(0x2002, 0x0010); // displacement
    exec_movep(&mut cpu, Size::Word, 0, 0, true, &mut memory);

    assert_eq!(memory.read_byte(0x1010), 0x56);
    assert_eq!(memory.read_byte(0x1012), 0x78);
}

#[test]
fn test_exec_movep_memory_to_register() {
    let (mut cpu, mut memory) = create_cpu();
    cpu.a[0] = 0x1000;
    cpu.pc = 0x2000;
    memory.write_word(0x2000, 0x0004); // displacement

    memory.write_byte(0x1004, 0x12);
    memory.write_byte(0x1006, 0x34);
    memory.write_byte(0x1008, 0x56);
    memory.write_byte(0x100A, 0x78);

    // Memory to Register, Size::Long
    exec_movep(&mut cpu, Size::Long, 0, 0, false, &mut memory);
    assert_eq!(cpu.d[0], 0x12345678);

    // Try Word
    cpu.d[1] = 0xAAAAAAAA;
    cpu.pc = 0x2002;
    memory.write_word(0x2002, 0x0010); // displacement
    memory.write_byte(0x1010, 0x56);
    memory.write_byte(0x1012, 0x78);

    exec_movep(&mut cpu, Size::Word, 1, 0, false, &mut memory);
    assert_eq!(cpu.d[1], 0xAAAA5678);
}

#[test]
fn test_exec_swap() {
    let (mut cpu, _memory) = create_cpu();
    cpu.d[0] = 0x12345678;
    exec_swap(&mut cpu, 0);
    assert_eq!(cpu.d[0], 0x56781234);
    assert!(!cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::OVERFLOW));
    assert!(!cpu.get_flag(flags::CARRY));

    cpu.d[1] = 0x80000000;
    exec_swap(&mut cpu, 1);
    assert_eq!(cpu.d[1], 0x00008000);
    assert!(!cpu.get_flag(flags::NEGATIVE));

    cpu.d[2] = 0x00008000;
    exec_swap(&mut cpu, 2);
    assert_eq!(cpu.d[2], 0x80000000);
    assert!(cpu.get_flag(flags::NEGATIVE));
}

#[test]
fn test_exec_ext() {
    let (mut cpu, _memory) = create_cpu();

    // Ext Word
    cpu.d[0] = 0xAAAAAA80; // Negative byte
    exec_ext(&mut cpu, Size::Word, 0);
    assert_eq!(cpu.d[0], 0xAAAAFF80); // Sign extended byte to word
    assert!(cpu.get_flag(flags::NEGATIVE));

    cpu.d[1] = 0xAAAAAA7F; // Positive byte
    exec_ext(&mut cpu, Size::Word, 1);
    assert_eq!(cpu.d[1], 0xAAAA007F); // Sign extended byte to word
    assert!(!cpu.get_flag(flags::NEGATIVE));

    // Ext Long
    cpu.d[2] = 0xAAAA8000; // Negative word
    exec_ext(&mut cpu, Size::Long, 2);
    assert_eq!(cpu.d[2], 0xFFFF8000); // Sign extended word to long
    assert!(cpu.get_flag(flags::NEGATIVE));

    cpu.d[3] = 0xAAAA7FFF; // Positive word
    exec_ext(&mut cpu, Size::Long, 3);
    assert_eq!(cpu.d[3], 0x00007FFF); // Sign extended word to long
    assert!(!cpu.get_flag(flags::NEGATIVE));
}

#[test]
fn test_exec_movem_to_memory_predec() {
    let (mut cpu, mut memory) = create_cpu();
    cpu.pc = 0x2000;
    // Pre-decrement order reverse, so D0 (bit 15) and A0 (bit 7) -> 0x8080
    memory.write_word(0x2000, 0x8080);

    cpu.a[7] = 0x8000;
    cpu.d[0] = 0x11111111;
    cpu.a[0] = 0x22222222;

    exec_movem(&mut cpu, Size::Long, true, AddressingMode::AddressPreDecrement(7), &mut memory);

    assert_eq!(cpu.a[7], 0x7FF8);
    // Reversed order: A0 (high addr) then D0 (low addr)
    assert_eq!(memory.read_long(0x7FFC), 0x22222222);
    assert_eq!(memory.read_long(0x7FF8), 0x11111111);
}

#[test]
fn test_exec_movem_to_memory_normal() {
    let (mut cpu, mut memory) = create_cpu();
    cpu.pc = 0x2000;
    // D0 (bit 0), D1 (bit 1) -> 0x0003
    memory.write_word(0x2000, 0x0003);

    cpu.a[0] = 0x1000;
    cpu.d[0] = 0x11111111;
    cpu.d[1] = 0x22222222;

    exec_movem(&mut cpu, Size::Long, true, AddressingMode::AddressIndirect(0), &mut memory);

    assert_eq!(memory.read_long(0x1000), 0x11111111);
    assert_eq!(memory.read_long(0x1004), 0x22222222);
    assert_eq!(cpu.a[0], 0x1000); // Unchanged
}

#[test]
fn test_exec_movem_from_memory_postinc() {
    let (mut cpu, mut memory) = create_cpu();
    cpu.pc = 0x2000;
    // D0 (bit 0), D1 (bit 1), A1 (bit 9) -> 0x0203
    memory.write_word(0x2000, 0x0203);

    cpu.a[0] = 0x1000;
    memory.write_long(0x1000, 0x11111111);
    memory.write_long(0x1004, 0x22222222);
    memory.write_long(0x1008, 0x33333333);

    exec_movem(&mut cpu, Size::Long, false, AddressingMode::AddressPostIncrement(0), &mut memory);

    assert_eq!(cpu.d[0], 0x11111111);
    assert_eq!(cpu.d[1], 0x22222222);
    assert_eq!(cpu.a[1], 0x33333333);
    assert_eq!(cpu.a[0], 0x100C);
}
