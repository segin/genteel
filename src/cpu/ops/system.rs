//! Program control and system instructions: branches, jumps, returns,
//! traps, LINK/UNLK, SR/CCR/USP moves, STOP and RESET.

use super::bits::LogicOp;
use crate::cpu::addressing::EffectiveAddress;
use crate::cpu::decoder::{AddressingMode, Condition, Size};
use crate::cpu::{flags, vectors, Cpu, Exec};
use crate::memory::MemoryInterface;

/// Bcc/BRA. An 8-bit displacement of 0 means a 16-bit displacement word
/// follows. The base for the displacement is the opcode address + 2.
pub fn exec_bcc<M: MemoryInterface>(
    cpu: &mut Cpu,
    condition: Condition,
    displacement: i16,
    memory: &mut M,
) -> Exec {
    let base = cpu.pc;
    if cpu.check_condition(condition) {
        let disp = if displacement == 0 {
            cpu.ext_word_deferred(memory) as i16
        } else {
            displacement
        };
        cpu.idle(2);
        cpu.jump_and_fill(base.wrapping_add(disp as i32 as u32))
    } else {
        cpu.idle(4);
        if displacement == 0 {
            cpu.ext_word(memory);
        }
        cpu.prefetch();
        Ok(())
    }
}

/// BSR: push the return address, then branch.
pub fn exec_bsr<M: MemoryInterface>(cpu: &mut Cpu, displacement: i16, memory: &mut M) -> Exec {
    let base = cpu.pc;
    let disp = if displacement == 0 {
        cpu.ext_word_deferred(memory) as i16
    } else {
        displacement
    };
    cpu.idle(2);
    let ret = cpu.pc;
    cpu.push_long(memory, ret)?;
    cpu.jump_and_fill(base.wrapping_add(disp as i32 as u32))
}

/// DBcc: if the condition is false, decrement Dn.w and branch unless it
/// became -1.
pub fn exec_dbcc<M: MemoryInterface>(
    cpu: &mut Cpu,
    condition: Condition,
    reg: u8,
    memory: &mut M,
) -> Exec {
    let base = cpu.pc;
    let disp = cpu.ext_word_deferred(memory) as i16;
    if cpu.check_condition(condition) {
        cpu.idle(4);
        cpu.prefetch();
        cpu.prefetch();
        return Ok(());
    }
    let r = reg as usize;
    let counter = (cpu.d[r] as u16).wrapping_sub(1);
    cpu.d[r] = (cpu.d[r] & 0xFFFF_0000) | counter as u32;
    cpu.idle(2);
    if counter == 0xFFFF {
        /* Loop expired: the branch-target fetch is discarded. */
        cpu.prefetch();
        cpu.prefetch();
        cpu.prefetch();
        Ok(())
    } else {
        cpu.jump_and_fill(base.wrapping_add(disp as i32 as u32))
    }
}

/// Extra internal clocks JMP/JSR spend computing a control address.
fn jump_ea_idle(mode: AddressingMode) -> u32 {
    match mode {
        AddressingMode::AddressDisplacement(_)
        | AddressingMode::PcDisplacement
        | AddressingMode::AbsoluteShort => 2,
        AddressingMode::AddressIndex(_) | AddressingMode::PcIndex => 6,
        _ => 0,
    }
}

pub fn exec_jmp<M: MemoryInterface>(cpu: &mut Cpu, dst: AddressingMode, memory: &mut M) -> Exec {
    let target = cpu.control_address(dst, memory);
    cpu.idle(jump_ea_idle(dst));
    cpu.jump_and_fill(target)
}

pub fn exec_jsr<M: MemoryInterface>(cpu: &mut Cpu, dst: AddressingMode, memory: &mut M) -> Exec {
    let target = cpu.control_address(dst, memory);
    cpu.idle(jump_ea_idle(dst));
    let ret = cpu.pc;
    cpu.jump_to(target)?;
    cpu.prefetch();
    cpu.push_long(memory, ret)?;
    cpu.prefetch();
    Ok(())
}

pub fn exec_rts<M: MemoryInterface>(cpu: &mut Cpu, memory: &mut M) -> Exec {
    let target = cpu.pop_long(memory)?;
    cpu.jump_and_fill(target)
}

pub fn exec_rtr<M: MemoryInterface>(cpu: &mut Cpu, memory: &mut M) -> Exec {
    let ccr = cpu.pop_word(memory)?;
    let target = cpu.pop_long(memory)?;
    cpu.set_ccr(ccr);
    cpu.jump_and_fill(target)
}

pub fn exec_rte<M: MemoryInterface>(cpu: &mut Cpu, memory: &mut M) -> Exec {
    if !cpu.require_supervisor(memory)? {
        return Ok(());
    }
    let sr = cpu.pop_word(memory)?;
    let target = cpu.pop_long(memory)?;
    cpu.set_sr(sr);
    cpu.jump_and_fill(target)
}

pub fn exec_trap<M: MemoryInterface>(cpu: &mut Cpu, vector: u8, memory: &mut M) -> Exec {
    cpu.idle(4);
    let pc = cpu.pc;
    cpu.exception(memory, vectors::TRAP_BASE + vector as u32, pc)
}

pub fn exec_trapv<M: MemoryInterface>(cpu: &mut Cpu, memory: &mut M) -> Exec {
    cpu.prefetch();
    if cpu.get_flag(flags::OVERFLOW) {
        let pc = cpu.pc;
        cpu.exception(memory, vectors::TRAPV, pc)
    } else {
        Ok(())
    }
}

/// LINK An,#d16: push An, An = SP, SP += d16. With A7 the pushed value is
/// the already decremented stack pointer.
pub fn exec_link<M: MemoryInterface>(cpu: &mut Cpu, reg: u8, memory: &mut M) -> Exec {
    let disp = cpu.ext_word(memory) as i16 as i32 as u32;
    let r = reg as usize;
    let sp = cpu.a[7].wrapping_sub(4);
    let val = if r == 7 { sp } else { cpu.a[r] };
    cpu.write(memory, sp, Size::Long, val)?;
    cpu.a[7] = sp;
    cpu.a[r] = sp;
    cpu.a[7] = cpu.a[7].wrapping_add(disp);
    cpu.prefetch();
    Ok(())
}

/// UNLK An: SP = An, then pop An.
pub fn exec_unlk<M: MemoryInterface>(cpu: &mut Cpu, reg: u8, memory: &mut M) -> Exec {
    let r = reg as usize;
    let addr = cpu.a[r];
    let val = cpu.read(memory, addr, Size::Long)?;
    cpu.a[7] = addr.wrapping_add(4);
    cpu.a[r] = val;
    cpu.prefetch();
    Ok(())
}

pub fn exec_move_to_sr<M: MemoryInterface>(
    cpu: &mut Cpu,
    src: AddressingMode,
    memory: &mut M,
) -> Exec {
    if !cpu.require_supervisor(memory)? {
        return Ok(());
    }
    let ea = cpu.calc_ea(src, Size::Word, memory);
    let val = cpu.read_operand(ea, Size::Word, memory)? as u16;
    cpu.idle(4);
    cpu.set_sr(val);
    cpu.prefetch();
    cpu.prefetch();
    Ok(())
}

pub fn exec_move_to_ccr<M: MemoryInterface>(
    cpu: &mut Cpu,
    src: AddressingMode,
    memory: &mut M,
) -> Exec {
    let ea = cpu.calc_ea(src, Size::Word, memory);
    let val = cpu.read_operand(ea, Size::Word, memory)? as u16;
    cpu.idle(4);
    cpu.set_ccr(val);
    cpu.prefetch();
    cpu.prefetch();
    Ok(())
}

/// MOVE from SR (not privileged on the 68000). A memory destination is read
/// before it is written.
pub fn exec_move_from_sr<M: MemoryInterface>(
    cpu: &mut Cpu,
    dst: AddressingMode,
    memory: &mut M,
) -> Exec {
    let ea = cpu.calc_ea(dst, Size::Word, memory);
    let sr = cpu.sr as u32;
    if let EffectiveAddress::Memory(addr) = ea {
        cpu.read(memory, addr, Size::Word)?;
        cpu.prefetch();
        cpu.write(memory, addr, Size::Word, sr)
    } else {
        cpu.prefetch();
        cpu.idle(2);
        cpu.write_operand(ea, Size::Word, sr, memory)
    }
}

/// ANDI/ORI/EORI to CCR (`to_sr` false) or SR (`to_sr` true, privileged).
pub fn exec_logic_to_sr<M: MemoryInterface>(
    cpu: &mut Cpu,
    op: LogicOp,
    to_sr: bool,
    memory: &mut M,
) -> Exec {
    if to_sr && !cpu.require_supervisor(memory)? {
        return Ok(());
    }
    let imm = cpu.ext_word(memory);
    cpu.idle(8);
    if to_sr {
        let v = op.apply(cpu.sr as u32, imm as u32) as u16;
        cpu.set_sr(v);
    } else {
        let v = op.apply(cpu.sr as u32 & 0xFF, imm as u32 & 0xFF) as u16;
        cpu.set_ccr(v);
    }
    cpu.prefetch();
    cpu.prefetch();
    Ok(())
}

pub fn exec_move_usp<M: MemoryInterface>(
    cpu: &mut Cpu,
    reg: u8,
    to_usp: bool,
    memory: &mut M,
) -> Exec {
    if !cpu.require_supervisor(memory)? {
        return Ok(());
    }
    if to_usp {
        cpu.usp = cpu.a[reg as usize];
    } else {
        cpu.a[reg as usize] = cpu.usp;
    }
    cpu.prefetch();
    Ok(())
}

/// STOP #imm: load SR and wait for an interrupt.
pub fn exec_stop<M: MemoryInterface>(cpu: &mut Cpu, memory: &mut M) -> Exec {
    if !cpu.require_supervisor(memory)? {
        return Ok(());
    }
    let imm = cpu.ext_word_deferred(memory);
    cpu.set_sr(imm);
    cpu.prefetch();
    cpu.stopped = true;
    Ok(())
}

/// RESET asserts the external reset line for 124 clocks; no CPU state
/// changes.
pub fn exec_reset<M: MemoryInterface>(cpu: &mut Cpu, memory: &mut M) -> Exec {
    if !cpu.require_supervisor(memory)? {
        return Ok(());
    }
    cpu.idle(128);
    cpu.prefetch();
    Ok(())
}

/// Scc <ea>: set the byte to all ones if the condition holds, else zero. A
/// memory destination is read first.
pub fn exec_scc<M: MemoryInterface>(
    cpu: &mut Cpu,
    condition: Condition,
    dst: AddressingMode,
    memory: &mut M,
) -> Exec {
    let ea = cpu.calc_ea(dst, Size::Byte, memory);
    let cond = cpu.check_condition(condition);
    let val = if cond { 0xFF } else { 0 };
    if let EffectiveAddress::Memory(addr) = ea {
        cpu.read(memory, addr, Size::Byte)?;
        cpu.prefetch();
        cpu.write(memory, addr, Size::Byte, val)
    } else {
        cpu.prefetch();
        if cond {
            cpu.idle(2);
        }
        cpu.write_operand(ea, Size::Byte, val, memory)
    }
}
