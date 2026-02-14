use crate::cpu::addressing::{calculate_ea, read_ea, write_ea};
use crate::cpu::decoder::{AddressingMode, Size};
use crate::cpu::flags;
use crate::cpu::Cpu;
use crate::memory::MemoryInterface;

pub fn exec_add<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    dst: AddressingMode,
    direction: bool,
    memory: &mut M,
) -> u32 {
    let mut cycles = 4u32;

    // Source is always the EA when direction=false, Dn when direction=true
    let (src_mode, dst_mode) = if direction {
        (
            AddressingMode::DataRegister(((cpu.pc.wrapping_sub(2) >> 9) & 7) as u8),
            dst,
        )
    } else {
        (src, dst)
    };

    let (src_ea, src_cycles) =
        calculate_ea(src_mode, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += src_cycles;
    let src_val = cpu.cpu_read_ea(src_ea, size, memory);

    let (dst_ea, dst_cycles) =
        calculate_ea(dst_mode, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += dst_cycles;
    let dst_val = cpu.cpu_read_ea(dst_ea, size, memory);

    let (result, carry, overflow) = cpu.add_with_flags(src_val, dst_val, size);

    cpu.cpu_write_ea(dst_ea, size, result, memory);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::CARRY, carry);
    cpu.set_flag(flags::EXTEND, carry);
    cpu.set_flag(flags::OVERFLOW, overflow);

    cycles
}

pub fn exec_adda<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> u32 {
    let mut cycles = 4u32;

    let (src_ea, src_cycles) = calculate_ea(src, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += src_cycles;

    let src_val = cpu.cpu_read_ea(src_ea, size, memory);

    // Sign-extend source to 32 bits
    let src_val = match size {
        Size::Word => (src_val as i16) as i32 as u32,
        Size::Long => src_val,
        Size::Byte => src_val,
    };

    cpu.a[dst_reg as usize] = cpu.a[dst_reg as usize].wrapping_add(src_val);

    // ADDA does not affect flags
    cycles + if size == Size::Long { 4 } else { 0 }
}

pub fn exec_addq<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    data: u8,
    memory: &mut M,
) -> u32 {
    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let dst_val = cpu.cpu_read_ea(dst_ea, size, memory);

    let (result, carry, overflow) = cpu.add_with_flags(data as u32, dst_val, size);

    write_ea(dst_ea, size, result, &mut cpu.d, &mut cpu.a, memory);

    // ADDQ to An does not affect flags
    if !matches!(dst, AddressingMode::AddressRegister(_)) {
        cpu.update_nz_flags(result, size);
        cpu.set_flag(flags::CARRY, carry);
        cpu.set_flag(flags::EXTEND, carry);
        cpu.set_flag(flags::OVERFLOW, overflow);
    }

    4 + cycles
}

pub fn exec_addi<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    memory: &mut M,
) -> u32 {
    // Read immediate value from extension word(s)
    let imm = match size {
        Size::Byte => (cpu.read_word(cpu.pc, memory) & 0xFF) as u32,
        Size::Word => cpu.read_word(cpu.pc, memory) as u32,
        Size::Long => cpu.read_long(cpu.pc, memory),
    };
    cpu.pc = cpu.pc.wrapping_add(if size == Size::Long { 4 } else { 2 });

    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let dst_val = cpu.cpu_read_ea(dst_ea, size, memory);

    let (result, carry, overflow) = cpu.add_with_flags(imm, dst_val, size);
    write_ea(dst_ea, size, result, &mut cpu.d, &mut cpu.a, memory);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::CARRY, carry);
    cpu.set_flag(flags::EXTEND, carry);
    cpu.set_flag(flags::OVERFLOW, overflow);

    8 + cycles
}

pub fn exec_subi<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    memory: &mut M,
) -> u32 {
    let imm = match size {
        Size::Byte => (cpu.read_word(cpu.pc, memory) & 0xFF) as u32,
        Size::Word => cpu.read_word(cpu.pc, memory) as u32,
        Size::Long => cpu.read_long(cpu.pc, memory),
    };
    cpu.pc = cpu.pc.wrapping_add(if size == Size::Long { 4 } else { 2 });

    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let dst_val = cpu.cpu_read_ea(dst_ea, size, memory);

    let (result, borrow, overflow) = cpu.sub_with_flags(dst_val, imm, size);
    write_ea(dst_ea, size, result, &mut cpu.d, &mut cpu.a, memory);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::CARRY, borrow);
    cpu.set_flag(flags::EXTEND, borrow);
    cpu.set_flag(flags::OVERFLOW, overflow);

    8 + cycles
}

pub fn exec_sub<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    dst: AddressingMode,
    direction: bool,
    memory: &mut M,
) -> u32 {
    let mut cycles = 4u32;

    let (src_mode, dst_mode) = if direction {
        (
            AddressingMode::DataRegister(((cpu.pc.wrapping_sub(2) >> 9) & 7) as u8),
            dst,
        )
    } else {
        (src, dst)
    };

    let (src_ea, src_cycles) =
        calculate_ea(src_mode, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += src_cycles;
    let src_val = cpu.cpu_read_ea(src_ea, size, memory);

    let (dst_ea, dst_cycles) =
        calculate_ea(dst_mode, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += dst_cycles;
    let dst_val = cpu.cpu_read_ea(dst_ea, size, memory);

    let (result, borrow, overflow) = cpu.sub_with_flags(dst_val, src_val, size);

    cpu.cpu_write_ea(dst_ea, size, result, memory);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::CARRY, borrow);
    cpu.set_flag(flags::EXTEND, borrow);
    cpu.set_flag(flags::OVERFLOW, overflow);

    cycles
}

pub fn exec_addx<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src_reg: u8,
    dst_reg: u8,
    memory_mode: bool,
    memory: &mut M,
) -> u32 {
    let (src_val, dst_val, dst_addr, cycles) =
        fetch_operands_with_decrement(cpu, src_reg, dst_reg, memory_mode, size, memory);

    if cpu.pending_exception {
        return cycles;
    }

    let x = if cpu.get_flag(flags::EXTEND) { 1 } else { 0 };
    let (result, carry, overflow) = cpu.addx_with_flags(src_val, dst_val, x, size);

    if let Some(addr) = dst_addr {
        cpu.cpu_write_memory(addr, size, result, memory);
    } else {
        cpu.write_data_reg(dst_reg, size, result);
    }

    let msb = match size {
        Size::Byte => 0x80,
        Size::Word => 0x8000,
        Size::Long => 0x80000000,
    };

    cpu.set_flag(flags::NEGATIVE, (result & msb) != 0);
    if result != 0 {
        cpu.set_flag(flags::ZERO, false);
    }
    cpu.set_flag(flags::OVERFLOW, overflow);
    cpu.set_flag(flags::CARRY, carry);
    cpu.set_flag(flags::EXTEND, carry);

    cycles
}

pub fn exec_subx<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src_reg: u8,
    dst_reg: u8,
    memory_mode: bool,
    memory: &mut M,
) -> u32 {
    let (src_val, dst_val, dst_addr, cycles) =
        fetch_operands_with_decrement(cpu, src_reg, dst_reg, memory_mode, size, memory);

    if cpu.pending_exception {
        return cycles;
    }

    let x = if cpu.get_flag(flags::EXTEND) { 1 } else { 0 };
    let (result, borrow, overflow) = cpu.subx_with_flags(dst_val, src_val, x, size);

    if let Some(addr) = dst_addr {
        cpu.cpu_write_memory(addr, size, result, memory);
    } else {
        cpu.write_data_reg(dst_reg, size, result);
    }

    let msb = match size {
        Size::Byte => 0x80,
        Size::Word => 0x8000,
        Size::Long => 0x80000000,
    };

    cpu.set_flag(flags::NEGATIVE, (result & msb) != 0);
    if result != 0 {
        cpu.set_flag(flags::ZERO, false);
    }
    cpu.set_flag(flags::OVERFLOW, overflow);
    cpu.set_flag(flags::CARRY, borrow);
    cpu.set_flag(flags::EXTEND, borrow);

    cycles
}

pub fn exec_negx<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    memory: &mut M,
) -> u32 {
    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let dst_val = cpu.cpu_read_ea(dst_ea, size, memory);

    let x = if cpu.get_flag(flags::EXTEND) { 1 } else { 0 };
    let (result, borrow, overflow) = cpu.subx_with_flags(0, dst_val, x, size);

    cpu.cpu_write_ea(dst_ea, size, result, memory);

    let msb = match size {
        Size::Byte => 0x80,
        Size::Word => 0x8000,
        Size::Long => 0x80000000,
    };

    cpu.set_flag(flags::NEGATIVE, (result & msb) != 0);
    if result != 0 {
        cpu.set_flag(flags::ZERO, false);
    }
    cpu.set_flag(flags::OVERFLOW, overflow);
    cpu.set_flag(flags::CARRY, borrow);
    cpu.set_flag(flags::EXTEND, borrow);

    cycles
        + match size {
            Size::Long => 8,
            _ => 4,
        }
}

pub fn exec_mulu<M: MemoryInterface>(
    cpu: &mut Cpu,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> u32 {
    let mut cycles = 4u32;
    let (src_ea, src_cycles) =
        calculate_ea(src, Size::Word, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += src_cycles;

    let src_val = cpu.cpu_read_ea(src_ea, Size::Word, memory) as u16;
    let dst_val = cpu.d[dst_reg as usize] as u16;

    let result = (src_val as u32) * (dst_val as u32);
    cpu.d[dst_reg as usize] = result;

    cpu.update_nz_flags(result, Size::Long);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::CARRY, false);

    cycles + 70
}

pub fn exec_muls<M: MemoryInterface>(
    cpu: &mut Cpu,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> u32 {
    let mut cycles = 4u32;
    let (src_ea, src_cycles) =
        calculate_ea(src, Size::Word, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += src_cycles;

    let src_val = read_ea(src_ea, Size::Word, &cpu.d, &cpu.a, memory) as i16;
    let dst_val = cpu.d[dst_reg as usize] as i16;

    let result = (src_val as i32) * (dst_val as i32);
    cpu.d[dst_reg as usize] = result as u32;

    cpu.update_nz_flags(result as u32, Size::Long);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::CARRY, false);

    cycles + 70
}

pub fn exec_divu<M: MemoryInterface>(
    cpu: &mut Cpu,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> u32 {
    let mut cycles = 4u32;
    let (src_ea, src_cycles) =
        calculate_ea(src, Size::Word, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += src_cycles;

    let src_val = cpu.cpu_read_ea(src_ea, Size::Word, memory) as u16;

    if src_val == 0 {
        // Divide by zero trap
        #[cfg(debug_assertions)]
        eprintln!("TRAP 5: Division by zero at PC={:08X}", cpu.pc);
        return cycles + 38;
    }

    let dst_val = cpu.d[dst_reg as usize];
    let quotient = dst_val / (src_val as u32);
    let remainder = dst_val % (src_val as u32);

    if quotient > 0xFFFF {
        cpu.set_flag(flags::OVERFLOW, true);
        cpu.set_flag(flags::CARRY, false);
        return cycles + 10;
    }

    let result = (remainder << 16) | quotient;
    cpu.d[dst_reg as usize] = result;

    let n = (quotient & 0x8000) != 0;
    cpu.set_flag(flags::NEGATIVE, n);
    cpu.set_flag(flags::ZERO, quotient == 0);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::CARRY, false);

    cycles + 140
}

pub fn exec_divs<M: MemoryInterface>(
    cpu: &mut Cpu,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> u32 {
    let mut cycles = 4u32;
    let (src_ea, src_cycles) =
        calculate_ea(src, Size::Word, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += src_cycles;

    let src_val = read_ea(src_ea, Size::Word, &cpu.d, &cpu.a, memory) as i16;

    if src_val == 0 {
        #[cfg(debug_assertions)]
        eprintln!("TRAP 5: Division by zero at PC={:08X}", cpu.pc);
        return cycles + 38;
    }

    let dst_val = cpu.d[dst_reg as usize] as i32;
    let quotient = dst_val / (src_val as i32);
    let remainder = dst_val % (src_val as i32);

    if quotient > 32767 || quotient < -32768 {
        cpu.set_flag(flags::OVERFLOW, true);
        cpu.set_flag(flags::CARRY, false);
        return cycles + 10;
    }

    let result = ((remainder as u32 & 0xFFFF) << 16) | (quotient as u32 & 0xFFFF);
    cpu.d[dst_reg as usize] = result;

    cpu.set_flag(flags::NEGATIVE, quotient < 0);
    cpu.set_flag(flags::ZERO, quotient == 0);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::CARRY, false);

    cycles + 158
}

pub fn exec_abcd<M: MemoryInterface>(
    cpu: &mut Cpu,
    src_reg: u8,
    dst_reg: u8,
    memory_mode: bool,
    memory: &mut M,
) -> u32 {
    let (src_val, dst_val, dst_addr, mut cycles) =
        fetch_operands_with_decrement(cpu, src_reg, dst_reg, memory_mode, Size::Byte, memory);

    if cpu.pending_exception {
        return cycles;
    }

    if !memory_mode {
        cycles += 2;
    }

    let x = if cpu.get_flag(flags::EXTEND) { 1 } else { 0 };

    let mut tmp = (src_val & 0x0F) as u16 + (dst_val & 0x0F) as u16 + x as u16;
    if tmp > 9 {
        tmp += 6;
    }
    tmp += (src_val & 0xF0) as u16 + (dst_val & 0xF0) as u16;

    let carry = tmp > 0x99;
    if carry {
        tmp += 0x60;
    }

    let res = (tmp & 0xFF) as u8;

    if let Some(addr) = dst_addr {
        memory.write_byte(addr, res);
    } else {
        cpu.d[dst_reg as usize] = (cpu.d[dst_reg as usize] & 0xFFFFFF00) | res as u32;
    }

    if res != 0 {
        cpu.set_flag(flags::ZERO, false);
    }
    cpu.set_flag(flags::CARRY, carry);
    cpu.set_flag(flags::EXTEND, carry);
    cpu.set_flag(flags::NEGATIVE, (res & 0x80) != 0);
    cpu.set_flag(flags::OVERFLOW, false);

    cycles
}

pub fn exec_sbcd<M: MemoryInterface>(
    cpu: &mut Cpu,
    src_reg: u8,
    dst_reg: u8,
    memory_mode: bool,
    memory: &mut M,
) -> u32 {
    let (src_val, dst_val, dst_addr, mut cycles) =
        fetch_operands_with_decrement(cpu, src_reg, dst_reg, memory_mode, Size::Byte, memory);

    if cpu.pending_exception {
        return cycles;
    }

    if !memory_mode {
        cycles += 2;
    }

    let x = if cpu.get_flag(flags::EXTEND) { 1 } else { 0 };

    let mut tmp = (dst_val & 0x0F) as i16 - (src_val & 0x0F) as i16 - x as i16;
    if tmp < 0 {
        tmp -= 6;
    }
    tmp += (dst_val & 0xF0) as i16 - (src_val & 0xF0) as i16;

    let carry = tmp < 0;
    if carry {
        tmp -= 0x60;
    }

    let res = (tmp & 0xFF) as u8;

    if let Some(addr) = dst_addr {
        memory.write_byte(addr, res);
    } else {
        cpu.d[dst_reg as usize] = (cpu.d[dst_reg as usize] & 0xFFFFFF00) | res as u32;
    }

    if res != 0 {
        cpu.set_flag(flags::ZERO, false);
    }
    cpu.set_flag(flags::CARRY, carry);
    cpu.set_flag(flags::EXTEND, carry);
    cpu.set_flag(flags::NEGATIVE, (res & 0x80) != 0);
    cpu.set_flag(flags::OVERFLOW, false);

    cycles
}

pub fn exec_nbcd<M: MemoryInterface>(cpu: &mut Cpu, dst: AddressingMode, memory: &mut M) -> u32 {
    let mut cycles = 6u32;
    let (dst_ea, dst_cycles) =
        calculate_ea(dst, Size::Byte, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += dst_cycles;

    let dst_val = cpu.cpu_read_ea(dst_ea, Size::Byte, memory) as u8;
    let x = if cpu.get_flag(flags::EXTEND) { 1 } else { 0 };

    let mut tmp = 0 - (dst_val & 0x0F) as i16 - x as i16;
    if tmp < 0 {
        tmp -= 6;
    }
    tmp += 0 - (dst_val & 0xF0) as i16;

    let carry = tmp < 0;
    if carry {
        tmp -= 0x60;
    }

    let res = (tmp & 0xFF) as u8;

    cpu.cpu_write_ea(dst_ea, Size::Byte, res as u32, memory);

    if res != 0 {
        cpu.set_flag(flags::ZERO, false);
    }
    cpu.set_flag(flags::CARRY, carry);
    cpu.set_flag(flags::EXTEND, carry);
    cpu.set_flag(flags::NEGATIVE, (res & 0x80) != 0);
    cpu.set_flag(flags::OVERFLOW, false);

    cycles
}

pub fn exec_cmp<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> u32 {
    let (src_ea, cycles) = calculate_ea(src, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let src_val = cpu.cpu_read_ea(src_ea, size, memory);

    let dst_val = match size {
        Size::Byte => cpu.d[dst_reg as usize] & 0xFF,
        Size::Word => cpu.d[dst_reg as usize] & 0xFFFF,
        Size::Long => cpu.d[dst_reg as usize],
    };

    let (result, borrow, overflow) = cpu.sub_with_flags(dst_val, src_val, size);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::CARRY, borrow);
    cpu.set_flag(flags::OVERFLOW, overflow);

    4 + cycles
}

pub fn exec_cmpa<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> u32 {
    let (src_ea, cycles) = calculate_ea(src, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let src_val = read_ea(src_ea, size, &cpu.d, &cpu.a, memory);

    // Sign-extend source to 32 bits
    let src_val = match size {
        Size::Word => (src_val as i16) as i32 as u32,
        Size::Long => src_val,
        Size::Byte => src_val,
    };

    let dst_val = cpu.a[dst_reg as usize];

    let (result, borrow, overflow) = cpu.sub_with_flags(dst_val, src_val, Size::Long);

    cpu.update_nz_flags(result, Size::Long);
    cpu.set_flag(flags::CARRY, borrow);
    cpu.set_flag(flags::OVERFLOW, overflow);

    6 + cycles
}

pub fn exec_cmpi<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    memory: &mut M,
) -> u32 {
    let imm = match size {
        Size::Byte => (cpu.read_word(cpu.pc, memory) & 0xFF) as u32,
        Size::Word => cpu.read_word(cpu.pc, memory) as u32,
        Size::Long => cpu.read_long(cpu.pc, memory),
    };
    cpu.pc = cpu.pc.wrapping_add(if size == Size::Long { 4 } else { 2 });

    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let dst_val = cpu.cpu_read_ea(dst_ea, size, memory);

    let (result, borrow, overflow) = cpu.sub_with_flags(dst_val, imm, size);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::CARRY, borrow);
    cpu.set_flag(flags::OVERFLOW, overflow);
    // Note: CMPI does NOT set X flag

    8 + cycles
}

pub fn exec_cmpm<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    ax: u8,
    ay: u8,
    memory: &mut M,
) -> u32 {
    let ay_addr = cpu.a[ay as usize];
    let src_val = cpu.cpu_read_memory(ay_addr, size, memory);
    cpu.a[ay as usize] = ay_addr.wrapping_add(size.bytes());

    let ax_addr = cpu.a[ax as usize];
    let dst_val = cpu.cpu_read_memory(ax_addr, size, memory);
    cpu.a[ax as usize] = ax_addr.wrapping_add(size.bytes());

    let (_, borrow, overflow) = cpu.sub_with_flags(dst_val, src_val, size);

    let res = dst_val.wrapping_sub(src_val); // Need actual result for NZ bits
    cpu.update_nz_flags(res, size);
    cpu.set_flag(flags::CARRY, borrow);
    cpu.set_flag(flags::OVERFLOW, overflow);

    match size {
        Size::Byte | Size::Word => 12,
        Size::Long => 20,
    }
}

pub fn exec_tst<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    memory: &mut M,
) -> u32 {
    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let val = cpu.cpu_read_ea(dst_ea, size, memory);

    cpu.update_nz_flags(val, size);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::CARRY, false);

    4 + cycles
}

pub fn exec_suba<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> u32 {
    let mut cycles = 4u32;

    let (src_ea, src_cycles) = calculate_ea(src, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += src_cycles;

    let src_val = read_ea(src_ea, size, &cpu.d, &cpu.a, memory);

    let src_val = match size {
        Size::Word => (src_val as i16) as i32 as u32,
        Size::Long => src_val,
        Size::Byte => src_val,
    };

    cpu.a[dst_reg as usize] = cpu.a[dst_reg as usize].wrapping_sub(src_val);

    cycles + if size == Size::Long { 4 } else { 0 }
}

pub fn exec_subq<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    data: u8,
    memory: &mut M,
) -> u32 {
    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let dst_val = cpu.cpu_read_ea(dst_ea, size, memory);

    let (result, borrow, overflow) = cpu.sub_with_flags(dst_val, data as u32, size);

    cpu.cpu_write_ea(dst_ea, size, result, memory);

    if !matches!(dst, AddressingMode::AddressRegister(_)) {
        cpu.update_nz_flags(result, size);
        cpu.set_flag(flags::CARRY, borrow);
        cpu.set_flag(flags::EXTEND, borrow);
        cpu.set_flag(flags::OVERFLOW, overflow);
    }

    4 + cycles
}

pub fn exec_neg<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    memory: &mut M,
) -> u32 {
    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let val = cpu.cpu_read_ea(dst_ea, size, memory);

    let (result, _borrow, overflow) = cpu.sub_with_flags(0, val, size);

    write_ea(dst_ea, size, result, &mut cpu.d, &mut cpu.a, memory);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::CARRY, val != 0);
    cpu.set_flag(flags::EXTEND, val != 0);
    cpu.set_flag(flags::OVERFLOW, overflow);

    4 + cycles
}

pub fn exec_chk<M: MemoryInterface>(
    cpu: &mut Cpu,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> u32 {
    let mut cycles = 10u32;
    let (src_ea, src_cycles) =
        calculate_ea(src, Size::Word, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += src_cycles;

    let bound = cpu.cpu_read_ea(src_ea, Size::Word, memory) as i16;
    let dn = (cpu.d[dst_reg as usize] & 0xFFFF) as i16;

    if dn < 0 {
        cpu.set_flag(flags::NEGATIVE, true);
        return cpu.process_exception(6, memory); // CHK exception
    }
    if dn > bound {
        cpu.set_flag(flags::NEGATIVE, false);
        return cpu.process_exception(6, memory);
    }

    cycles
}

fn fetch_operands_with_decrement<M: MemoryInterface>(
    cpu: &mut Cpu,
    src_reg: u8,
    dst_reg: u8,
    memory_mode: bool,
    size: Size,
    memory: &mut M,
) -> (u32, u32, Option<u32>, u32) {
    if memory_mode {
        let src_addr = cpu.a[src_reg as usize].wrapping_sub(size.bytes());
        cpu.a[src_reg as usize] = src_addr;
        let src = cpu.cpu_read_memory(src_addr, size, memory);

        let dst_addr = cpu.a[dst_reg as usize].wrapping_sub(size.bytes());
        cpu.a[dst_reg as usize] = dst_addr;
        let dst = cpu.cpu_read_memory(dst_addr, size, memory);

        let cycles = match size {
            Size::Byte | Size::Word => 18,
            Size::Long => 30,
        };
        (src, dst, Some(dst_addr), cycles)
    } else {
        let src = match size {
            Size::Byte => cpu.d[src_reg as usize] & 0xFF,
            Size::Word => cpu.d[src_reg as usize] & 0xFFFF,
            Size::Long => cpu.d[src_reg as usize],
        };
        let dst = match size {
            Size::Byte => cpu.d[dst_reg as usize] & 0xFF,
            Size::Word => cpu.d[dst_reg as usize] & 0xFFFF,
            Size::Long => cpu.d[dst_reg as usize],
        };
        (
            src,
            dst,
            None,
            match size {
                Size::Byte | Size::Word => 4,
                Size::Long => 8,
            },
        )
    }
}
