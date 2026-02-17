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
                carry = (val & msb) != 0;
            }
        } else {
            result = ((val << effective_count) | (val >> (bits - effective_count))) & mask;
            carry = ((val >> (bits - effective_count)) & 1) != 0;
        }
    } else if effective_count == 0 {
        result = val;
        if count_val > 0 {
            carry = (val & 1) != 0;
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
    use crate::cpu::decoder::{AddressingMode, BitSource, Size};
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
    fn test_exec_or_memory_to_reg() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0xAA;
        cpu.a[0] = 0x2000;
        memory.write_byte(0x2000, 0x55);

        // OR.B (A0), D0
        let cycles = exec_or(
            &mut cpu,
            Size::Byte,
            AddressingMode::AddressIndirect(0),
            AddressingMode::DataRegister(0),
            false,
            &mut memory,
        );

        assert_eq!(cpu.d[0] & 0xFF, 0xFF); // 0xAA | 0x55 = 0xFF
        assert!(cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));
        assert_eq!(cycles, 8); // 4 (base) + 4 (AddrIndirect) + 0 (DataReg)
    }

    #[test]
    fn test_exec_btst_reg_long() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0b00000000_00000000_00000000_00000100; // Bit 2 is set

        // Test bit 2 (should be 1 -> Z=0)
        cpu.d[1] = 2;
        let cycles = exec_btst(
            &mut cpu,
            BitSource::Register(1),
            AddressingMode::DataRegister(0),
            &mut memory,
        );

        // Bit is 1, so Z flag should be 0 (FALSE)
        assert!(!cpu.get_flag(flags::ZERO));
        // BTST on register takes 10 cycles: 4 base + 0 (DataReg EA) + 6 (Test reg)
        assert_eq!(cycles, 10);

        // Test bit 1 (should be 0 -> Z=1)
        cpu.d[1] = 1; // Test bit 1
        exec_btst(
            &mut cpu,
            BitSource::Register(1),
            AddressingMode::DataRegister(0),
            &mut memory,
        );
        assert!(cpu.get_flag(flags::ZERO));

        // Test Modulo 32: Bit 34 (34 % 32 = 2) should test bit 2
        cpu.d[1] = 34;
        exec_btst(
            &mut cpu,
            BitSource::Register(1),
            AddressingMode::DataRegister(0),
            &mut memory,
        );
        assert!(!cpu.get_flag(flags::ZERO));
    }

    #[test]
    fn test_exec_btst_mem_byte() {
        let (mut cpu, mut memory) = create_test_setup();
        memory.write_byte(0x2000, 0b00000100); // Bit 2 is set
        cpu.a[0] = 0x2000;

        // Test bit 2 (should be 1 -> Z=0)
        cpu.d[1] = 2;
        let cycles = exec_btst(
            &mut cpu,
            BitSource::Register(1),
            AddressingMode::AddressIndirect(0),
            &mut memory,
        );

        assert!(!cpu.get_flag(flags::ZERO));
        // BTST on memory takes 12 cycles: 4 base + 4 (AddrIndirect EA) + 4 (Test mem)
        assert_eq!(cycles, 12);

        // Test bit 1 (should be 0 -> Z=1)
        cpu.d[1] = 1;
        exec_btst(
            &mut cpu,
            BitSource::Register(1),
            AddressingMode::AddressIndirect(0),
            &mut memory,
        );
        assert!(cpu.get_flag(flags::ZERO));

        // Test Modulo 8: Bit 10 (10 % 8 = 2) should test bit 2
        cpu.d[1] = 10;
        exec_btst(
            &mut cpu,
            BitSource::Register(1),
            AddressingMode::AddressIndirect(0),
            &mut memory,
        );
        assert!(!cpu.get_flag(flags::ZERO));
    }

    #[test]
    fn test_exec_btst_imm() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0b00000000_00000000_00000000_00000100; // Bit 2 is set

        // Immediate bit number is read from PC.
        // We need to set PC and write the immediate word to memory.
        cpu.pc = 0x1000;
        memory.write_word(0x1000, 0x0002); // Bit 2

        let cycles = exec_btst(
            &mut cpu,
            BitSource::Immediate,
            AddressingMode::DataRegister(0),
            &mut memory,
        );

        // PC should advance by 2
        assert_eq!(cpu.pc, 0x1002);
        assert!(!cpu.get_flag(flags::ZERO));
        // BTST Immediate Reg: 4 base + 0 EA + 6 Test + (fetch immediate?)
        // Total = 10.
        assert_eq!(cycles, 10);
    }

    #[test]
    fn test_exec_eor_byte() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0x000000FF;
        cpu.d[1] = 0x00000055;

        // EOR.B D0, D1
        let cycles = exec_eor(
            &mut cpu,
            Size::Byte,
            0,
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert_eq!(cpu.d[1], 0x000000AA); // 0xFF ^ 0x55 = 0xAA (10101010)
        assert!(cpu.get_flag(flags::NEGATIVE)); // Bit 7 is 1
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));
        assert_eq!(cycles, 4);
    }

    #[test]
    fn test_exec_eor_word() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0x0000FFFF;
        cpu.d[1] = 0x00005555;

        let cycles = exec_eor(
            &mut cpu,
            Size::Word,
            0,
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert_eq!(cpu.d[1], 0x0000AAAA);
        assert!(cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));
        assert_eq!(cycles, 4);
    }

    #[test]
    fn test_exec_eor_long() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0xFFFFFFFF;
        cpu.d[1] = 0x55555555;

        let cycles = exec_eor(
            &mut cpu,
            Size::Long,
            0,
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert_eq!(cpu.d[1], 0xAAAAAAAA);
        assert!(cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));
        assert_eq!(cycles, 4);
    }

    #[test]
    fn test_exec_eor_zero() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0xFF;
        cpu.d[1] = 0xFF;

        // Pre-set flags
        cpu.set_flag(flags::NEGATIVE, true);
        cpu.set_flag(flags::CARRY, true);
        cpu.set_flag(flags::OVERFLOW, true);

        let cycles = exec_eor(
            &mut cpu,
            Size::Byte,
            0,
            AddressingMode::DataRegister(1),
            &mut memory,
        );

        assert_eq!(cpu.d[1] & 0xFF, 0x00);
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));
        assert_eq!(cycles, 4);
    }

    #[test]
    fn test_exec_eor_memory() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0xFF;
        cpu.a[0] = 0x2000;
        memory.write_byte(0x2000, 0x55);

        // EOR.B D0, (A0)
        let cycles = exec_eor(
            &mut cpu,
            Size::Byte,
            0,
            AddressingMode::AddressIndirect(0),
            &mut memory,
        );

        assert_eq!(memory.read_byte(0x2000), 0xAA);
        assert!(cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));
        assert_eq!(cycles, 8); // 4 (base) + 4 (AddrIndirect)
    }

    #[test]
    fn test_exec_and_byte() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0x123456AA;
        cpu.d[1] = 0x77665555;

        let cycles = exec_and(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            true,
            &mut memory,
        );

        assert_eq!(cpu.d[1], 0x77665500); // 0xAA & 0x55 = 0x00
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(cpu.get_flag(flags::ZERO));
        assert!(!cpu.get_flag(flags::CARRY));
        assert!(!cpu.get_flag(flags::OVERFLOW));
        assert_eq!(cycles, 4);
    }

    #[test]
    fn test_exec_and_word() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0x1234F0F0;
        cpu.d[1] = 0x77660F0F;

        let cycles = exec_and(
            &mut cpu,
            Size::Word,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            true,
            &mut memory,
        );

        assert_eq!(cpu.d[1], 0x77660000); // 0xF0F0 & 0x0F0F = 0x0000
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(cpu.get_flag(flags::ZERO));
        assert_eq!(cycles, 4);
    }

    #[test]
    fn test_exec_and_long() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0xF0F0F0F0;
        cpu.d[1] = 0x0F0F0F0F;

        let cycles = exec_and(
            &mut cpu,
            Size::Long,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            true,
            &mut memory,
        );

        assert_eq!(cpu.d[1], 0x00000000);
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(cpu.get_flag(flags::ZERO));
        assert_eq!(cycles, 4);
    }

    #[test]
    fn test_exec_and_negative() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0xFFFFFFFF;
        cpu.d[1] = 0x80000000;

        let cycles = exec_and(
            &mut cpu,
            Size::Long,
            AddressingMode::DataRegister(0),
            AddressingMode::DataRegister(1),
            true,
            &mut memory,
        );

        assert_eq!(cpu.d[1], 0x80000000);
        assert!(cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
        assert_eq!(cycles, 4);
    }

    #[test]
    fn test_exec_and_zero() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0;
        cpu.d[1] = 0xFFFFFFFF;
        cpu.set_flag(flags::CARRY, true);
        cpu.set_flag(flags::OVERFLOW, true);
        cpu.set_flag(flags::NEGATIVE, true);

        let cycles = exec_and(
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
    fn test_exec_and_memory() {
        let (mut cpu, mut memory) = create_test_setup();
        cpu.d[0] = 0x0000000F;
        cpu.a[0] = 0x2000;
        memory.write_byte(0x2000, 0xF0);

        // AND.B D0, (A0)
        let cycles = exec_and(
            &mut cpu,
            Size::Byte,
            AddressingMode::DataRegister(0),
            AddressingMode::AddressIndirect(0),
            true,
            &mut memory,
        );

        assert_eq!(memory.read_byte(0x2000), 0x00);
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(cpu.get_flag(flags::ZERO));
        assert_eq!(cycles, 8); // 4 (base) + 0 (DataReg) + 4 (AddrIndirect)
    }
}
