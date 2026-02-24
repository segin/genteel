use crate::cpu::addressing::{calculate_ea, EffectiveAddress};
use crate::cpu::decoder::{AddressingMode, Condition, Size};
use crate::cpu::Cpu;
use crate::memory::MemoryInterface;

pub fn exec_bra<M: MemoryInterface>(cpu: &mut Cpu, displacement: i16, memory: &mut M) -> u32 {
    if displacement == 0 {
        // 16-bit displacement follows
        let disp = cpu.read_word(cpu.pc, memory) as i16;
        cpu.pc = (cpu.pc as i32 + disp as i32) as u32;
        10
    } else {
        cpu.pc = (cpu.pc.wrapping_sub(2) as i32 + 2 + displacement as i32) as u32;
        10
    }
}

pub fn exec_bsr<M: MemoryInterface>(cpu: &mut Cpu, displacement: i16, memory: &mut M) -> u32 {
    let return_addr = if displacement == 0 {
        cpu.pc + 2
    } else {
        cpu.pc
    };

    // Push return address
    cpu.a[7] = cpu.a[7].wrapping_sub(4);
    cpu.write_long(cpu.a[7], return_addr, memory);

    if displacement == 0 {
        let disp = cpu.read_word(cpu.pc, memory) as i16;
        cpu.pc = (cpu.pc as i32 + disp as i32) as u32;
        18
    } else {
        cpu.pc = (cpu.pc.wrapping_sub(2) as i32 + 2 + displacement as i32) as u32;
        18
    }
}

pub fn exec_bcc<M: MemoryInterface>(
    cpu: &mut Cpu,
    condition: Condition,
    displacement: i16,
    memory: &mut M,
) -> u32 {
    if cpu.test_condition(condition) {
        if displacement == 0 {
            let disp = cpu.read_word(cpu.pc, memory) as i16;
            cpu.pc = (cpu.pc as i32 + disp as i32) as u32;
            10
        } else {
            cpu.pc = (cpu.pc.wrapping_sub(2) as i32 + 2 + displacement as i32) as u32;
            10
        }
    } else {
        if displacement == 0 {
            cpu.pc = cpu.pc.wrapping_add(2);
        }
        8
    }
}

pub fn exec_scc<M: MemoryInterface>(
    cpu: &mut Cpu,
    condition: Condition,
    dst: AddressingMode,
    memory: &mut M,
) -> u32 {
    let mut cycles = 4u32;
    let (dst_ea, dst_cycles) =
        calculate_ea(dst, Size::Byte, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += dst_cycles;

    let val = if cpu.test_condition(condition) {
        0xFF
    } else {
        0x00
    };
    cpu.cpu_write_ea(dst_ea, Size::Byte, val, memory);

    cycles
        + if matches!(dst, AddressingMode::DataRegister(_)) {
            0
        } else {
            4
        }
}

pub fn exec_dbcc<M: MemoryInterface>(
    cpu: &mut Cpu,
    condition: Condition,
    reg: u8,
    memory: &mut M,
) -> u32 {
    if cpu.test_condition(condition) {
        cpu.pc = cpu.pc.wrapping_add(2); // Skip displacement word
        12
    } else {
        let counter = (cpu.d[reg as usize] as u16).wrapping_sub(1);
        cpu.d[reg as usize] = (cpu.d[reg as usize] & 0xFFFF0000) | counter as u32;

        if counter == 0xFFFF {
            cpu.pc = cpu.pc.wrapping_add(2);
            14
        } else {
            let disp = cpu.read_word(cpu.pc, memory) as i16;
            cpu.pc = (cpu.pc as i32 + disp as i32) as u32;
            10
        }
    }
}

pub fn exec_jmp<M: MemoryInterface>(cpu: &mut Cpu, dst: AddressingMode, memory: &mut M) -> u32 {
    let (ea, cycles) = calculate_ea(dst, Size::Long, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);

    if let EffectiveAddress::Memory(addr) = ea {
        cpu.pc = addr;
    }

    4 + cycles
}

pub fn exec_jsr<M: MemoryInterface>(cpu: &mut Cpu, dst: AddressingMode, memory: &mut M) -> u32 {
    let (ea, cycles) = calculate_ea(dst, Size::Long, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);

    if let EffectiveAddress::Memory(addr) = ea {
        // Push return address
        cpu.a[7] = cpu.a[7].wrapping_sub(4);
        cpu.write_long(cpu.a[7], cpu.pc, memory);
        cpu.pc = addr;
    }

    12 + cycles
}

pub fn exec_rts<M: MemoryInterface>(cpu: &mut Cpu, memory: &mut M) -> u32 {
    cpu.pc = memory.read_long(cpu.a[7]);
    cpu.a[7] = cpu.a[7].wrapping_add(4);
    16
}

pub fn exec_link<M: MemoryInterface>(
    cpu: &mut Cpu,
    reg: u8,
    displacement: i16,
    memory: &mut M,
) -> u32 {
    let old_an = cpu.a[reg as usize];
    cpu.push_long(old_an, memory);
    cpu.a[reg as usize] = cpu.a[7];
    cpu.a[7] = cpu.a[7].wrapping_add(displacement as u32);
    16
}

pub fn exec_unlk<M: MemoryInterface>(cpu: &mut Cpu, reg: u8, memory: &mut M) -> u32 {
    cpu.a[7] = cpu.a[reg as usize];
    let old_an = cpu.pop_long(memory);
    cpu.a[reg as usize] = old_an;
    12
}

pub fn exec_trap<M: MemoryInterface>(cpu: &mut Cpu, vector: u8, memory: &mut M) -> u32 {
    // TRAP #n uses vectors 32-47 (0x20-0x2F).
    cpu.process_exception(32 + vector as u32, memory)
}

pub fn exec_rte<M: MemoryInterface>(cpu: &mut Cpu, memory: &mut M) -> u32 {
    if (cpu.sr & 0x2000) == 0 {
        // Not supervisor
        return cpu.process_exception(8, memory); // Privilege Violation
    }

    let new_sr = cpu.pop_word(memory);
    let new_pc = cpu.pop_long(memory);

    cpu.set_sr(new_sr);
    cpu.pc = new_pc;

    20
}

pub fn exec_stop<M: MemoryInterface>(cpu: &mut Cpu, memory: &mut M) -> u32 {
    if (cpu.sr & 0x2000) == 0 {
        return cpu.process_exception(8, memory);
    }

    let imm = memory.read_word(cpu.pc);
    cpu.pc = cpu.pc.wrapping_add(2);
    cpu.set_sr(imm);
    cpu.halted = true; // STOP stops the processor until interrupt/reset.
                       // In emulator, we might just set a flag.
                       // For now, halted = true is close, but interrupts should wake it.
                       // We'll leave it as halted.
    4
}

pub fn exec_move_usp<M: MemoryInterface>(
    cpu: &mut Cpu,
    reg: u8,
    to_usp: bool,
    memory: &mut M,
) -> u32 {
    if (cpu.sr & 0x2000) == 0 {
        return cpu.process_exception(8, memory); // Privilege violation
    }
    if to_usp {
        cpu.usp = cpu.a[reg as usize];
    } else {
        cpu.a[reg as usize] = cpu.usp;
    }
    4
}

pub fn exec_rtr<M: MemoryInterface>(cpu: &mut Cpu, memory: &mut M) -> u32 {
    let ccr = cpu.pop_word(memory);
    let new_pc = cpu.pop_long(memory);

    // Only restore lower 5 bits (CCR portion)
    cpu.sr = (cpu.sr & 0xFF00) | (ccr & 0x00FF);
    cpu.pc = new_pc;

    20
}

pub fn exec_move_to_sr<M: MemoryInterface>(
    cpu: &mut Cpu,
    src: AddressingMode,
    memory: &mut M,
) -> u32 {
    if (cpu.sr & 0x2000) == 0 {
        return cpu.process_exception(8, memory); // Privilege violation
    }

    let mut cycles = 12u32;
    let (src_ea, src_cycles) =
        calculate_ea(src, Size::Word, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += src_cycles;

    let val = cpu.cpu_read_ea(src_ea, Size::Word, memory) as u16;
    cpu.set_sr(val);
    cycles
}

pub fn exec_move_from_sr<M: MemoryInterface>(
    cpu: &mut Cpu,
    dst: AddressingMode,
    memory: &mut M,
) -> u32 {
    // On 68000, this is not privileged. On 68010+, it is.
    let mut cycles = 6u32;
    let (dst_ea, dst_cycles) =
        calculate_ea(dst, Size::Word, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += dst_cycles;

    cpu.cpu_write_ea(dst_ea, Size::Word, cpu.sr as u32, memory);
    cycles
}

pub fn exec_move_to_ccr<M: MemoryInterface>(
    cpu: &mut Cpu,
    src: AddressingMode,
    memory: &mut M,
) -> u32 {
    let mut cycles = 12u32;
    let (src_ea, src_cycles) =
        calculate_ea(src, Size::Word, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += src_cycles;

    let val = cpu.cpu_read_ea(src_ea, Size::Word, memory) as u16;
    cpu.sr = (cpu.sr & 0xFF00) | (val & 0x00FF);
    cycles
}

pub fn exec_andi_to_ccr<M: MemoryInterface>(cpu: &mut Cpu, memory: &mut M) -> u32 {
    let imm = memory.read_word(cpu.pc) & 0x00FF;
    cpu.pc = cpu.pc.wrapping_add(2);
    cpu.sr = (cpu.sr & 0xFF00) | ((cpu.sr & imm) & 0x00FF);
    20
}

pub fn exec_andi_to_sr<M: MemoryInterface>(cpu: &mut Cpu, memory: &mut M) -> u32 {
    if (cpu.sr & 0x2000) == 0 {
        return cpu.process_exception(8, memory);
    }
    let imm = memory.read_word(cpu.pc);
    cpu.pc = cpu.pc.wrapping_add(2);
    cpu.set_sr(cpu.sr & imm);
    20
}

pub fn exec_ori_to_ccr<M: MemoryInterface>(cpu: &mut Cpu, memory: &mut M) -> u32 {
    let imm = memory.read_word(cpu.pc) & 0x00FF;
    cpu.pc = cpu.pc.wrapping_add(2);
    cpu.sr = (cpu.sr & 0xFF00) | ((cpu.sr | imm) & 0x00FF);
    20
}

pub fn exec_ori_to_sr<M: MemoryInterface>(cpu: &mut Cpu, memory: &mut M) -> u32 {
    if (cpu.sr & 0x2000) == 0 {
        return cpu.process_exception(8, memory);
    }
    let imm = memory.read_word(cpu.pc);
    cpu.pc = cpu.pc.wrapping_add(2);
    cpu.set_sr(cpu.sr | imm);
    20
}

pub fn exec_eori_to_ccr<M: MemoryInterface>(cpu: &mut Cpu, memory: &mut M) -> u32 {
    let imm = memory.read_word(cpu.pc) & 0x00FF;
    cpu.pc = cpu.pc.wrapping_add(2);
    cpu.sr = (cpu.sr & 0xFF00) | ((cpu.sr ^ imm) & 0x00FF);
    20
}

pub fn exec_eori_to_sr<M: MemoryInterface>(cpu: &mut Cpu, memory: &mut M) -> u32 {
    if (cpu.sr & 0x2000) == 0 {
        return cpu.process_exception(8, memory);
    }
    let imm = memory.read_word(cpu.pc);
    cpu.pc = cpu.pc.wrapping_add(2);
    cpu.set_sr(cpu.sr ^ imm);
    20
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::flags;
    use crate::cpu::Cpu;
    use crate::memory::Memory;

    fn create_test_cpu() -> (Cpu, Memory) {
        let mut memory = Memory::new(0x10000);
        // Initial SP and PC
        memory.write_long(0, 0x1000); // SP
        memory.write_long(4, 0x100); // PC
        let cpu = Cpu::new(&mut memory);
        (cpu, memory)
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
}
