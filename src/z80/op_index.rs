use crate::memory::{IoInterface, MemoryInterface};
use crate::z80::op_cb::CbOps;
use crate::z80::{flags, Z80};

pub trait IndexOps {
    fn execute_dd_prefix(&mut self) -> u8;
    fn execute_fd_prefix(&mut self) -> u8;
}

impl<M: MemoryInterface, I: IoInterface> IndexOps for Z80<M, I> {
    fn execute_dd_prefix(&mut self) -> u8 {
        execute_index_prefix(self, true)
    }

    fn execute_fd_prefix(&mut self) -> u8 {
        execute_index_prefix(self, false)
    }
}

fn get_index_val<M: MemoryInterface, I: IoInterface>(cpu: &Z80<M, I>, is_ix: bool) -> u16 {
    if is_ix {
        cpu.ix
    } else {
        cpu.iy
    }
}

fn set_index_val<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, val: u16, is_ix: bool) {
    if is_ix {
        cpu.ix = val;
    } else {
        cpu.iy = val;
    }
}

fn get_index_h<M: MemoryInterface, I: IoInterface>(cpu: &Z80<M, I>, is_ix: bool) -> u8 {
    if is_ix {
        cpu.ixh()
    } else {
        cpu.iyh()
    }
}

fn set_index_h<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, val: u8, is_ix: bool) {
    if is_ix {
        cpu.set_ixh(val);
    } else {
        cpu.set_iyh(val);
    }
}

fn get_index_l<M: MemoryInterface, I: IoInterface>(cpu: &Z80<M, I>, is_ix: bool) -> u8 {
    if is_ix {
        cpu.ixl()
    } else {
        cpu.iyl()
    }
}

fn set_index_l<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, val: u8, is_ix: bool) {
    if is_ix {
        cpu.set_ixl(val);
    } else {
        cpu.set_iyl(val);
    }
}

fn add_index<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, value: u16, is_ix: bool) {
    let idx = if is_ix { cpu.ix } else { cpu.iy } as u32;
    let v = value as u32;
    let result = idx + v;

    cpu.set_flag(flags::CARRY, result > 0xFFFF);
    cpu.set_flag(flags::HALF_CARRY, ((idx & 0x0FFF) + (v & 0x0FFF)) > 0x0FFF);
    cpu.set_flag(flags::ADD_SUB, false);

    cpu.memptr = idx.wrapping_add(1) as u16;
    if is_ix {
        cpu.ix = result as u16;
    } else {
        cpu.iy = result as u16;
    }
}

fn calc_index_addr<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    offset: i8,
    is_ix: bool,
) -> u16 {
    let idx = get_index_val(cpu, is_ix);
    let addr = (idx as i16 + offset as i16) as u16;
    cpu.memptr = addr;
    addr
}

fn execute_index_alu<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    op_index: u8,
    val: u8,
) {
    match op_index {
        0 => cpu.add_a(val, false),
        1 => cpu.add_a(val, true),
        2 => cpu.sub_a(val, false, true),
        3 => cpu.sub_a(val, true, true),
        4 => cpu.and_a(val),
        5 => cpu.xor_a(val),
        6 => cpu.or_a(val),
        7 => cpu.sub_a(val, false, false),
        _ => {}
    }
}

fn execute_index_add_16<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    opcode: u8,
    is_ix: bool,
) -> u8 {
    match opcode {
        0x09 => {
            let val = cpu.bc();
            add_index(cpu, val, is_ix);
            15
        }
        0x19 => {
            let val = cpu.de();
            add_index(cpu, val, is_ix);
            15
        }
        0x29 => {
            let val = get_index_val(cpu, is_ix);
            add_index(cpu, val, is_ix);
            15
        }
        0x39 => {
            add_index(cpu, cpu.sp, is_ix);
            15
        }
        _ => 8,
    }
}

fn execute_index_load_store_16<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    opcode: u8,
    is_ix: bool,
) -> u8 {
    match opcode {
        0x21 => {
            let val = cpu.fetch_word();
            set_index_val(cpu, val, is_ix);
            14
        }
        0x22 => {
            let addr = cpu.fetch_word();
            let val = get_index_val(cpu, is_ix);
            cpu.write_word(addr, val);
            20
        }
        0x2A => {
            let addr = cpu.fetch_word();
            let val = cpu.read_word(addr);
            set_index_val(cpu, val, is_ix);
            20
        }
        _ => 8,
    }
}

fn execute_index_inc_dec_16<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    opcode: u8,
    is_ix: bool,
) -> u8 {
    match opcode {
        0x23 => {
            let val = get_index_val(cpu, is_ix);
            set_index_val(cpu, val.wrapping_add(1), is_ix);
            10
        }
        0x2B => {
            let val = get_index_val(cpu, is_ix);
            set_index_val(cpu, val.wrapping_sub(1), is_ix);
            10
        }
        _ => 8,
    }
}

fn execute_index_8bit_halves<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    opcode: u8,
    is_ix: bool,
) -> u8 {
    match opcode {
        0x24 => {
            let val = get_index_h(cpu, is_ix);
            let res = cpu.inc(val);
            set_index_h(cpu, res, is_ix);
            8
        }
        0x25 => {
            let val = get_index_h(cpu, is_ix);
            let res = cpu.dec(val);
            set_index_h(cpu, res, is_ix);
            8
        }
        0x26 => {
            let n = cpu.fetch_byte();
            set_index_h(cpu, n, is_ix);
            11
        }
        0x2C => {
            let val = get_index_l(cpu, is_ix);
            let res = cpu.inc(val);
            set_index_l(cpu, res, is_ix);
            8
        }
        0x2D => {
            let val = get_index_l(cpu, is_ix);
            let res = cpu.dec(val);
            set_index_l(cpu, res, is_ix);
            8
        }
        0x2E => {
            let n = cpu.fetch_byte();
            set_index_l(cpu, n, is_ix);
            11
        }
        _ => 8,
    }
}

fn execute_index_mem_8bit<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    opcode: u8,
    is_ix: bool,
) -> u8 {
    let d = cpu.fetch_byte() as i8;
    let addr = calc_index_addr(cpu, d, is_ix);
    match opcode {
        0x34 => {
            let val = cpu.read_byte(addr);
            let result = cpu.inc(val);
            cpu.write_byte(addr, result);
            23
        }
        0x35 => {
            let val = cpu.read_byte(addr);
            let result = cpu.dec(val);
            cpu.write_byte(addr, result);
            23
        }
        0x36 => {
            let n = cpu.fetch_byte();
            cpu.write_byte(addr, n);
            19
        }
        _ => 8,
    }
}

fn execute_index_alu_mem<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    opcode: u8,
    is_ix: bool,
) -> u8 {
    let d = cpu.fetch_byte() as i8;
    let addr = calc_index_addr(cpu, d, is_ix);
    let val = cpu.read_byte(addr);
    execute_index_alu(cpu, (opcode >> 3) & 0x07, val);
    19
}

fn execute_index_load_r_mem<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    opcode: u8,
    is_ix: bool,
) -> u8 {
    let d = cpu.fetch_byte() as i8;
    let addr = calc_index_addr(cpu, d, is_ix);
    let val = cpu.read_byte(addr);
    let r = (opcode >> 3) & 0x07;
    cpu.set_reg(r, val);
    19
}

fn execute_index_load_mem_r<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    opcode: u8,
    is_ix: bool,
) -> u8 {
    let d = cpu.fetch_byte() as i8;
    let addr = calc_index_addr(cpu, d, is_ix);
    let r = opcode & 0x07;
    let val = cpu.get_reg(r);
    cpu.write_byte(addr, val);
    19
}

fn execute_index_stack_control<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    opcode: u8,
    is_ix: bool,
) -> u8 {
    match opcode {
        0xE1 => {
            let val = cpu.pop();
            set_index_val(cpu, val, is_ix);
            14
        }
        0xE3 => {
            let val = cpu.read_word(cpu.sp);
            let idx = get_index_val(cpu, is_ix);
            cpu.memptr = val;
            cpu.write_word(cpu.sp, idx);
            cpu.memptr = val;
            set_index_val(cpu, val, is_ix);
            cpu.memptr = val;
            23
        }
        0xE5 => {
            let idx = get_index_val(cpu, is_ix);
            cpu.push(idx);
            15
        }
        0xE9 => {
            cpu.pc = get_index_val(cpu, is_ix);
            8
        }
        0xF9 => {
            cpu.sp = get_index_val(cpu, is_ix);
            10
        }
        _ => 8,
    }
}

fn execute_index_undoc_load<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    opcode: u8,
    is_ix: bool,
) -> u8 {
    // Opcode 0x76 (HALT) is handled by caller
    let r_src = opcode & 0x07;
    let r_dest = (opcode >> 3) & 0x07;
    let val = get_index_byte(cpu, r_src, is_ix);
    set_index_byte(cpu, r_dest, val, is_ix);
    8
}

fn execute_index_undoc_alu<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    opcode: u8,
    is_ix: bool,
) -> u8 {
    let val = get_index_byte(cpu, opcode & 0x07, is_ix);
    execute_index_alu(cpu, (opcode >> 3) & 0x07, val);
    8
}

fn execute_index_prefix<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    is_ix: bool,
) -> u8 {
    let opcode = cpu.fetch_byte();

    match opcode {
        0x09 | 0x19 | 0x29 | 0x39 => execute_index_add_16(cpu, opcode, is_ix),
        0x21 | 0x22 | 0x2A => execute_index_load_store_16(cpu, opcode, is_ix),
        0x23 | 0x2B => execute_index_inc_dec_16(cpu, opcode, is_ix),
        0x24 | 0x25 | 0x26 | 0x2C | 0x2D | 0x2E => execute_index_8bit_halves(cpu, opcode, is_ix),
        0x34..=0x36 => execute_index_mem_8bit(cpu, opcode, is_ix),

        // Specific ALU ops
        0x86 | 0x8E | 0x96 | 0x9E | 0xA6 | 0xAE | 0xB6 | 0xBE => {
            execute_index_alu_mem(cpu, opcode, is_ix)
        }

        // LD r, (IX/IY+d)
        0x46 | 0x4E | 0x56 | 0x5E | 0x66 | 0x6E | 0x7E => {
            execute_index_load_r_mem(cpu, opcode, is_ix)
        }
        // LD (IX/IY+d), r
        0x70..=0x75 | 0x77 => execute_index_load_mem_r(cpu, opcode, is_ix),

        0x76 => {
            cpu.halted = true;
            8
        }

        // Generic Undocumented (using index halves)
        // Note: 0x76 HALT is handled above, and specific LDs are also handled above.
        0x40..=0x7F => execute_index_undoc_load(cpu, opcode, is_ix),

        // Generic Undocumented ALU
        // Note: Specific ALU ops (IX+d) are handled above.
        0x80..=0xBF => execute_index_undoc_alu(cpu, opcode, is_ix),

        0xE1 | 0xE3 | 0xE5 | 0xE9 | 0xF9 => execute_index_stack_control(cpu, opcode, is_ix),

        0xCB => {
            let d = cpu.fetch_byte() as i8;
            let addr = calc_index_addr(cpu, d, is_ix);
            let opcode = cpu.fetch_byte();
            cpu.execute_indexed_cb(opcode, addr)
        }
        _ => 8, // Treat as NOP
    }
}

fn get_index_byte<M: MemoryInterface, I: IoInterface>(cpu: &Z80<M, I>, r: u8, is_ix: bool) -> u8 {
    match r {
        0 => cpu.b,
        1 => cpu.c,
        2 => cpu.d,
        3 => cpu.e,
        4 => {
            if is_ix {
                cpu.ixh()
            } else {
                cpu.iyh()
            }
        }
        5 => {
            if is_ix {
                cpu.ixl()
            } else {
                cpu.iyl()
            }
        }
        7 => cpu.a,
        _ => 0,
    }
}

fn set_index_byte<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    r: u8,
    val: u8,
    is_ix: bool,
) {
    match r {
        0 => cpu.b = val,
        1 => cpu.c = val,
        2 => cpu.d = val,
        3 => cpu.e = val,
        4 => {
            if is_ix {
                cpu.set_ixh(val)
            } else {
                cpu.set_iyh(val)
            }
        }
        5 => {
            if is_ix {
                cpu.set_ixl(val)
            } else {
                cpu.set_iyl(val)
            }
        }
        7 => cpu.a = val,
        _ => {}
    }
}
