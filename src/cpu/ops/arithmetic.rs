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

    if matches!(dst, AddressingMode::AddressRegister(_)) {
        let reg = match dst {
            AddressingMode::AddressRegister(r) => r as usize,
            _ => unreachable!(),
        };
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
    cpu.a[dst_reg as usize] = dst_val.wrapping_sub(src_val);

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

    if matches!(dst, AddressingMode::AddressRegister(_)) {
        let reg = match dst {
            AddressingMode::AddressRegister(r) => r as usize,
            _ => unreachable!(),
        };
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

    if quotient > 0xFFFF {
        cpu.set_flag(flags::OVERFLOW, true);
        cpu.set_flag(flags::CARRY, false);
    } else {
        cpu.d[dst_reg as usize] = (remainder << 16) | (quotient & 0xFFFF);
        cpu.set_flag(flags::ZERO, (quotient & 0xFFFF) == 0);
        cpu.set_flag(flags::NEGATIVE, (quotient & 0x8000) != 0);
        cpu.set_flag(flags::OVERFLOW, false);
        cpu.set_flag(flags::CARRY, false);
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
    use crate::cpu::Cpu;
    use crate::cpu::flags;
    use crate::memory::Memory;

    fn create_test_cpu() -> (Cpu, Memory) {
        let mut memory = Memory::new(0x10000);
        // Initial SP and PC
        memory.write_long(0x0, 0x8000); // SP
        memory.write_long(0x4, 0x1000); // PC

        // Exception Vector 6 (CHK instruction)
        memory.write_long(0x18, 0x2000); // Handler address
        // Zero Divide Vector (5)
        memory.write_long(0x14, 0x3000);

        let mut cpu = Cpu::new(&mut memory);
        // Reset state
        cpu.d = [0; 8];
        cpu.a = [0; 8];
        cpu.pc = 0x100;
        cpu.sr = 0;
        (cpu, memory)
    }

    fn create_test_setup() -> (Cpu, Memory) {
        create_test_cpu()
    }

    #[test]
    fn test_abcd_basic() {
        let (mut cpu, mut memory) = create_test_setup();
        // 0x45 + 0x33 = 0x78
        cpu.d[0] = 0x45;
        cpu.d[1] = 0x33;
        cpu.set_flag(flags::EXTEND, false);
        cpu.set_flag(flags::ZERO, true);

        let cycles = exec_abcd(&mut cpu, 0, 1, false, &mut memory);

        assert_eq!(cpu.d[1] & 0xFF, 0x78);
        assert!(!cpu.get_flag(flags::EXTEND));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::ZERO)); // Non-zero result clears Z
        assert_eq!(cycles, 6);
    }

    #[test]
    fn test_abcd_decimal_adjust() {
        let (mut cpu, mut memory) = create_test_setup();
        // Lower nibble adjust: 0x09 + 0x01 = 0x0A -> 0x10
        cpu.d[0] = 0x09;
        cpu.d[1] = 0x01;
        cpu.set_flag(flags::EXTEND, false);

        exec_abcd(&mut cpu, 0, 1, false, &mut memory);
        assert_eq!(cpu.d[1] & 0xFF, 0x10);
        assert!(!cpu.get_flag(flags::EXTEND));

        // Upper nibble adjust: 0x90 + 0x10 = 0xA0 -> 0x00 (Carry)
        cpu.d[0] = 0x90;
        cpu.d[1] = 0x10;
        cpu.set_flag(flags::EXTEND, false);

        exec_abcd(&mut cpu, 0, 1, false, &mut memory);
        assert_eq!(cpu.d[1] & 0xFF, 0x00);
        assert!(cpu.get_flag(flags::EXTEND));
        assert!(cpu.get_flag(flags::CARRY));
    }

    #[test]
    fn test_abcd_extend_flag() {
        let (mut cpu, mut memory) = create_test_setup();
        // 0x05 + 0x05 + X(1) = 0x11
        cpu.d[0] = 0x05;
        cpu.d[1] = 0x05;
        cpu.set_flag(flags::EXTEND, true);

        exec_abcd(&mut cpu, 0, 1, false, &mut memory);
        assert_eq!(cpu.d[1] & 0xFF, 0x11);
    }

    #[test]
    fn test_abcd_zero_flag_persistence() {
        let (mut cpu, mut memory) = create_test_setup();

        // Case 1: Result is zero, Z should remain set (if it was set)
        cpu.d[0] = 0x00;
        cpu.d[1] = 0x00;
        cpu.set_flag(flags::EXTEND, false);
        cpu.set_flag(flags::ZERO, true); // Z previously set (e.g. from previous byte)

        exec_abcd(&mut cpu, 0, 1, false, &mut memory);
        assert_eq!(cpu.d[1] & 0xFF, 0x00);
        assert!(cpu.get_flag(flags::ZERO));

        // Case 2: Result is zero, Z should remain clear (if it was clear)
        cpu.set_flag(flags::ZERO, false);
        exec_abcd(&mut cpu, 0, 1, false, &mut memory);
        assert!(!cpu.get_flag(flags::ZERO));

        // Case 3: Result is non-zero, Z should be cleared
        cpu.d[0] = 0x01;
        cpu.d[1] = 0x00;
        cpu.set_flag(flags::ZERO, true);

        exec_abcd(&mut cpu, 0, 1, false, &mut memory);
        assert_eq!(cpu.d[1] & 0xFF, 0x01);
        assert!(!cpu.get_flag(flags::ZERO));
    }

    #[test]
    fn test_divu_zero_unit() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = 100;
        cpu.d[1] = 0;

        exec_divu(&mut cpu, AddressingMode::DataRegister(1), 0, &mut memory);

        assert!(cpu.pending_exception);
        assert_eq!(cpu.pc, 0x3000);
    }

    #[test]
    fn test_divu_overflow_unit() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = 0xFFFFFFFF;
        cpu.d[1] = 1;

        cpu.sr = 0;
        exec_divu(&mut cpu, AddressingMode::DataRegister(1), 0, &mut memory);

        assert!(cpu.get_flag(flags::OVERFLOW));
        assert!(!cpu.get_flag(flags::CARRY));
        assert_eq!(cpu.d[0], 0xFFFFFFFF);
    }

    #[test]
    fn test_divu_normal_unit() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = 20;
        cpu.d[1] = 3;

        exec_divu(&mut cpu, AddressingMode::DataRegister(1), 0, &mut memory);

        assert_eq!(cpu.d[0], 0x00020006);
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::OVERFLOW));
        assert!(!cpu.get_flag(flags::CARRY));
    }

    #[test]
    fn test_divu_flags_unit() {
        let (mut cpu, mut memory) = create_test_cpu();

        // Zero case
        cpu.d[0] = 0;
        cpu.d[1] = 5;
        exec_divu(&mut cpu, AddressingMode::DataRegister(1), 0, &mut memory);
        assert!(cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::NEGATIVE));

        // Negative case (MSB set)
        cpu.d[0] = 0x8000;
        cpu.d[1] = 1;
        exec_divu(&mut cpu, AddressingMode::DataRegister(1), 0, &mut memory);
        assert!(cpu.get_flag(flags::NEGATIVE));
    }

    #[test]
    fn test_exec_chk_within_bounds() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = 10;
        memory.write_word(0x100, 20); // Bound = 20

        // We write the address of the bound (0x100) at PC (0x1000) for AbsoluteShort
        cpu.pc = 0x1000;
        memory.write_word(0x1000, 0x100);

        let _cycles = exec_chk(
            &mut cpu,
            AddressingMode::AbsoluteShort,
            0, // D0
            &mut memory,
        );

        assert!(!cpu.pending_exception);
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
    }

    #[test]
    fn test_exec_chk_negative() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = 0xFFFB; // -5 as i16
        memory.write_word(0x100, 20); // Bound = 20

        cpu.pc = 0x1000;
        memory.write_word(0x1000, 0x100);

        let _cycles = exec_chk(
            &mut cpu,
            AddressingMode::AbsoluteShort,
            0, // D0
            &mut memory,
        );

        assert!(cpu.pending_exception);
        assert!(cpu.get_flag(flags::NEGATIVE));
        assert_eq!(cpu.pc, 0x2000);
    }

    #[test]
    fn test_exec_chk_greater_than_bound() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = 30;
        memory.write_word(0x100, 20); // Bound = 20

        cpu.pc = 0x1000;
        memory.write_word(0x1000, 0x100);

        let _cycles = exec_chk(
            &mut cpu,
            AddressingMode::AbsoluteShort,
            0, // D0
            &mut memory,
        );

        assert!(cpu.pending_exception);
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert_eq!(cpu.pc, 0x2000);
    }

    #[test]
    fn test_exec_chk_zero() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = 0;
        memory.write_word(0x100, 20); // Bound = 20

        cpu.pc = 0x1000;
        memory.write_word(0x1000, 0x100);

        let _cycles = exec_chk(
            &mut cpu,
            AddressingMode::AbsoluteShort,
            0, // D0
            &mut memory,
        );

        assert!(!cpu.pending_exception);
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(cpu.get_flag(flags::ZERO));
    }

    #[test]
    fn test_exec_chk_equal_to_bound() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = 20;
        memory.write_word(0x100, 20); // Bound = 20

        cpu.pc = 0x1000;
        memory.write_word(0x1000, 0x100);

        let _cycles = exec_chk(
            &mut cpu,
            AddressingMode::AbsoluteShort,
            0, // D0
            &mut memory,
        );

        assert!(!cpu.pending_exception);
        assert!(!cpu.get_flag(flags::NEGATIVE));
    }

    #[test]
    fn test_exec_mulu_basic() {
        let (mut cpu, mut memory) = create_test_cpu();

        // MULU D1, D0
        // D1 = 20
        // D0 = 10
        cpu.d[0] = 10;
        cpu.d[1] = 20;

        // Set flags to true to verify they are cleared/updated
        cpu.set_flag(flags::CARRY, true);
        cpu.set_flag(flags::OVERFLOW, true);
        cpu.set_flag(flags::ZERO, true);
        cpu.set_flag(flags::NEGATIVE, true);

        let cycles = exec_mulu(&mut cpu, AddressingMode::DataRegister(1), 0, &mut memory);

        assert_eq!(cpu.d[0], 200);
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));
        assert!(cycles > 0);
    }

    #[test]
    fn test_exec_mulu_zero() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = 10;
        cpu.d[1] = 0;

        // Set flags to verify correct updates
        cpu.set_flag(flags::CARRY, true);
        cpu.set_flag(flags::OVERFLOW, true);
        cpu.set_flag(flags::NEGATIVE, true);

        exec_mulu(&mut cpu, AddressingMode::DataRegister(1), 0, &mut memory);

        assert_eq!(cpu.d[0], 0);
        assert!(cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));
    }

    #[test]
    fn test_exec_mulu_large() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = 0xFFFF;
        cpu.d[1] = 0xFFFF;

        // Set flags to verify correct updates
        cpu.set_flag(flags::CARRY, true);
        cpu.set_flag(flags::OVERFLOW, true);

        exec_mulu(&mut cpu, AddressingMode::DataRegister(1), 0, &mut memory);

        // 0xFFFF * 0xFFFF = 0xFFFE0001
        assert_eq!(cpu.d[0], 0xFFFE0001);
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(cpu.get_flag(flags::NEGATIVE)); // MSB is set
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));
    }

    #[test]
    fn test_exec_mulu_immediate() {
         let (mut cpu, mut memory) = create_test_cpu();
         cpu.d[0] = 10;

         // Setup immediate value in memory at PC
         cpu.pc = 0x200;
         memory.write_word(0x200, 20); // Immediate value 20

         exec_mulu(&mut cpu, AddressingMode::Immediate, 0, &mut memory);

         assert_eq!(cpu.d[0], 200);
         assert_eq!(cpu.pc, 0x202); // PC should advance by 2
    }

    #[test]
    fn test_exec_add_byte_reg_reg() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = 0x10;
        cpu.d[1] = 0x20;

        // ADD.B D0, D1
        let cycles = exec_add(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert_eq!(cpu.d[1] & 0xFF, 0x30);
        assert_eq!(cycles, 4);
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));
        assert!(!cpu.get_flag(flags::EXTEND));
    }

    #[test]
    fn test_exec_add_word() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = 0x1000;
        cpu.d[1] = 0x2000;

        // ADD.W D0, D1
        let cycles = exec_add(
            &mut cpu,
            Size::Word,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert_eq!(cpu.d[1] & 0xFFFF, 0x3000);
        assert_eq!(cycles, 4);
    }

    #[test]
    fn test_exec_add_word_flags() {
        let (mut cpu, mut memory) = create_test_cpu();
        // 0x7FFF + 0x0001 = 0x8000 (Overflow, Negative, no Carry)
        cpu.d[0] = 0x0001;
        cpu.d[1] = 0x7FFF;

        exec_add(
            &mut cpu,
            Size::Word,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert_eq!(cpu.d[1] & 0xFFFF, 0x8000);
        assert!(cpu.get_flag(flags::NEGATIVE));
        assert!(cpu.get_flag(flags::OVERFLOW));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::EXTEND));
        assert!(!cpu.get_flag(flags::ZERO));

        // 0xFFFF + 0x0001 = 0x0000 (Carry, Extend, Zero, no Overflow)
        cpu.d[0] = 0x0001;
        cpu.d[1] = 0xFFFF;

        exec_add(
            &mut cpu,
            Size::Word,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert_eq!(cpu.d[1] & 0xFFFF, 0x0000);
        assert!(cpu.get_flag(flags::ZERO));
        assert!(cpu.get_flag(flags::CARRY));
        assert!(cpu.get_flag(flags::EXTEND));
        assert!(!cpu.get_flag(flags::OVERFLOW));
        assert!(!cpu.get_flag(flags::NEGATIVE));
    }

    #[test]
    fn test_exec_add_long() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = 0x10000000;
        cpu.d[1] = 0x20000000;

        // ADD.L D0, D1
        let cycles = exec_add(
            &mut cpu,
            Size::Long,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert_eq!(cpu.d[1], 0x30000000);
        assert_eq!(cycles, 8);
    }

    #[test]
    fn test_exec_add_long_memory() {
        let (mut cpu, mut memory) = create_test_cpu();

        cpu.d[0] = 0x12345678;
        let addr = 0x2000;
        memory.write_long(addr, 0x11111111);

        cpu.a[0] = addr;

        // ADD.L D0, (A0)
        exec_add(
            &mut cpu,
            Size::Long,
            AddressingMode::DataRegister(0),
            AddressingMode::AddressIndirect(0),
            &mut memory,
        );

        let result = memory.read_long(addr);
        assert_eq!(result, 0x23456789);
        assert!(!cpu.get_flag(flags::ZERO));
    }

    #[test]
    fn test_exec_add_memory_dest() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = 0x10;
        cpu.a[0] = 0x2000;
        memory.write_byte(0x2000, 0x20);

        // ADD.B D0, (A0) -> Mem[2000] = Mem[2000] + D0
        let cycles = exec_add(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            AddressingMode::AddressIndirect(0),
            &mut memory,
        );

        assert_eq!(memory.read_byte(0x2000), 0x30);
        assert_eq!(cycles, 8);
    }

    #[test]
    fn test_exec_add_memory_src() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.a[0] = 0x2000;
        memory.write_byte(0x2000, 0x10);
        cpu.d[0] = 0x20;

        // ADD.B (A0), D0 -> D0 = D0 + Mem[2000]
        let _cycles = exec_add(
            &mut cpu,
            Size::Byte,
            AddressingMode::AddressIndirect(0),
            AddressingMode::DataRegister(0),
            &mut memory,
        );

        assert_eq!(cpu.d[0] & 0xFF, 0x30);
    }

    #[test]
    fn test_exec_add_flags_zero() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = 0;
        cpu.d[1] = 0;
        cpu.set_flag(flags::NEGATIVE, true);

        exec_add(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert!(cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::NEGATIVE));
    }

    #[test]
    fn test_exec_add_flags_negative() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = 0x80; // -128
        cpu.d[1] = 0x00;

        exec_add(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert!(cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
    }

    #[test]
    fn test_abcd_memory_mode() {
        let (mut cpu, mut memory) = create_test_setup();

        // ABCD -(A0), -(A1)
        // A0 = 0x2001, A1 = 0x3001
        // Mem[0x2000] = 0x15
        // Mem[0x3000] = 0x25
        // Result: 0x40 at Mem[0x3000]

        cpu.a[0] = 0x2001;
        cpu.a[1] = 0x3001;
        memory.write_byte(0x2000, 0x15);
        memory.write_byte(0x3000, 0x25);
        cpu.set_flag(flags::EXTEND, false);

        let cycles = exec_abcd(&mut cpu, 0, 1, true, &mut memory);

        assert_eq!(memory.read_byte(0x3000), 0x40);
        assert_eq!(cpu.a[0], 0x2000);
        assert_eq!(cpu.a[1], 0x3000);
        assert_eq!(cycles, 18);
    }

    #[test]
    fn test_abcd_carry_generation() {
        let (mut cpu, mut memory) = create_test_setup();
        // 0x99 + 0x01 = 0x00, Carry = 1
        cpu.d[0] = 0x99;
        cpu.d[1] = 0x01;
        cpu.set_flag(flags::EXTEND, false);
        cpu.set_flag(flags::ZERO, true);

        exec_abcd(&mut cpu, 0, 1, false, &mut memory);

        assert_eq!(cpu.d[1] & 0xFF, 0x00);
        assert!(cpu.get_flag(flags::EXTEND));
        assert!(cpu.get_flag(flags::CARRY));
        assert!(cpu.get_flag(flags::ZERO)); // Result 0, Z remains set
    }

    #[test]
    fn test_exec_add_flags_carry_overflow() {
        let (mut cpu, mut memory) = create_test_cpu();
        
        // 127 + 1 = 128 (-128 signed). Overflow!
        cpu.d[0] = 0x7F;
        cpu.d[1] = 0x01;

        exec_add(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert_eq!(cpu.d[1] & 0xFF, 0x80);
        assert!(cpu.get_flag(flags::OVERFLOW));
        assert!(cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::CARRY)); // No carry out of bit 7 (127+1 = 128)

        // 255 + 1 = 0. Carry!
        cpu.d[0] = 0xFF;
        cpu.d[1] = 0x01;

        exec_add(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert_eq!(cpu.d[1] & 0xFF, 0x00);
        assert!(cpu.get_flag(flags::CARRY));
        assert!(cpu.get_flag(flags::EXTEND));
        assert!(cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::OVERFLOW)); // -1 + 1 = 0. No signed overflow.
    }

    #[test]
    fn test_exec_sub_byte() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = 0x10;
        cpu.d[1] = 0x20;

        // SUB.B D0, D1 -> D1 - D0 = 0x20 - 0x10 = 0x10
        exec_sub(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert_eq!(cpu.d[1] & 0xFF, 0x10);
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));
    }

    #[test]
    fn test_exec_sub_word() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = 0x1000;
        cpu.d[1] = 0x2000;

        // SUB.W D0, D1 -> D1 - D0 = 0x1000
        exec_sub(
            &mut cpu,
            Size::Word,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert_eq!(cpu.d[1] & 0xFFFF, 0x1000);
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));
    }

    #[test]
    fn test_exec_sub_long() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = 0x100000;
        cpu.d[1] = 0x200000;

        // SUB.L D0, D1 -> D1 - D0 = 0x100000
        exec_sub(
            &mut cpu,
            Size::Long,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert_eq!(cpu.d[1], 0x100000);
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));
    }

    #[test]
    fn test_exec_sub_borrow() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = 0x20;
        cpu.d[1] = 0x10;

        // SUB.B D0, D1 -> 0x10 - 0x20 = 0xF0 (-16)
        exec_sub(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert_eq!(cpu.d[1] & 0xFF, 0xF0);
        assert!(cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(cpu.get_flag(flags::CARRY));
        assert!(cpu.get_flag(flags::EXTEND));
        assert!(!cpu.get_flag(flags::OVERFLOW));
    }

    #[test]
    fn test_exec_sub_overflow() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = 0x01;
        cpu.d[1] = 0x80; // -128 as i8

        // SUB.B D0, D1 -> -128 - 1 = -129 -> +127 (0x7F) overflow
        exec_sub(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert_eq!(cpu.d[1] & 0xFF, 0x7F);
        assert!(!cpu.get_flag(flags::NEGATIVE)); // Result is positive
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(cpu.get_flag(flags::OVERFLOW));
        assert!(!cpu.get_flag(flags::CARRY));
    }

    #[test]
    fn test_exec_sub_zero() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = 0x1234;
        cpu.d[1] = 0x1234;

        // SUB.W D0, D1 -> 0
        exec_sub(
            &mut cpu,
            Size::Word,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert_eq!(cpu.d[1] & 0xFFFF, 0);
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));
    }

    #[test]
    fn test_exec_sub_negative() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = 0x10;
        cpu.d[1] = 0x00;

        // SUB.B D0, D1 -> 0 - 0x10 = 0xF0
        exec_sub(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert_eq!(cpu.d[1] & 0xFF, 0xF0);
        assert!(cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));
    }

    #[test]
    fn test_exec_sub_memory() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = 0x00001000;
        cpu.a[0] = 0x2000;
        memory.write_word(0x2000, 0x0500);

        // SUB.W (A0), D0
        // D0 = 0x1000 - 0x0500 = 0x0B00
        exec_sub(
            &mut cpu,
            Size::Word,
            AddressingMode::AddressIndirect(0),
            AddressingMode::DataRegister(0),
            &mut memory,
        );

        assert_eq!(cpu.d[0] & 0xFFFF, 0x0B00);
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));
    }
}
