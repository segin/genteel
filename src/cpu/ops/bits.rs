use crate::cpu::addressing::{calculate_ea, read_ea};
use crate::cpu::decoder::{AddressingMode, BitSource, ShiftCount, Size};
use crate::cpu::flags;
use crate::cpu::Cpu;
use crate::memory::MemoryInterface;

pub fn exec_and<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    dst: AddressingMode,
    _direction: bool,
    memory: &mut M,
) -> u32 {
    let mut cycles = 4u32;

    let (src_ea, src_cycles) = calculate_ea(src, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += src_cycles;
    let src_val = cpu.cpu_read_ea(src_ea, size, memory);

    let (dst_ea, dst_cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += dst_cycles;
    let dst_val = cpu.cpu_read_ea(dst_ea, size, memory);

    let result = src_val & dst_val;

    cpu.cpu_write_ea(dst_ea, size, result, memory);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::CARRY, false);

    cycles
}

pub fn exec_andi<M: MemoryInterface>(
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

    let result = dst_val & imm;
    cpu.cpu_write_ea(dst_ea, size, result, memory);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::CARRY, false);
    cpu.set_flag(flags::OVERFLOW, false);

    8 + cycles
}

pub fn exec_or<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    dst: AddressingMode,
    _direction: bool,
    memory: &mut M,
) -> u32 {
    let mut cycles = 4u32;

    let (src_ea, src_cycles) = calculate_ea(src, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += src_cycles;
    let src_val = cpu.cpu_read_ea(src_ea, size, memory);

    let (dst_ea, dst_cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += dst_cycles;
    let dst_val = cpu.cpu_read_ea(dst_ea, size, memory);

    let result = src_val | dst_val;

    cpu.cpu_write_ea(dst_ea, size, result, memory);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::CARRY, false);

    cycles
}

pub fn exec_ori<M: MemoryInterface>(
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

    let result = dst_val | imm;
    cpu.cpu_write_ea(dst_ea, size, result, memory);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::CARRY, false);
    cpu.set_flag(flags::OVERFLOW, false);

    8 + cycles
}

pub fn exec_eor<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src_reg: u8,
    dst: AddressingMode,
    memory: &mut M,
) -> u32 {
    let src_val = match size {
        Size::Byte => cpu.d[src_reg as usize] & 0xFF,
        Size::Word => cpu.d[src_reg as usize] & 0xFFFF,
        Size::Long => cpu.d[src_reg as usize],
    };

    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let dst_val = cpu.cpu_read_ea(dst_ea, size, memory);

    let result = src_val ^ dst_val;

    cpu.cpu_write_ea(dst_ea, size, result, memory);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::CARRY, false);

    4 + cycles
}

pub fn exec_eori<M: MemoryInterface>(
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

    let result = dst_val ^ imm;
    cpu.cpu_write_ea(dst_ea, size, result, memory);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::CARRY, false);
    cpu.set_flag(flags::OVERFLOW, false);

    8 + cycles
}

pub fn exec_not<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    memory: &mut M,
) -> u32 {
    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let val = read_ea(dst_ea, size, &cpu.d, &cpu.a, memory);

    let result = !val;

    cpu.cpu_write_ea(dst_ea, size, result, memory);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::CARRY, false);

    4 + cycles
}

pub fn exec_shift<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    count: ShiftCount,
    left: bool,
    arithmetic: bool,
    memory: &mut M,
) -> u32 {
    let count_val = match count {
        ShiftCount::Immediate(n) => n as u32,
        ShiftCount::Register(r) => cpu.d[r as usize] & 63,
    };

    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let val = read_ea(dst_ea, size, &cpu.d, &cpu.a, memory);

    let (mask, size_bits, sign_bit) = match size {
        Size::Byte => (0xFFu32, 8u32, 0x80u32),
        Size::Word => (0xFFFF, 16, 0x8000),
        Size::Long => (0xFFFFFFFF, 32, 0x80000000),
    };

    let val = val & mask;
    let mut result = val;
    let mut carry = false;
    let mut overflow = false;

    if count_val > 0 {
        if left {
            if count_val >= size_bits {
                result = 0;
                carry = if count_val == size_bits {
                    (val & 1) != 0
                } else {
                    false
                };
                if arithmetic {
                    // For ASL, overflow occurs if the result is not the same as the original value multiplied by 2^n
                    // which for large shifts means we lost non-zero bits.
                    overflow = val != 0;
                }
            } else {
                carry = ((val >> (size_bits - count_val)) & 1) != 0;
                result = (val << count_val) & mask;
                if arithmetic {
                    // Check if the bits that passed through the MSB were all consistent
                    // This means bits [size-1 .. size-1-count] must be all 0s or all 1s.
                    // Mask for these bits:
                    let check_mask = mask & (!0u32 << (size_bits - count_val - 1));
                    let masked = val & check_mask;
                    overflow = (masked != 0) && (masked != check_mask);
                }
            }
        } else {
            // Right shift
            if count_val >= size_bits {
                if arithmetic && (val & sign_bit) != 0 {
                    result = mask; // Sign extended -1
                    carry = true; // Last bit shifted out was sign bit (1)
                } else {
                    result = 0;
                    carry = if arithmetic {
                        false // Sign bit was 0
                    } else {
                        // Logical: Last bit out depends on count
                        if count_val == size_bits {
                            (val & sign_bit) != 0
                        } else {
                            false
                        }
                    };
                }
            } else {
                carry = ((val >> (count_val - 1)) & 1) != 0;
                if arithmetic {
                    let shifted = val >> count_val;
                    if (val & sign_bit) != 0 {
                        // Sign extend
                        let sign_mask = mask & (!0u32 << (size_bits - count_val));
                        result = shifted | sign_mask;
                    } else {
                        result = shifted;
                    }
                } else {
                    result = val >> count_val;
                }
            }
        }
    }

    cpu.cpu_write_ea(dst_ea, size, result, memory);

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

pub fn exec_shift_mem<M: MemoryInterface>(
    cpu: &mut Cpu,
    dst: AddressingMode,
    left: bool,
    arithmetic: bool,
    memory: &mut M,
) -> u32 {
    // Memory shifts are always word size, count 1
    let cycles = exec_shift(
        cpu,
        Size::Word,
        dst,
        ShiftCount::Immediate(1),
        left,
        arithmetic,
        memory,
    );
    // V is always cleared for memory shifts
    cpu.set_flag(flags::OVERFLOW, false);
    cycles + 2 // Memory shifts take 8 cycles + EA (exec_shift returns 6 + cycles + 2*1 = 8 + cycles)
}

pub fn exec_rotate<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    count: ShiftCount,
    left: bool,
    _extend: bool,
    memory: &mut M,
) -> u32 {
    let count_val = match count {
        ShiftCount::Immediate(n) => n as u32,
        ShiftCount::Register(r) => cpu.d[r as usize] & 63,
    };

    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let val = read_ea(dst_ea, size, &cpu.d, &cpu.a, memory);

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
                carry = (val & 1) != 0;
            }
        } else {
            result = ((val << effective_count) | (val >> (bits - effective_count))) & mask;
            carry = ((val >> (bits - effective_count)) & 1) != 0;
        }
    } else if effective_count == 0 {
        result = val;
        if count_val > 0 {
            carry = (val & msb) != 0;
        }
    } else {
        result = ((val >> effective_count) | (val << (bits - effective_count))) & mask;
        carry = ((val >> (effective_count - 1)) & 1) != 0;
    }

    cpu.cpu_write_ea(dst_ea, size, result, memory);

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::CARRY, carry);
    cpu.set_flag(flags::OVERFLOW, false);

    6 + cycles + 2 * count_val
}

pub fn exec_roxl<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    count: ShiftCount,
    memory: &mut M,
) -> u32 {
    let count_val = match count {
        ShiftCount::Immediate(n) => n as u32,
        ShiftCount::Register(r) => cpu.d[r as usize] & 63,
    };

    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let val = cpu.cpu_read_ea(dst_ea, size, memory);

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

    cpu.cpu_write_ea(dst_ea, size, res, memory);
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

pub fn exec_roxr<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    count: ShiftCount,
    memory: &mut M,
) -> u32 {
    let count_val = match count {
        ShiftCount::Immediate(n) => n as u32,
        ShiftCount::Register(r) => cpu.d[r as usize] & 63,
    };

    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    let val = cpu.cpu_read_ea(dst_ea, size, memory);

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

    cpu.cpu_write_ea(dst_ea, size, res, memory);
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

enum BitOp {
    Test,
    Set,
    Clear,
    Change,
}

fn exec_bit_instruction<M: MemoryInterface>(
    cpu: &mut Cpu,
    bit: BitSource,
    dst: AddressingMode,
    memory: &mut M,
    op: BitOp,
) -> u32 {
    let bit_num = cpu.fetch_bit_num(bit, memory);
    let is_memory = !matches!(dst, AddressingMode::DataRegister(_));
    let size = if is_memory { Size::Byte } else { Size::Long };

    let mut cycles = if matches!(op, BitOp::Test) {
        4u32
    } else {
        8u32
    };
    let (dst_ea, dst_cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += dst_cycles;

    let val = cpu.cpu_read_ea(dst_ea, size, memory);
    let bit_idx = cpu.resolve_bit_index(bit_num, is_memory);
    let bit_val = (val >> bit_idx) & 1;

    cpu.set_flag(flags::ZERO, bit_val == 0);

    match op {
        BitOp::Test => {
            if is_memory {
                cycles += 4;
            } else {
                cycles += 6;
            }
        }
        BitOp::Set => {
            let new_val = val | (1 << bit_idx);
            cpu.cpu_write_ea(dst_ea, size, new_val, memory);
        }
        BitOp::Clear => {
            let new_val = val & !(1 << bit_idx);
            cpu.cpu_write_ea(dst_ea, size, new_val, memory);
        }
        BitOp::Change => {
            let new_val = val ^ (1 << bit_idx);
            cpu.cpu_write_ea(dst_ea, size, new_val, memory);
        }
    }

    cycles
}

pub fn exec_btst<M: MemoryInterface>(
    cpu: &mut Cpu,
    bit: BitSource,
    dst: AddressingMode,
    memory: &mut M,
) -> u32 {
    exec_bit_instruction(cpu, bit, dst, memory, BitOp::Test)
}

pub fn exec_bset<M: MemoryInterface>(
    cpu: &mut Cpu,
    bit: BitSource,
    dst: AddressingMode,
    memory: &mut M,
) -> u32 {
    exec_bit_instruction(cpu, bit, dst, memory, BitOp::Set)
}

pub fn exec_bclr<M: MemoryInterface>(
    cpu: &mut Cpu,
    bit: BitSource,
    dst: AddressingMode,
    memory: &mut M,
) -> u32 {
    exec_bit_instruction(cpu, bit, dst, memory, BitOp::Clear)
}

pub fn exec_bchg<M: MemoryInterface>(
    cpu: &mut Cpu,
    bit: BitSource,
    dst: AddressingMode,
    memory: &mut M,
) -> u32 {
    exec_bit_instruction(cpu, bit, dst, memory, BitOp::Change)
}

pub fn exec_tas<M: MemoryInterface>(cpu: &mut Cpu, dst: AddressingMode, memory: &mut M) -> u32 {
    let mut cycles = 4u32;
    let (dst_ea, dst_cycles) =
        calculate_ea(dst, Size::Byte, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += dst_cycles;

    let val = cpu.cpu_read_ea(dst_ea, Size::Byte, memory) as u8;

    // Set flags based on original value
    cpu.set_flag(flags::NEGATIVE, (val & 0x80) != 0);
    cpu.set_flag(flags::ZERO, val == 0);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::CARRY, false);

    // Set high bit (atomically on real hardware)
    let new_val = val | 0x80;
    cpu.cpu_write_ea(dst_ea, Size::Byte, new_val as u32, memory);

    cycles + 4
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::decoder::{AddressingMode, ShiftCount, Size};
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
    fn test_exec_or_byte() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0x123456AA;
        cpu.d[1] = 0x77665555;

        let cycles = exec_or(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            true,
            &mut memory,
        );

        assert_eq!(cpu.d[1], 0x776655FF); // 0xAA | 0x55 = 0xFF
        assert!(cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));
        assert_eq!(cycles, 4);
    }

    #[test]
    fn test_exec_or_word() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0x1234F0F0;
        cpu.d[1] = 0x77660F0F;

        let cycles = exec_or(
            &mut cpu,
            Size::Word,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            true,
            &mut memory,
        );

        assert_eq!(cpu.d[1], 0x7766FFFF); // 0xF0F0 | 0x0F0F = 0xFFFF
        assert!(cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
        assert_eq!(cycles, 4);
    }

    #[test]
    fn test_exec_or_long() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0xF0F0F0F0;
        cpu.d[1] = 0x0F0F0F0F;

        let cycles = exec_or(
            &mut cpu,
            Size::Long,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            true,
            &mut memory,
        );

        assert_eq!(cpu.d[1], 0xFFFFFFFF);
        assert!(cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
        assert_eq!(cycles, 4);
    }

    #[test]
    fn test_exec_or_zero() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0;
        cpu.d[1] = 0;
        cpu.set_flag(flags::CARRY, true);
        cpu.set_flag(flags::OVERFLOW, true);
        cpu.set_flag(flags::NEGATIVE, true);

        let cycles = exec_or(
            &mut cpu,
            Size::Long,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            true,
            &mut memory,
        );

        assert_eq!(cpu.d[1], 0);
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));
        assert_eq!(cycles, 4);
    }

    #[test]
    fn test_exec_or_memory() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0x000000FF;
        cpu.a[0] = 0x2000;
        memory.write_byte(0x2000, 0x00);

        // OR.B D0, (A0)
        let cycles = exec_or(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            AddressingMode::AddressIndirect(0),
            true,
            &mut memory,
        );

        assert_eq!(memory.read_byte(0x2000), 0xFF);
        assert!(cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
        assert_eq!(cycles, 8); // 4 (base) + 0 (DataReg) + 4 (AddrIndirect)
    }

    #[test]
    fn test_exec_or_flags_positive() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0x00;
        cpu.d[1] = 0x7F; // Positive signed byte

        // Set flags to ensure they are cleared correctly
        cpu.set_flag(flags::NEGATIVE, true);
        cpu.set_flag(flags::ZERO, true);
        cpu.set_flag(flags::CARRY, true);
        cpu.set_flag(flags::OVERFLOW, true);

        let cycles = exec_or(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            true,
            &mut memory,
        );

        assert_eq!(cpu.d[1] & 0xFF, 0x7F);
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));
        assert_eq!(cycles, 4);
    }

    #[test]
    fn test_exec_or_post_increment() {
        let (mut cpu, mut memory) = create_test_setup();
        // Setup A0 to point to 0x2000
        cpu.a[0] = 0x2000;
        // Write value to memory at 0x2000
        memory.write_byte(0x2000, 0x11);

        // Setup D0 with 0x22
        cpu.d[0] = 0x22;

        // OR.B (A0)+, D0
        let cycles = exec_or(
            &mut cpu,
            Size::Byte,
            AddressingMode::AddressPostIncrement(0),
            AddressingMode::DataRegister(0),
            true,
            &mut memory,
        );

        // Result: 0x11 | 0x22 = 0x33
        assert_eq!(cpu.d[0] & 0xFF, 0x33);
        // A0 should be incremented by 1 (Byte size)
        assert_eq!(cpu.a[0], 0x2001);

        assert_eq!(cycles, 8);
    }

    #[test]
    fn test_exec_or_complex_patterns() {
        let (mut cpu, mut memory) = create_test_setup();

        // Pattern 1: Alternating bits
        cpu.d[0] = 0xAAAAAAAA;
        cpu.d[1] = 0x55555555;

        exec_or(
            &mut cpu,
            Size::Long,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            true,
            &mut memory,
        );

        assert_eq!(cpu.d[1], 0xFFFFFFFF);
        assert!(cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));

        // Pattern 2: Sparse bits
        cpu.d[2] = 0x00010001;
        cpu.d[3] = 0x10001000;

        exec_or(
            &mut cpu,
            Size::Long,
            AddressingMode::DataRegister(2),
            AddressingMode::DataRegister(3),
            true,
            &mut memory,
        );

        assert_eq!(cpu.d[3], 0x10011001);
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
    }

    #[test]
    fn test_exec_lsl() {
        let (mut cpu, mut memory) = create_test_setup();

        // Case 1: LSL.B #1, D0 (0x01 -> 0x02)
        cpu.d[0] = 0x01;
        exec_shift(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            ShiftCount::Immediate(1),
            true,  // left
            false, // arithmetic (logical for LSL)
            &mut memory,
        );
        assert_eq!(cpu.d[0] & 0xFF, 0x02);
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::EXTEND));
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::OVERFLOW));

        // Case 2: LSL.B #1, D0 (0x80 -> 0x00)
        cpu.d[0] = 0x80;
        exec_shift(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            ShiftCount::Immediate(1),
            true,
            false,
            &mut memory,
        );
        assert_eq!(cpu.d[0] & 0xFF, 0x00);
        assert!(cpu.get_flag(flags::CARRY)); // Bit 7 shifted out
        assert!(cpu.get_flag(flags::EXTEND));
        assert!(cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::OVERFLOW)); // LSL clears V

        // Case 3: LSL.B by 0, D0 (No change)
        cpu.d[1] = 0; // Count = 0
        cpu.d[0] = 0xFF;
        cpu.set_flag(flags::CARRY, true); // Should be cleared
        exec_shift(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            ShiftCount::Register(1),
            true,
            false,
            &mut memory,
        );
        assert_eq!(cpu.d[0] & 0xFF, 0xFF);
        assert!(!cpu.get_flag(flags::CARRY)); // Cleared when count is 0
        assert!(cpu.get_flag(flags::NEGATIVE));
    }

    #[test]
    fn test_exec_lsr() {
        let (mut cpu, mut memory) = create_test_setup();

        // Case 1: LSR.B #1, D0 (0x02 -> 0x01)
        cpu.d[0] = 0x02;
        exec_shift(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            ShiftCount::Immediate(1),
            false, // right
            false, // logical
            &mut memory,
        );
        assert_eq!(cpu.d[0] & 0xFF, 0x01);
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::EXTEND));
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::OVERFLOW));

        // Case 2: LSR.B #1, D0 (0x01 -> 0x00)
        cpu.d[0] = 0x01;
        exec_shift(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            ShiftCount::Immediate(1),
            false,
            false,
            &mut memory,
        );
        assert_eq!(cpu.d[0] & 0xFF, 0x00);
        assert!(cpu.get_flag(flags::CARRY)); // Bit 0 shifted out
        assert!(cpu.get_flag(flags::EXTEND));
        assert!(cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::OVERFLOW));
    }

    #[test]
    fn test_exec_asl() {
        let (mut cpu, mut memory) = create_test_setup();

        // Case 1: ASL.B #1, D0 (0x01 -> 0x02). V=0.
        cpu.d[0] = 0x01;
        exec_shift(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            ShiftCount::Immediate(1),
            true, // left
            true, // arithmetic
            &mut memory,
        );
        assert_eq!(cpu.d[0] & 0xFF, 0x02);
        assert!(!cpu.get_flag(flags::OVERFLOW));
        assert!(!cpu.get_flag(flags::CARRY));

        // Case 2: ASL.B #1, D0 (0x40 -> 0x80). Sign change 0->1. V=1.
        cpu.d[0] = 0x40;
        exec_shift(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            ShiftCount::Immediate(1),
            true,
            true,
            &mut memory,
        );
        assert_eq!(cpu.d[0] & 0xFF, 0x80);
        assert!(cpu.get_flag(flags::OVERFLOW));
        assert!(cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::CARRY));

        // Case 3: ASL.B #1, D0 (0x80 -> 0x00). Sign change 1->0. V=1.
        cpu.d[0] = 0x80;
        exec_shift(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            ShiftCount::Immediate(1),
            true,
            true,
            &mut memory,
        );
        assert_eq!(cpu.d[0] & 0xFF, 0x00);
        assert!(cpu.get_flag(flags::OVERFLOW));
        assert!(cpu.get_flag(flags::ZERO));
        assert!(cpu.get_flag(flags::CARRY));

        // Case 4: ASL.B #1, D0 (0xC0 -> 0x80). Sign change 1->1. V=0.
        cpu.d[0] = 0xC0;
        exec_shift(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            ShiftCount::Immediate(1),
            true,
            true,
            &mut memory,
        );
        assert_eq!(cpu.d[0] & 0xFF, 0x80);
        assert!(!cpu.get_flag(flags::OVERFLOW));
        assert!(cpu.get_flag(flags::NEGATIVE));
        assert!(cpu.get_flag(flags::CARRY));
    }

    #[test]
    fn test_exec_asr() {
        let (mut cpu, mut memory) = create_test_setup();

        // Case 1: ASR.B #1, D0 (0x80 -> 0xC0). Sign preserved (-128 -> -64).
        cpu.d[0] = 0x80;
        exec_shift(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            ShiftCount::Immediate(1),
            false, // right
            true,  // arithmetic
            &mut memory,
        );
        assert_eq!(cpu.d[0] & 0xFF, 0xC0);
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::EXTEND));
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::OVERFLOW));

        // Case 2: ASR.B #1, D0 (0x02 -> 0x01).
        cpu.d[0] = 0x02;
        exec_shift(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            ShiftCount::Immediate(1),
            false,
            true,
            &mut memory,
        );
        assert_eq!(cpu.d[0] & 0xFF, 0x01);
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::EXTEND));
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::OVERFLOW));

        // Case 3: ASR.B #1, D0 (0x01 -> 0x00). C=1.
        cpu.d[0] = 0x01;
        exec_shift(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            ShiftCount::Immediate(1),
            false,
            true,
            &mut memory,
        );
        assert_eq!(cpu.d[0] & 0xFF, 0x00);
        assert!(cpu.get_flag(flags::CARRY));
        assert!(cpu.get_flag(flags::EXTEND));
        assert!(cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::NEGATIVE));
    }

    #[test]
    fn test_exec_shift_counts() {
        let (mut cpu, mut memory) = create_test_setup();

        // Case 1: LSL.B #8, D0. 8 is large for byte (clears it).
        // 0xFF -> 0x00.
        cpu.d[0] = 0xFF;
        exec_shift(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            ShiftCount::Immediate(8),
            true, // left
            false,
            &mut memory,
        );
        assert_eq!(cpu.d[0] & 0xFF, 0x00);
        assert!(cpu.get_flag(flags::CARRY)); // Last bit shifted out was 1.
        assert!(cpu.get_flag(flags::ZERO));

        // Case 2: Register Count Modulo 63.
        // D1 = 64 (0x40). 64 & 63 = 0.
        // LSL.B D1, D0. Shift by 0.
        cpu.d[1] = 64;
        cpu.d[0] = 0xFF;
        cpu.set_flag(flags::CARRY, true);
        exec_shift(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            ShiftCount::Register(1),
            true,
            false,
            &mut memory,
        );
        assert_eq!(cpu.d[0] & 0xFF, 0xFF);
        assert!(!cpu.get_flag(flags::CARRY)); // Cleared for count 0.

        // Case 3: Register Count Modulo 63.
        // D1 = 33 (0x21). 33 & 63 = 33.
        // LSL.L D1, D0. Shift by 33.
        // 0xFFFFFFFF << 33 = 0.
        cpu.d[1] = 33;
        cpu.d[0] = 0xFFFFFFFF;
        exec_shift(
            &mut cpu,
            Size::Long,
            AddressingMode::DataRegister(0),
            ShiftCount::Register(1),
            true,
            false,
            &mut memory,
        );
        assert_eq!(cpu.d[0], 0);
        // Last shifted bit depends on the sequence.
        // 32nd shift: 1 shifted out (result 0). 33rd shift: 0 shifted out (result 0).
        // C should be 0.
        assert!(!cpu.get_flag(flags::CARRY));
    }

    #[test]
    fn test_exec_eor_byte() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0xFF;
        cpu.d[1] = 0xAA;

        let cycles = exec_eor(
            &mut cpu,
            Size::Byte,
            0,
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert_eq!(cpu.d[1] & 0xFF, 0x55); // 0xFF ^ 0xAA = 0x55
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));
        assert_eq!(cycles, 4); // 4 + 0 cycles
    }

    #[test]
    fn test_exec_eor_word() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0xFFFF;
        cpu.d[1] = 0xAAAA;

        let cycles = exec_eor(
            &mut cpu,
            Size::Word,
            0,
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert_eq!(cpu.d[1] & 0xFFFF, 0x5555);
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
        assert_eq!(cycles, 4);
    }

    #[test]
    fn test_exec_eor_long() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0xFFFFFFFF;
        cpu.d[1] = 0xAAAAAAAA;

        let cycles = exec_eor(
            &mut cpu,
            Size::Long,
            0,
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert_eq!(cpu.d[1], 0x55555555);
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
        assert_eq!(cycles, 4);
    }

    #[test]
    fn test_exec_eor_memory() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0xFF;
        memory.write_byte(0x2000, 0xAA);
        cpu.a[0] = 0x2000;

        let cycles = exec_eor(
            &mut cpu,
            Size::Byte,
            0,
            AddressingMode::AddressIndirect(0),
            &mut memory,
        );

        assert_eq!(memory.read_byte(0x2000), 0x55);
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));
        assert_eq!(cycles, 8); // 4 + 4 cycles
    }

    #[test]
    fn test_exec_eor_flags() {
        let (mut cpu, mut memory) = create_test_setup();
        // Test Zero flag
        cpu.d[0] = 0xAA;
        cpu.d[1] = 0xAA;
        cpu.set_flag(flags::CARRY, true);
        cpu.set_flag(flags::OVERFLOW, true);

        exec_eor(
            &mut cpu,
            Size::Byte,
            0,
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert_eq!(cpu.d[1] & 0xFF, 0x00);
        assert!(cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));

        // Test Negative flag
        cpu.d[0] = 0x00;
        cpu.d[1] = 0x80;

        exec_eor(
            &mut cpu,
            Size::Byte,
            0,
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert_eq!(cpu.d[1] & 0xFF, 0x80);
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));
    }

    #[test]
    fn test_exec_and_byte() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0xAA;
        cpu.d[1] = 0x55;

        let cycles = exec_and(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            true,
            &mut memory,
        );

        assert_eq!(cpu.d[1] & 0xFF, 0x00); // 0xAA & 0x55 = 0x00
        assert!(cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));
        assert_eq!(cycles, 4);

        cpu.d[0] = 0xFF;
        cpu.d[1] = 0xAA;

        exec_and(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            true,
            &mut memory,
        );

        assert_eq!(cpu.d[1] & 0xFF, 0xAA); // 0xFF & 0xAA = 0xAA
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(cpu.get_flag(flags::NEGATIVE));
    }

    #[test]
    fn test_exec_shift_comprehensive() {
        struct TestCase {
            desc: &'static str,
            size: Size,
            // If use_reg_count is true, count is put in D1 and Register(1) is used.
            // Otherwise Immediate(count as u8) is used.
            count: u32,
            use_reg_count: bool,
            left: bool,
            arithmetic: bool,
            initial_val: u32,
            initial_x: bool,

            // Expected results
            expected_val: u32,
            expected_c: bool,
            expected_v: bool,
            expected_z: bool,
            expected_n: bool,
            // If None, expects X to be same as expected_c.
            // If Some(val), expects X to be val.
            expected_x: Option<bool>,
        }

        let cases = vec![
            // --- LSL (Logical Shift Left) ---
            // LSL.B #1, 0x01 -> 0x02. C=0, V=0, Z=0, N=0, X=0
            TestCase {
                desc: "LSL.B #1, 0x01",
                size: Size::Byte,
                count: 1,
                use_reg_count: false,
                left: true,
                arithmetic: false,
                initial_val: 0x01,
                initial_x: false,
                expected_val: 0x02,
                expected_c: false,
                expected_v: false,
                expected_z: false,
                expected_n: false,
                expected_x: None,
            },
            // LSL.B #1, 0x80 -> 0x00. C=1, V=0, Z=1, N=0, X=1
            TestCase {
                desc: "LSL.B #1, 0x80",
                size: Size::Byte,
                count: 1,
                use_reg_count: false,
                left: true,
                arithmetic: false,
                initial_val: 0x80,
                initial_x: false,
                expected_val: 0x00,
                expected_c: true,
                expected_v: false,
                expected_z: true,
                expected_n: false,
                expected_x: None,
            },
            // LSL.W #1, 0x0001 -> 0x0002.
            TestCase {
                desc: "LSL.W #1, 0x0001",
                size: Size::Word,
                count: 1,
                use_reg_count: false,
                left: true,
                arithmetic: false,
                initial_val: 0x0001,
                initial_x: false,
                expected_val: 0x0002,
                expected_c: false,
                expected_v: false,
                expected_z: false,
                expected_n: false,
                expected_x: None,
            },
            // LSL.L #1, 0x00010000 -> 0x00020000.
            TestCase {
                desc: "LSL.L #1, 0x00010000",
                size: Size::Long,
                count: 1,
                use_reg_count: false,
                left: true,
                arithmetic: false,
                initial_val: 0x00010000,
                initial_x: false,
                expected_val: 0x00020000,
                expected_c: false,
                expected_v: false,
                expected_z: false,
                expected_n: false,
                expected_x: None,
            },
            // --- LSR (Logical Shift Right) ---
            // LSR.B #1, 0x02 -> 0x01.
            TestCase {
                desc: "LSR.B #1, 0x02",
                size: Size::Byte,
                count: 1,
                use_reg_count: false,
                left: false,
                arithmetic: false,
                initial_val: 0x02,
                initial_x: false,
                expected_val: 0x01,
                expected_c: false,
                expected_v: false,
                expected_z: false,
                expected_n: false,
                expected_x: None,
            },
            // LSR.B #1, 0x01 -> 0x00. C=1.
            TestCase {
                desc: "LSR.B #1, 0x01",
                size: Size::Byte,
                count: 1,
                use_reg_count: false,
                left: false,
                arithmetic: false,
                initial_val: 0x01,
                initial_x: false,
                expected_val: 0x00,
                expected_c: true,
                expected_v: false,
                expected_z: true,
                expected_n: false,
                expected_x: None,
            },
            // --- ASL (Arithmetic Shift Left) ---
            // ASL is LSL but sets V on sign change.
            // ASL.B #1, 0x40 (01000000) -> 0x80 (10000000). Sign changed 0->1. V=1.
            TestCase {
                desc: "ASL.B #1, 0x40 (Overflow)",
                size: Size::Byte,
                count: 1,
                use_reg_count: false,
                left: true,
                arithmetic: true,
                initial_val: 0x40,
                initial_x: false,
                expected_val: 0x80,
                expected_c: false,
                expected_v: true,
                expected_z: false,
                expected_n: true,
                expected_x: None,
            },
            // ASL.B #1, 0x01 -> 0x02. No sign change. V=0.
            TestCase {
                desc: "ASL.B #1, 0x01",
                size: Size::Byte,
                count: 1,
                use_reg_count: false,
                left: true,
                arithmetic: true,
                initial_val: 0x01,
                initial_x: false,
                expected_val: 0x02,
                expected_c: false,
                expected_v: false,
                expected_z: false,
                expected_n: false,
                expected_x: None,
            },
            // --- ASR (Arithmetic Shift Right) ---
            // ASR preserves MSB (sign bit).
            // ASR.B #1, 0x80 (-128) -> 0xC0 (-64). 10000000 -> 11000000.
            TestCase {
                desc: "ASR.B #1, 0x80",
                size: Size::Byte,
                count: 1,
                use_reg_count: false,
                left: false,
                arithmetic: true,
                initial_val: 0x80,
                initial_x: false,
                expected_val: 0xC0,
                expected_c: false,
                expected_v: false,
                expected_z: false,
                expected_n: true,
                expected_x: None,
            },
            // ASR.B #1, 0x01 -> 0x00. C=1.
            TestCase {
                desc: "ASR.B #1, 0x01",
                size: Size::Byte,
                count: 1,
                use_reg_count: false,
                left: false,
                arithmetic: true,
                initial_val: 0x01,
                initial_x: false,
                expected_val: 0x00,
                expected_c: true,
                expected_v: false,
                expected_z: true,
                expected_n: false,
                expected_x: None,
            },
            // --- Shift Counts and Edge Cases ---

            // Shift by 0 (Immediate). Should clear C, Clear V, Unaffected X.
            // LSL.B #0, 0xFF. Res=0xFF. C=0, V=0. X=initial (true).
            TestCase {
                desc: "LSL.B #0, 0xFF",
                size: Size::Byte,
                count: 0,
                use_reg_count: false,
                left: true,
                arithmetic: false,
                initial_val: 0xFF,
                initial_x: true,
                expected_val: 0xFF,
                expected_c: false,
                expected_v: false,
                expected_z: false,
                expected_n: true,
                expected_x: Some(true),
            },
            // Shift by 8 (Size of Byte). Clears register. C=last bit out.
            // LSL.B #8, 0xFF. 11111111 << 8 -> 00000000. Last bit out was 1.
            TestCase {
                desc: "LSL.B #8, 0xFF",
                size: Size::Byte,
                count: 8,
                use_reg_count: false,
                left: true,
                arithmetic: false,
                initial_val: 0xFF,
                initial_x: false,
                expected_val: 0x00,
                expected_c: true,
                expected_v: false,
                expected_z: true,
                expected_n: false,
                expected_x: None,
            },
            // Shift by 9 (Size + 1). Clears register. C=0 (last bit out was 0 from the previous shifts).
            // 0xFF << 8 -> C=1, Val=0. Then << 1 -> C=0, Val=0.
            TestCase {
                desc: "LSL.B #9, 0xFF",
                size: Size::Byte,
                count: 9,
                use_reg_count: false,
                left: true,
                arithmetic: false,
                initial_val: 0xFF,
                initial_x: false,
                expected_val: 0x00,
                expected_c: false,
                expected_v: false,
                expected_z: true,
                expected_n: false,
                expected_x: None,
            },
            // Register Count: Shift by 0.
            // D1 = 0.
            TestCase {
                desc: "LSL.B D1(0), 0xFF",
                size: Size::Byte,
                count: 0,
                use_reg_count: true,
                left: true,
                arithmetic: false,
                initial_val: 0xFF,
                initial_x: true,
                expected_val: 0xFF,
                expected_c: false,
                expected_v: false,
                expected_z: false,
                expected_n: true,
                expected_x: Some(true),
            },
            // Register Count Modulo 63: Shift by 64 (should be 0).
            TestCase {
                desc: "LSL.B D1(64), 0xFF",
                size: Size::Byte,
                count: 64,
                use_reg_count: true,
                left: true,
                arithmetic: false,
                initial_val: 0xFF,
                initial_x: true,
                expected_val: 0xFF,
                expected_c: false,
                expected_v: false,
                expected_z: false,
                expected_n: true,
                expected_x: Some(true),
            },
        ];

        for case in cases {
            let (mut cpu, mut memory) = create_test_setup();

            // Setup operands
            cpu.d[0] = case.initial_val;

            let count_arg = if case.use_reg_count {
                cpu.d[1] = case.count;
                ShiftCount::Register(1)
            } else {
                ShiftCount::Immediate(case.count as u8)
            };

            // Setup initial flags
            cpu.set_flag(flags::EXTEND, case.initial_x);
            // Set other flags to known bad state to ensure they are updated
            cpu.set_flag(flags::CARRY, !case.expected_c);
            cpu.set_flag(flags::OVERFLOW, !case.expected_v);
            cpu.set_flag(flags::ZERO, !case.expected_z);
            cpu.set_flag(flags::NEGATIVE, !case.expected_n);

            exec_shift(
                &mut cpu,
                case.size,
                AddressingMode::DataRegister(0),
                count_arg,
                case.left,
                case.arithmetic,
                &mut memory,
            );

            // Mask result to size
            let res_masked = match case.size {
                Size::Byte => cpu.d[0] & 0xFF,
                Size::Word => cpu.d[0] & 0xFFFF,
                Size::Long => cpu.d[0],
            };

            assert_eq!(
                res_masked, case.expected_val,
                "{}: Value mismatch",
                case.desc
            );
            assert_eq!(
                cpu.get_flag(flags::CARRY),
                case.expected_c,
                "{}: C flag mismatch",
                case.desc
            );
            assert_eq!(
                cpu.get_flag(flags::OVERFLOW),
                case.expected_v,
                "{}: V flag mismatch",
                case.desc
            );
            assert_eq!(
                cpu.get_flag(flags::ZERO),
                case.expected_z,
                "{}: Z flag mismatch",
                case.desc
            );
            assert_eq!(
                cpu.get_flag(flags::NEGATIVE),
                case.expected_n,
                "{}: N flag mismatch",
                case.desc
            );

            if let Some(expected_x) = case.expected_x {
                assert_eq!(
                    cpu.get_flag(flags::EXTEND),
                    expected_x,
                    "{}: X flag mismatch (Explicit)",
                    case.desc
                );
            } else {
                assert_eq!(
                    cpu.get_flag(flags::EXTEND),
                    case.expected_c,
                    "{}: X flag mismatch (Implicit=C)",
                    case.desc
                );
            }
        }
    }

    #[test]
    fn test_exec_bit_ops_memory_modulo() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.a[0] = 0x2000;

        // BSET bit 8 (mod 8 = 0)
        memory.write_byte(0x2000, 0x00);
        cpu.d[0] = 8;
        exec_bset(&mut cpu, BitSource::Register(0), AddressingMode::AddressIndirect(0), &mut memory);
        assert_eq!(memory.read_byte(0x2000), 0x01);
        assert!(cpu.get_flag(flags::ZERO)); // bit 0 was clear

        // BCLR bit 9 (mod 8 = 1)
        memory.write_byte(0x2000, 0x02);
        cpu.d[0] = 9;
        exec_bclr(&mut cpu, BitSource::Register(0), AddressingMode::AddressIndirect(0), &mut memory);
        assert_eq!(memory.read_byte(0x2000), 0x00);
        assert!(!cpu.get_flag(flags::ZERO)); // bit 1 was set

        // BCHG bit 10 (mod 8 = 2)
        memory.write_byte(0x2000, 0x00);
        cpu.d[0] = 10;
        exec_bchg(&mut cpu, BitSource::Register(0), AddressingMode::AddressIndirect(0), &mut memory);
        assert_eq!(memory.read_byte(0x2000), 0x04);
        assert!(cpu.get_flag(flags::ZERO)); // bit 2 was clear

        // BTST bit 11 (mod 8 = 3)
        memory.write_byte(0x2000, 0x08);
        cpu.d[0] = 11;
        exec_btst(&mut cpu, BitSource::Register(0), AddressingMode::AddressIndirect(0), &mut memory);
        assert!(!cpu.get_flag(flags::ZERO)); // bit 3 is set
    }

    #[test]
    fn test_exec_bit_ops_register_modulo() {
        let (mut cpu, mut memory) = create_test_setup();

        // BSET bit 32 (mod 32 = 0)
        cpu.d[0] = 0x00000000;
        cpu.d[1] = 32;
        exec_bset(&mut cpu, BitSource::Register(1), AddressingMode::DataRegister(0), &mut memory);
        assert_eq!(cpu.d[0], 0x00000001);
        assert!(cpu.get_flag(flags::ZERO));

        // BCLR bit 33 (mod 32 = 1)
        cpu.d[0] = 0x00000002;
        cpu.d[1] = 33;
        exec_bclr(&mut cpu, BitSource::Register(1), AddressingMode::DataRegister(0), &mut memory);
        assert_eq!(cpu.d[0], 0x00000000);
        assert!(!cpu.get_flag(flags::ZERO));
    }
}
