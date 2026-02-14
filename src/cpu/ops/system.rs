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
    memory.write_long(cpu.a[7], return_addr);

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
        memory.write_long(cpu.a[7], cpu.pc);
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
