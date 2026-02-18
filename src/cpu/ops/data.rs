use crate::cpu::addressing::{calculate_ea, EffectiveAddress};
use crate::cpu::decoder::{AddressingMode, Size};
use crate::cpu::{flags, Cpu};
use crate::memory::MemoryInterface;

pub fn exec_move<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    dst: AddressingMode,
    memory: &mut M,
) -> u32 {
    let mut cycles = match size {
        Size::Byte | Size::Word => 4,
        Size::Long => 4, // MOVE.L is also 4 cycles for reg-reg
    };

    let (src_ea, src_cycles) = calculate_ea(src, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += src_cycles;
    let val = cpu.cpu_read_ea(src_ea, size, memory);

    let (dst_ea, dst_cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += dst_cycles;

    // MOVE flags: N and Z set according to data, V and C always cleared.
    cpu.update_nz_flags(val, size);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::CARRY, false);

    cpu.cpu_write_ea(dst_ea, size, val, memory);

    cycles
}

pub fn exec_movea<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> u32 {
    let cycles = match size {
        Size::Word => 4,
        Size::Long => 4,
        _ => 4,
    };

    let (src_ea, src_cycles) = calculate_ea(src, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let total_cycles = cycles + src_cycles;

    let val = match size {
        Size::Word => (cpu.cpu_read_ea(src_ea, size, memory) as i16) as i32 as u32,
        Size::Long => cpu.cpu_read_ea(src_ea, size, memory),
        _ => cpu.cpu_read_ea(src_ea, size, memory),
    };

    cpu.a[dst_reg as usize] = val;

    total_cycles
}

pub fn exec_moveq(cpu: &mut Cpu, dst_reg: u8, data: i8) -> u32 {
    let val = data as i32 as u32;
    cpu.d[dst_reg as usize] = val;

    cpu.update_nz_flags(val, Size::Long);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::CARRY, false);

    4
}

pub fn exec_lea<M: MemoryInterface>(
    cpu: &mut Cpu,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> u32 {
    let (ea, cycles) = calculate_ea(src, Size::Long, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);

    if let EffectiveAddress::Memory(addr) = ea {
        cpu.a[dst_reg as usize] = addr;
    }

    // LEA does not affect flags
    4 + cycles
}

pub fn exec_pea<M: MemoryInterface>(cpu: &mut Cpu, src: AddressingMode, memory: &mut M) -> u32 {
    let (ea, cycles) = calculate_ea(src, Size::Long, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);

    if let EffectiveAddress::Memory(addr) = ea {
        cpu.a[7] = cpu.a[7].wrapping_sub(4);
        cpu.write_long(cpu.a[7], addr, memory);
    }

    // PEA does not affect flags
    // Cycles: 12 + ea_cycles (standard 68000)
    12 + cycles
}

pub fn exec_exg(cpu: &mut Cpu, rx: u8, ry: u8, mode: u8) -> u32 {
    match mode {
        0x08 => {
            // Data registers
            cpu.d.swap(rx as usize, ry as usize);
        }
        0x09 => {
            // Address registers
            cpu.a.swap(rx as usize, ry as usize);
        }
        0x11 => {
            // Data and Address register
            std::mem::swap(&mut cpu.d[rx as usize], &mut cpu.a[ry as usize]);
        }
        _ => {}
    }
    6
}

pub fn exec_movep<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    reg: u8,
    an: u8,
    direction: bool,
    memory: &mut M,
) -> u32 {
    let addr = cpu.a[an as usize];
    let mut cycles = match size {
        Size::Word => 16,
        Size::Long => 24,
        _ => 0,
    };

    if direction {
        // Register to memory
        let val = cpu.d[reg as usize];
        match size {
            Size::Word => {
                memory.write_byte(addr, (val >> 8) as u8);
                memory.write_byte(addr.wrapping_add(2), (val & 0xFF) as u8);
            }
            Size::Long => {
                memory.write_byte(addr, (val >> 24) as u8);
                memory.write_byte(addr.wrapping_add(2), (val >> 16) as u8);
                memory.write_byte(addr.wrapping_add(4), (val >> 8) as u8);
                memory.write_byte(addr.wrapping_add(6), (val & 0xFF) as u8);
            }
            _ => cycles = 0,
        }
    } else {
        // Memory to register
        let mut val = 0u32;
        match size {
            Size::Word => {
                val |= (memory.read_byte(addr) as u32) << 8;
                val |= memory.read_byte(addr.wrapping_add(2)) as u32;
                cpu.d[reg as usize] = (cpu.d[reg as usize] & 0xFFFF0000) | val;
            }
            Size::Long => {
                val |= (memory.read_byte(addr) as u32) << 24;
                val |= (memory.read_byte(addr.wrapping_add(2)) as u32) << 16;
                val |= (memory.read_byte(addr.wrapping_add(4)) as u32) << 8;
                val |= memory.read_byte(addr.wrapping_add(6)) as u32;
                cpu.d[reg as usize] = val;
            }
            _ => cycles = 0,
        }
    }

    cycles
}

pub fn exec_movem<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    to_memory: bool,
    ea: AddressingMode,
    memory: &mut M,
) -> u32 {
    let mask = cpu.read_word(cpu.pc, memory);
    cpu.pc = cpu.pc.wrapping_add(2);

    let reg_size: u32 = if size == Size::Word { 2 } else { 4 };
    let mut cycles = 8u32;

    let base_addr = match ea {
        AddressingMode::AddressPostIncrement(reg) => {
            let addr = cpu.a[reg as usize];
            cycles += 4; // Cycles for (An)+
            addr
        }
        AddressingMode::AddressPreDecrement(reg) => {
            let addr = cpu.a[reg as usize];
            cycles += 6; // Cycles for -(An)
            addr
        }
        _ => {
            let (ea_result, ea_cycles) =
                calculate_ea(ea, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
            cycles += ea_cycles;
            match ea_result {
                EffectiveAddress::Memory(addr) => addr,
                _ => return cycles, // Invalid for MOVEM
            }
        }
    };

    if to_memory {
        // Registers to Memory
        let is_predec = matches!(ea, AddressingMode::AddressPreDecrement(_));
        let mut addr = base_addr;

        if is_predec {
            // Predecrement: Store A7-A0, then D7-D0 (reverse order)
            for i in (0..16).rev() {
                if (mask & (1 << (15 - i))) != 0 {
                    addr = addr.wrapping_sub(reg_size);
                    let val = if i < 8 { cpu.d[i] } else { cpu.a[i - 8] };
                    if size == Size::Word {
                        cpu.write_word(addr, val as u16, memory);
                    } else {
                        cpu.write_long(addr, val, memory);
                    }
                    cycles += if size == Size::Word { 4 } else { 8 };
                }
            }
            // Update An for predecrement mode
            if let AddressingMode::AddressPreDecrement(reg) = ea {
                cpu.a[reg as usize] = addr;
            }
        } else {
            // Normal: Store D0-D7, then A0-A7
            for i in 0..16 {
                if (mask & (1 << i)) != 0 {
                    let val = if i < 8 { cpu.d[i] } else { cpu.a[i - 8] };
                    if size == Size::Word {
                        cpu.write_word(addr, val as u16, memory);
                    } else {
                        cpu.write_long(addr, val, memory);
                    }
                    addr = addr.wrapping_add(reg_size);
                    cycles += if size == Size::Word { 4 } else { 8 };
                }
            }
        }
    } else {
        // Memory to Registers
        let mut addr = base_addr;

        for i in 0..16 {
            if (mask & (1 << i)) != 0 {
                if i < 8 {
                    // Data register: Word load is sign-extended to 32 bits, Long load is normal
                    if size == Size::Word {
                        cpu.d[i] = cpu.read_word(addr, memory) as i16 as i32 as u32;
                    } else {
                        cpu.d[i] = cpu.read_long(addr, memory);
                    }
                } else {
                    // Address register: Word load is sign-extended, Long load is normal
                    if size == Size::Word {
                        cpu.a[i - 8] = cpu.read_word(addr, memory) as i16 as i32 as u32;
                    } else {
                        cpu.a[i - 8] = cpu.read_long(addr, memory);
                    }
                }
                addr = addr.wrapping_add(reg_size);
                cycles += if size == Size::Word { 4 } else { 8 };
            }
        }

        // Update An for postincrement mode
        if let AddressingMode::AddressPostIncrement(reg) = ea {
            cpu.a[reg as usize] = addr;
        }
    }

    cycles
}

pub fn exec_swap(cpu: &mut Cpu, reg: u8) -> u32 {
    let val = cpu.d[reg as usize];
    let swapped = val.rotate_left(16);
    cpu.d[reg as usize] = swapped;

    cpu.update_nz_flags(swapped, Size::Long);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::CARRY, false);

    4
}

pub fn exec_ext(cpu: &mut Cpu, size: Size, reg: u8) -> u32 {
    let val = cpu.d[reg as usize];
    let result = match size {
        Size::Word => (val as i8) as i16 as u16 as u32,
        Size::Long => (val as i16) as i32 as u32,
        _ => val,
    };

    match size {
        Size::Word => cpu.d[reg as usize] = (cpu.d[reg as usize] & 0xFFFF0000) | result,
        Size::Long => cpu.d[reg as usize] = result,
        _ => {}
    }

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::CARRY, false);

    4
}
