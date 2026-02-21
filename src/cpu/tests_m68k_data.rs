//! M68k Data Movement Tests
//!
//! Exhaustive tests for M68k data movement instructions (MOVE, MOVEA, MOVEM, MOVEP, EXG, etc.).

#![cfg(test)]

use crate::cpu::flags;
use crate::cpu::Cpu;
use crate::memory::{Memory, MemoryInterface};

fn create_cpu() -> (Cpu, Memory) {
    let mut memory = Memory::new(0x100000);
    let mut cpu = Cpu::new(&mut memory);
    cpu.pc = 0x1000;
    cpu.a[7] = 0x8000;
    cpu.sr |= flags::SUPERVISOR;
    (cpu, memory)
}

fn write_op(memory: &mut Memory, opcodes: &[u16]) {
    let mut addr = 0x1000u32;
    for &op in opcodes {
        memory.write_word(addr, op);
        addr += 2;
    }
}

// ============================================================================
// MOVE Tests
// ============================================================================

#[test]
fn test_move_b_d0_d1() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.B D0, D1
    // 00 01 (Byte) 001 (D1) 000 (Mode Dn) 000 (Mode Dn) 000 (Src D0)
    // -> 0001 001 000 000 000 -> 0x1200
    write_op(&mut memory, &[0x1200]);
    cpu.d[0] = 0x55;
    cpu.d[1] = 0x33;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1] & 0xFF, 0x55);
}

#[test]
fn test_move_w_d0_d1() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.W D0, D1
    // Size: 11 (Word)
    // 00 11 001 000 000 000 -> 0x3200
    write_op(&mut memory, &[0x3200]);
    cpu.d[0] = 0x1234;
    cpu.d[1] = 0x4321;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1] & 0xFFFF, 0x1234);
}

#[test]
fn test_move_l_d0_d1() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.L D0, D1
    // Size: 10 (Long)
    // 00 10 001 000 000 000 -> 0x2200
    write_op(&mut memory, &[0x2200]);
    cpu.d[0] = 0x12345678;
    cpu.d[1] = 0x11111111;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1], 0x12345678);
}

#[test]
fn test_move_flags() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.B D0, D1
    write_op(&mut memory, &[0x1200]);

    // Case 1: Negative
    cpu.pc = 0x1000;
    cpu.d[0] = 0x80;
    cpu.set_flag(flags::ZERO, true);
    cpu.set_flag(flags::NEGATIVE, false);
    cpu.set_flag(flags::OVERFLOW, true); // Should be cleared
    cpu.set_flag(flags::CARRY, true); // Should be cleared
    cpu.step_instruction(&mut memory);
    assert!(cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::OVERFLOW));
    assert!(!cpu.get_flag(flags::CARRY));

    // Case 2: Zero
    cpu.pc = 0x1000;
    write_op(&mut memory, &[0x1200]); // Re-write op
    cpu.d[0] = 0x00;
    cpu.step_instruction(&mut memory);
    assert!(!cpu.get_flag(flags::NEGATIVE));
    assert!(cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::OVERFLOW));
    assert!(!cpu.get_flag(flags::CARRY));
}

#[test]
fn test_move_immediate_to_d0() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.L #$12345678, D0
    write_op(&mut memory, &[0x203C, 0x1234, 0x5678]);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0x12345678);
}

#[test]
fn test_move_d0_to_memory() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.W D0, (A0)
    write_op(&mut memory, &[0x3080]);
    cpu.d[0] = 0xABCD;
    cpu.a[0] = 0x2000;
    cpu.step_instruction(&mut memory);
    assert_eq!(memory.read_word(0x2000), 0xABCD);
}

#[test]
fn test_move_memory_to_d0() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.B (A0), D0
    write_op(&mut memory, &[0x1010]);
    cpu.a[0] = 0x2000;
    memory.write_byte(0x2000, 0x42);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x42);
}

// ============================================================================
// MOVEA Tests
// ============================================================================

#[test]
fn test_movea_w() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVEA.W D0, A0
    write_op(&mut memory, &[0x3040]);
    cpu.d[0] = 0xFFFF; // -1
    cpu.a[0] = 0x0000;
    cpu.set_flag(flags::ZERO, true); // Should not change
    cpu.step_instruction(&mut memory);

    // MOVEA sign extends word to long
    assert_eq!(cpu.a[0], 0xFFFFFFFF);
    // MOVEA does NOT affect flags
    assert!(cpu.get_flag(flags::ZERO));
}

#[test]
fn test_movea_l() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVEA.L D0, A0
    write_op(&mut memory, &[0x2040]);
    cpu.d[0] = 0x12345678;
    cpu.a[0] = 0x00000000;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.a[0], 0x12345678);
}

// ============================================================================
// MOVEM Tests
// ============================================================================

#[test]
fn test_movem_read_long_postinc() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVEM.L (A0)+, D0/D1/A1
    // Opcode: 0100 1100 11 011 000 (Size=L, Dir=M->R, Mode=(A0)+) -> 0x4CD8
    // Mask: 0000 0010 0000 0011 (A1, D1, D0) -> 0x0203
    write_op(&mut memory, &[0x4CD8, 0x0203]);

    cpu.a[0] = 0x2000;
    memory.write_long(0x2000, 0x11111111); // D0
    memory.write_long(0x2004, 0x22222222); // D1
    memory.write_long(0x2008, 0x33333333); // A1

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.d[0], 0x11111111);
    assert_eq!(cpu.d[1], 0x22222222);
    assert_eq!(cpu.a[1], 0x33333333);
    assert_eq!(cpu.a[0], 0x200C); // Incremented by 12 bytes
}

#[test]
fn test_movem_read_word_sign_extend() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVEM.W (A0), D0/A0
    // Opcode: 0100 1100 10 010 000 (Size=W, Dir=M->R, Mode=(A0)) -> 0x4C90
    // Mask: 0000 0001 0000 0001 (A0, D0) -> 0x0101
    write_op(&mut memory, &[0x4C90, 0x0101]);

    cpu.a[0] = 0x2000;
    memory.write_word(0x2000, 0xFFFF); // -1 (D0)
    memory.write_word(0x2002, 0x7FFF); // Max pos (A0)

    cpu.step_instruction(&mut memory);

    // D0 should be sign extended to 0xFFFFFFFF
    assert_eq!(cpu.d[0], 0xFFFFFFFF);
    // A0 (register being loaded) should be sign extended.
    // Order: D0 then A0. So A0 is overwritten by the loaded value.
    assert_eq!(cpu.a[0], 0x00007FFF);
}

#[test]
fn test_movem_write_long_predec() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVEM.L D0/A0, -(A7)
    // Opcode: 0100 1000 11 100 111 (Size=L, Dir=R->M, Mode=-(A7)) -> 0x48E7
    // Mask:
    // Standard: D0=Bit0, A0=Bit8.
    // Pre-decrement (Reversed): A7=Bit0 ... A0=Bit7 ... D0=Bit15.
    // We want D0 and A0. D0 is Bit 15. A0 is Bit 7.
    // Mask = 1000 0000 1000 0000 -> 0x8080
    write_op(&mut memory, &[0x48E7, 0x8080]);

    cpu.a[7] = 0x8000;
    cpu.d[0] = 0xDD00DD00;
    cpu.a[0] = 0xAA00AA00;

    cpu.step_instruction(&mut memory);

    // Order for pre-decrement: A7->A0, then D7->D0.
    // High Addr -> Low Addr.
    // 1. A0 (Bit 7). Addr -= 4 -> 0x7FFC. Write A0.
    // 2. D0 (Bit 15). Addr -= 4 -> 0x7FF8. Write D0.

    // So Mem[0x7FFC] = A0
    //    Mem[0x7FF8] = D0

    assert_eq!(memory.read_long(0x7FFC), 0xAA00AA00);
    assert_eq!(memory.read_long(0x7FF8), 0xDD00DD00);
    assert_eq!(cpu.a[7], 0x7FF8);
}

#[test]
fn test_movem_write_word_control() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVEM.W D0/D1, $2000
    // Opcode: 0100 1000 10 111 001 (Size=W, Dir=R->M, Mode=Abs.L) -> 0x48B9
    // Mask: 0000 0000 0000 0011 (D0, D1) -> 0x0003
    write_op(&mut memory, &[0x48B9, 0x0003]);
    // Extension words for address
    memory.write_long(0x1004, 0x00002000); // 0x1002 is Mask. 0x1004 is Abs Addr.

    cpu.d[0] = 0x1234;
    cpu.d[1] = 0x5678;

    cpu.step_instruction(&mut memory);

    // Standard order: D0 then D1.
    // Addr = 0x2000. Write D0 (Word). Addr += 2. Write D1 (Word).

    assert_eq!(memory.read_word(0x2000), 0x1234);
    assert_eq!(memory.read_word(0x2002), 0x5678);
}

#[test]
fn test_movem_reg_to_mem_control_long() {
    let (mut cpu, mut memory) = create_cpu();

    // MOVEM.L D2/D3, (A0)
    // Opcode: 0100 1000 11 010 000 = 0x48D0
    // Mask: D2 (Bit 2), D3 (Bit 3). Mask = 0x000C.

    write_op(&mut memory, &[0x48D0, 0x000C]);

    cpu.d[2] = 0x55555555;
    cpu.d[3] = 0x66666666;
    cpu.a[0] = 0x3000;

    cpu.step_instruction(&mut memory);

    // Check A0 unchanged
    assert_eq!(cpu.a[0], 0x3000);

    // Check memory contents
    // Order: Low Reg (D2) to Low Addr, High Reg (D3) to High Addr
    assert_eq!(memory.read_long(0x3000), 0x55555555);
    assert_eq!(memory.read_long(0x3004), 0x66666666);
}

#[test]
fn test_movem_mem_to_reg_control_long() {
    let (mut cpu, mut memory) = create_cpu();

    // MOVEM.L (A0), D2/D3
    // Opcode: 0100 1100 11 010 000 = 0x4CD0
    // Mask: D2 (Bit 2), D3 (Bit 3). Mask = 0x000C.

    write_op(&mut memory, &[0x4CD0, 0x000C]);

    cpu.a[0] = 0x3000;
    memory.write_long(0x3000, 0x77777777);
    memory.write_long(0x3004, 0x88888888);

    cpu.step_instruction(&mut memory);

    // Check A0 unchanged
    assert_eq!(cpu.a[0], 0x3000);

    // Check registers
    assert_eq!(cpu.d[2], 0x77777777);
    assert_eq!(cpu.d[3], 0x88888888);
}

#[test]
fn test_movem_all_registers() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVEM.L D0-D7/A0-A7, -(A7)
    // Opcode: 0100 1000 11 100 111 (Size=L, Dir=R->M, Mode=-(A7)) -> 0x48E7
    // Mask: All bits set -> 0xFFFF
    write_op(&mut memory, &[0x48E7, 0xFFFF]);

    // Initialize registers
    for i in 0..8 {
        cpu.d[i] = 0xD0 + i as u32;
        cpu.a[i] = 0xA0 + i as u32;
    }

    // Set SP (A7) to a safe location
    cpu.a[7] = 0x8000;

    // Note: The A7 pushed is the INITIAL value (0x8000).
    // M68k documentation says: "The value of the stack pointer saved is the initial value".

    cpu.step_instruction(&mut memory);

    // Total 16 registers * 4 bytes = 64 bytes (0x40).
    // Final SP should be 0x8000 - 0x40 = 0x7FC0.
    assert_eq!(cpu.a[7], 0x7FC0);

    // Check last register written (D0) at lowest address (0x7FC0)
    assert_eq!(memory.read_long(0x7FC0), 0xD0);

    // Check first register written (A7) at highest address (0x7FFC)
    // Should be initial value (0x8000)
    assert_eq!(memory.read_long(0x7FFC), 0x8000);
}

// ============================================================================
// EXG Tests
// ============================================================================

#[test]
fn test_exg_data_data() {
    let (mut cpu, mut memory) = create_cpu();
    // EXG D0, D1
    // Opcode: 1100 000 1 01000 001 -> 0xC141
    write_op(&mut memory, &[0xC141]);
    cpu.d[0] = 0x11111111;
    cpu.d[1] = 0x22222222;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0x22222222);
    assert_eq!(cpu.d[1], 0x11111111);
}

#[test]
fn test_exg_addr_addr() {
    let (mut cpu, mut memory) = create_cpu();
    // EXG A0, A1
    // Opcode: 1100 000 1 01001 001 -> 0xC149
    write_op(&mut memory, &[0xC149]);
    cpu.a[0] = 0x33333333;
    cpu.a[1] = 0x44444444;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.a[0], 0x44444444);
    assert_eq!(cpu.a[1], 0x33333333);
}

#[test]
fn test_exg_data_addr() {
    let (mut cpu, mut memory) = create_cpu();
    // EXG D0, A0
    // Opcode: 1100 000 1 10001 000 -> 0xC188
    write_op(&mut memory, &[0xC188]);
    cpu.d[0] = 0x55555555;
    cpu.a[0] = 0x66666666;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0x66666666);
    assert_eq!(cpu.a[0], 0x55555555);
}
