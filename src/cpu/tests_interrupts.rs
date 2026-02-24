//! M68k External Interrupt Tests
//!
//! Tests for interrupt handling, masking, and NMI logic.

#![cfg(test)]

use crate::cpu::flags;
use crate::cpu::Cpu;
use crate::memory::{Memory, MemoryInterface};

fn create_cpu() -> (Cpu, Memory) {
    let mut memory = Memory::new(0x100000);
    let mut cpu = Cpu::new(&mut memory);
    cpu.pc = 0x1000;
    cpu.a[7] = 0x8000;
    // Set SR to 0x2700 (Supervisor + Interrupt mask 7) by default in new(),
    // but let's make it clear and adjustable in tests.
    cpu.sr = 0x2000; // Supervisor, Mask 0
    (cpu, memory)
}

fn write_op(memory: &mut Memory, opcodes: &[u16]) {
    let mut addr = 0x1000u32;
    for &op in opcodes {
        memory.write_word(addr, op);
        addr += 2;
    }
}

#[test]
fn test_interrupt_masked() {
    let (mut cpu, mut memory) = create_cpu();

    // Set mask to 4
    cpu.sr = (cpu.sr & !flags::INTERRUPT_MASK) | 0x0400;

    // Request level 3 (should be ignored)
    cpu.request_interrupt(3);

    // Write NOP
    write_op(&mut memory, &[0x4E71]);

    // Step
    let cycles = cpu.step_instruction(&mut memory);

    // Should execute NOP (4 cycles) not interrupt (44 cycles)
    assert_eq!(cycles, 4);
    assert_eq!(cpu.pc, 0x1002);
    // Interrupt still pending
    assert_eq!(cpu.pending_interrupt, 3);
}

#[test]
fn test_interrupt_processing() {
    let (mut cpu, mut memory) = create_cpu();

    // Set mask to 3
    cpu.sr = (cpu.sr & !flags::INTERRUPT_MASK) | 0x0300;

    // Request level 4 (should be processed)
    cpu.request_interrupt(4);

    // Vector 28 (Level 4 Autovector) -> 0x70
    memory.write_long(0x70, 0x2000);

    // Write NOP at ISR
    memory.write_word(0x2000, 0x4E71);

    // Step
    let cycles = cpu.step_instruction(&mut memory);

    // Should be interrupt cycles (44)
    assert_eq!(cycles, 44);
    assert_eq!(cpu.pc, 0x2000);

    // Check Stack
    // Pushed PC (0x1000) and SR (Old SR)
    let pushed_sr = memory.read_word(cpu.a[7]);
    let pushed_pc = memory.read_long(cpu.a[7] + 2);

    assert_eq!(pushed_sr & flags::INTERRUPT_MASK, 0x0300);
    assert_eq!(pushed_pc, 0x1000);

    // Check New SR
    // Supervisor set, Trace cleared, Mask updated to 4
    assert!(cpu.get_flag(flags::SUPERVISOR));
    assert!(!cpu.get_flag(flags::TRACE));
    assert_eq!(cpu.sr & flags::INTERRUPT_MASK, 0x0400);

    // Pending interrupt should be cleared (if no other pending)
    assert_eq!(cpu.pending_interrupt, 0);
}

#[test]
fn test_nmi_level_7() {
    let (mut cpu, mut memory) = create_cpu();

    // Set mask to 7 (should block everything but 7)
    cpu.sr = (cpu.sr & !flags::INTERRUPT_MASK) | 0x0700;

    // Request level 7
    cpu.request_interrupt(7);

    // Vector 31 (Level 7 Autovector) -> 0x7C
    memory.write_long(0x7C, 0x3000);

    // Step
    let cycles = cpu.step_instruction(&mut memory);

    assert_eq!(cycles, 44);
    assert_eq!(cpu.pc, 0x3000);
    assert_eq!(cpu.sr & flags::INTERRUPT_MASK, 0x0700); // Mask stays 7
}

#[test]
fn test_interrupt_clears_halted() {
    let (mut cpu, mut memory) = create_cpu();

    cpu.halted = true;
    cpu.sr = (cpu.sr & !flags::INTERRUPT_MASK) | 0x0000;

    cpu.request_interrupt(1);

    // Vector 25 (Level 1) -> 0x64
    memory.write_long(0x64, 0x4000);

    let cycles = cpu.step_instruction(&mut memory);

    assert_eq!(cycles, 44);
    assert!(!cpu.halted);
}

#[test]
fn test_multiple_interrupts_priority() {
    let (mut cpu, mut memory) = create_cpu();

    cpu.sr = (cpu.sr & !flags::INTERRUPT_MASK) | 0x0000;

    // Request 3 and 5
    cpu.request_interrupt(3);
    cpu.request_interrupt(5);

    assert_eq!(cpu.pending_interrupt, 5); // Highest

    // Vector 29 (Level 5) -> 0x74
    memory.write_long(0x74, 0x5000);

    // Write NOP at ISR
    memory.write_word(0x5000, 0x4E71);

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.pc, 0x5000);
    assert_eq!(cpu.sr & flags::INTERRUPT_MASK, 0x0500);

    // Now pending should be 3
    assert_eq!(cpu.pending_interrupt, 3);

    // Should 3 fire? No, because current mask is 5.

    let cycles = cpu.step_instruction(&mut memory);
    assert_eq!(cycles, 4); // NOP
    assert_eq!(cpu.pc, 0x5002);

    // Restore SR to 0 to unmask level 3
    // Manually setting SR to unmask
    cpu.sr = (cpu.sr & !flags::INTERRUPT_MASK) | 0x0000;

    // Vector 27 (Level 3) -> 0x6C
    memory.write_long(0x6C, 0x6000);

    let cycles = cpu.step_instruction(&mut memory);
    assert_eq!(cycles, 44);
    assert_eq!(cpu.pc, 0x6000);
}
