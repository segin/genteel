use crate::cpu::addressing::{calculate_ea, EffectiveAddress};
use crate::cpu::decoder::{AddressingMode, Size};
use crate::cpu::flags;
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

    // Calculate source EA
    let (src_ea, src_cycles) = calculate_ea(src, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += src_cycles;
    if cpu.pending_exception {
        return cycles;
    }

    // Read source value
    let value = cpu.cpu_read_ea(src_ea, size, memory);
    if cpu.pending_exception {
        return cycles;
    }

    // Calculate destination EA
    let (dst_ea, dst_cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += dst_cycles;
    if cpu.pending_exception {
        return cycles;
    }

    // Write to destination
    cpu.cpu_write_ea(dst_ea, size, value, memory);

    // Update flags
    cpu.update_nz_flags(value, size);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::CARRY, false);

    cycles
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

    let value = cpu.cpu_read_ea(src_ea, size, memory);

    // Sign-extend to 32 bits for word size
    let value = match size {
        Size::Word => (value as i16) as i32 as u32,
        Size::Long => value,
        Size::Byte => value, // Should not happen for MOVEA
    };

    cpu.a[dst_reg as usize] = value;

    // MOVEA does not affect flags
    cycles
}

pub fn exec_moveq(cpu: &mut Cpu, dst_reg: u8, data: i8) -> u32 {
    let value = data as i32 as u32;
    cpu.d[dst_reg as usize] = value;

    cpu.update_nz_flags(value, Size::Long);
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

    4 + cycles
}

pub fn exec_clr<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    memory: &mut M,
) -> u32 {
    let (dst_ea, cycles) = calculate_ea(dst, size, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);

    cpu.cpu_write_ea(dst_ea, size, 0, memory);

    // CLR always sets Z=1, N=0, V=0, C=0
    cpu.sr = (cpu.sr & !0x000F) | flags::ZERO;

    4 + cycles
}

pub fn exec_movep<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    reg: u8,
    an: u8,
    reg_to_mem: bool,
    memory: &mut M,
) -> u32 {
    let disp = cpu.read_word(cpu.pc, memory) as i16;
    cpu.pc = cpu.pc.wrapping_add(2);

    let addr = cpu.a[an as usize].wrapping_add(disp as u32);

    match size {
        Size::Word => {
            if reg_to_mem {
                let val = cpu.d[reg as usize] as u16;
                cpu.write_byte(addr, (val >> 8) as u8, memory);
                cpu.write_byte(addr.wrapping_add(2), val as u8, memory);
            } else {
                let hi = memory.read_byte(addr);
                let lo = memory.read_byte(addr.wrapping_add(2));
                let val = ((hi as u16) << 8) | (lo as u16);
                cpu.d[reg as usize] = (cpu.d[reg as usize] & 0xFFFF0000) | (val as u32);
            }
            16
        }
        Size::Long => {
            if reg_to_mem {
                let val = cpu.d[reg as usize];
                cpu.write_byte(addr, (val >> 24) as u8, memory);
                cpu.write_byte(addr.wrapping_add(2), (val >> 16) as u8, memory);
                cpu.write_byte(addr.wrapping_add(4), (val >> 8) as u8, memory);
                cpu.write_byte(addr.wrapping_add(6), val as u8, memory);
            } else {
                let b3 = memory.read_byte(addr);
                let b2 = memory.read_byte(addr.wrapping_add(2));
                let b1 = memory.read_byte(addr.wrapping_add(4));
                let b0 = memory.read_byte(addr.wrapping_add(6));
                cpu.d[reg as usize] =
                    ((b3 as u32) << 24) | ((b2 as u32) << 16) | ((b1 as u32) << 8) | (b0 as u32);
            }
            24
        }
        _ => 4, // Should not happen for MOVEC
    }
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
                    // Data register: Word load affects only lower 16 bits, Long load affects all
                    if size == Size::Word {
                        let val = cpu.read_word(addr, memory);
                        cpu.d[i] = (cpu.d[i] & 0xFFFF0000) | (val as u32);
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

pub fn exec_pea<M: MemoryInterface>(cpu: &mut Cpu, src: AddressingMode, memory: &mut M) -> u32 {
    let mut cycles = 12u32;
    let (src_ea, src_cycles) =
        calculate_ea(src, Size::Long, &mut cpu.d, &mut cpu.a, &mut cpu.pc, memory);
    cycles += src_cycles;

    let addr = match src_ea {
        EffectiveAddress::Memory(a) => a,
        _ => 0, // Should not happen for control addressing modes
    };

    cpu.push_long(addr, memory);
    cycles
}

pub fn exec_exg(cpu: &mut Cpu, rx: u8, ry: u8, mode: u8) -> u32 {
    // Mode comes from bits 3-7 of opcode.
    // 01000 (8): Dx, Dy
    // 01001 (9): Ax, Ay
    // 10001 (17): Dx, Ay

    match mode {
        0x08 => {
            // Dx, Dy
            cpu.d.swap(rx as usize, ry as usize);
        }
        0x09 => {
            // Ax, Ay
            cpu.a.swap(rx as usize, ry as usize);
        }
        0x11 => {
            // Dx, Ay
            std::mem::swap(&mut cpu.d[rx as usize], &mut cpu.a[ry as usize]);
        }
        _ => {
            // Should not happen if decoder is correct
            #[cfg(debug_assertions)]
            eprintln!("Invalid EXG mode: {:02X}", mode);
        }
    }

    6
}

pub fn exec_swap(cpu: &mut Cpu, reg: u8) -> u32 {
    let val = cpu.d[reg as usize];
    let result = val.rotate_left(16);
    cpu.d[reg as usize] = result;

    cpu.update_nz_flags(result, Size::Long);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::CARRY, false);

    4
}

pub fn exec_ext(cpu: &mut Cpu, size: Size, reg: u8) -> u32 {
    let val = cpu.d[reg as usize];
    let result = match size {
        Size::Word => (val as i8) as i16 as u32 & 0xFFFF | (val & 0xFFFF0000),
        Size::Long => (val as i16) as i32 as u32,
        Size::Byte => val, // Should not happen
    };
    cpu.d[reg as usize] = result;

    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::OVERFLOW, false);
    cpu.set_flag(flags::CARRY, false);

    4
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn test_movep_word_reg_to_mem() {
        let (mut cpu, mut memory) = create_test_setup();
        // MOVEP.W D0, (A0) with displacement 0
        let displacement: i16 = 0;
        let pc = cpu.pc;
        memory.write_word(pc, displacement as u16);

        cpu.d[0] = 0xABCD1234;
        cpu.a[0] = 0x2000;

        let cycles = exec_movep(
            &mut cpu,
            Size::Word,
            0, // D0
            0, // A0
            true, // reg_to_mem
            &mut memory,
        );

        assert_eq!(cycles, 16);
        assert_eq!(memory.read_byte(0x2000), 0x12);
        assert_eq!(memory.read_byte(0x2002), 0x34);
        // Ensure other bytes are untouched
        assert_eq!(memory.read_byte(0x2001), 0x00);
        assert_eq!(memory.read_byte(0x2003), 0x00);
    }

    #[test]
    fn test_movep_word_mem_to_reg() {
        let (mut cpu, mut memory) = create_test_setup();
        // MOVEP.W (A0), D0 with displacement 0
        let displacement: i16 = 0;
        let pc = cpu.pc;
        memory.write_word(pc, displacement as u16);

        cpu.d[0] = 0xABCD1234;
        cpu.a[0] = 0x2000;
        memory.write_byte(0x2000, 0x56);
        memory.write_byte(0x2002, 0x78);

        let cycles = exec_movep(
            &mut cpu,
            Size::Word,
            0, // D0
            0, // A0
            false, // mem_to_reg
            &mut memory,
        );

        assert_eq!(cycles, 16);
        // Should preserve high word of register
        assert_eq!(cpu.d[0], 0xABCD5678);
    }

    #[test]
    fn test_movep_long_reg_to_mem() {
        let (mut cpu, mut memory) = create_test_setup();
        // MOVEP.L D0, (A0) with displacement 0
        let displacement: i16 = 0;
        let pc = cpu.pc;
        memory.write_word(pc, displacement as u16);

        cpu.d[0] = 0x12345678;
        cpu.a[0] = 0x2000;

        let cycles = exec_movep(
            &mut cpu,
            Size::Long,
            0, // D0
            0, // A0
            true, // reg_to_mem
            &mut memory,
        );

        assert_eq!(cycles, 24);
        assert_eq!(memory.read_byte(0x2000), 0x12);
        assert_eq!(memory.read_byte(0x2002), 0x34);
        assert_eq!(memory.read_byte(0x2004), 0x56);
        assert_eq!(memory.read_byte(0x2006), 0x78);
    }

    #[test]
    fn test_movep_long_mem_to_reg() {
        let (mut cpu, mut memory) = create_test_setup();
        // MOVEP.L (A0), D0 with displacement 0
        let displacement: i16 = 0;
        let pc = cpu.pc;
        memory.write_word(pc, displacement as u16);

        cpu.d[0] = 0xFFFFFFFF;
        cpu.a[0] = 0x2000;
        memory.write_byte(0x2000, 0x12);
        memory.write_byte(0x2002, 0x34);
        memory.write_byte(0x2004, 0x56);
        memory.write_byte(0x2006, 0x78);

        let cycles = exec_movep(
            &mut cpu,
            Size::Long,
            0, // D0
            0, // A0
            false, // mem_to_reg
            &mut memory,
        );

        assert_eq!(cycles, 24);
        assert_eq!(cpu.d[0], 0x12345678);
    }

    #[test]
    fn test_movep_displacement() {
        let (mut cpu, mut memory) = create_test_setup();
        // Test positive displacement
        // MOVEP.W D0, (A0) with displacement 4
        let displacement: i16 = 4;
        let pc = cpu.pc;
        memory.write_word(pc, displacement as u16);

        cpu.d[0] = 0xABCD1234;
        cpu.a[0] = 0x2000;

        exec_movep(
            &mut cpu,
            Size::Word,
            0, // D0
            0, // A0
            true, // reg_to_mem
            &mut memory,
        );

        // Address should be 0x2000 + 4 = 0x2004
        assert_eq!(memory.read_byte(0x2004), 0x12);
        assert_eq!(memory.read_byte(0x2006), 0x34);

        // Test negative displacement
        // MOVEP.W D0, (A0) with displacement -4 (0xFFFC)
        let displacement: i16 = -4;
        let pc = cpu.pc; // PC moved forward by 2 in previous call
        memory.write_word(pc, displacement as u16);

        cpu.d[0] = 0xABCD5678;
        cpu.a[0] = 0x2000;

        exec_movep(
            &mut cpu,
            Size::Word,
            0, // D0
            0, // A0
            true, // reg_to_mem
            &mut memory,
        );

        // Address should be 0x2000 - 4 = 0x1FFC
        assert_eq!(memory.read_byte(0x1FFC), 0x56);
        assert_eq!(memory.read_byte(0x1FFE), 0x78);
    }
}
