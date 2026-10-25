//! Data movement instructions: MOVE, MOVEA, MOVEQ, LEA, PEA, EXG, MOVEP,
//! MOVEM, SWAP and EXT.

use crate::cpu::addressing::{address_step, EffectiveAddress};
use crate::cpu::decoder::{AddressingMode, Size};
use crate::cpu::{Cpu, Exec};
use crate::memory::MemoryInterface;

/// MOVE <ea>,<ea>. Flags are updated before the destination write, so a
/// faulting write still leaves them set.
pub fn exec_move<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    dst: AddressingMode,
    memory: &mut M,
) -> Exec {
    let src_ea = cpu.calc_ea(src, size, memory);
    let val = cpu.read_operand(src_ea, size, memory)?;
    let src_is_mem = matches!(src_ea, EffectiveAddress::Memory(_));

    match dst {
        AddressingMode::DataRegister(r) => {
            cpu.prefetch();
            cpu.set_logic_flags(val, size);
            cpu.d[r as usize] = size.apply(cpu.d[r as usize], val);
            Ok(())
        }
        AddressingMode::AddressIndirect(r) => {
            cpu.set_logic_flags(val, size);
            cpu.write(memory, cpu.a[r as usize], size, val)?;
            cpu.prefetch();
            Ok(())
        }
        AddressingMode::AddressPostIncrement(r) => {
            cpu.set_logic_flags(val, size);
            let addr = cpu.a[r as usize];
            cpu.write(memory, addr, size, val)?;
            cpu.a[r as usize] = addr.wrapping_add(address_step(r, size));
            cpu.prefetch();
            Ok(())
        }
        AddressingMode::AddressPreDecrement(r) => {
            /* The queue refill comes first; long data is written low word
            first. */
            cpu.prefetch();
            cpu.set_logic_flags(val, size);
            let r = r as usize;
            if size == Size::Long {
                /* An moves down one word per half written. */
                let lo_addr = cpu.a[r].wrapping_sub(2);
                cpu.a[r] = lo_addr;
                cpu.write(memory, lo_addr, Size::Word, val & 0xFFFF)?;
                let hi_addr = lo_addr.wrapping_sub(2);
                cpu.a[r] = hi_addr;
                cpu.write(memory, hi_addr, Size::Word, val >> 16)
            } else {
                let addr = cpu.a[r].wrapping_sub(address_step(r as u8, size));
                cpu.a[r] = addr;
                cpu.write(memory, addr, size, val)
            }
        }
        AddressingMode::AbsoluteLong if src_is_mem => {
            /* With a memory source the low address word is still in the
            queue when the write starts; both refills follow the write. */
            let hi = cpu.ext_word(memory) as u32;
            let lo = cpu.ext_word_deferred(memory) as u32;
            cpu.set_logic_flags(val, size);
            cpu.write(memory, (hi << 16) | lo, size, val)?;
            cpu.prefetch();
            cpu.prefetch();
            Ok(())
        }
        _ => {
            let dst_ea = cpu.calc_ea_with(dst, size, memory, false);
            cpu.set_logic_flags(val, size);
            cpu.write_operand(dst_ea, size, val, memory)?;
            cpu.prefetch();
            Ok(())
        }
    }
}

/// MOVEA <ea>,An (word sources sign-extended, flags untouched).
pub fn exec_movea<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> Exec {
    let ea = cpu.calc_ea(src, size, memory);
    let val = cpu.read_operand(ea, size, memory)?;
    cpu.prefetch();
    cpu.a[dst_reg as usize] = if size == Size::Word {
        val as u16 as i16 as i32 as u32
    } else {
        val
    };
    Ok(())
}

pub fn exec_moveq(cpu: &mut Cpu, dst_reg: u8, data: i8) -> Exec {
    let val = data as i32 as u32;
    cpu.d[dst_reg as usize] = val;
    cpu.set_logic_flags(val, Size::Long);
    cpu.prefetch();
    Ok(())
}

/// LEA <ea>,An.
pub fn exec_lea<M: MemoryInterface>(
    cpu: &mut Cpu,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> Exec {
    let addr = control_ea(cpu, src, memory);
    cpu.prefetch();
    cpu.a[dst_reg as usize] = addr;
    Ok(())
}

/// Control-mode address with LEA/PEA timing: extension words refill the
/// queue, an index costs 2 clocks before and 2 after its fetch.
fn control_ea<M: MemoryInterface>(cpu: &mut Cpu, mode: AddressingMode, memory: &mut M) -> u32 {
    match mode {
        AddressingMode::AddressIndex(_) | AddressingMode::PcIndex => {
            let addr = match cpu.calc_ea(mode, Size::Long, memory) {
                EffectiveAddress::Memory(a) => a,
                _ => 0,
            };
            cpu.idle(2);
            addr
        }
        _ => match cpu.calc_ea(mode, Size::Long, memory) {
            EffectiveAddress::Memory(a) => a,
            _ => 0,
        },
    }
}

/// PEA <ea>.
pub fn exec_pea<M: MemoryInterface>(cpu: &mut Cpu, src: AddressingMode, memory: &mut M) -> Exec {
    let addr = control_ea(cpu, src, memory);
    if matches!(
        src,
        AddressingMode::AbsoluteShort | AddressingMode::AbsoluteLong
    ) {
        cpu.push_long(memory, addr)?;
        cpu.prefetch();
    } else {
        cpu.prefetch();
        cpu.push_long(memory, addr)?;
    }
    Ok(())
}

pub fn exec_exg(cpu: &mut Cpu, rx: u8, ry: u8, mode: u8) -> Exec {
    let (x, y) = (rx as usize, ry as usize);
    match mode {
        0x08 => cpu.d.swap(x, y),
        0x09 => cpu.a.swap(x, y),
        0x11 => std::mem::swap(&mut cpu.d[x], &mut cpu.a[y]),
        _ => {}
    }
    cpu.prefetch();
    cpu.idle(2);
    Ok(())
}

/// MOVEP: transfer bytes between Dn and alternate memory bytes.
pub fn exec_movep<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    reg: u8,
    an: u8,
    to_memory: bool,
    memory: &mut M,
) -> Exec {
    let disp = cpu.ext_word(memory) as i16 as i32 as u32;
    let addr = cpu.a[an as usize].wrapping_add(disp);
    let count = if size == Size::Long { 4 } else { 2 };
    if to_memory {
        let val = cpu.d[reg as usize];
        for i in 0..count {
            let shift = 8 * (count - 1 - i);
            cpu.write(memory, addr.wrapping_add(2 * i), Size::Byte, val >> shift)?;
        }
    } else {
        let mut val = 0u32;
        for i in 0..count {
            val = (val << 8) | cpu.read(memory, addr.wrapping_add(2 * i), Size::Byte)?;
        }
        cpu.d[reg as usize] = size.apply(cpu.d[reg as usize], val);
    }
    cpu.prefetch();
    Ok(())
}

/// MOVEM register list transfers.
pub fn exec_movem<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    to_memory: bool,
    ea: AddressingMode,
    memory: &mut M,
) -> Exec {
    let mask = cpu.ext_word(memory);
    let step = size.bytes();

    if to_memory {
        if let AddressingMode::AddressPreDecrement(r) = ea {
            /* Registers are stored A7..D0 at descending addresses; the mask
            is bit-reversed. The stored An is its value before the
            instruction. */
            let mut addr = cpu.a[r as usize];
            for i in 0..16 {
                if mask & (1 << i) != 0 {
                    let reg = 15 - i;
                    let val = if reg >= 8 { cpu.a[reg - 8] } else { cpu.d[reg] };
                    addr = addr.wrapping_sub(step);
                    if size == Size::Long {
                        cpu.write(memory, addr.wrapping_add(2), Size::Word, val & 0xFFFF)?;
                        cpu.write(memory, addr, Size::Word, val >> 16)?;
                    } else {
                        cpu.write(memory, addr, size, val)?;
                    }
                }
            }
            cpu.a[r as usize] = addr;
            cpu.prefetch();
            return Ok(());
        }
        let mut addr = match cpu.calc_ea(ea, size, memory) {
            EffectiveAddress::Memory(a) => a,
            _ => return Ok(()),
        };
        for i in 0..16 {
            if mask & (1 << i) != 0 {
                let val = if i >= 8 { cpu.a[i - 8] } else { cpu.d[i] };
                cpu.write(memory, addr, size, val)?;
                addr = addr.wrapping_add(step);
            }
        }
        cpu.prefetch();
        return Ok(());
    }

    /* Memory to registers. Word values are sign-extended into the whole
    register. One extra word is read after the last register. */
    let (mut addr, postinc) = match ea {
        AddressingMode::AddressPostIncrement(r) => (cpu.a[r as usize], Some(r)),
        _ => match cpu.calc_ea(ea, size, memory) {
            EffectiveAddress::Memory(a) => (a, None),
            _ => return Ok(()),
        },
    };
    let result = (|| -> Exec {
        for i in 0..16 {
            if mask & (1 << i) != 0 {
                let raw = cpu.read(memory, addr, size)?;
                let val = if size == Size::Word {
                    raw as u16 as i16 as i32 as u32
                } else {
                    raw
                };
                if i >= 8 {
                    cpu.a[i - 8] = val;
                } else {
                    cpu.d[i] = val;
                }
                addr = addr.wrapping_add(step);
            }
        }
        cpu.read(memory, addr, Size::Word)?;
        Ok(())
    })();
    match (result, postinc) {
        (Ok(()), Some(r)) => cpu.a[r as usize] = addr,
        /* An odd address faults on the first read; by then the hardware
        has already advanced An by one word. */
        (Err(fault), Some(r)) => {
            cpu.a[r as usize] = fault.addr.wrapping_add(2);
            return Err(fault);
        }
        (Err(fault), None) => return Err(fault),
        (Ok(()), None) => {}
    }
    cpu.prefetch();
    Ok(())
}

pub fn exec_swap(cpu: &mut Cpu, reg: u8) -> Exec {
    let v = cpu.d[reg as usize].rotate_left(16);
    cpu.d[reg as usize] = v;
    cpu.set_logic_flags(v, Size::Long);
    cpu.prefetch();
    Ok(())
}

pub fn exec_ext(cpu: &mut Cpu, size: Size, reg: u8) -> Exec {
    let r = reg as usize;
    if size == Size::Word {
        let v = cpu.d[r] as u8 as i8 as i16 as u16 as u32;
        cpu.d[r] = (cpu.d[r] & 0xFFFF_0000) | v;
        cpu.set_logic_flags(v, Size::Word);
    } else {
        let v = cpu.d[r] as u16 as i16 as i32 as u32;
        cpu.d[r] = v;
        cpu.set_logic_flags(v, Size::Long);
    }
    cpu.prefetch();
    Ok(())
}
