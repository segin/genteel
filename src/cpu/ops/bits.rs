use crate::cpu::Cpu;
use crate::cpu::decoder::{Size, AddressingMode, ShiftCount, BitSource};
use crate::cpu::addressing::{calculate_ea, read_ea, write_ea};
use crate::cpu::flags;

pub fn exec_and(cpu: &mut Cpu, size: Size, src: AddressingMode, dst: AddressingMode, _direction: bool) -> u32 {
    let mut cycles = 4u32;

    let (src_ea, src_cycles) = calculate_ea(src, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, &mut cpu.memory);
    cycles += src_cycles;
    let src_val = cpu.cpu_read_ea(src_ea, size);

    let (dst_ea, dst_cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, &mut cpu.memory);
    cycles += dst_cycles;
    let dst_val = cpu.cpu_read_ea(dst_ea, size);

    let result = src_val & dst_val;

    cpu.cpu_write_ea(dst_ea, size, result);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::CARRY, false);

    cycles
}

pub fn exec_andi(cpu: &mut Cpu, size: Size, dst: AddressingMode) -> u32 {
    let imm = match size {
        Size::Byte => (cpu.read_word(cpu.pc) & 0xFF) as u32,
        Size::Word => cpu.read_word(cpu.pc) as u32,
        Size::Long => cpu.read_long(cpu.pc),
    };
    cpu.pc = cpu.pc.wrapping_add(if size == Size::Long { 4 } else { 2 });

    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, &mut cpu.memory);
    let dst_val = cpu.cpu_read_ea(dst_ea, size);

    let result = dst_val & imm;
    write_ea(dst_ea, size, result, &mut cpu.d, &mut cpu.a, &mut cpu.memory);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::CARRY, false);
    cpu.set_flag(flags::OVERFLOW, false);

    8 + cycles
}

pub fn exec_or(cpu: &mut Cpu, size: Size, src: AddressingMode, dst: AddressingMode, _direction: bool) -> u32 {
    let mut cycles = 4u32;

    let (src_ea, src_cycles) = calculate_ea(src, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, &mut cpu.memory);
    cycles += src_cycles;
    let src_val = cpu.cpu_read_ea(src_ea, size);

    let (dst_ea, dst_cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, &mut cpu.memory);
    cycles += dst_cycles;
    let dst_val = cpu.cpu_read_ea(dst_ea, size);

    let result = src_val | dst_val;

    cpu.cpu_write_ea(dst_ea, size, result);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::CARRY, false);

    cycles
}

pub fn exec_ori(cpu: &mut Cpu, size: Size, dst: AddressingMode) -> u32 {
    let imm = match size {
        Size::Byte => (cpu.read_word(cpu.pc) & 0xFF) as u32,
        Size::Word => cpu.read_word(cpu.pc) as u32,
        Size::Long => cpu.read_long(cpu.pc),
    };
    cpu.pc = cpu.pc.wrapping_add(if size == Size::Long { 4 } else { 2 });

    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, &mut cpu.memory);
    let dst_val = cpu.cpu_read_ea(dst_ea, size);

    let result = dst_val | imm;
    write_ea(dst_ea, size, result, &mut cpu.d, &mut cpu.a, &mut cpu.memory);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::CARRY, false);
    cpu.set_flag(flags::OVERFLOW, false);

    8 + cycles
}

pub fn exec_eor(cpu: &mut Cpu, size: Size, src_reg: u8, dst: AddressingMode) -> u32 {
    let src_val = match size {
        Size::Byte => cpu.d[src_reg as usize] & 0xFF,
        Size::Word => cpu.d[src_reg as usize] & 0xFFFF,
        Size::Long => cpu.d[src_reg as usize],
    };

    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, &mut cpu.memory);
    let dst_val = cpu.cpu_read_ea(dst_ea, size);

    let result = src_val ^ dst_val;

    cpu.cpu_write_ea(dst_ea, size, result);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::CARRY, false);

    4 + cycles
}

pub fn exec_eori(cpu: &mut Cpu, size: Size, dst: AddressingMode) -> u32 {
    let imm = match size {
        Size::Byte => (cpu.read_word(cpu.pc) & 0xFF) as u32,
        Size::Word => cpu.read_word(cpu.pc) as u32,
        Size::Long => cpu.read_long(cpu.pc),
    };
    cpu.pc = cpu.pc.wrapping_add(if size == Size::Long { 4 } else { 2 });

    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, &mut cpu.memory);
    let dst_val = cpu.cpu_read_ea(dst_ea, size);

    let result = dst_val ^ imm;
    write_ea(dst_ea, size, result, &mut cpu.d, &mut cpu.a, &mut cpu.memory);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::CARRY, false);
    cpu.set_flag(flags::OVERFLOW, false);

    8 + cycles
}

pub fn exec_not(cpu: &mut Cpu, size: Size, dst: AddressingMode) -> u32 {
    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, &mut cpu.memory);
    let val = read_ea(dst_ea, size, &cpu.d, &cpu.a, &mut cpu.memory);

    let result = !val;

    write_ea(dst_ea, size, result, &mut cpu.d, &mut cpu.a, &mut cpu.memory);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::CARRY, false);

    4 + cycles
}

pub fn exec_shift(cpu: &mut Cpu, size: Size, dst: AddressingMode, count: ShiftCount, left: bool, arithmetic: bool) -> u32 {
    let count_val = match count {
        ShiftCount::Immediate(n) => n as u32,
        ShiftCount::Register(r) => cpu.d[r as usize] & 63,
    };

    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, &mut cpu.memory);
    let val = read_ea(dst_ea, size, &cpu.d, &cpu.a, &mut cpu.memory);

    let (mask, sign_bit) = match size {
        Size::Byte => (0xFFu32, 0x80u32),
        Size::Word => (0xFFFF, 0x8000),
        Size::Long => (0xFFFFFFFF, 0x80000000),
    };

    let val = val & mask;
    let mut result = val;
    let mut carry = false;
    let mut overflow = false;

    for _ in 0..count_val {
        if left {
            carry = (result & sign_bit) != 0;
            result = (result << 1) & mask;
            if arithmetic {
                overflow = overflow || (carry != ((result & sign_bit) != 0));
            }
        } else {
            carry = (result & 1) != 0;
            if arithmetic {
                // ASR: preserve sign bit
                let sign = result & sign_bit;
                result = (result >> 1) | sign;
            } else {
                result >>= 1;
            }
        }
    }

    write_ea(dst_ea, size, result, &mut cpu.d, &mut cpu.a, &mut cpu.memory);

    cpu.update_nz_flags(result, size);
    if count_val > 0 {
        cpu.set_flag(flags::CARRY, carry);
        cpu.set_flag(flags::EXTEND, carry);
    } else {
        cpu.set_flag(flags::CARRY, false);
    }
    cpu.set_flag(flags::OVERFLOW, overflow);

    6 + cycles + 2 * count_val
}

pub fn exec_rotate(cpu: &mut Cpu, size: Size, dst: AddressingMode, count: ShiftCount, left: bool, _extend: bool) -> u32 {
    let count_val = match count {
        ShiftCount::Immediate(n) => n as u32,
        ShiftCount::Register(r) => cpu.d[r as usize] & 63,
    };

    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, &mut cpu.memory);
    let val = read_ea(dst_ea, size, &cpu.d, &cpu.a, &mut cpu.memory);

    let (mask, bits) = match size {
        Size::Byte => (0xFFu32, 8u32),
        Size::Word => (0xFFFF, 16),
        Size::Long => (0xFFFFFFFF, 32),
    };

    let val = val & mask;
    let effective_count = count_val % bits;
    let msb = 1 << (bits - 1);
    let result;
    let mut carry = false;

    if left {
        if effective_count == 0 {
            result = val;
            if count_val > 0 {
                carry = (val & msb) != 0;
            }
        } else {
            result = ((val << effective_count) | (val >> (bits - effective_count))) & mask;
            carry = ((val >> (bits - effective_count)) & 1) != 0;
        }
    } else {
        if effective_count == 0 {
            result = val;
            if count_val > 0 {
                carry = (val & 1) != 0;
            }
        } else {
            result = ((val >> effective_count) | (val << (bits - effective_count))) & mask;
            carry = ((val >> (effective_count - 1)) & 1) != 0;
        }
    }

    write_ea(dst_ea, size, result, &mut cpu.d, &mut cpu.a, &mut cpu.memory);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::CARRY, carry);
    cpu.set_flag(flags::OVERFLOW, false);

    6 + cycles + 2 * count_val
}

pub fn exec_roxl(cpu: &mut Cpu, size: Size, dst: AddressingMode, count: ShiftCount) -> u32 {
    let count_val = match count {
        ShiftCount::Immediate(n) => n as u32,
        ShiftCount::Register(r) => cpu.d[r as usize] & 63,
    };

    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, &mut cpu.memory);
    let val = cpu.cpu_read_ea(dst_ea, size);

    let (mask, msb) = match size {
        Size::Byte => (0xFFu32, 0x80u32),
        Size::Word => (0xFFFF, 0x8000),
        Size::Long => (0xFFFFFFFF, 0x80000000),
    };

    let mut res = val & mask;
    let mut x = cpu.get_flag(flags::EXTEND);
    let mut last_carry = x;

    for _ in 0..count_val {
        let next_x = (res & msb) != 0;
        res = ((res << 1) | (if x { 1 } else { 0 })) & mask;
        x = next_x;
        last_carry = x;
    }

    cpu.cpu_write_ea(dst_ea, size, res);
    cpu.update_nz_flags(res, size);
    cpu.set_flag(flags::OVERFLOW, false);
    if count_val > 0 {
        cpu.set_flag(flags::CARRY, last_carry);
        cpu.set_flag(flags::EXTEND, last_carry);
    } else {
        cpu.set_flag(flags::CARRY, cpu.get_flag(flags::EXTEND));
    }

    cycles + 6 + 2 * count_val
}

pub fn exec_roxr(cpu: &mut Cpu, size: Size, dst: AddressingMode, count: ShiftCount) -> u32 {
    let count_val = match count {
        ShiftCount::Immediate(n) => n as u32,
        ShiftCount::Register(r) => cpu.d[r as usize] & 63,
    };

    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, &mut cpu.memory);
    let val = cpu.cpu_read_ea(dst_ea, size);

    let (mask, msb) = match size {
        Size::Byte => (0xFFu32, 0x80u32),
        Size::Word => (0xFFFF, 0x8000),
        Size::Long => (0xFFFFFFFF, 0x80000000),
    };

    let mut res = val & mask;
    let mut x = cpu.get_flag(flags::EXTEND);
    let mut last_carry = x;

    for _ in 0..count_val {
        let next_x = (res & 1) != 0;
        res = (res >> 1) | (if x { msb } else { 0 });
        x = next_x;
        last_carry = x;
    }

    cpu.cpu_write_ea(dst_ea, size, res);
    cpu.update_nz_flags(res, size);
    cpu.set_flag(flags::OVERFLOW, false);
    if count_val > 0 {
        cpu.set_flag(flags::CARRY, last_carry);
        cpu.set_flag(flags::EXTEND, last_carry);
    } else {
        cpu.set_flag(flags::CARRY, cpu.get_flag(flags::EXTEND));
    }

    cycles + 6 + 2 * count_val
}

pub fn exec_btst(cpu: &mut Cpu, bit: BitSource, dst: AddressingMode) -> u32 {
    let bit_num = cpu.fetch_bit_num(bit);
    let is_memory = !matches!(dst, AddressingMode::DataRegister(_));
    let size = if is_memory { Size::Byte } else { Size::Long };

    let mut cycles = 4u32;
    let (dst_ea, dst_cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, &mut cpu.memory);
    cycles += dst_cycles;

    let val = if matches!(dst, AddressingMode::Immediate) {
        // Immediate data for BTST is valid? No, destination EA.
        // BTST #n, #m is not valid.
        // But BTST #n, (xxx) is.
        cpu.cpu_read_ea(dst_ea, size)
    } else {
         cpu.cpu_read_ea(dst_ea, size)
    };

    let bit_idx = cpu.resolve_bit_index(bit_num, is_memory);
    let bit_val = (val >> bit_idx) & 1;

    cpu.set_flag(flags::ZERO, bit_val == 0);

    if is_memory { cycles += 4; } else { cycles += 6; } // Timing approx
    cycles
}

pub fn exec_bset(cpu: &mut Cpu, bit: BitSource, dst: AddressingMode) -> u32 {
    let bit_num = cpu.fetch_bit_num(bit);
    let is_memory = !matches!(dst, AddressingMode::DataRegister(_));
    let size = if is_memory { Size::Byte } else { Size::Long };

    let mut cycles = 8u32;
    let (dst_ea, dst_cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, &mut cpu.memory);
    cycles += dst_cycles;

    let val = cpu.cpu_read_ea(dst_ea, size);
    let bit_idx = cpu.resolve_bit_index(bit_num, is_memory);
    let bit_val = (val >> bit_idx) & 1;

    cpu.set_flag(flags::ZERO, bit_val == 0);

    let new_val = val | (1 << bit_idx);
    cpu.cpu_write_ea(dst_ea, size, new_val);

    cycles
}

pub fn exec_bclr(cpu: &mut Cpu, bit: BitSource, dst: AddressingMode) -> u32 {
    let bit_num = cpu.fetch_bit_num(bit);
    let is_memory = !matches!(dst, AddressingMode::DataRegister(_));
    let size = if is_memory { Size::Byte } else { Size::Long };

    let mut cycles = 8u32;
    let (dst_ea, dst_cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, &mut cpu.memory);
    cycles += dst_cycles;

    let val = cpu.cpu_read_ea(dst_ea, size);
    let bit_idx = cpu.resolve_bit_index(bit_num, is_memory);
    let bit_val = (val >> bit_idx) & 1;

    cpu.set_flag(flags::ZERO, bit_val == 0);

    let new_val = val & !(1 << bit_idx);
    cpu.cpu_write_ea(dst_ea, size, new_val);

    cycles
}

pub fn exec_bchg(cpu: &mut Cpu, bit: BitSource, dst: AddressingMode) -> u32 {
    let bit_num = cpu.fetch_bit_num(bit);
    let is_memory = !matches!(dst, AddressingMode::DataRegister(_));
    let size = if is_memory { Size::Byte } else { Size::Long };

    let mut cycles = 8u32;
    let (dst_ea, dst_cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, &mut cpu.memory);
    cycles += dst_cycles;

    let val = cpu.cpu_read_ea(dst_ea, size);
    let bit_idx = cpu.resolve_bit_index(bit_num, is_memory);
    let bit_val = (val >> bit_idx) & 1;

    cpu.set_flag(flags::ZERO, bit_val == 0);

    let new_val = val ^ (1 << bit_idx);
    cpu.cpu_write_ea(dst_ea, size, new_val);

    cycles
}

pub fn exec_tas(cpu: &mut Cpu, dst: AddressingMode) -> u32 {
    let mut cycles = 4u32;
    let (dst_ea, dst_cycles) = calculate_ea(dst, Size::Byte, &mut cpu.d, &mut cpu.a, &mut cpu.pc, &mut cpu.memory);
    cycles += dst_cycles;

    let val = cpu.cpu_read_ea(dst_ea, Size::Byte) as u8;

    // Set flags based on original value
    cpu.set_flag(flags::NEGATIVE, (val & 0x80) != 0);
    cpu.set_flag(flags::ZERO, val == 0);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::CARRY, false);

    // Set high bit (atomically on real hardware)
    let new_val = val | 0x80;
    cpu.cpu_write_ea(dst_ea, Size::Byte, new_val as u32);

    cycles + 4
}
