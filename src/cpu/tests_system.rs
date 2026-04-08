#![cfg(test)]

use crate::cpu::flags;
use crate::cpu::ops::system::*;
use crate::cpu::test_utils::create_test_cpu;
use crate::memory::MemoryInterface;

#[test]
fn test_exec_bra_short_forward() {
    let (mut cpu, mut memory) = create_test_cpu();
    cpu.pc = 0x1002; // PC after instruction word
    let cycles = exec_bra(&mut cpu, 0x06, &mut memory);
    assert_eq!(cycles, 10);
    assert_eq!(cpu.pc, 0x1008);
}

#[test]
fn test_exec_bra_short_backward() {
    let (mut cpu, mut memory) = create_test_cpu();
    cpu.pc = 0x1002; // PC after instruction word
    let cycles = exec_bra(&mut cpu, -2, &mut memory); // 0xFE
    assert_eq!(cycles, 10);
    assert_eq!(cpu.pc, 0x1000);
}

#[test]
fn test_exec_bra_word_forward() {
    let (mut cpu, mut memory) = create_test_cpu();
    cpu.pc = 0x1002;
    memory.write_word(0x1002, 0x0100);
    let cycles = exec_bra(&mut cpu, 0, &mut memory);
    assert_eq!(cycles, 10);
    assert_eq!(cpu.pc, 0x1102);
}

#[test]
fn test_exec_bra_word_backward() {
    let (mut cpu, mut memory) = create_test_cpu();
    cpu.pc = 0x1102;
    memory.write_word(0x1102, -0x100i16 as u16); // -256
    let cycles = exec_bra(&mut cpu, 0, &mut memory);
    assert_eq!(cycles, 10);
    assert_eq!(cpu.pc, 0x1002);
}

#[test]
fn test_exec_trap_user_to_supervisor() {
    let (mut cpu, mut memory) = create_test_cpu();

    // Setup initial state: User Mode
    cpu.pc = 0x1000;
    cpu.sr = 0x0000; // User mode, no flags
    cpu.usp = 0x2000; // User stack
    cpu.ssp = 0x4000; // Supervisor stack
    cpu.a[7] = cpu.usp; // Active stack is USP in User mode

    // Setup vector table
    // TRAP #2 -> Vector 32 + 2 = 34. Address = 34 * 4 = 136 (0x88)
    memory.write_long(136, 0x3000); // Target PC

    // Call exec_trap
    let cycles = exec_trap(&mut cpu, 2, &mut memory);

    // Verify
    assert_eq!(cycles, 34); // process_exception returns 34
    assert_eq!(cpu.pc, 0x3000); // Jumped to vector

    // SR Check: Supervisor bit set, Trace bit cleared
    assert!((cpu.sr & flags::SUPERVISOR) != 0);
    assert!((cpu.sr & flags::TRACE) == 0);

    // Stack verification
    // Should have switched to SSP (0x4000)
    // Pushed PC (4 bytes) -> 0x3FFC
    // Pushed SR (2 bytes) -> 0x3FFA
    assert_eq!(cpu.a[7], 0x3FFA); // A7 should be SSP now
    assert_eq!(cpu.usp, 0x2000); // USP preserved

    assert_eq!(memory.read_word(0x3FFA), 0x0000); // Old SR (User mode)
    assert_eq!(memory.read_long(0x3FFC), 0x1000); // Old PC
}

#[test]
fn test_exec_trap_supervisor_to_supervisor() {
    let (mut cpu, mut memory) = create_test_cpu();

    // Setup initial state: Supervisor Mode
    cpu.pc = 0x1000;
    cpu.sr = flags::SUPERVISOR;
    cpu.usp = 0x2000;
    cpu.ssp = 0x4000;
    cpu.a[7] = cpu.ssp; // Active stack is SSP

    // Setup vector table
    // TRAP #3 -> Vector 32 + 3 = 35. Address = 35 * 4 = 140 (0x8C)
    memory.write_long(140, 0x5000); // Target PC

    // Call exec_trap
    let cycles = exec_trap(&mut cpu, 3, &mut memory);

    assert_eq!(cycles, 34);
    assert_eq!(cpu.pc, 0x5000);
    assert!((cpu.sr & flags::SUPERVISOR) != 0);

    // Stack verification
    // Should continue using SSP (0x4000)
    // Pushed PC -> 0x3FFC
    // Pushed SR -> 0x3FFA
    assert_eq!(cpu.a[7], 0x3FFA);

    assert_eq!(memory.read_word(0x3FFA), flags::SUPERVISOR); // Old SR
    assert_eq!(memory.read_long(0x3FFC), 0x1000); // Old PC
}

#[test]
fn test_exec_trap_vectors() {
    // Iterate through all 16 vectors (0-15) for TRAP #n
    for vector in 0..16u8 {
        let (mut cpu, mut memory) = create_test_cpu();

        // Setup
        let initial_pc = 0x200;
        cpu.pc = initial_pc;
        cpu.sr = 0x0000; // User mode, no flags
        let initial_sp = cpu.a[7];

        // Set exception vector handler address
        // TRAP #n uses vectors 32-47.
        // Address = (32 + vector) * 4
        let vector_num = 32 + vector as u32;
        let handler_addr = 0x4000 + (vector as u32 * 0x10);
        memory.write_long(vector_num * 4, handler_addr);

        // Execute TRAP
        let cycles = exec_trap(&mut cpu, vector, &mut memory);

        // Verify Cycles: Standard exception processing takes 34 cycles
        assert_eq!(cycles, 34, "TRAP #{} should take 34 cycles", vector);

        // Verify PC Jump
        assert_eq!(
            cpu.pc, handler_addr,
            "TRAP #{} should jump to handler",
            vector
        );

        // Verify Stack Usage
        // 6 bytes pushed: 4 bytes (PC) + 2 bytes (SR)
        assert_eq!(cpu.a[7], initial_sp - 6, "SP should be decremented by 6");

        // Check pushed SR (at SP)
        let pushed_sr = memory.read_word(cpu.a[7]);
        assert_eq!(pushed_sr, 0x0000, "Pushed SR should match old SR");

        // Check pushed PC (at SP+2)
        let pushed_pc = memory.read_long(cpu.a[7] + 2);
        assert_eq!(pushed_pc, initial_pc, "Pushed PC should match old PC");

        // Verify New SR
        // Supervisor bit (0x2000) should be set
        // Trace bit (0x8000) should be cleared
        assert_eq!(cpu.sr & 0x2000, 0x2000, "Supervisor bit should be set");
        assert_eq!(cpu.sr & 0x8000, 0x0000, "Trace bit should be cleared");
    }
}

#[test]
fn test_exec_trap_trace_bit() {
    let (mut cpu, mut memory) = create_test_cpu();
    let vector = 5;

    cpu.pc = 0x200;
    // Set Trace bit (bit 15) and verify it gets cleared in new SR but saved in old SR
    cpu.sr = 0x8000;

    // Set vector
    let handler = 0x5000;
    memory.write_long((32 + vector as u32) * 4, handler);

    exec_trap(&mut cpu, vector, &mut memory);

    // Old SR on stack should have Trace bit set
    let pushed_sr = memory.read_word(cpu.a[7]);
    assert_eq!(
        pushed_sr & 0x8000,
        0x8000,
        "Pushed SR should preserve Trace bit"
    );

    // New SR should have Trace bit cleared
    assert_eq!(cpu.sr & 0x8000, 0, "New SR should have Trace bit cleared");

    // And Supervisor bit set
    assert_eq!(cpu.sr & 0x2000, 0x2000, "Supervisor bit should be set");
}

#[test]
fn test_exec_move_usp() {
    let (mut cpu, mut memory) = create_test_cpu();

    // 1. Test Privilege Violation (User Mode)
    cpu.sr = 0x0000; // User mode
                     // Setup Exception Vector 8 (Privilege Violation)
    let vector_addr = 8 * 4;
    let handler_addr = 0x4000;
    memory.write_long(vector_addr, handler_addr);

    let initial_pc = cpu.pc;

    // Execute MOVE USP, A0 (to_usp = false)
    let cycles = exec_move_usp(&mut cpu, 0, false, &mut memory);

    // Should trigger exception (34 cycles)
    assert_eq!(cycles, 34);
    assert_eq!(cpu.pc, handler_addr);
    assert_eq!(cpu.sr & flags::SUPERVISOR, flags::SUPERVISOR); // Switched to supervisor

    // Verify pushed PC matches instruction address
    let pushed_pc = memory.read_long(cpu.a[7] + 2);
    assert_eq!(pushed_pc, initial_pc);

    // 2. Test Move to USP (MOVE An, USP)
    // Reset CPU to Supervisor
    cpu.sr = flags::SUPERVISOR;
    cpu.pc = 0x100;

    let val_to_write = 0xDEADBEEF;
    let reg_idx = 1;
    cpu.a[reg_idx] = val_to_write;
    cpu.usp = 0; // Clear USP

    let cycles = exec_move_usp(&mut cpu, reg_idx as u8, true, &mut memory);

    assert_eq!(cycles, 4);
    assert_eq!(cpu.usp, val_to_write);

    // 3. Test Move from USP (MOVE USP, An)
    let val_in_usp = 0xCAFEBABE;
    let reg_idx = 2;
    cpu.usp = val_in_usp;
    cpu.a[reg_idx] = 0;

    let cycles = exec_move_usp(&mut cpu, reg_idx as u8, false, &mut memory);

    assert_eq!(cycles, 4);
    assert_eq!(cpu.a[reg_idx], val_in_usp);
}

#[test]
fn test_exec_rtr() {
    let (mut cpu, mut memory) = create_test_cpu();

    // Setup
    let initial_sp = 0x2000;
    cpu.a[7] = initial_sp;
    // Set SR to have high byte bits set (Supervisor, Int Mask)
    // and low byte cleared to verify update.
    cpu.sr = 0x2700;

    // Target state
    let target_pc = 0x4000;
    let target_ccr = 0x001F; // All flags set (X, N, Z, V, C)

    // Push PC (4 bytes)
    cpu.push_long(target_pc, &mut memory);
    // Push CCR (2 bytes)
    // exec_rtr pops word, but only uses low byte for CCR.
    // We push a word.
    cpu.push_word(target_ccr, &mut memory);

    // Verify stack setup
    // SP should be 0x2000 - 4 - 2 = 0x1FFA
    assert_eq!(cpu.a[7], 0x1FFA);

    // Execute RTR
    let cycles = exec_rtr(&mut cpu, &mut memory);

    // Verify Return Cycles
    assert_eq!(cycles, 20);

    // Verify PC updated
    assert_eq!(cpu.pc, target_pc);

    // Verify SR
    // Upper byte should be preserved (0x27)
    // Lower byte should be target_ccr low byte (0x1F)
    assert_eq!(cpu.sr & 0xFF00, 0x2700, "SR upper byte should be preserved");
    assert_eq!(
        cpu.sr & 0x00FF,
        target_ccr & 0x00FF,
        "SR lower byte should match popped CCR"
    );

    // Verify SP restored
    assert_eq!(cpu.a[7], initial_sp, "SP should be restored");
}

#[test]
fn test_exec_stop_privilege_violation() {
    let (mut cpu, mut memory) = create_test_cpu();

    // Setup state to non-supervisor (user mode)
    cpu.sr = 0x0000;
    let initial_sp = cpu.a[7];
    cpu.pc = 0x1000;

    let cycles = exec_stop(&mut cpu, &mut memory);

    // Privilege violation takes 34 cycles
    assert_eq!(cycles, 34);

    // Supervisor bit should be set
    assert_eq!(cpu.sr & flags::SUPERVISOR, flags::SUPERVISOR);

    // It pushed PC and SR onto the supervisor stack
    // Ssp should be initial_ssp - 6 (since privilege violation pushes pc + sr)
    assert_eq!(cpu.a[7], initial_sp - 6);
}

#[test]
fn test_exec_stop_supervisor() {
    let (mut cpu, mut memory) = create_test_cpu();

    // Set supervisor
    cpu.sr = flags::SUPERVISOR;
    cpu.pc = 0x1000;

    // Next word in memory is the new SR
    memory.write_word(0x1000, 0x2700); // Set supervisor and interrupt mask 7

    let cycles = exec_stop(&mut cpu, &mut memory);

    assert_eq!(cycles, 4);
    assert!(cpu.halted);
    assert_eq!(cpu.pc, 0x1002);
    assert_eq!(cpu.sr & flags::SUPERVISOR, flags::SUPERVISOR);
    assert_eq!(cpu.sr & flags::INTERRUPT_MASK, flags::INTERRUPT_MASK);
}

#[test]
fn test_exec_bsr_short() {
    let (mut cpu, mut memory) = create_test_cpu();
    cpu.pc = 0x1002;
    cpu.a[7] = 0x2000;

    let cycles = exec_bsr(&mut cpu, 0x06, &mut memory);

    assert_eq!(cycles, 18);
    assert_eq!(cpu.pc, 0x1008);
    assert_eq!(cpu.a[7], 0x1FFC);
    assert_eq!(memory.read_long(0x1FFC), 0x1002);
}

#[test]
fn test_exec_bsr_word() {
    let (mut cpu, mut memory) = create_test_cpu();
    cpu.pc = 0x1002;
    cpu.a[7] = 0x2000;
    memory.write_word(0x1002, 0x0100);

    let cycles = exec_bsr(&mut cpu, 0, &mut memory);

    assert_eq!(cycles, 18);
    assert_eq!(cpu.pc, 0x1102);
    assert_eq!(cpu.a[7], 0x1FFC);
    assert_eq!(memory.read_long(0x1FFC), 0x1004);
}

#[test]
fn test_exec_bcc() {
    use crate::cpu::decoder::Condition;
    let (mut cpu, mut memory) = create_test_cpu();

    // Test branch taken
    cpu.pc = 0x1002;
    cpu.sr = flags::ZERO; // Set Zero flag, so Equal (EQ) condition is true
    let cycles = exec_bcc(&mut cpu, Condition::Equal, 0x06, &mut memory);
    assert_eq!(cycles, 10);
    assert_eq!(cpu.pc, 0x1008);

    // Test branch not taken
    cpu.pc = 0x1002;
    cpu.sr = 0; // Clear Zero flag, so Equal (EQ) condition is false
    let cycles = exec_bcc(&mut cpu, Condition::Equal, 0x06, &mut memory);
    assert_eq!(cycles, 8); // Wait, exec_bcc returns 10 if taken, 8 if not taken (depending on implementation, let's check).
    assert_eq!(cpu.pc, 0x1002);
}

#[test]
fn test_exec_scc() {
    use crate::cpu::decoder::{Condition, AddressingMode};
    let (mut cpu, mut memory) = create_test_cpu();

    // Test true condition (should set to 0xFF)
    cpu.sr = flags::ZERO; // Set Equal condition
    let dst = AddressingMode::DataRegister(0);
    cpu.d[0] = 0x12345678; // Initial value

    let cycles = exec_scc(&mut cpu, Condition::Equal, dst, &mut memory);
    assert_eq!(cycles, 4);
    assert_eq!(cpu.d[0], 0x123456FF);

    // Test false condition (should set to 0x00)
    cpu.sr = 0; // Clear Equal condition
    cpu.d[0] = 0x12345678; // Initial value

    let cycles = exec_scc(&mut cpu, Condition::Equal, dst, &mut memory);
    assert_eq!(cycles, 4);
    assert_eq!(cpu.d[0], 0x12345600);
}

#[test]
fn test_exec_dbcc() {
    use crate::cpu::decoder::Condition;
    let (mut cpu, mut memory) = create_test_cpu();

    // Test condition met (does not loop)
    cpu.pc = 0x1002;
    cpu.sr = flags::ZERO; // Set Equal condition
    cpu.d[0] = 5;

    let cycles = exec_dbcc(&mut cpu, Condition::Equal, 0, &mut memory);
    assert_eq!(cycles, 12);
    assert_eq!(cpu.pc, 0x1004); // PC advances past word
    assert_eq!(cpu.d[0], 5);    // Reg not decremented

    // Test condition not met, reg > -1 (loops)
    cpu.pc = 0x1002;
    memory.write_word(0x1002, 0x0006); // displacement
    cpu.sr = 0; // Clear condition
    cpu.d[0] = 5;

    let cycles = exec_dbcc(&mut cpu, Condition::Equal, 0, &mut memory);
    assert_eq!(cycles, 10);
    assert_eq!(cpu.pc, 0x1008); // Branch taken (1002 + 6 = 1008)
    assert_eq!(cpu.d[0], 4);    // Reg decremented

    // Test condition not met, reg == -1 after decrement (does not loop)
    cpu.pc = 0x1002;
    cpu.sr = 0;
    cpu.d[0] = 0;

    let cycles = exec_dbcc(&mut cpu, Condition::Equal, 0, &mut memory);
    assert_eq!(cycles, 14);
    assert_eq!(cpu.pc, 0x1004); // PC advances past word
    assert_eq!(cpu.d[0] as i16, -1);
}

#[test]
fn test_exec_jmp() {
    use crate::cpu::decoder::AddressingMode;
    let (mut cpu, mut memory) = create_test_cpu();

    cpu.pc = 0x1000;
    cpu.a[0] = 0x2000;
    let dst = AddressingMode::AddressIndirect(0);

    let cycles = exec_jmp(&mut cpu, dst, &mut memory);
    assert_eq!(cycles, 8); // Depending on AddressingMode cycles, might need to adjust
    assert_eq!(cpu.pc, 0x2000);
}

#[test]
fn test_exec_jsr() {
    use crate::cpu::decoder::AddressingMode;
    let (mut cpu, mut memory) = create_test_cpu();

    cpu.pc = 0x1000;
    cpu.a[7] = 0x3000;
    cpu.a[0] = 0x2000;
    let dst = AddressingMode::AddressIndirect(0);

    let _cycles = exec_jsr(&mut cpu, dst, &mut memory);
    // Return address is PC (0x1000) - jsr instruction might push different PC, let's test what it pushes
    assert_eq!(cpu.pc, 0x2000);
    assert_eq!(cpu.a[7], 0x2FFC);
    assert_eq!(memory.read_long(0x2FFC), 0x1000);
}

#[test]
fn test_exec_rts() {
    let (mut cpu, mut memory) = create_test_cpu();

    cpu.a[7] = 0x2FFC;
    memory.write_long(0x2FFC, 0x4000); // Return address

    let cycles = exec_rts(&mut cpu, &mut memory);
    assert_eq!(cycles, 16);
    assert_eq!(cpu.pc, 0x4000);
    assert_eq!(cpu.a[7], 0x3000);
}

#[test]
fn test_exec_rte() {
    let (mut cpu, mut memory) = create_test_cpu();

    cpu.sr = flags::SUPERVISOR;
    cpu.a[7] = 0x3000;
    cpu.push_long(0x5000, &mut memory); // PC
    cpu.push_word(0x001F, &mut memory); // SR (User mode, all flags set)

    let cycles = exec_rte(&mut cpu, &mut memory);
    assert_eq!(cycles, 20); // standard cycles for RTE
    assert_eq!(cpu.pc, 0x5000);
    assert_eq!(cpu.sr, 0x001F); // Entire SR updated
    assert_eq!(cpu.sr & flags::SUPERVISOR, 0); // Should be in user mode now
    assert_eq!(cpu.a[7], cpu.usp); // Stack pointer changed to USP
}

#[test]
fn test_exec_link_unlk() {
    let (mut cpu, mut memory) = create_test_cpu();

    // Initial state
    cpu.a[7] = 0x3000;
    cpu.a[0] = 0x12345678; // Frame pointer reg
    let displacement: i16 = -16;

    // Test LINK
    let cycles = exec_link(&mut cpu, 0, displacement, &mut memory);
    assert_eq!(cycles, 16);

    // SP should be decremented by 4 (push An), then set to An, then decremented by 16
    // 0x3000 - 4 = 0x2FFC
    // A0 = 0x2FFC
    // SP = 0x2FFC - 16 = 0x2FEC
    assert_eq!(memory.read_long(0x2FFC), 0x12345678); // Old A0 pushed
    assert_eq!(cpu.a[0], 0x2FFC); // A0 updated to new frame pointer
    assert_eq!(cpu.a[7], 0x2FEC); // SP allocated locals

    // Test UNLK
    let cycles_unlk = exec_unlk(&mut cpu, 0, &mut memory);
    assert_eq!(cycles_unlk, 12);

    // SP should be loaded from A0 (0x2FFC)
    // Then An should be popped from stack (0x12345678)
    // SP should increment by 4 to 0x3000
    assert_eq!(cpu.a[7], 0x3000);
    assert_eq!(cpu.a[0], 0x12345678);
}
