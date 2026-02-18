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
fn test_movem_reg_to_mem_predec() {
    let (mut cpu, mut memory) = create_cpu();

    // MOVEM.L D0/D1, -(A0)
    // Opcode: 0100 1000 11 100 000 = 0x48E0
    // Mask: D0 (Bit 0) and D1 (Bit 1).
    // In Predecrement mode, mask is reversed: Bit 0=A7... Bit 15=D0.
    // So D0 is Bit 15, D1 is Bit 14. Mask = 1100 0000 0000 0000 = 0xC000.

    write_op(&mut memory, &[0x48E0, 0xC000]);

    cpu.d[0] = 0x11111111;
    cpu.d[1] = 0x22222222;
    cpu.a[0] = 0x2000;

    cpu.step_instruction(&mut memory);

    // Check A0 decremented by 8 bytes (2 longs)
    assert_eq!(cpu.a[0], 0x1FF8);

    // Check memory contents
    // Order: D1 pushed first (High Addr), then D0 (Low Addr)
    // 0x1FF8: D0
    // 0x1FFC: D1
    assert_eq!(memory.read_long(0x1FF8), 0x11111111);
    assert_eq!(memory.read_long(0x1FFC), 0x22222222);
}

#[test]
fn test_movem_mem_to_reg_postinc() {
    let (mut cpu, mut memory) = create_cpu();

    // MOVEM.L (A0)+, D0/D1
    // Opcode: 0100 1100 11 011 000 = 0x4CD8
    // Mask: D0 (Bit 0), D1 (Bit 1). Mask = 0x0003.

    write_op(&mut memory, &[0x4CD8, 0x0003]);

    cpu.a[0] = 0x2000;
    memory.write_long(0x2000, 0x33333333); // For D0
    memory.write_long(0x2004, 0x44444444); // For D1

    cpu.step_instruction(&mut memory);

    // Check A0 incremented by 8 bytes
    assert_eq!(cpu.a[0], 0x2008);

    // Check registers
    assert_eq!(cpu.d[0], 0x33333333);
    assert_eq!(cpu.d[1], 0x44444444);
}

#[test]
fn test_movem_reg_to_mem_control() {
    let (mut cpu, mut memory) = create_cpu();

    // MOVEM.L D2/D3, (A0)
    // Opcode: 0100 1000 11 010 000 = 0x48D0
    // Mask: D2 (Bit 2), D3 (Bit 3). Mask = 0x000C.
    // Standard mask order: Bit 0=D0... Bit 15=A7.

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
fn test_movem_mem_to_reg_control() {
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
fn test_movem_word_size() {
    let (mut cpu, mut memory) = create_cpu();

    // MOVEM.W D0/D1, -(A0)
    // Opcode: 0100 1000 10 100 000 = 0x48A0 (Size bit 6=0)
    // Mask: D0 (Bit 15), D1 (Bit 14). Mask = 0xC000.

    write_op(&mut memory, &[0x48A0, 0xC000]);

    cpu.d[0] = 0x11112222;
    cpu.d[1] = 0x33334444;
    cpu.a[0] = 0x4000;

    cpu.step_instruction(&mut memory);

    // Check A0 decremented by 4 bytes (2 words)
    assert_eq!(cpu.a[0], 0x3FFC);

    // Check memory contents (Words)
    // 0x3FFC: D0.W (0x2222)
    // 0x3FFE: D1.W (0x4444)
    assert_eq!(memory.read_word(0x3FFC), 0x2222);
    assert_eq!(memory.read_word(0x3FFE), 0x4444);
}

#[test]
fn test_movem_sign_extension() {
    let (mut cpu, mut memory) = create_cpu();

    // MOVEM.W (A0)+, D0/A1
    // Opcode: 0100 1100 10 011 000 = 0x4C98 (Size bit 6=0)
    // Mask: D0 (Bit 0), A1 (Bit 9). Mask = 0x0201.

    write_op(&mut memory, &[0x4C98, 0x0201]);

    cpu.a[0] = 0x5000;
    memory.write_word(0x5000, 0xFFFF); // -1
    memory.write_word(0x5002, 0xFFFF); // -1

    // Pre-fill registers with known values to check upper bits
    cpu.d[0] = 0xAAAA0000;
    cpu.a[1] = 0xBBBB0000;

    cpu.step_instruction(&mut memory);

    // D0: Data Register Word load DOES sign extend for MOVEM.
    // Result: 0xFFFFFFFF
    assert_eq!(cpu.d[0], 0xFFFFFFFF);

    // A1: Address Register Word load DOES sign extend.
    // Result: 0xFFFFFFFF
    assert_eq!(cpu.a[1], 0xFFFFFFFF);
}

#[test]
fn test_movem_all_regs() {
    let (mut cpu, mut memory) = create_cpu();

    // MOVEM.L D0-D7/A0-A7, -(A7)  (Push All)
    // Opcode: 0100 1000 11 100 111 = 0x48E7
    // Mask: All bits set = 0xFFFF.

    write_op(&mut memory, &[0x48E7, 0xFFFF]);

    // Initialize registers
    for i in 0..8 {
        cpu.d[i] = 0xD0 + i as u32;
        cpu.a[i] = 0xA0 + i as u32;
    }
    // Set SP (A7) to a safe location
    cpu.a[7] = 0x8000;

    // Note: The A7 pushed is the INITIAL value (0x8000), not the decremented one?
    // M68k documentation says: "The value of the stack pointer saved is the initial value".
    // Let's verify if `exec_movem` handles this.
    // Code: `let base_addr = ... cpu.a[reg]`.
    // It uses `base_addr` which is the initial value.
    // Wait, for `-(An)`, `base_addr` IS the initial value.
    // Then inside loop: `addr = addr.wrapping_sub(reg_size)`.
    // Then `write...(addr, val)`.
    // The value written is `cpu.a[i]`.
    // If we are pushing A7 (i=15), we write `cpu.a[7]`.
    // Since `cpu.a[7]` hasn't been modified yet (modification happens AFTER loop), it writes the INITIAL value.
    // This matches M68k behavior.

    cpu.step_instruction(&mut memory);

    // Check A7 final value: Decremented by 16 * 4 = 64 bytes (0x40).
    // 0x8000 - 0x40 = 0x7FC0.
    assert_eq!(cpu.a[7], 0x7FC0);

    // Verify memory contents.
    // Order (Low to High): D0, D1... D7, A0... A7.
    let mut addr = 0x7FC0;
    for i in 0..8 {
        assert_eq!(memory.read_long(addr), 0xD0 + i as u32, "D{}", i);
        addr += 4;
    }
    for i in 0..8 {
        // A7 pushed value should be 0x8000
        let expected = if i == 7 { 0x8000 } else { 0xA0 + i as u32 };
        assert_eq!(memory.read_long(addr), expected, "A{}", i);
        addr += 4;
    }
}
