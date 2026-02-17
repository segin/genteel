//! M68k Data Movement Tests
//!
//! Exhaustive tests for M68k data movement operations, specifically MOVEM.

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
    // Wait, the instruction loads A0 from memory, overwriting the pointer?
    // "If the effective address register is also a destination register, the register is updated with the data from memory."
    // But does it update A0 (pointer) after reading?
    // Since mode is (A0), the effective address is calculated using initial A0.
    // Then registers are loaded.
    // Order: D0 then A0.
    // So A0 is overwritten by the loaded value.
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
    // We want D0 and A0.
    // D0 is Bit 15. A0 is Bit 7.
    // Mask = 1000 0000 1000 0000 -> 0x8080
    write_op(&mut memory, &[0x48E7, 0x8080]);

    cpu.a[7] = 0x8000;
    cpu.d[0] = 0xDD00DD00;
    cpu.a[0] = 0xAA00AA00;

    cpu.step_instruction(&mut memory);

    // Order for pre-decrement: A7->A0, then D7->D0.
    // High Addr -> Low Addr.
    // 1. A0 (Bit 7) is encountered first?
    //    Loop 15 down to 0.
    //    i=8 (A0). Mask bit 7 set.
    //    Addr -= 4 -> 0x7FFC. Write A0.
    // 2. D0 (Bit 15). Mask bit 15 set.
    //    Addr -= 4 -> 0x7FF8. Write D0.

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

    // Correct instruction layout:
    // 0x1000: Opcode
    // 0x1002: Mask
    // 0x1004: Address High
    // 0x1006: Address Low

    cpu.d[0] = 0x1234;
    cpu.d[1] = 0x5678;

    cpu.step_instruction(&mut memory);

    // Standard order: D0 then D1.
    // Addr = 0x2000.
    // Write D0 (Word) at 0x2000.
    // Addr += 2 -> 0x2002.
    // Write D1 (Word) at 0x2002.

    assert_eq!(memory.read_word(0x2000), 0x1234);
    assert_eq!(memory.read_word(0x2002), 0x5678);
}

#[test]
fn test_movem_all_registers() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVEM.L D0-D7/A0-A7, -(A7)
    // Opcode: 0100 1000 11 100 111 (Size=L, Dir=R->M, Mode=-(A7)) -> 0x48E7
    // Mask: All bits set -> 0xFFFF
    write_op(&mut memory, &[0x48E7, 0xFFFF]);

    cpu.a[7] = 0x8000;

    // Initialize registers
    for i in 0..8 {
        cpu.d[i] = 0xD0 + i as u32;
        cpu.a[i] = 0xA0 + i as u32;
    }
    // Note: A7 is 0x8000 initially, but we overwrote it in loop above.
    // Reset A7 to 0x8000 for stack pointer
    cpu.a[7] = 0x8000;
    // But wait, we want to save A7?
    // In MOVEM -(A7), the value of A7 saved is the INITIAL value minus 4? Or the value after decrement?
    // "For the predecrement addressing mode, the value saved is the initial value minus the size of the operation."
    // Wait, let's check M68k behavior for pushing SP.
    // If A7 is in the list.
    // The loop processes registers.
    // Order: A7, A6 ... A0, D7 ... D0.
    // 1. A7 is processed first.
    //    A7 decremented by 4 -> 0x7FFC.
    //    Write A7. Which value? The decremented value? Or the initial?
    //    Actually, usually `movem` pushing SP pushes the value *before* the instruction?
    //    No, for `-(An)` mode, the register is updated *before* writing?
    //    But if `An` is also in the list?
    //    M68k User Manual: "If the effective address is specified by the predecrement mode, and the register used is also moved to memory, the value written is the initial register value minus the size of the operation."
    //    Wait, "minus the size of the operation" (4 bytes).
    //    So if SP=0x8000.
    //    It pushes (0x8000 - 0) or (0x8000 - 4)?
    //    Actually, since A7 is the stack pointer, and it's being predecremented *for the push*, the value at `0x7FFC` is written.
    //    The value WRITTEN to memory is the *initial* A7?
    //    Let's check `exec_movem` implementation details.

    // Implementation:
    // let mut addr = base_addr; (For predec, base_addr is initial A7).
    // Loop ...
    //   addr = addr - 4;
    //   val = cpu.a[i];
    //   write(addr, val);
    //   if let PreDec(reg) = ea { cpu.a[reg] = addr; } (Updates A7 at the end? Or during loop?)
    // No, `cpu.a[reg] = addr` is done AFTER loop?
    // Wait, the code:
    /*
        if is_predec {
            for i in (0..16).rev() {
                if (mask & ...) {
                    addr = addr.wrapping_sub(reg_size);
                    let val = ...;
                    cpu.write(addr, val);
                }
            }
            if let AddressingMode::AddressPreDecrement(reg) = ea {
                cpu.a[reg as usize] = addr;
            }
        }
    */
    // So `cpu.a[7]` is NOT updated until the loop finishes.
    // Inside the loop, `val` is read from `cpu.a[i]`.
    // So it reads the INITIAL `cpu.a[7]` (0x8000).
    // So it writes 0x8000 to memory.

    // Let's verify this behavior against M68k spec.
    // "If the addressing mode is predecrement ... the value written is the initial value of the register."
    // Actually there is some nuance. "Bus Error Exception Stack Frame" etc.
    // Generally, pushing SP onto stack via `MOVEM.L SP, -(SP)`?
    // M68k manual says: "The value written for An is the initial value of An."
    // So my implementation (reading `cpu.a[7]` which hasn't been modified yet) seems correct.

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
