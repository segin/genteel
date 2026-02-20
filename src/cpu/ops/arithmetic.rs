use crate::cpu::addressing::calculate_ea;
use crate::cpu::decoder::{AddressingMode, Size};
use crate::cpu::{flags, Cpu};
use crate::memory::MemoryInterface;

pub fn exec_add<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    dst: AddressingMode,
    memory: &mut M,
) -> u32 {
    let mut cycles = match size {
        Size::Byte | Size::Word => 4,
        Size::Long => 8,
    };

    let (src_ea, src_cycles) = calculate_ea(src, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += src_cycles;
    let src_val = cpu.cpu_read_ea(src_ea, size, memory);

    let (dst_ea, dst_cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
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
    let mut cycles = match size {
        Size::Byte | Size::Word => 8,
        Size::Long => 6,
    };

    let (src_ea, src_cycles) = calculate_ea(src, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += src_cycles;
    let src_val = match size {
        Size::Byte | Size::Word => (cpu.cpu_read_ea(src_ea, size, memory) as i16) as i32 as u32,
        Size::Long => cpu.cpu_read_ea(src_ea, size, memory),
    };

    let dst_val = cpu.a[dst_reg as usize];
    cpu.a[dst_reg as usize] = dst_val.wrapping_add(src_val);

    cycles
}

pub fn exec_addi<M: MemoryInterface>(
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

    let (result, carry, overflow) = cpu.add_with_flags(dst_val, imm, size);
    cpu.cpu_write_ea(dst_ea, size, result, memory);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::CARRY, carry);
    cpu.set_flag(flags::EXTEND, carry);
    cpu.set_flag(flags::OVERFLOW, overflow);

    8 + cycles
}

pub fn exec_addq<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    data: u8,
    dst: AddressingMode,
    memory: &mut M,
) -> u32 {
    let mut cycles = match size {
        Size::Byte | Size::Word => 4,
        Size::Long => 8,
    };

    let (dst_ea, dst_cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += dst_cycles;

    if let AddressingMode::AddressRegister(reg) = dst {
        let reg = reg as usize;
        let val = cpu.a[reg];
        let add_val = if data == 0 { 8 } else { data as u32 };
        cpu.a[reg] = val.wrapping_add(add_val);
        8
    } else {
        let val = cpu.cpu_read_ea(dst_ea, size, memory);
        let add_val = if data == 0 { 8 } else { data as u32 };
        let (result, carry, overflow) = cpu.add_with_flags(val, add_val, size);

        cpu.cpu_write_ea(dst_ea, size, result, memory);

        cpu.update_nz_flags(result, size);
        cpu.set_flag(flags::CARRY, carry);
        cpu.set_flag(flags::EXTEND, carry);
        cpu.set_flag(flags::OVERFLOW, overflow);
        cycles
    }
}

pub fn exec_addx<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src_reg: u8,
    dst_reg: u8,
    memory_mode: bool,
    memory: &mut M,
) -> u32 {
    let (src_val, dst_val, dst_addr) =
        fetch_predec_or_reg_operands(cpu, src_reg, dst_reg, size, memory_mode, memory);

    let cycles = if dst_addr.is_some() {
        match size {
            Size::Byte | Size::Word => 18,
            Size::Long => 30,
        }
    } else {
        match size {
            Size::Byte | Size::Word => 4,
            Size::Long => 8,
        }
    };

    if cpu.pending_exception {
        return cycles;
    }

    let x = if cpu.get_flag(flags::EXTEND) { 1 } else { 0 };
    let (result, carry, overflow) = cpu.addx_with_flags(src_val, dst_val, x, size);

    if let Some(addr) = dst_addr {
        cpu.cpu_write_memory(addr, size, result, memory);
    } else {
        cpu.d[dst_reg as usize] = size.apply(cpu.d[dst_reg as usize], result);
    }

    if result != 0 {
        cpu.set_flag(flags::ZERO, false);
    }
    cpu.set_flag(flags::NEGATIVE, size.is_negative(result));
    cpu.set_flag(flags::CARRY, carry);
    cpu.set_flag(flags::EXTEND, carry);
    cpu.set_flag(flags::OVERFLOW, overflow);

    cycles
}

pub fn exec_sub<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    dst: AddressingMode,
    memory: &mut M,
) -> u32 {
    let mut cycles = match size {
        Size::Byte | Size::Word => 4,
        Size::Long => 8,
    };

    let (src_ea, src_cycles) = calculate_ea(src, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += src_cycles;
    let src_val = cpu.cpu_read_ea(src_ea, size, memory);

    let (dst_ea, dst_cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += dst_cycles;
    let dst_val = cpu.cpu_read_ea(dst_ea, size, memory);

    let (result, carry, overflow) = cpu.sub_with_flags(dst_val, src_val, size);

    cpu.cpu_write_ea(dst_ea, size, result, memory);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::CARRY, carry);
    cpu.set_flag(flags::EXTEND, carry);
    cpu.set_flag(flags::OVERFLOW, overflow);

    cycles
}

pub fn exec_suba<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> u32 {
    let mut cycles = match size {
        Size::Byte | Size::Word => 8,
        Size::Long => 6,
    };

    let (src_ea, src_cycles) = calculate_ea(src, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += src_cycles;
    let src_val = match size {
        Size::Byte | Size::Word => (cpu.cpu_read_ea(src_ea, size, memory) as i16) as i32 as u32,
        Size::Long => cpu.cpu_read_ea(src_ea, size, memory),
    };

    let dst_val = cpu.a[dst_reg as usize];
    cpu.a[dst_reg as usize] = dst_val.wrapping_add(src_val);

    cycles
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

    let (result, carry, overflow) = cpu.sub_with_flags(dst_val, imm, size);
    cpu.cpu_write_ea(dst_ea, size, result, memory);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::CARRY, carry);
    cpu.set_flag(flags::EXTEND, carry);
    cpu.set_flag(flags::OVERFLOW, overflow);

    8 + cycles
}

pub fn exec_subq<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    data: u8,
    dst: AddressingMode,
    memory: &mut M,
) -> u32 {
    let mut cycles = match size {
        Size::Byte | Size::Word => 4,
        Size::Long => 8,
    };

    let (dst_ea, dst_cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += dst_cycles;

    if let AddressingMode::AddressRegister(reg) = dst {
        let reg = reg as usize;
        let val = cpu.a[reg];
        let sub_val = if data == 0 { 8 } else { data as u32 };
        cpu.a[reg] = val.wrapping_sub(sub_val);
        8
    } else {
        let val = cpu.cpu_read_ea(dst_ea, size, memory);
        let sub_val = if data == 0 { 8 } else { data as u32 };
        let (result, carry, overflow) = cpu.sub_with_flags(val, sub_val, size);

        cpu.cpu_write_ea(dst_ea, size, result, memory);

        cpu.update_nz_flags(result, size);
        cpu.set_flag(flags::CARRY, carry);
        cpu.set_flag(flags::EXTEND, carry);
        cpu.set_flag(flags::OVERFLOW, overflow);
        cycles
    }
}

pub fn exec_subx<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src_reg: u8,
    dst_reg: u8,
    memory_mode: bool,
    memory: &mut M,
) -> u32 {
    let (src_val, dst_val, dst_addr) =
        fetch_predec_or_reg_operands(cpu, src_reg, dst_reg, size, memory_mode, memory);

    let cycles = if dst_addr.is_some() {
        match size {
            Size::Byte | Size::Word => 18,
            Size::Long => 30,
        }
    } else {
        match size {
            Size::Byte | Size::Word => 4,
            Size::Long => 8,
        }
    };

    if cpu.pending_exception {
        return cycles;
    }

    let x = if cpu.get_flag(flags::EXTEND) { 1 } else { 0 };
    let (result, carry, overflow) = cpu.subx_with_flags(dst_val, src_val, x, size);

    if let Some(addr) = dst_addr {
        cpu.cpu_write_memory(addr, size, result, memory);
    } else {
        cpu.d[dst_reg as usize] = size.apply(cpu.d[dst_reg as usize], result);
    }

    if result != 0 {
        cpu.set_flag(flags::ZERO, false);
    }
    cpu.set_flag(flags::NEGATIVE, size.is_negative(result));
    cpu.set_flag(flags::CARRY, carry);
    cpu.set_flag(flags::EXTEND, carry);
    cpu.set_flag(flags::OVERFLOW, overflow);

    cycles
}

pub fn exec_cmp<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> u32 {
    let mut cycles = match size {
        Size::Byte | Size::Word => 4,
        Size::Long => 6,
    };

    let (src_ea, src_cycles) = calculate_ea(src, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += src_cycles;
    let src_val = cpu.cpu_read_ea(src_ea, size, memory);
    let dst_val = match size {
        Size::Byte => cpu.d[dst_reg as usize] & 0xFF,
        Size::Word => cpu.d[dst_reg as usize] & 0xFFFF,
        Size::Long => cpu.d[dst_reg as usize],
    };

    let (result, carry, overflow) = cpu.sub_with_flags(dst_val, src_val, size);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::CARRY, carry);
    cpu.set_flag(flags::OVERFLOW, overflow);

    cycles
}

pub fn exec_cmpa<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> u32 {
    let (src_ea, src_cycles) = calculate_ea(src, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let src_val = match size {
        Size::Word => (cpu.cpu_read_ea(src_ea, size, memory) as i16) as i32 as u32,
        _ => cpu.cpu_read_ea(src_ea, size, memory),
    };
    let dst_val = cpu.a[dst_reg as usize];

    let (result, carry, overflow) = cpu.sub_with_flags(dst_val, src_val, Size::Long);

    cpu.update_nz_flags(result, Size::Long);
    cpu.set_flag(flags::CARRY, carry);
    cpu.set_flag(flags::OVERFLOW, overflow);

    6 + src_cycles
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

    let (result, carry, overflow) = cpu.sub_with_flags(dst_val, imm, size);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::CARRY, carry);
    cpu.set_flag(flags::OVERFLOW, overflow);

    8 + cycles
}

pub fn exec_cmpm<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src_reg: u8,
    dst_reg: u8,
    memory: &mut M,
) -> u32 {
    let src_val = fetch_postinc_operand(cpu, src_reg, size, memory);
    let dst_val = fetch_postinc_operand(cpu, dst_reg, size, memory);

    let (result, carry, overflow) = cpu.sub_with_flags(dst_val, src_val, size);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::CARRY, carry);
    cpu.set_flag(flags::OVERFLOW, overflow);

    match size {
        Size::Byte | Size::Word => 12,
        Size::Long => 20,
    }
}

pub fn exec_neg<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    memory: &mut M,
) -> u32 {
    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let val = cpu.cpu_read_ea(dst_ea, size, memory);

    let (result, carry, overflow) = cpu.sub_with_flags(0, val, size);
    cpu.cpu_write_ea(dst_ea, size, result, memory);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::CARRY, carry);
    cpu.set_flag(flags::EXTEND, carry);
    cpu.set_flag(flags::OVERFLOW, overflow);

    4 + cycles
}

pub fn exec_negx<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    memory: &mut M,
) -> u32 {
    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let val = cpu.cpu_read_ea(dst_ea, size, memory);

    let x = if cpu.get_flag(flags::EXTEND) { 1 } else { 0 };
    let (result, carry, overflow) = cpu.subx_with_flags(0, val, x, size);
    cpu.cpu_write_ea(dst_ea, size, result, memory);

    if result != 0 {
        cpu.set_flag(flags::ZERO, false);
    }
    cpu.set_flag(flags::NEGATIVE, size.is_negative(result));
    cpu.set_flag(flags::CARRY, carry);
    cpu.set_flag(flags::EXTEND, carry);
    cpu.set_flag(flags::OVERFLOW, overflow);

    4 + cycles
}

pub fn exec_mulu<M: MemoryInterface>(
    cpu: &mut Cpu,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> u32 {
    let (src_ea, cycles) =
        calculate_ea(src, Size::Word, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let src_val = cpu.cpu_read_ea(src_ea, Size::Word, memory) as u16;
    let dst_val = (cpu.d[dst_reg as usize] & 0xFFFF) as u16;

    let result = (src_val as u32) * (dst_val as u32);
    cpu.d[dst_reg as usize] = result;

    cpu.set_flag(flags::ZERO, result == 0);
    cpu.set_flag(flags::NEGATIVE, (result & 0x80000000) != 0);
    cpu.set_flag(flags::CARRY, false);
    cpu.set_flag(flags::OVERFLOW, false);

    70 + cycles
}

pub fn exec_muls<M: MemoryInterface>(
    cpu: &mut Cpu,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> u32 {
    let (src_ea, cycles) =
        calculate_ea(src, Size::Word, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let src_val = cpu.cpu_read_ea(src_ea, Size::Word, memory) as i16;
    let dst_val = (cpu.d[dst_reg as usize] & 0xFFFF) as i16;

    let result = (src_val as i32) * (dst_val as i32);
    cpu.d[dst_reg as usize] = result as u32;

    cpu.set_flag(flags::ZERO, result == 0);
    cpu.set_flag(flags::NEGATIVE, result < 0);
    cpu.set_flag(flags::CARRY, false);
    cpu.set_flag(flags::OVERFLOW, false);

    70 + cycles
}

pub fn exec_divu<M: MemoryInterface>(
    cpu: &mut Cpu,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> u32 {
    let (src_ea, cycles) =
        calculate_ea(src, Size::Word, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let divisor = cpu.cpu_read_ea(src_ea, Size::Word, memory) as u16;

    if divisor == 0 {
        cpu.process_exception(5, memory);
        return 0;
    }

    let dividend = cpu.d[dst_reg as usize];
    let quotient = dividend / (divisor as u32);
    let remainder = dividend % (divisor as u32);

    // C is always cleared
    cpu.set_flag(flags::CARRY, false);

    if quotient > 0xFFFF {
        cpu.set_flag(flags::OVERFLOW, true);
    } else {
        cpu.d[dst_reg as usize] = (remainder << 16) | (quotient & 0xFFFF);
        cpu.set_flag(flags::ZERO, (quotient & 0xFFFF) == 0);
        cpu.set_flag(flags::NEGATIVE, (quotient & 0x8000) != 0);
        cpu.set_flag(flags::OVERFLOW, false);
    }

    140 + cycles
}

pub fn exec_divs<M: MemoryInterface>(
    cpu: &mut Cpu,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> u32 {
    let (src_ea, cycles) =
        calculate_ea(src, Size::Word, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let divisor = cpu.cpu_read_ea(src_ea, Size::Word, memory) as i16;

    if divisor == 0 {
        cpu.process_exception(5, memory);
        return 0;
    }

    let dividend = cpu.d[dst_reg as usize] as i32;
    let divisor = divisor as i32;
    let (quotient, overflow) = dividend.overflowing_div(divisor);

    if overflow || !(-32768..=32767).contains(&quotient) {
        cpu.set_flag(flags::OVERFLOW, true);
        cpu.set_flag(flags::CARRY, false);
    } else {
        let remainder = dividend % divisor;
        cpu.d[dst_reg as usize] = ((remainder as u32) << 16) | ((quotient as u32) & 0xFFFF);
        cpu.set_flag(flags::ZERO, (quotient as i16) == 0);
        cpu.set_flag(flags::NEGATIVE, (quotient as i16) < 0);
        cpu.set_flag(flags::OVERFLOW, false);
        cpu.set_flag(flags::CARRY, false);
    }

    158 + cycles
}

pub fn exec_clr<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    memory: &mut M,
) -> u32 {
    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cpu.cpu_write_ea(dst_ea, size, 0, memory);

    cpu.set_flag(flags::ZERO, true);
    cpu.set_flag(flags::NEGATIVE, false);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::CARRY, false);

    4 + cycles
}

pub fn exec_tst<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    memory: &mut M,
) -> u32 {
    let (src_ea, cycles) = calculate_ea(src, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let val = cpu.cpu_read_ea(src_ea, size, memory);

    cpu.update_nz_flags(val, size);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::CARRY, false);

    4 + cycles
}

pub fn exec_abcd<M: MemoryInterface>(
    cpu: &mut Cpu,
    src_reg: u8,
    dst_reg: u8,
    memory_mode: bool,
    memory: &mut M,
) -> u32 {
    let (src_val, dst_val, dst_addr) =
        fetch_predec_or_reg_operands(cpu, src_reg, dst_reg, Size::Byte, memory_mode, memory);

    let mut result =
        (src_val & 0x0F) + (dst_val & 0x0F) + (if cpu.get_flag(flags::EXTEND) { 1 } else { 0 });
    if result > 9 {
        result += 6;
    }
    let mut carry = result > 0x0F;
    result = (src_val & 0xF0) + (dst_val & 0xF0) + (if carry { 0x10 } else { 0 }) + (result & 0x0F);
    if result > 0x9F {
        result += 0x60;
        carry = true;
    } else {
        carry = false;
    }

    let result = result as u8;
    if let Some(addr) = dst_addr {
        cpu.cpu_write_memory(addr, Size::Byte, result as u32, memory);
    } else {
        cpu.d[dst_reg as usize] = Size::Byte.apply(cpu.d[dst_reg as usize], result as u32);
    }

    if result != 0 {
        cpu.set_flag(flags::ZERO, false);
    }
    cpu.set_flag(flags::CARRY, carry);
    cpu.set_flag(flags::EXTEND, carry);

    if memory_mode {
        18
    } else {
        6
    }
}

pub fn exec_sbcd<M: MemoryInterface>(
    cpu: &mut Cpu,
    src_reg: u8,
    dst_reg: u8,
    memory_mode: bool,
    memory: &mut M,
) -> u32 {
    let (src_val, dst_val, dst_addr) =
        fetch_predec_or_reg_operands(cpu, src_reg, dst_reg, Size::Byte, memory_mode, memory);

    let mut result = (dst_val as i32 & 0x0F)
        - (src_val as i32 & 0x0F)
        - (if cpu.get_flag(flags::EXTEND) { 1 } else { 0 });
    if result < 0 {
        result -= 6;
    }
    let mut carry = result < 0;
    result = (dst_val as i32 & 0xF0) - (src_val as i32 & 0xF0) - (if carry { 0x10 } else { 0 })
        + (result & 0x0F);
    if result < 0 {
        result -= 0x60;
        carry = true;
    } else {
        carry = false;
    }

    let result = (result & 0xFF) as u8;
    if let Some(addr) = dst_addr {
        cpu.cpu_write_memory(addr, Size::Byte, result as u32, memory);
    } else {
        cpu.d[dst_reg as usize] = Size::Byte.apply(cpu.d[dst_reg as usize], result as u32);
    }

    if result != 0 {
        cpu.set_flag(flags::ZERO, false);
    }
    cpu.set_flag(flags::CARRY, carry);
    cpu.set_flag(flags::EXTEND, carry);

    if memory_mode {
        18
    } else {
        6
    }
}

pub fn exec_nbcd<M: MemoryInterface>(cpu: &mut Cpu, dst: AddressingMode, memory: &mut M) -> u32 {
    let (dst_ea, cycles) =
        calculate_ea(dst, Size::Byte, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let val = cpu.cpu_read_ea(dst_ea, Size::Byte, memory);

    let mut result = 0i32 - (val as i32 & 0x0F) - (if cpu.get_flag(flags::EXTEND) { 1 } else { 0 });
    if result < 0 {
        result -= 6;
    }
    let mut carry = result < 0;
    result = 0i32 - (val as i32 & 0xF0) - (if carry { 0x10 } else { 0 }) + (result & 0x0F);
    if result < 0 {
        result -= 0x60;
        carry = true;
    } else {
        carry = false;
    }

    let result = (result & 0xFF) as u8;
    cpu.cpu_write_ea(dst_ea, Size::Byte, result as u32, memory);

    if result != 0 {
        cpu.set_flag(flags::ZERO, false);
    }
    cpu.set_flag(flags::CARRY, carry);
    cpu.set_flag(flags::EXTEND, carry);

    6 + cycles
}

pub fn exec_chk<M: MemoryInterface>(
    cpu: &mut Cpu,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> u32 {
    let (src_ea, cycles) =
        calculate_ea(src, Size::Word, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let bound = cpu.cpu_read_ea(src_ea, Size::Word, memory) as i16;
    let val = (cpu.d[dst_reg as usize] & 0xFFFF) as i16;

    cpu.set_flag(flags::ZERO, val == 0);
    cpu.set_flag(flags::NEGATIVE, val < 0);
    // X is unaffected, C and V are undefined? Some sources say N is set if val < 0, val > bound.

    if val < 0 {
        cpu.set_flag(flags::NEGATIVE, true);
        cpu.process_exception(6, memory);
    } else if val > bound {
        cpu.set_flag(flags::NEGATIVE, false);
        cpu.process_exception(6, memory);
    }

    10 + cycles
}

fn fetch_predec_or_reg_operands<M: MemoryInterface>(
    cpu: &mut Cpu,
    src_reg: u8,
    dst_reg: u8,
    size: Size,
    memory_mode: bool,
    memory: &mut M,
) -> (u32, u32, Option<u32>) {
    if memory_mode {
        let src_addr = cpu.a[src_reg as usize].wrapping_sub(size.bytes());
        cpu.a[src_reg as usize] = src_addr;
        let src = cpu.cpu_read_memory(src_addr, size, memory);

        let dst_addr = cpu.a[dst_reg as usize].wrapping_sub(size.bytes());
        cpu.a[dst_reg as usize] = dst_addr;
        let dst = cpu.cpu_read_memory(dst_addr, size, memory);

        (src, dst, Some(dst_addr))
    } else {
        (cpu.d[src_reg as usize], cpu.d[dst_reg as usize], None)
    }
}

fn fetch_postinc_operand<M: MemoryInterface>(
    cpu: &mut Cpu,
    reg: u8,
    size: Size,
    memory: &mut M,
) -> u32 {
    let addr = cpu.a[reg as usize];
    let val = cpu.cpu_read_memory(addr, size, memory);
    cpu.a[reg as usize] = addr.wrapping_add(size.bytes());
    val
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::decoder::AddressingMode;
    use crate::cpu::flags;
    use crate::cpu::Cpu;
    use crate::memory::Memory;

    fn create_test_setup() -> (Cpu, Memory) {
        let mut memory = Memory::new(0x10000);
        // Initialize memory with basic vector table
        memory.write_long(0x0, 0x8000); // Stack pointer
        memory.write_long(0x4, 0x1000); // PC
        let cpu = Cpu::new(&mut memory);
        (cpu, memory)
    }

    #[test]
    fn test_exec_divu_basic() {
        let (mut cpu, mut memory) = create_test_setup();

        // D0 = 200, D1 = 10
        cpu.d[0] = 200;
        cpu.d[1] = 10;

        // DIVU D1, D0
        let cycles = exec_divu(
            &mut cpu,
            AddressingMode::DataRegister(1), // src = D1
            0, // dst = D0
            &mut memory,
        );

        // Expected: Quotient 20, Remainder 0
        // Result in D0: 0x00000014
        assert_eq!(cpu.d[0], 20);
        assert!(!cpu.get_flag(flags::ZERO)); // 20 != 0
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::OVERFLOW));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(cycles > 0);
    }

    #[test]
    fn test_exec_divu_remainder() {
        let (mut cpu, mut memory) = create_test_setup();

        // D0 = 205, D1 = 10
        cpu.d[0] = 205;
        cpu.d[1] = 10;

        // DIVU D1, D0
        exec_divu(
            &mut cpu,
            AddressingMode::DataRegister(1), // src = D1
            0, // dst = D0
            &mut memory,
        );

        // Expected: Quotient 20 (0x14), Remainder 5 (0x05)
        // Result in D0: 0x00050014
        assert_eq!(cpu.d[0], 0x00050014);
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::OVERFLOW));
        assert!(!cpu.get_flag(flags::CARRY));
    }

    #[test]
    fn test_exec_divu_overflow() {
        let (mut cpu, mut memory) = create_test_setup();

        // D0 = 0x20000 (131072), D1 = 2
        // Quotient = 65536 (0x10000) which is > 0xFFFF
        cpu.d[0] = 0x20000;
        cpu.d[1] = 2;

        // DIVU D1, D0
        exec_divu(
            &mut cpu,
            AddressingMode::DataRegister(1), // src = D1
            0, // dst = D0
            &mut memory,
        );

        // Expected: Overflow set, register unchanged
        assert!(cpu.get_flag(flags::OVERFLOW));
        assert_eq!(cpu.d[0], 0x20000);
    }

    #[test]
    fn test_exec_divu_zero() {
        let (mut cpu, mut memory) = create_test_setup();

        // D0 = 100, D1 = 0
        cpu.d[0] = 100;
        cpu.d[1] = 0;

        // Set up divide by zero vector (Vector 5)
        // Address 0x14 -> Handler 0x2000
        memory.write_long(0x14, 0x2000);

        // DIVU D1, D0
        exec_divu(
            &mut cpu,
            AddressingMode::DataRegister(1), // src = D1
            0, // dst = D0
            &mut memory,
        );

        // Expected: Exception processing
        // PC should be 0x2000
        assert_eq!(cpu.pc, 0x2000);

        // Register should be unchanged
        assert_eq!(cpu.d[0], 100);
    }
}
