//! Arithmetic instructions: ADD/SUB families, compares, negation, CLR/TST,
//! multiply/divide, BCD and CHK.

use crate::cpu::addressing::{address_step, EffectiveAddress};
use crate::cpu::decoder::{AddressingMode, Size};
use crate::cpu::{flags, vectors, Cpu, Exec};
use crate::memory::MemoryInterface;

/// True when the operand comes from a register or the instruction stream
/// (no data bus cycle), which some long operations pay for with extra
/// internal cycles.
fn is_reg_or_imm(ea: EffectiveAddress) -> bool {
    !matches!(ea, EffectiveAddress::Memory(_))
}

/// Set X, N, Z, V and C from an add/subtract result.
fn set_arith_flags(cpu: &mut Cpu, result: u32, carry: bool, overflow: bool, size: Size) {
    cpu.update_nz_flags(result, size);
    cpu.set_flag(flags::CARRY, carry);
    cpu.set_flag(flags::EXTEND, carry);
    cpu.set_flag(flags::OVERFLOW, overflow);
}

fn add_or_sub(cpu: &Cpu, dst: u32, src: u32, size: Size, sub: bool) -> (u32, bool, bool) {
    if sub {
        cpu.sub_with_flags(dst, src, size)
    } else {
        cpu.add_with_flags(src, dst, size)
    }
}

/// Read-modify-write of a data-alterable destination: read, prefetch, then
/// write back. Register destinations spend `reg_long_idle` extra clocks for
/// long operands.
pub(crate) fn rmw<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    memory: &mut M,
    reg_long_idle: u32,
    op: impl FnOnce(&mut Cpu, u32) -> u32,
) -> Exec {
    let ea = cpu.calc_ea(dst, size, memory);
    let val = cpu.read_operand(ea, size, memory)?;
    let result = op(cpu, val);
    cpu.prefetch();
    if let EffectiveAddress::Memory(_) = ea {
        cpu.write_operand(ea, size, result, memory)
    } else {
        if size == Size::Long {
            cpu.idle(reg_long_idle);
        }
        cpu.write_operand(ea, size, result, memory)
    }
}

/// ADD/SUB in both directions (`<ea>,Dn` and `Dn,<ea>`).
pub fn exec_add_sub<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    dst: AddressingMode,
    sub: bool,
    memory: &mut M,
) -> Exec {
    if let AddressingMode::DataRegister(dn) = dst {
        let ea = cpu.calc_ea(src, size, memory);
        let s = cpu.read_operand(ea, size, memory)?;
        cpu.prefetch();
        if size == Size::Long {
            cpu.idle(if is_reg_or_imm(ea) { 4 } else { 2 });
        }
        let d = cpu.d[dn as usize];
        let (res, c, v) = add_or_sub(cpu, d, s, size, sub);
        cpu.d[dn as usize] = size.apply(d, res);
        set_arith_flags(cpu, res, c, v, size);
        Ok(())
    } else {
        let AddressingMode::DataRegister(sreg) = src else {
            return Ok(());
        };
        let s = cpu.d[sreg as usize];
        rmw(cpu, size, dst, memory, 4, |cpu, d| {
            let (res, c, v) = add_or_sub(cpu, d, s, size, sub);
            set_arith_flags(cpu, res, c, v, size);
            res
        })
    }
}

/// ADDA/SUBA: whole-register result, flags untouched.
pub fn exec_adda_suba<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    dst_reg: u8,
    sub: bool,
    memory: &mut M,
) -> Exec {
    let ea = cpu.calc_ea(src, size, memory);
    let raw = cpu.read_operand(ea, size, memory)?;
    let s = if size == Size::Word {
        raw as u16 as i16 as i32 as u32
    } else {
        raw
    };
    cpu.prefetch();
    cpu.idle(if size == Size::Word || is_reg_or_imm(ea) {
        4
    } else {
        2
    });
    let a = &mut cpu.a[dst_reg as usize];
    *a = if sub {
        a.wrapping_sub(s)
    } else {
        a.wrapping_add(s)
    };
    Ok(())
}

/// ADDI/SUBI.
pub fn exec_addi_subi<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    sub: bool,
    memory: &mut M,
) -> Exec {
    let imm = cpu.fetch_immediate(size, memory);
    rmw(cpu, size, dst, memory, 4, |cpu, d| {
        let (res, c, v) = add_or_sub(cpu, d, imm, size, sub);
        set_arith_flags(cpu, res, c, v, size);
        res
    })
}

/// ADDQ/SUBQ. An address register destination is a flag-free long add.
pub fn exec_addq_subq<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    data: u8,
    dst: AddressingMode,
    sub: bool,
    memory: &mut M,
) -> Exec {
    let q = if data == 0 { 8 } else { data as u32 };
    if let AddressingMode::AddressRegister(r) = dst {
        cpu.prefetch();
        cpu.idle(if size == Size::Long { 2 } else { 4 });
        let a = &mut cpu.a[r as usize];
        *a = if sub {
            a.wrapping_sub(q)
        } else {
            a.wrapping_add(q)
        };
        return Ok(());
    }
    rmw(cpu, size, dst, memory, 4, |cpu, d| {
        let (res, c, v) = add_or_sub(cpu, d, q, size, sub);
        set_arith_flags(cpu, res, c, v, size);
        res
    })
}

/// ADDX/SUBX (register or `-(Ay),-(Ax)` forms). Z is only ever cleared.
pub fn exec_addx_subx<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src_reg: u8,
    dst_reg: u8,
    memory_mode: bool,
    sub: bool,
    memory: &mut M,
) -> Exec {
    let x = cpu.get_flag(flags::EXTEND) as u32;
    let compute = |cpu: &mut Cpu, s: u32, d: u32| -> u32 {
        let (res, c, v) = if sub {
            cpu.subx_with_flags(d, s, x, size)
        } else {
            cpu.addx_with_flags(s, d, x, size)
        };
        if res != 0 {
            cpu.set_flag(flags::ZERO, false);
        }
        cpu.set_flag(flags::NEGATIVE, size.is_negative(res));
        cpu.set_flag(flags::CARRY, c);
        cpu.set_flag(flags::EXTEND, c);
        cpu.set_flag(flags::OVERFLOW, v);
        res
    };
    if memory_mode {
        let (s, d, daddr) = fetch_predec_pair(cpu, src_reg, dst_reg, size, memory)?;
        let res = compute(cpu, s, d);
        if size == Size::Long {
            /* The low word is written, then the queue refilled, then the
            high word. */
            cpu.write(memory, daddr.wrapping_add(2), Size::Word, res & 0xFFFF)?;
            cpu.prefetch();
            cpu.write(memory, daddr, Size::Word, res >> 16)
        } else {
            cpu.prefetch();
            cpu.write(memory, daddr, size, res)
        }
    } else {
        let s = cpu.d[src_reg as usize];
        let d = cpu.d[dst_reg as usize];
        let res = compute(cpu, s, d);
        cpu.prefetch();
        if size == Size::Long {
            cpu.idle(4);
        }
        cpu.d[dst_reg as usize] = size.apply(d, res);
        Ok(())
    }
}

/// Read the `-(Ay)` source and `-(Ax)` destination operands of ADDX, SUBX,
/// ABCD and SBCD. Returns (source, destination, destination address).
fn fetch_predec_pair<M: MemoryInterface>(
    cpu: &mut Cpu,
    src_reg: u8,
    dst_reg: u8,
    size: Size,
    memory: &mut M,
) -> Exec<(u32, u32, u32)> {
    cpu.idle(2);
    let s = read_predec(cpu, src_reg, size, memory)?;
    let d = read_predec(cpu, dst_reg, size, memory)?;
    Ok((s, d, cpu.a[dst_reg as usize]))
}

/// `-(An)` operand read for the X/BCD forms. Long operands are read low
/// word first, each word predecrementing An by 2.
fn read_predec<M: MemoryInterface>(
    cpu: &mut Cpu,
    reg: u8,
    size: Size,
    memory: &mut M,
) -> Exec<u32> {
    let r = reg as usize;
    if size == Size::Long {
        let lo_addr = cpu.a[r].wrapping_sub(2);
        cpu.a[r] = lo_addr;
        let lo = cpu.read(memory, lo_addr, Size::Word)?;
        let hi_addr = lo_addr.wrapping_sub(2);
        cpu.a[r] = hi_addr;
        let hi = cpu.read(memory, hi_addr, Size::Word)?;
        Ok((hi << 16) | lo)
    } else {
        let addr = cpu.a[r].wrapping_sub(address_step(reg, size));
        cpu.a[r] = addr;
        cpu.read(memory, addr, size)
    }
}

fn set_cmp_flags(cpu: &mut Cpu, dst: u32, src: u32, size: Size) {
    let (res, c, v) = cpu.sub_with_flags(dst, src, size);
    cpu.update_nz_flags(res, size);
    cpu.set_flag(flags::CARRY, c);
    cpu.set_flag(flags::OVERFLOW, v);
}

/// CMP <ea>,Dn.
pub fn exec_cmp<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> Exec {
    let ea = cpu.calc_ea(src, size, memory);
    let s = cpu.read_operand(ea, size, memory)?;
    cpu.prefetch();
    if size == Size::Long {
        cpu.idle(2);
    }
    let d = cpu.d[dst_reg as usize];
    set_cmp_flags(cpu, d, s, size);
    Ok(())
}

/// CMPA <ea>,An (word sources are sign-extended; compare is always long).
pub fn exec_cmpa<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> Exec {
    let ea = cpu.calc_ea(src, size, memory);
    let raw = cpu.read_operand(ea, size, memory)?;
    let s = if size == Size::Word {
        raw as u16 as i16 as i32 as u32
    } else {
        raw
    };
    cpu.prefetch();
    cpu.idle(2);
    let d = cpu.a[dst_reg as usize];
    set_cmp_flags(cpu, d, s, Size::Long);
    Ok(())
}

/// CMPI #imm,<ea>.
pub fn exec_cmpi<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    memory: &mut M,
) -> Exec {
    let imm = cpu.fetch_immediate(size, memory);
    let ea = cpu.calc_ea(dst, size, memory);
    let d = cpu.read_operand(ea, size, memory)?;
    cpu.prefetch();
    if size == Size::Long && is_reg_or_imm(ea) {
        cpu.idle(2);
    }
    set_cmp_flags(cpu, d, imm, size);
    Ok(())
}

/// CMPM (Ay)+,(Ax)+.
pub fn exec_cmpm<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    ax: u8,
    ay: u8,
    memory: &mut M,
) -> Exec {
    let saddr = cpu.a[ay as usize];
    cpu.a[ay as usize] = saddr.wrapping_add(address_step(ay, size));
    let s = cpu.read(memory, saddr, size)?;
    let daddr = cpu.a[ax as usize];
    cpu.a[ax as usize] = daddr.wrapping_add(address_step(ax, size));
    let d = cpu.read(memory, daddr, size)?;
    cpu.prefetch();
    set_cmp_flags(cpu, d, s, size);
    Ok(())
}

/// NEG <ea>.
pub fn exec_neg<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    memory: &mut M,
) -> Exec {
    rmw(cpu, size, dst, memory, 2, |cpu, v| {
        let (res, c, o) = cpu.sub_with_flags(0, v, size);
        set_arith_flags(cpu, res, c, o, size);
        res
    })
}

/// NEGX <ea> (Z is only ever cleared).
pub fn exec_negx<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    memory: &mut M,
) -> Exec {
    rmw(cpu, size, dst, memory, 2, |cpu, v| {
        let x = cpu.get_flag(flags::EXTEND) as u32;
        let (res, c, o) = cpu.subx_with_flags(0, v, x, size);
        if res != 0 {
            cpu.set_flag(flags::ZERO, false);
        }
        cpu.set_flag(flags::NEGATIVE, size.is_negative(res));
        cpu.set_flag(flags::CARRY, c);
        cpu.set_flag(flags::EXTEND, c);
        cpu.set_flag(flags::OVERFLOW, o);
        res
    })
}

/// CLR <ea>. The 68000 reads a memory destination before clearing it.
pub fn exec_clr<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    memory: &mut M,
) -> Exec {
    rmw(cpu, size, dst, memory, 2, |cpu, _| {
        cpu.sr = (cpu.sr & !(flags::NEGATIVE | flags::OVERFLOW | flags::CARRY)) | flags::ZERO;
        0
    })
}

/// TST <ea>.
pub fn exec_tst<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    memory: &mut M,
) -> Exec {
    let ea = cpu.calc_ea(dst, size, memory);
    let v = cpu.read_operand(ea, size, memory)?;
    cpu.prefetch();
    cpu.set_logic_flags(v, size);
    Ok(())
}

/// Internal clocks MULU spends after its final prefetch: 38 + 2n total
/// where n is the number of set bits in the source.
pub fn mulu_cycles(src: u16) -> u32 {
    38 + 2 * src.count_ones()
}

/// MULS: 38 + 2n where n counts the 01/10 bit transitions in `src << 1`.
pub fn muls_cycles(src: u16) -> u32 {
    let v = (src as u32) << 1;
    let transitions = (v ^ (v >> 1)) & 0xFFFF;
    38 + 2 * transitions.count_ones()
}

fn mul_common<M: MemoryInterface>(
    cpu: &mut Cpu,
    src: AddressingMode,
    dst_reg: u8,
    signed: bool,
    memory: &mut M,
) -> Exec {
    let ea = cpu.calc_ea(src, Size::Word, memory);
    let s = cpu.read_operand(ea, Size::Word, memory)? as u16;
    cpu.prefetch();
    let d = cpu.d[dst_reg as usize] as u16;
    let (result, cycles) = if signed {
        (
            ((s as i16 as i32) * (d as i16 as i32)) as u32,
            muls_cycles(s),
        )
    } else {
        ((s as u32) * (d as u32), mulu_cycles(s))
    };
    /* 4 of the base clocks are the prefetch already counted. */
    cpu.idle(cycles - 4);
    cpu.d[dst_reg as usize] = result;
    cpu.set_logic_flags(result, Size::Long);
    Ok(())
}

pub fn exec_mulu<M: MemoryInterface>(
    cpu: &mut Cpu,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> Exec {
    mul_common(cpu, src, dst_reg, false, memory)
}

pub fn exec_muls<M: MemoryInterface>(
    cpu: &mut Cpu,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> Exec {
    mul_common(cpu, src, dst_reg, true, memory)
}

/// DIVU execution time (excluding the effective address), following the
/// hardware's shift-and-subtract microcode.
pub fn divu_cycles(dividend: u32, divisor: u16) -> u32 {
    if (dividend >> 16) >= divisor as u32 {
        return 10;
    }
    let mut mcycles: u32 = 38;
    let hdivisor = (divisor as u32) << 16;
    let mut dividend = dividend;
    for _ in 0..15 {
        let temp = dividend;
        dividend <<= 1;
        if (temp as i32) < 0 {
            dividend = dividend.wrapping_sub(hdivisor);
        } else {
            mcycles += 2;
            if dividend >= hdivisor {
                dividend = dividend.wrapping_sub(hdivisor);
                mcycles -= 1;
            }
        }
    }
    mcycles * 2
}

/// DIVS execution time (excluding the effective address).
pub fn divs_cycles(dividend: i32, divisor: i16) -> u32 {
    let mut mcycles: u32 = 6;
    if dividend < 0 {
        mcycles += 1;
    }
    let adividend = dividend.unsigned_abs();
    let adivisor = divisor.unsigned_abs() as u32;
    /* Early overflow exit: |dividend| / 2^15 >= |divisor| (SingleStepTests
    timing). */
    if (adividend >> 15) >= adivisor {
        return (mcycles + 2) * 2;
    }
    let mut aquot = adividend / adivisor;
    mcycles += 55;
    if divisor >= 0 {
        if dividend >= 0 {
            mcycles -= 1;
        } else {
            mcycles += 1;
        }
    }
    for _ in 0..15 {
        if (aquot as i16) >= 0 {
            mcycles += 1;
        }
        aquot <<= 1;
    }
    mcycles * 2
}

/// Divide-by-zero trap shared by DIVU and DIVS.
fn divide_by_zero<M: MemoryInterface>(cpu: &mut Cpu, memory: &mut M) -> Exec {
    cpu.set_flag(flags::CARRY, false);
    cpu.idle(8);
    let pc = cpu.pc;
    cpu.exception(memory, vectors::ZERO_DIVIDE, pc)
}

pub fn exec_divu<M: MemoryInterface>(
    cpu: &mut Cpu,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> Exec {
    let ea = cpu.calc_ea(src, Size::Word, memory);
    let divisor = cpu.read_operand(ea, Size::Word, memory)? as u16;
    let dividend = cpu.d[dst_reg as usize];
    if divisor == 0 {
        return divide_by_zero(cpu, memory);
    }
    cpu.idle(divu_cycles(dividend, divisor) - 4);
    cpu.prefetch();
    let quotient = dividend / divisor as u32;
    let remainder = dividend % divisor as u32;
    cpu.set_flag(flags::CARRY, false);
    if quotient > 0xFFFF {
        /* Overflow: the register, N and Z are unchanged. */
        cpu.set_flag(flags::OVERFLOW, true);
    } else {
        cpu.d[dst_reg as usize] = (remainder << 16) | quotient;
        cpu.set_flag(flags::OVERFLOW, false);
        cpu.update_nz_flags(quotient, Size::Word);
    }
    Ok(())
}

pub fn exec_divs<M: MemoryInterface>(
    cpu: &mut Cpu,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> Exec {
    let ea = cpu.calc_ea(src, Size::Word, memory);
    let divisor = cpu.read_operand(ea, Size::Word, memory)? as u16 as i16;
    let dividend = cpu.d[dst_reg as usize] as i32;
    if divisor == 0 {
        return divide_by_zero(cpu, memory);
    }
    cpu.idle(divs_cycles(dividend, divisor) - 4);
    cpu.prefetch();
    cpu.set_flag(flags::CARRY, false);
    let quotient = (dividend as i64) / (divisor as i64);
    let remainder = (dividend as i64) % (divisor as i64);
    if !(-32768..=32767).contains(&quotient) {
        /* Overflow: the register, N and Z are unchanged. */
        cpu.set_flag(flags::OVERFLOW, true);
    } else {
        cpu.d[dst_reg as usize] = ((remainder as u32) << 16) | (quotient as u32 & 0xFFFF);
        cpu.set_flag(flags::OVERFLOW, false);
        cpu.update_nz_flags(quotient as u32, Size::Word);
    }
    Ok(())
}

/// ABCD core: returns the byte result and updates X, N, Z (cleared only),
/// V and C the way the hardware's decimal adjust does.
pub fn bcd_add(cpu: &mut Cpu, src: u8, dst: u8) -> u8 {
    let x = cpu.get_flag(flags::EXTEND) as u16;
    let (s, d) = (src as u16, dst as u16);
    let lo = (s & 0x0F) + (d & 0x0F) + x;
    let uncorrected = (s & 0xF0) + (d & 0xF0) + lo;
    let mut result = uncorrected;
    if lo > 9 {
        result += 6;
    }
    let carry = (result & 0x3F0) > 0x90;
    if carry {
        result += 0x60;
    }
    finish_bcd(
        cpu,
        result as u8,
        carry,
        (uncorrected & 0x80) == 0 && (result & 0x80) != 0,
    )
}

/// SBCD core (`dst - src - X`); also used by NBCD with `dst = 0`.
pub fn bcd_sub(cpu: &mut Cpu, src: u8, dst: u8) -> u8 {
    let x = cpu.get_flag(flags::EXTEND) as u16;
    let (s, d) = (src as u16, dst as u16);
    let lo = (d & 0x0F).wrapping_sub(s & 0x0F).wrapping_sub(x);
    let hi = (d & 0xF0).wrapping_sub(s & 0xF0);
    let uncorrected = hi.wrapping_add(lo);
    let mut result = uncorrected;
    let mut adjust = 0u16;
    if lo & 0xF0 != 0 {
        adjust = 6;
        result = result.wrapping_sub(6);
    }
    if d.wrapping_sub(s).wrapping_sub(x) & 0x100 != 0 {
        result = result.wrapping_sub(0x60);
    }
    let carry = d.wrapping_sub(s).wrapping_sub(adjust).wrapping_sub(x) & 0x300 > 0xFF;
    finish_bcd(
        cpu,
        result as u8,
        carry,
        (uncorrected & 0x80) != 0 && (result & 0x80) == 0,
    )
}

fn finish_bcd(cpu: &mut Cpu, result: u8, carry: bool, overflow: bool) -> u8 {
    if result != 0 {
        cpu.set_flag(flags::ZERO, false);
    }
    cpu.set_flag(flags::NEGATIVE, result & 0x80 != 0);
    cpu.set_flag(flags::CARRY, carry);
    cpu.set_flag(flags::EXTEND, carry);
    cpu.set_flag(flags::OVERFLOW, overflow);
    result
}

fn exec_bcd_pair<M: MemoryInterface>(
    cpu: &mut Cpu,
    src_reg: u8,
    dst_reg: u8,
    memory_mode: bool,
    memory: &mut M,
    op: fn(&mut Cpu, u8, u8) -> u8,
) -> Exec {
    if memory_mode {
        let (s, d, daddr) = fetch_predec_pair(cpu, src_reg, dst_reg, Size::Byte, memory)?;
        let res = op(cpu, s as u8, d as u8);
        cpu.prefetch();
        cpu.write(memory, daddr, Size::Byte, res as u32)
    } else {
        let s = cpu.d[src_reg as usize] as u8;
        let d = cpu.d[dst_reg as usize] as u8;
        let res = op(cpu, s, d);
        cpu.prefetch();
        cpu.idle(2);
        cpu.d[dst_reg as usize] = Size::Byte.apply(cpu.d[dst_reg as usize], res as u32);
        Ok(())
    }
}

pub fn exec_abcd<M: MemoryInterface>(
    cpu: &mut Cpu,
    src_reg: u8,
    dst_reg: u8,
    memory_mode: bool,
    memory: &mut M,
) -> Exec {
    exec_bcd_pair(cpu, src_reg, dst_reg, memory_mode, memory, bcd_add)
}

pub fn exec_sbcd<M: MemoryInterface>(
    cpu: &mut Cpu,
    src_reg: u8,
    dst_reg: u8,
    memory_mode: bool,
    memory: &mut M,
) -> Exec {
    exec_bcd_pair(cpu, src_reg, dst_reg, memory_mode, memory, bcd_sub)
}

/// NBCD <ea>.
pub fn exec_nbcd<M: MemoryInterface>(cpu: &mut Cpu, dst: AddressingMode, memory: &mut M) -> Exec {
    let ea = cpu.calc_ea(dst, Size::Byte, memory);
    let v = cpu.read_operand(ea, Size::Byte, memory)? as u8;
    let res = bcd_sub(cpu, v, 0);
    cpu.prefetch();
    if !matches!(ea, EffectiveAddress::Memory(_)) {
        cpu.idle(2);
    }
    cpu.write_operand(ea, Size::Byte, res as u32, memory)
}

/// CHK <ea>,Dn: trap (vector 6) when Dn < 0 or Dn > bound.
pub fn exec_chk<M: MemoryInterface>(
    cpu: &mut Cpu,
    src: AddressingMode,
    dst_reg: u8,
    memory: &mut M,
) -> Exec {
    let ea = cpu.calc_ea(src, Size::Word, memory);
    let bound = cpu.read_operand(ea, Size::Word, memory)? as u16 as i16;
    cpu.prefetch();
    let val = cpu.d[dst_reg as usize] as u16 as i16;
    /* V and C are cleared, Z reflects Dn; N is only changed when trapping:
    the upper-bound test comes first and leaves N = sign of Dn. */
    cpu.sr &= !(flags::OVERFLOW | flags::CARRY | flags::ZERO);
    if val == 0 {
        cpu.sr |= flags::ZERO;
    }
    if val > bound {
        cpu.set_flag(flags::NEGATIVE, val < 0);
        cpu.idle(4);
        let pc = cpu.pc;
        cpu.exception(memory, vectors::CHK, pc)
    } else if val < 0 {
        cpu.set_flag(flags::NEGATIVE, true);
        cpu.idle(6);
        let pc = cpu.pc;
        cpu.exception(memory, vectors::CHK, pc)
    } else {
        cpu.idle(6);
        Ok(())
    }
}
