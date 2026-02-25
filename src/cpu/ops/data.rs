use crate::cpu::addressing::{calculate_ea, EffectiveAddress};
use crate::cpu::decoder::{AddressingMode, Size};
use crate::cpu::Cpu;
use crate::memory::MemoryInterface;

pub fn exec_move<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    dst: AddressingMode,
    memory: &mut M,
) -> u32 {
    let mut cycles = 4u32;

    let (src_ea, src_cycles) = calculate_ea(src, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += src_cycles;

    let val = cpu.cpu_read_ea(src_ea, size, memory);

    let (dst_ea, dst_cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += dst_cycles;

    cpu.cpu_write_ea(dst_ea, size, val, memory);

    // Update flags
    cpu.update_nz_flags(val, size);
    cpu.set_flag(crate::cpu::flags::CARRY, false);
    cpu.set_flag(crate::cpu::flags::OVERFLOW, false);

    // MOVE to register is faster than MOVE to memory
    if matches!(dst, AddressingMode::DataRegister(_)) {
        cycles
    } else {
        cycles + 4
    }
}

pub fn exec_movea<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> u32 {
    let mut cycles = 4u32;

    let (src_ea, src_cycles) = calculate_ea(src, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += src_cycles;

    let val = cpu.cpu_read_ea(src_ea, size, memory);

    // MOVEA sign-extends word to long
    let val = if size == Size::Word {
        (val as i16) as i32 as u32
    } else {
        val
    };

    cpu.a[dst_reg as usize] = val;

    cycles
}

pub fn exec_moveq(cpu: &mut Cpu, dst_reg: u8, data: u8) -> u32 {
    // MOVEQ sign-extends 8-bit data to 32 bits
    let val = (data as i8) as i32 as u32;
    cpu.d[dst_reg as usize] = val;

    // Update flags
    cpu.update_nz_flags(val, Size::Long);
    cpu.set_flag(crate::cpu::flags::CARRY, false);
    cpu.set_flag(crate::cpu::flags::OVERFLOW, false);

    4
}

pub fn exec_lea<M: MemoryInterface>(
    cpu: &mut Cpu,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> u32 {
    let (src_ea, cycles) =
        calculate_ea(src, Size::Long, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);

    if let EffectiveAddress::Memory(addr) = src_ea {
        cpu.a[dst_reg as usize] = addr;
    }

    cycles
}

pub fn exec_pea<M: MemoryInterface>(cpu: &mut Cpu, src: AddressingMode, memory: &mut M) -> u32 {
    let (src_ea, cycles) =
        calculate_ea(src, Size::Long, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);

    if let EffectiveAddress::Memory(addr) = src_ea {
        cpu.a[7] = cpu.a[7].wrapping_sub(4);
        cpu.write_long(cpu.a[7], addr, memory);
    }

    cycles + 4
}

pub fn exec_exg(cpu: &mut Cpu, rx: u8, ry: u8, mode: u8) -> u32 {
    match mode {
        0x40 => {
            // Data registers
            let temp = cpu.d[rx as usize];
            cpu.d[rx as usize] = cpu.d[ry as usize];
            cpu.d[ry as usize] = temp;
        }
        0x48 => {
            // Address registers
            let temp = cpu.a[rx as usize];
            cpu.a[rx as usize] = cpu.a[ry as usize];
            cpu.a[ry as usize] = temp;
        }
        0x50 => {
            // Data and Address register
            let temp = cpu.d[rx as usize];
            cpu.d[rx as usize] = cpu.a[ry as usize];
            cpu.a[ry as usize] = temp;
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
    let displacement = cpu.read_word(cpu.pc, memory) as i16;
    cpu.pc = cpu.pc.wrapping_add(2);
    let mut addr = (cpu.a[an as usize] as i32 + displacement as i32) as u32;

    if direction {
        // Register to memory
        let val = cpu.d[reg as usize];
        if size == Size::Word {
            cpu.write_byte(addr, (val >> 8) as u8, memory);
            cpu.write_byte(addr.wrapping_add(2), (val & 0xFF) as u8, memory);
        } else {
            cpu.write_byte(addr, (val >> 24) as u8, memory);
            cpu.write_byte(addr.wrapping_add(2), (val >> 16) as u8, memory);
            cpu.write_byte(addr.wrapping_add(4), (val >> 8) as u8, memory);
            cpu.write_byte(addr.wrapping_add(6), (val & 0xFF) as u8, memory);
        }
    } else {
        // Memory to register
        let mut val = 0u32;
        if size == Size::Word {
            val |= (cpu.cpu_read_memory(addr, Size::Byte, memory) << 8);
            val |= cpu.cpu_read_memory(addr.wrapping_add(2), Size::Byte, memory);
            cpu.d[reg as usize] = (cpu.d[reg as usize] & 0xFFFF0000) | val;
        } else {
            val |= (cpu.cpu_read_memory(addr, Size::Byte, memory) << 24);
            val |= (cpu.cpu_read_memory(addr.wrapping_add(2), Size::Byte, memory) << 16);
            val |= (cpu.cpu_read_memory(addr.wrapping_add(4), Size::Byte, memory) << 8);
            val |= cpu.cpu_read_memory(addr.wrapping_add(6), Size::Byte, memory);
            cpu.d[reg as usize] = val;
        }
    }

    if size == Size::Word {
        16
    } else {
        24
    }
}

pub fn exec_movem<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    to_memory: bool,
    ea: AddressingMode,
    memory: &mut M,
) -> u32 {
    let mut cycles = 0u32;
    let mask = cpu.read_word(cpu.pc, memory);
    cpu.pc = cpu.pc.wrapping_add(2);

    let reg_size = size.bytes();

    let base_addr = match ea {
        AddressingMode::AddressPreDecrement(reg) => cpu.a[reg as usize],
        AddressingMode::AddressPostIncrement(reg) => {
            let addr = cpu.a[reg as usize];
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
            // The mask is reversed: bit 0 corresponds to A7 (i=15), bit 15 to D0 (i=0)
            let mut m = mask;
            while m != 0 {
                let bit = m.trailing_zeros();
                m &= !(1 << bit);
                let i = (15 - bit) as usize;

                addr = addr.wrapping_sub(reg_size);
                let val = if i < 8 { cpu.d[i] } else { cpu.a[i - 8] };
                if size == Size::Word {
                    cpu.write_word(addr, val as u16, memory);
                } else {
                    cpu.write_long(addr, val, memory);
                }
                cycles += if size == Size::Word { 4 } else { 8 };
            }

            // Update An for predecrement mode
            if let AddressingMode::AddressPreDecrement(reg) = ea {
                cpu.a[reg as usize] = addr;
            }
        } else {
            // Normal: Store D0-D7, then A0-A7
            let mut m = mask;
            while m != 0 {
                let i = m.trailing_zeros();
                m &= !(1 << i);
                let i = i as usize;

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
    } else {
        // Memory to Registers
        let mut addr = base_addr;
        let mut m = mask;

        while m != 0 {
            let i = m.trailing_zeros();
            m &= !(1 << i);
            let i = i as usize;

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

        // Update An for postincrement mode
        if let AddressingMode::AddressPostIncrement(reg) = ea {
            cpu.a[reg as usize] = addr;
        }
    }

    cycles + 8
}

pub fn exec_swap(cpu: &mut Cpu, reg: u8) -> u32 {
    let val = cpu.d[reg as usize];
    let low = (val & 0xFFFF) << 16;
    let high = (val & 0xFFFF0000) >> 16;
    let res = low | high;
    cpu.d[reg as usize] = res;

    // Update flags
    cpu.update_nz_flags(res, Size::Long);
    cpu.set_flag(crate::cpu::flags::CARRY, false);
    cpu.set_flag(crate::cpu::flags::OVERFLOW, false);

    4
}

pub fn exec_ext(cpu: &mut Cpu, size: Size, reg: u8) -> u32 {
    let reg_idx = reg as usize;
    let val = cpu.d[reg_idx];

    let res = if size == Size::Word {
        // Byte to Word
        let low_byte = (val & 0xFF) as i8;
        (val & 0xFFFF0000) | (low_byte as i16 as u16 as u32)
    } else {
        // Word to Long
        let low_word = (val & 0xFFFF) as i16;
        low_word as i32 as u32
    };

    cpu.d[reg_idx] = res;

    // Update flags
    cpu.update_nz_flags(res, size);
    cpu.set_flag(crate::cpu::flags::CARRY, false);
    cpu.set_flag(crate::cpu::flags::OVERFLOW, false);

    4
}
