//! M68k Control Flow Tests
//!
//! Tests for branches, jumps, subroutines, and exceptions.
//! Covers Bcc, DBcc, Scc, JMP, JSR, RTS, RTR, RTE, LINK, UNLK, TRAP.

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
// BRA Tests
// ============================================================================

#[test]
fn test_bra_forward_short() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x6006]); // BRA.S +6
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x1008); // 0x1000 + 2 + 6
}

#[test]
fn test_bra_backward_short() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x60FE]); // BRA.S -2 (infinite loop)
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x1000); // Back to start
}

#[test]
fn test_bra_word_displacement() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x6000, 0x0100]); // BRA.W +256
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x1102); // 0x1000 + 2 + 256
}

// ============================================================================
// Bcc Tests - All 16 Conditions
// ============================================================================

#[test]
fn test_bcc_carry_clear() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x6406]); // BCC.S +6
    cpu.set_flag(flags::CARRY, false);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x1008); // Branch taken
}

#[test]
fn test_bcc_carry_set_no_branch() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x6406]); // BCC.S +6
    cpu.set_flag(flags::CARRY, true);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x1002); // Branch not taken
}

#[test]
fn test_bcs_carry_set() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x6506]); // BCS.S +6
    cpu.set_flag(flags::CARRY, true);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x1008);
}

#[test]
fn test_beq_zero_set() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x6706]); // BEQ.S +6
    cpu.set_flag(flags::ZERO, true);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x1008);
}

#[test]
fn test_bne_zero_clear() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x6606]); // BNE.S +6
    cpu.set_flag(flags::ZERO, false);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x1008);
}

#[test]
fn test_bmi_negative_set() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x6B06]); // BMI.S +6
    cpu.set_flag(flags::NEGATIVE, true);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x1008);
}

#[test]
fn test_bpl_negative_clear() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x6A06]); // BPL.S +6
    cpu.set_flag(flags::NEGATIVE, false);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x1008);
}

#[test]
fn test_bvs_overflow_set() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x6906]); // BVS.S +6
    cpu.set_flag(flags::OVERFLOW, true);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x1008);
}

#[test]
fn test_bvc_overflow_clear() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x6806]); // BVC.S +6
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x1008);
}

#[test]
fn test_bhi_unsigned_higher() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x6206]); // BHI.S +6 (C=0 AND Z=0)
    cpu.set_flag(flags::CARRY, false);
    cpu.set_flag(flags::ZERO, false);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x1008);
}

#[test]
fn test_bls_unsigned_lower_same() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x6306]); // BLS.S +6 (C=1 OR Z=1)
    cpu.set_flag(flags::CARRY, true);
    cpu.set_flag(flags::ZERO, false);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x1008);
}

#[test]
fn test_bge_signed_ge() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x6C06]); // BGE.S +6 (N XOR V = 0)
    cpu.set_flag(flags::NEGATIVE, false);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x1008);
}

#[test]
fn test_blt_signed_lt() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x6D06]); // BLT.S +6 (N XOR V = 1)
    cpu.set_flag(flags::NEGATIVE, true);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x1008);
}

#[test]
fn test_bgt_signed_gt() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x6E06]); // BGT.S +6 (N XOR V = 0 AND Z = 0)
    cpu.set_flag(flags::NEGATIVE, false);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::ZERO, false);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x1008);
}

#[test]
fn test_ble_signed_le() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x6F06]); // BLE.S +6 (N XOR V = 1 OR Z = 1)
    cpu.set_flag(flags::ZERO, true);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x1008);
}

// ============================================================================
// DBcc Tests
// ============================================================================

#[test]
fn test_dbf_loop() {
    let (mut cpu, mut memory) = create_cpu();
    // DBF decrements first, then checks. Counter wraps 3->2->1->0->-1.
    // At -1 (0xFFFF), it exits. So 4 iterations from counter=3.
    write_op(&mut memory, &[0x51C8, 0xFFFE]); // DBF D0, -2 (back to start)
    cpu.d[0] = 3;

    let mut iterations = 0;
    while iterations < 10 {
        cpu.step_instruction(&mut memory);
        iterations += 1;
        if cpu.pc == 0x1004 {
            break;
        } // Fell through
        cpu.pc = 0x1000; // Reset for next iteration
    }
    assert_eq!(iterations, 4);
}

#[test]
fn test_dbeq_condition_true() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x57C8, 0xFFFC]); // DBEQ D0, -4
    cpu.d[0] = 100;
    cpu.set_flag(flags::ZERO, true); // Condition true = no loop
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x1004); // Falls through
}

// ============================================================================
// Scc Tests
// ============================================================================

#[test]
fn test_st_always_true() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x50C0]); // ST D0
    cpu.d[0] = 0;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0xFF);
}

#[test]
fn test_sf_always_false() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x51C0]); // SF D0
    cpu.d[0] = 0xFF;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x00);
}

#[test]
fn test_seq_zero_set() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x57C0]); // SEQ D0
    cpu.d[0] = 0;
    cpu.set_flag(flags::ZERO, true);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0xFF);
}

#[test]
fn test_sne_zero_clear() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x56C0]); // SNE D0
    cpu.d[0] = 0;
    cpu.set_flag(flags::ZERO, false);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0xFF);
}

// ============================================================================
// JMP Tests
// ============================================================================

#[test]
fn test_jmp_absolute() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4EF9, 0x0002, 0x0000]); // JMP $00020000
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x20000);
}

#[test]
fn test_jmp_indirect() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4ED0]); // JMP (A0)
    cpu.a[0] = 0x3000;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x3000);
}

// ============================================================================
// JSR/RTS Tests
// ============================================================================

#[test]
fn test_jsr_rts_roundtrip() {
    let (mut cpu, mut memory) = create_cpu();
    // JSR $2000.W
    write_op(&mut memory, &[0x4EB8, 0x2000]);
    // Put RTS at $2000
    memory.write_word(0x2000, 0x4E75);

    cpu.step_instruction(&mut memory); // JSR
    assert_eq!(cpu.pc, 0x2000);
    assert_eq!(cpu.a[7], 0x7FFC); // Stack pushed

    cpu.step_instruction(&mut memory); // RTS
    assert_eq!(cpu.pc, 0x1004); // Return address
    assert_eq!(cpu.a[7], 0x8000);
}

#[test]
fn test_bsr_rts() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x6100, 0x0012]); // BSR.W +18 (from PC of ext word 0x1002 -> 0x1014)
    memory.write_word(0x1014, 0x4E75); // RTS at target (0x1002 + 0x12 = 0x1014)

    cpu.step_instruction(&mut memory); // BSR
    assert_eq!(cpu.pc, 0x1014);

    cpu.step_instruction(&mut memory); // RTS
    assert_eq!(cpu.pc, 0x1004);
}

// ============================================================================
// LINK/UNLK Tests
// ============================================================================

#[test]
fn test_link_unlk() {
    let (mut cpu, mut memory) = create_cpu();
    // LINK A6, #-8
    write_op(&mut memory, &[0x4E56, 0xFFF8]);
    cpu.a[6] = 0x11111111;
    cpu.a[7] = 0x8000;

    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.a[7], 0x7FF4); // SP - 4 - 8
    assert_eq!(cpu.a[6], 0x7FFC); // Old SP - 4
    assert_eq!(memory.read_long(0x7FFC), 0x11111111);

    // UNLK A6
    write_op(&mut memory, &[0x4E5E]);
    cpu.invalidate_cache();
    cpu.pc = 0x1000;
    cpu.invalidate_cache();
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.a[6], 0x11111111);
    assert_eq!(cpu.a[7], 0x8000);
}

// ============================================================================
// TRAP Tests
// ============================================================================

#[test]
fn test_trap_vector() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4E40]); // TRAP #0
    memory.write_long(0x80, 0x3000); // Vector 32 (TRAP #0)
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x3000);
}

#[test]
fn test_trap_15() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4E4F]); // TRAP #15
    memory.write_long(0xBC, 0x4000); // Vector 47 (TRAP #15)
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x4000);
}

// ============================================================================
// NOP Test
// ============================================================================

#[test]
fn test_nop() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4E71]); // NOP
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x1002);
}

// ============================================================================
// ILLEGAL Test
// ============================================================================

#[test]
fn test_illegal() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4AFC]); // ILLEGAL
    memory.write_long(0x10, 0x5000); // Illegal instruction vector (vector 4)
    cpu.step_instruction(&mut memory);
    // ILLEGAL triggers exception 4, which reads from vector 4*4 = 0x10
    assert_eq!(cpu.pc, 0x5000);
}

// ============================================================================
// TST Tests
// ============================================================================

#[test]
fn test_tst_b_zero() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4A00]); // TST.B D0
    cpu.d[0] = 0;
    cpu.step_instruction(&mut memory);
    assert!(cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::NEGATIVE));
}

#[test]
fn test_tst_b_negative() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4A00]); // TST.B D0
    cpu.d[0] = 0x80;
    cpu.step_instruction(&mut memory);
    assert!(cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::ZERO));
}

#[test]
fn test_tst_w() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4A40]); // TST.W D0
    cpu.d[0] = 0x8000;
    cpu.step_instruction(&mut memory);
    assert!(cpu.get_flag(flags::NEGATIVE));
}

#[test]
fn test_tst_l() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4A80]); // TST.L D0
    cpu.d[0] = 0x80000000;
    cpu.step_instruction(&mut memory);
    assert!(cpu.get_flag(flags::NEGATIVE));
}

// ============================================================================
// STOP Tests
// ============================================================================

#[test]
fn test_stop_privilege_violation() {
    let (mut cpu, mut memory) = create_cpu();

    // STOP #$2000 opcode: 0x4E72, immediate: 0x2000
    write_op(&mut memory, &[0x4E72, 0x2000]);

    // Setup Stacks
    cpu.ssp = 0x8000; // Valid Supervisor Stack
    cpu.usp = 0xA000; // Valid User Stack

    // Set User Mode
    cpu.sr &= !flags::SUPERVISOR;
    cpu.a[7] = cpu.usp; // Active stack is now USP

    // Set Vector 8 (Privilege Violation)
    memory.write_long(32, 0x4000); // 8 * 4 = 32

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.pc, 0x4000);
    // Exception should set supervisor bit
    assert!(cpu.sr & flags::SUPERVISOR != 0);
    // Should NOT be halted
    assert!(!cpu.halted);
}

#[test]
fn test_stop_supervisor_behavior() {
    let (mut cpu, mut memory) = create_cpu();

    // STOP #$2200 opcode: 0x4E72, immediate: 0x2200 (Supervisor + Interrupt mask 2)
    write_op(&mut memory, &[0x4E72, 0x2200]);

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.sr, 0x2200);
    assert!(cpu.halted);
}

// ============================================================================
// RTE Tests
// ============================================================================

#[test]
fn test_rte_privilege_violation() {
    let (mut cpu, mut memory) = create_cpu();

    // RTE opcode
    write_op(&mut memory, &[0x4E73]);

    // Setup Stacks
    cpu.ssp = 0x8000;
    cpu.usp = 0xA000;

    // Set User Mode
    cpu.sr &= !flags::SUPERVISOR;
    cpu.a[7] = cpu.usp;

    // Set Vector 8 (Privilege Violation)
    memory.write_long(32, 0x4000);

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.pc, 0x4000);
    assert!(cpu.sr & flags::SUPERVISOR != 0);
}

#[test]
fn test_rte_supervisor() {
    let (mut cpu, mut memory) = create_cpu();

    // RTE opcode
    write_op(&mut memory, &[0x4E73]);

    // Push stack frame for RTE (SR, PC)
    // RTE pops SR (Word), then PC (Long)
    // Initial SP is 0x8000 (from create_cpu)

    let target_pc = 0x2000;
    let target_sr = 0x0000; // User mode, no flags

    // Manual Push
    cpu.a[7] = cpu.a[7].wrapping_sub(4);
    memory.write_long(cpu.a[7], target_pc);
    cpu.a[7] = cpu.a[7].wrapping_sub(2);
    memory.write_word(cpu.a[7], target_sr);

    // CPU is already in supervisor mode from create_cpu()

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.pc, target_pc);
    assert_eq!(cpu.sr, target_sr);
    // Should now be in user mode because we popped 0x0000 into SR
    assert_eq!(cpu.sr & flags::SUPERVISOR, 0);
}
