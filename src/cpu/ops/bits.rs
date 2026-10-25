//! Logical, shift/rotate, bit-manipulation and TAS instructions.

use super::arithmetic::rmw;
use crate::cpu::addressing::EffectiveAddress;
use crate::cpu::decoder::{AddressingMode, BitSource, ShiftCount, Size};
use crate::cpu::{flags, Cpu, Exec};
use crate::memory::MemoryInterface;

/// Binary logic operation shared by AND/OR/EOR and their immediate and
/// CCR/SR forms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicOp {
    And,
    Or,
    Eor,
}

impl LogicOp {
    #[inline]
    pub fn apply(self, a: u32, b: u32) -> u32 {
        match self {
            LogicOp::And => a & b,
            LogicOp::Or => a | b,
            LogicOp::Eor => a ^ b,
        }
    }
}

/// AND/OR/EOR between a data register and `<ea>` in either direction.
pub fn exec_logic<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    src: AddressingMode,
    dst: AddressingMode,
    op: LogicOp,
    memory: &mut M,
) -> Exec {
    match (src, dst) {
        (_, AddressingMode::DataRegister(dn)) if op != LogicOp::Eor => {
            let ea = cpu.calc_ea(src, size, memory);
            let s = cpu.read_operand(ea, size, memory)?;
            cpu.prefetch();
            if size == Size::Long {
                cpu.idle(if matches!(ea, EffectiveAddress::Memory(_)) {
                    2
                } else {
                    4
                });
            }
            let d = cpu.d[dn as usize];
            let res = op.apply(d, s) & size.mask();
            cpu.d[dn as usize] = size.apply(d, res);
            cpu.set_logic_flags(res, size);
            Ok(())
        }
        (AddressingMode::DataRegister(sreg), _) => {
            let s = cpu.d[sreg as usize];
            rmw(cpu, size, dst, memory, 4, |cpu, d| {
                let res = op.apply(d, s) & size.mask();
                cpu.set_logic_flags(res, size);
                res
            })
        }
        _ => Ok(()),
    }
}

/// ANDI/ORI/EORI #imm,<ea>.
pub fn exec_logic_imm<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    op: LogicOp,
    memory: &mut M,
) -> Exec {
    let imm = cpu.fetch_immediate(size, memory);
    rmw(cpu, size, dst, memory, 4, |cpu, d| {
        let res = op.apply(d, imm) & size.mask();
        cpu.set_logic_flags(res, size);
        res
    })
}

/// NOT <ea>.
pub fn exec_not<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    memory: &mut M,
) -> Exec {
    rmw(cpu, size, dst, memory, 2, |cpu, v| {
        let res = !v & size.mask();
        cpu.set_logic_flags(res, size);
        res
    })
}

/// The eight shift/rotate operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShiftOp {
    Asl,
    Asr,
    Lsl,
    Lsr,
    Rol,
    Ror,
    Roxl,
    Roxr,
}

/// Result of a shift or rotate: value, C, V and the new X (None leaves X
/// unchanged).
pub struct ShiftResult {
    pub value: u32,
    pub carry: bool,
    pub overflow: bool,
    pub extend: Option<bool>,
}

/// Compute a shift/rotate of `val` (masked to `size`) by `count` with the
/// incoming X flag `x`.
pub fn shift_value(op: ShiftOp, size: Size, val: u32, count: u32, x: bool) -> ShiftResult {
    let bits = size.bits();
    let mask = size.mask();
    let msb = size.sign_bit();
    let val = val & mask;
    if count == 0 {
        /* No shift: C is cleared (or copies X for ROXL/ROXR), X unchanged. */
        let carry = matches!(op, ShiftOp::Roxl | ShiftOp::Roxr) && x;
        return ShiftResult {
            value: val,
            carry,
            overflow: false,
            extend: None,
        };
    }
    match op {
        ShiftOp::Asl | ShiftOp::Lsl => {
            let (value, carry) = if count > bits {
                (0, false)
            } else if count == bits {
                (0, val & 1 != 0)
            } else {
                ((val << count) & mask, (val >> (bits - count)) & 1 != 0)
            };
            /* ASL sets V if the sign bit changed at any point. */
            let overflow = op == ShiftOp::Asl && {
                if count >= bits {
                    val != 0
                } else {
                    let check = mask & (mask << (bits - count - 1));
                    let top = val & check;
                    top != 0 && top != check
                }
            };
            ShiftResult {
                value,
                carry,
                overflow,
                extend: Some(carry),
            }
        }
        ShiftOp::Asr | ShiftOp::Lsr => {
            let negative = op == ShiftOp::Asr && val & msb != 0;
            let (value, carry) = if count >= bits {
                /* The last bit shifted out is the sign bit when the count
                equals the size; beyond that C (and X) end up clear. */
                let value = if negative { mask } else { 0 };
                (value, count == bits && val & msb != 0)
            } else {
                let mut v = val >> count;
                if negative {
                    v |= mask & !(mask >> count);
                }
                (v, (val >> (count - 1)) & 1 != 0)
            };
            ShiftResult {
                value,
                carry,
                overflow: false,
                extend: Some(carry),
            }
        }
        ShiftOp::Rol | ShiftOp::Ror => {
            let n = count % bits;
            let value = if n == 0 {
                val
            } else if op == ShiftOp::Rol {
                ((val << n) | (val >> (bits - n))) & mask
            } else {
                ((val >> n) | (val << (bits - n))) & mask
            };
            let carry = if op == ShiftOp::Rol {
                value & 1 != 0
            } else {
                value & msb != 0
            };
            ShiftResult {
                value,
                carry,
                overflow: false,
                extend: None,
            }
        }
        ShiftOp::Roxl | ShiftOp::Roxr => {
            /* A (bits + 1)-bit rotate through X. */
            let n = count % (bits + 1);
            let mut v = val;
            let mut xb = x;
            for _ in 0..n {
                if op == ShiftOp::Roxl {
                    let out = v & msb != 0;
                    v = ((v << 1) | xb as u32) & mask;
                    xb = out;
                } else {
                    let out = v & 1 != 0;
                    v = (v >> 1) | if xb { msb } else { 0 };
                    xb = out;
                }
            }
            ShiftResult {
                value: v,
                carry: xb,
                overflow: false,
                extend: Some(xb),
            }
        }
    }
}

fn apply_shift_flags(cpu: &mut Cpu, r: &ShiftResult, size: Size) {
    cpu.update_nz_flags(r.value, size);
    cpu.set_flag(flags::CARRY, r.carry);
    cpu.set_flag(flags::OVERFLOW, r.overflow);
    if let Some(x) = r.extend {
        cpu.set_flag(flags::EXTEND, x);
    }
}

/// Register shifts/rotates (`count` from immediate or Dn mod 64) and the
/// one-bit memory forms.
pub fn exec_shift<M: MemoryInterface>(
    cpu: &mut Cpu,
    size: Size,
    dst: AddressingMode,
    count: ShiftCount,
    op: ShiftOp,
    memory: &mut M,
) -> Exec {
    let x = cpu.get_flag(flags::EXTEND);
    if let AddressingMode::DataRegister(r) = dst {
        let n = match count {
            ShiftCount::Immediate(n) => n as u32,
            ShiftCount::Register(c) => cpu.d[c as usize] & 63,
        };
        let res = shift_value(op, size, cpu.d[r as usize], n, x);
        cpu.prefetch();
        cpu.idle(if size == Size::Long { 4 } else { 2 } + 2 * n);
        cpu.d[r as usize] = size.apply(cpu.d[r as usize], res.value);
        apply_shift_flags(cpu, &res, size);
        Ok(())
    } else {
        rmw(cpu, Size::Word, dst, memory, 0, |cpu, v| {
            let res = shift_value(op, Size::Word, v, 1, x);
            apply_shift_flags(cpu, &res, Size::Word);
            res.value
        })
    }
}

/// The four bit operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitOp {
    Test,
    Set,
    Clear,
    Change,
}

/// BTST/BSET/BCLR/BCHG with a static (#imm) or dynamic (Dn) bit number.
/// Register operands use bit number mod 32, memory operands mod 8.
pub fn exec_bit<M: MemoryInterface>(
    cpu: &mut Cpu,
    bit: BitSource,
    dst: AddressingMode,
    op: BitOp,
    memory: &mut M,
) -> Exec {
    let bit_num = match bit {
        BitSource::Immediate => (cpu.ext_word(memory) & 0xFF) as u32,
        BitSource::Register(r) => cpu.d[r as usize],
    };
    let modify = |v: u32, mask: u32| match op {
        BitOp::Test => v,
        BitOp::Set => v | mask,
        BitOp::Clear => v & !mask,
        BitOp::Change => v ^ mask,
    };
    if let AddressingMode::DataRegister(r) = dst {
        let n = bit_num & 31;
        let mask = 1u32 << n;
        let v = cpu.d[r as usize];
        cpu.set_flag(flags::ZERO, v & mask == 0);
        cpu.prefetch();
        cpu.idle(match op {
            BitOp::Test => 2,
            BitOp::Clear => {
                if n < 16 {
                    4
                } else {
                    6
                }
            }
            _ => {
                if n < 16 {
                    2
                } else {
                    4
                }
            }
        });
        cpu.d[r as usize] = modify(v, mask);
        return Ok(());
    }
    let ea = cpu.calc_ea(dst, Size::Byte, memory);
    let v = cpu.read_operand(ea, Size::Byte, memory)?;
    let mask = 1u32 << (bit_num & 7);
    cpu.set_flag(flags::ZERO, v & mask == 0);
    cpu.prefetch();
    if let EffectiveAddress::Immediate(_) = ea {
        cpu.idle(2);
    }
    if op != BitOp::Test {
        cpu.write_operand(ea, Size::Byte, modify(v, mask), memory)?;
    }
    Ok(())
}

/// TAS <ea>: test a byte and set its bit 7 using an indivisible
/// read-modify-write cycle. When the bus drops TAS write-back (Mega Drive), the
/// write half of the memory cycle has no effect.
pub fn exec_tas<M: MemoryInterface>(cpu: &mut Cpu, dst: AddressingMode, memory: &mut M) -> Exec {
    let ea = cpu.calc_ea(dst, Size::Byte, memory);
    let v = cpu.read_operand(ea, Size::Byte, memory)?;
    cpu.set_logic_flags(v, Size::Byte);
    match ea {
        EffectiveAddress::Memory(addr) => {
            cpu.idle(2);
            if memory.tas_write_back() {
                cpu.write(memory, addr, Size::Byte, v | 0x80)?;
            } else {
                cpu.idle(4);
            }
            cpu.prefetch();
            Ok(())
        }
        _ => {
            cpu.prefetch();
            cpu.write_operand(ea, Size::Byte, v | 0x80, memory)
        }
    }
}
