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

pub fn exec_reset<M: MemoryInterface>(cpu: &mut Cpu, memory: &mut M) -> u32 {
    if (cpu.sr & 0x2000) == 0 {
        return cpu.process_exception(8, memory);
    }
    // RESET asserts the RESET line for 124 cycles, plus instruction overhead.
    // Total 132 cycles. No internal CPU state changes.
    132
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
    use crate::cpu::test_utils::create_cpu;
    use crate::cpu::flags;
    use crate::cpu::decoder::{Condition, AddressingMode};

    #[test]
    fn test_exec_bsr_short() {
        let (mut cpu, mut memory) = create_cpu();
        cpu.pc = 0x1002;
        let initial_sp = cpu.a[7];

        let cycles = exec_bsr(&mut cpu, 0x06, &mut memory);

        assert_eq!(cycles, 18);
        assert_eq!(cpu.pc, 0x1008);
        assert_eq!(cpu.a[7], initial_sp - 4);
        assert_eq!(memory.read_long(cpu.a[7]), 0x1002);
    }

    #[test]
    fn test_exec_bsr_word() {
        let (mut cpu, mut memory) = create_cpu();
        cpu.pc = 0x1002;
        let initial_sp = cpu.a[7];
        memory.write_word(0x1002, 0x0100);

        let cycles = exec_bsr(&mut cpu, 0, &mut memory);

        assert_eq!(cycles, 18);
        assert_eq!(cpu.pc, 0x1102);
        assert_eq!(cpu.a[7], initial_sp - 4);
        assert_eq!(memory.read_long(cpu.a[7]), 0x1004); // Return address is instruction PC + 2
    }

    #[test]
    fn test_exec_jsr() {
        let (mut cpu, mut memory) = create_cpu();
        cpu.pc = 0x1000;
        let initial_sp = cpu.a[7];

        // JSR to Absolute Short (AddressingMode::AbsoluteShort)
        let addr_mode = AddressingMode::AbsoluteShort;
        // The word after the PC is the absolute short address
        memory.write_word(0x1000, 0x2000);

        let cycles = exec_jsr(&mut cpu, addr_mode, &mut memory);

        assert_eq!(cycles, 12 + 8); // 12 + 8 for AbsoluteShort
        assert_eq!(cpu.pc, 0x2000);
        assert_eq!(cpu.a[7], initial_sp - 4);
        assert_eq!(memory.read_long(cpu.a[7]), 0x1002);
    }

    #[test]
    fn test_exec_rts() {
        let (mut cpu, mut memory) = create_cpu();
        let initial_sp = cpu.a[7];

        cpu.a[7] = initial_sp - 4;
        memory.write_long(cpu.a[7], 0x2000);

        let cycles = exec_rts(&mut cpu, &mut memory);

        assert_eq!(cycles, 16);
        assert_eq!(cpu.pc, 0x2000);
        assert_eq!(cpu.a[7], initial_sp);
    }

    #[test]
    fn test_exec_bcc() {
        let (mut cpu, mut memory) = create_cpu();

        // Condition true, short displacement
        cpu.pc = 0x1002;
        // Condition::Always true
        let mut cycles = exec_bcc(&mut cpu, Condition::True, 0x06, &mut memory);
        assert_eq!(cycles, 10);
        assert_eq!(cpu.pc, 0x1008);

        // Condition true, word displacement
        cpu.pc = 0x1002;
        memory.write_word(0x1002, 0x0100);
        cycles = exec_bcc(&mut cpu, Condition::True, 0, &mut memory);
        assert_eq!(cycles, 10);
        assert_eq!(cpu.pc, 0x1102);

        // Condition false, short displacement
        cpu.pc = 0x1002;
        // Condition::False
        cycles = exec_bcc(&mut cpu, Condition::False, 0x06, &mut memory);
        assert_eq!(cycles, 8);
        assert_eq!(cpu.pc, 0x1002);

        // Condition false, word displacement
        cpu.pc = 0x1002;
        memory.write_word(0x1002, 0x0100);
        cycles = exec_bcc(&mut cpu, Condition::False, 0, &mut memory);
        assert_eq!(cycles, 8);
        assert_eq!(cpu.pc, 0x1004);
    }

    #[test]
    fn test_exec_scc() {
        let (mut cpu, mut memory) = create_cpu();

        // Condition true
        let mut cycles = exec_scc(&mut cpu, Condition::True, AddressingMode::DataRegister(0), &mut memory);
        assert_eq!(cycles, 4); // 4 + 0
        assert_eq!(cpu.d[0] & 0xFF, 0xFF);

        // Condition false
        cycles = exec_scc(&mut cpu, Condition::False, AddressingMode::DataRegister(0), &mut memory);
        assert_eq!(cycles, 4); // 4 + 0
        assert_eq!(cpu.d[0] & 0xFF, 0x00);

        // Memory addressing
        cycles = exec_scc(&mut cpu, Condition::True, AddressingMode::AddressIndirect(0), &mut memory);
        assert_eq!(cycles, 4 + 4 + 4); // base 4, EA calculation 4, write 4
        assert_eq!(memory.read_byte(cpu.a[0]), 0xFF);
    }

    #[test]
    fn test_exec_dbcc() {
        let (mut cpu, mut memory) = create_cpu();

        // Condition true (branch not taken, loop terminates)
        cpu.pc = 0x1000;
        let mut cycles = exec_dbcc(&mut cpu, Condition::True, 0, &mut memory);
        assert_eq!(cycles, 12);
        assert_eq!(cpu.pc, 0x1002); // Skip displacement word

        // Condition false, counter > 0 (branch taken)
        cpu.pc = 0x1000;
        cpu.d[0] = 0x00000005;
        memory.write_word(0x1000, 0x0100);
        cycles = exec_dbcc(&mut cpu, Condition::False, 0, &mut memory);
        assert_eq!(cycles, 10);
        assert_eq!(cpu.d[0] & 0xFFFF, 0x0004);
        assert_eq!(cpu.pc, 0x1100);

        // Condition false, counter == 0 before decrement (loop terminates, counter wraps to 0xFFFF)
        cpu.pc = 0x1000;
        cpu.d[0] = 0x00000000;
        cycles = exec_dbcc(&mut cpu, Condition::False, 0, &mut memory);
        assert_eq!(cycles, 14);
        assert_eq!(cpu.d[0] & 0xFFFF, 0xFFFF);
        assert_eq!(cpu.pc, 0x1002); // Skip displacement word
    }

    #[test]
    fn test_exec_jmp() {
        let (mut cpu, mut memory) = create_cpu();

        cpu.pc = 0x1000;

        // JMP Absolute Short
        memory.write_word(0x1000, 0x3000);
        let cycles = exec_jmp(&mut cpu, AddressingMode::AbsoluteShort, &mut memory);

        assert_eq!(cycles, 4 + 8); // 4 + ea calculation for absolute short
        assert_eq!(cpu.pc, 0x3000);
    }

    #[test]
    fn test_exec_link() {
        let (mut cpu, mut memory) = create_cpu();

        let initial_sp = 0x2000;
        cpu.a[7] = initial_sp;
        cpu.a[0] = 0x12345678; // Old frame pointer

        // Allocate 16 bytes on stack (-16)
        let cycles = exec_link(&mut cpu, 0, -16, &mut memory);

        assert_eq!(cycles, 16);
        assert_eq!(cpu.a[7], initial_sp - 4 - 16); // Stack pointer decremented by 4 (push An) and then added displacement
        assert_eq!(cpu.a[0], initial_sp - 4);      // An points to the pushed value
        assert_eq!(memory.read_long(cpu.a[0]), 0x12345678); // Old An saved on stack
    }

    #[test]
    fn test_exec_unlk() {
        let (mut cpu, mut memory) = create_cpu();

        let old_sp = 0x2000;
        let frame_pointer = old_sp - 4;
        let new_sp = frame_pointer - 16;

        cpu.a[7] = new_sp;
        cpu.a[0] = frame_pointer;
        memory.write_long(frame_pointer, 0x12345678); // Simulate old An

        let cycles = exec_unlk(&mut cpu, 0, &mut memory);

        assert_eq!(cycles, 12);
        assert_eq!(cpu.a[7], old_sp);
        assert_eq!(cpu.a[0], 0x12345678); // Old An restored
    }

    #[test]
    fn test_exec_rte_supervisor() {
        let (mut cpu, mut memory) = create_cpu();

        cpu.sr = flags::SUPERVISOR;
        let initial_sp = 0x2000;
        cpu.a[7] = initial_sp - 6;
        cpu.usp = initial_sp;

        memory.write_word(cpu.a[7], 0x001F); // Old SR (all CCR flags set, user mode)
        memory.write_long(cpu.a[7] + 2, 0x3000); // Old PC

        let cycles = exec_rte(&mut cpu, &mut memory);

        assert_eq!(cycles, 20);
        assert_eq!(cpu.usp, initial_sp); // In user mode, a[7] is USP
        assert_eq!(cpu.sr, 0x001F);
        assert_eq!(cpu.pc, 0x3000);
    }

    #[test]
    fn test_exec_rte_user() {
        let (mut cpu, mut memory) = create_cpu();

        // 1. Ensure in Supervisor Mode to swap stack pointers correctly
        cpu.set_sr(flags::SUPERVISOR);
        cpu.ssp = 0x8000;
        cpu.a[7] = 0x8000;

        // 2. Switch to User Mode
        cpu.set_sr(0x0000);
        let initial_usp = 0x7000;
        cpu.a[7] = initial_usp;
        cpu.pc = 0x1000;

        // Setup Privilege Violation vector (vector 8)
        memory.write_long(8 * 4, 0x4000);

        let cycles = exec_rte(&mut cpu, &mut memory);

        assert_eq!(cycles, 34); // Privilege violation exception processing time
        assert_eq!(cpu.pc, 0x4000);
        assert_eq!(cpu.sr & flags::SUPERVISOR, flags::SUPERVISOR); // Switched to supervisor
    }

    #[test]
    fn test_exec_reset_supervisor() {
        let (mut cpu, mut memory) = create_cpu();

        cpu.sr = flags::SUPERVISOR;

        let cycles = exec_reset(&mut cpu, &mut memory);

        assert_eq!(cycles, 132); // RESET assertion duration
    }

    #[test]
    fn test_exec_reset_user() {
        let (mut cpu, mut memory) = create_cpu();

        cpu.set_sr(flags::SUPERVISOR);
        cpu.ssp = 0x8000;
        cpu.a[7] = 0x8000;

        cpu.set_sr(0x0000); // User mode
        cpu.a[7] = 0x7000;
        cpu.pc = 0x1000;

        // Setup Privilege Violation vector (vector 8)
        memory.write_long(8 * 4, 0x4000);

        let cycles = exec_reset(&mut cpu, &mut memory);

        assert_eq!(cycles, 34); // Privilege violation exception processing time
        assert_eq!(cpu.pc, 0x4000);
        assert_eq!(cpu.sr & flags::SUPERVISOR, flags::SUPERVISOR); // Switched to supervisor
    }

    #[test]
    fn test_exec_move_to_sr() {
        let (mut cpu, mut memory) = create_cpu();

        cpu.sr = flags::SUPERVISOR;
        let cycles = exec_move_to_sr(&mut cpu, AddressingMode::DataRegister(0), &mut memory);

        assert_eq!(cycles, 12);
        assert_eq!(cpu.sr, 0x0000); // d0 is 0

        // Privilege violation
        cpu.set_sr(flags::SUPERVISOR);
        cpu.ssp = 0x8000;
        cpu.a[7] = 0x8000;
        cpu.set_sr(0x0000);
        cpu.a[7] = 0x7000;

        memory.write_long(8 * 4, 0x4000);
        let cycles_user = exec_move_to_sr(&mut cpu, AddressingMode::DataRegister(0), &mut memory);
        assert_eq!(cycles_user, 34);
    }

    #[test]
    fn test_exec_move_from_sr() {
        let (mut cpu, mut memory) = create_cpu();

        cpu.sr = 0x271F;
        let cycles = exec_move_from_sr(&mut cpu, AddressingMode::DataRegister(0), &mut memory);

        assert_eq!(cycles, 6);
        assert_eq!(cpu.d[0] & 0xFFFF, 0x271F);
    }

    #[test]
    fn test_exec_move_to_ccr() {
        let (mut cpu, mut memory) = create_cpu();

        cpu.sr = 0x2700;
        cpu.d[0] = 0x0000001F;
        let cycles = exec_move_to_ccr(&mut cpu, AddressingMode::DataRegister(0), &mut memory);

        assert_eq!(cycles, 12);
        assert_eq!(cpu.sr, 0x271F); // Upper byte preserved, lower byte set
    }

    #[test]
    fn test_exec_andi_to_ccr() {
        let (mut cpu, mut memory) = create_cpu();

        cpu.sr = 0x271F;
        cpu.pc = 0x1000;
        memory.write_word(0x1000, 0x000A); // AND with 0x0A

        let cycles = exec_andi_to_ccr(&mut cpu, &mut memory);

        assert_eq!(cycles, 20);
        assert_eq!(cpu.sr, 0x270A); // Upper preserved, lower ANDed
        assert_eq!(cpu.pc, 0x1002);
    }

    #[test]
    fn test_exec_andi_to_sr() {
        let (mut cpu, mut memory) = create_cpu();

        cpu.sr = 0x271F;
        cpu.pc = 0x1000;
        memory.write_word(0x1000, 0x070A); // AND with 0x070A

        let cycles = exec_andi_to_sr(&mut cpu, &mut memory);

        assert_eq!(cycles, 20);
        assert_eq!(cpu.sr, 0x070A); // Both bytes ANDed
        assert_eq!(cpu.pc, 0x1002);

        // Privilege violation
        cpu.set_sr(flags::SUPERVISOR);
        cpu.ssp = 0x8000;
        cpu.a[7] = 0x8000;
        cpu.set_sr(0x0000);
        cpu.a[7] = 0x7000;

        memory.write_long(8 * 4, 0x4000);
        let cycles_user = exec_andi_to_sr(&mut cpu, &mut memory);
        assert_eq!(cycles_user, 34);
    }

    #[test]
    fn test_exec_ori_to_ccr() {
        let (mut cpu, mut memory) = create_cpu();

        cpu.sr = 0x2705;
        cpu.pc = 0x1000;
        memory.write_word(0x1000, 0x000A); // OR with 0x0A

        let cycles = exec_ori_to_ccr(&mut cpu, &mut memory);

        assert_eq!(cycles, 20);
        assert_eq!(cpu.sr, 0x270F);
        assert_eq!(cpu.pc, 0x1002);
    }

    #[test]
    fn test_exec_ori_to_sr() {
        let (mut cpu, mut memory) = create_cpu();

        cpu.sr = 0x2005;
        cpu.pc = 0x1000;
        memory.write_word(0x1000, 0x070A); // OR with 0x070A

        let cycles = exec_ori_to_sr(&mut cpu, &mut memory);

        assert_eq!(cycles, 20);
        assert_eq!(cpu.sr, 0x270F);
        assert_eq!(cpu.pc, 0x1002);

        // Privilege violation
        cpu.set_sr(flags::SUPERVISOR);
        cpu.ssp = 0x8000;
        cpu.a[7] = 0x8000;
        cpu.set_sr(0x0000);
        cpu.a[7] = 0x7000;

        memory.write_long(8 * 4, 0x4000);
        let cycles_user = exec_ori_to_sr(&mut cpu, &mut memory);
        assert_eq!(cycles_user, 34);
    }

    #[test]
    fn test_exec_eori_to_ccr() {
        let (mut cpu, mut memory) = create_cpu();

        cpu.sr = 0x270F;
        cpu.pc = 0x1000;
        memory.write_word(0x1000, 0x000A); // XOR with 0x0A

        let cycles = exec_eori_to_ccr(&mut cpu, &mut memory);

        assert_eq!(cycles, 20);
        assert_eq!(cpu.sr, 0x2705);
        assert_eq!(cpu.pc, 0x1002);
    }

    #[test]
    fn test_exec_eori_to_sr() {
        let (mut cpu, mut memory) = create_cpu();

        cpu.sr = 0x270F;
        cpu.pc = 0x1000;
        memory.write_word(0x1000, 0x070A); // XOR with 0x070A

        let cycles = exec_eori_to_sr(&mut cpu, &mut memory);

        assert_eq!(cycles, 20);
        assert_eq!(cpu.sr, 0x2005);
        assert_eq!(cpu.pc, 0x1002);

        // Privilege violation
        cpu.set_sr(flags::SUPERVISOR);
        cpu.ssp = 0x8000;
        cpu.a[7] = 0x8000;
        cpu.set_sr(0x0000);
        cpu.a[7] = 0x7000;

        memory.write_long(8 * 4, 0x4000);
        let cycles_user = exec_eori_to_sr(&mut cpu, &mut memory);
        assert_eq!(cycles_user, 34);
    }
}
