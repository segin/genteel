use crate::memory::{IoInterface, MemoryInterface};
use crate::z80::op_cb::CbOps;
use crate::z80::op_ed::EdOps;
use crate::z80::op_index::IndexOps;
use crate::z80::{flags, Z80};

pub trait GeneralOps {
    fn execute_x0(&mut self, opcode: u8, y: u8, z: u8, p: u8, q: u8) -> u8;
    fn execute_x1(&mut self, y: u8, z: u8) -> u8;
    fn execute_x2(&mut self, y: u8, z: u8) -> u8;
    fn execute_x3(&mut self, opcode: u8, y: u8, z: u8, p: u8, q: u8) -> u8;
}

impl<M: MemoryInterface, I: IoInterface> GeneralOps for Z80<M, I> {
    fn execute_x0(&mut self, _opcode: u8, y: u8, z: u8, _p: u8, _q: u8) -> u8 {
        dispatch_z!(
            z,
            execute_x0_control_misc(self, y),
            execute_x0_load_add_hl(self, y),
            execute_x0_load_indirect(self, y),
            execute_x0_inc_dec_rp(self, y),
            execute_x0_inc_r(self, y),
            execute_x0_dec_r(self, y),
            execute_x0_ld_r_n(self, y),
            execute_x0_rotate_accum_flags(self, y)
        )
    }

    fn execute_x1(&mut self, y: u8, z: u8) -> u8 {
        if y == 6 && z == 6 {
            // HALT
            self.halted = true;
            4
        } else {
            // LD r, r'
            let val = self.get_reg(z);
            self.set_reg(y, val);
            if y == 6 || z == 6 {
                7
            } else {
                4
            }
        }
    }

    fn execute_x2(&mut self, y: u8, z: u8) -> u8 {
        // ALU operations
        let val = self.get_reg(z);
        match y {
            0 => self.add_a(val, false),        // ADD A, r
            1 => self.add_a(val, true),         // ADC A, r
            2 => self.sub_a(val, false, true),  // SUB r
            3 => self.sub_a(val, true, true),   // SBC A, r
            4 => self.and_a(val),               // AND r
            5 => self.xor_a(val),               // XOR r
            6 => self.or_a(val),                // OR r
            7 => self.sub_a(val, false, false), // CP r
            _ => {}
        }
        if z == 6 {
            7
        } else {
            4
        }
    }

    fn execute_x3(&mut self, _opcode: u8, y: u8, z: u8, _p: u8, _q: u8) -> u8 {
        dispatch_z!(
            z,
            execute_x3_ret_cc(self, y),
            execute_x3_pop_ret_exx(self, y),
            execute_x3_jp_cc(self, y),
            execute_x3_jp_out_ex_di_ei(self, y),
            execute_x3_call_cc(self, y),
            execute_x3_push_call_prefixes(self, y),
            execute_x3_alu_n(self, y),
            execute_x3_rst(self, y)
        )
    }
}

fn execute_x0_control_misc<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, y: u8) -> u8 {
    match y {
        0 => 4, // NOP
        1 => {
            // EX AF, AF'
            std::mem::swap(&mut cpu.a, &mut cpu.a_prime);
            std::mem::swap(&mut cpu.f, &mut cpu.f_prime);
            4
        }
        2 => {
            // DJNZ d
            let d = cpu.fetch_byte() as i8;
            cpu.b = cpu.b.wrapping_sub(1);
            if cpu.b != 0 {
                cpu.pc = (cpu.pc as i32 + d as i32) as u16;
                13
            } else {
                8
            }
        }
        3 => {
            // JR d
            let d = cpu.fetch_byte() as i8;
            cpu.pc = (cpu.pc as i32 + d as i32) as u16;
            12
        }
        4..=7 => {
            // JR cc, d
            let d = cpu.fetch_byte() as i8;
            if cpu.check_condition(y - 4) {
                cpu.pc = (cpu.pc as i32 + d as i32) as u16;
                12
            } else {
                7
            }
        }
        _ => 4,
    }
}

fn execute_x0_load_add_hl<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, y: u8) -> u8 {
    let p = (y >> 1) & 0x03;
    let q = y & 0x01;
    if q == 0 {
        // LD rp, nn
        let nn = cpu.fetch_word();
        cpu.set_rp(p, nn);
        10
    } else {
        // ADD HL, rp
        let rp = cpu.get_rp(p);
        cpu.add_hl(rp);
        11
    }
}

fn execute_x0_load_indirect<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, y: u8) -> u8 {
    let p = (y >> 1) & 0x03;
    let q = y & 0x01;
    match (p, q) {
        (0, 0) => {
            // LD (BC), A
            let addr = cpu.bc();
            cpu.write_byte(addr, cpu.a);
            cpu.memptr = ((cpu.a as u16) << 8) | (addr.wrapping_add(1) & 0xFF);
            7
        }
        (0, 1) => {
            // LD A, (BC)
            let addr = cpu.bc();
            cpu.a = cpu.read_byte(addr);
            cpu.memptr = addr.wrapping_add(1);
            7
        }
        (1, 0) => {
            // LD (DE), A
            let addr = cpu.de();
            cpu.write_byte(addr, cpu.a);
            cpu.memptr = ((cpu.a as u16) << 8) | (addr.wrapping_add(1) & 0xFF);
            7
        }
        (1, 1) => {
            // LD A, (DE)
            let addr = cpu.de();
            cpu.a = cpu.read_byte(addr);
            cpu.memptr = addr.wrapping_add(1);
            7
        }
        (2, 0) => {
            // LD (nn), HL
            let addr = cpu.fetch_word();
            cpu.write_word(addr, cpu.hl());
            cpu.memptr = addr.wrapping_add(1);
            16
        }
        (2, 1) => {
            // LD HL, (nn)
            let addr = cpu.fetch_word();
            let val = cpu.read_word(addr);
            cpu.set_hl(val);
            cpu.memptr = addr.wrapping_add(1);
            16
        }
        (3, 0) => {
            // LD (nn), A
            let addr = cpu.fetch_word();
            cpu.write_byte(addr, cpu.a);
            cpu.memptr = ((cpu.a as u16) << 8) | (addr.wrapping_add(1) & 0xFF);
            13
        }
        (3, 1) => {
            // LD A, (nn)
            let addr = cpu.fetch_word();
            cpu.a = cpu.read_byte(addr);
            cpu.memptr = addr.wrapping_add(1);
            13
        }
        _ => 4,
    }
}

fn execute_x0_inc_dec_rp<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, y: u8) -> u8 {
    let p = (y >> 1) & 0x03;
    let q = y & 0x01;
    // INC/DEC rp
    let rp = cpu.get_rp(p);
    if q == 0 {
        cpu.set_rp(p, rp.wrapping_add(1));
    } else {
        cpu.set_rp(p, rp.wrapping_sub(1));
    }
    6
}

fn execute_x0_op_r<M, I, F>(cpu: &mut Z80<M, I>, y: u8, op: F) -> u8
where
    M: MemoryInterface,
    I: IoInterface,
    F: FnOnce(&mut Z80<M, I>, u8) -> u8,
{
    let val = cpu.get_reg(y);
    let result = op(cpu, val);
    cpu.set_reg(y, result);
    if y == 6 {
        11
    } else {
        4
    }
}

fn execute_x0_inc_r<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, y: u8) -> u8 {
    // INC r
    execute_x0_op_r(cpu, y, |c, v| c.inc(v))
}

fn execute_x0_dec_r<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, y: u8) -> u8 {
    // DEC r
    execute_x0_op_r(cpu, y, |c, v| c.dec(v))
}

fn execute_x0_ld_r_n<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, y: u8) -> u8 {
    // LD r, n
    let n = cpu.fetch_byte();
    cpu.set_reg(y, n);
    if y == 6 {
        10
    } else {
        7
    }
}

fn execute_x0_rotate_accum_flags<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    y: u8,
) -> u8 {
    match y {
        0 => {
            cpu.rlca();
            4
        }
        1 => {
            cpu.rrca();
            4
        }
        2 => {
            cpu.rla();
            4
        }
        3 => {
            cpu.rra();
            4
        }
        4 => {
            // DAA - Decimal Adjust Accumulator
            // DAA adjusts A to valid BCD based on N, H, C flags
            let mut correction: u8 = 0;
            let mut carry = cpu.get_flag(flags::CARRY);

            if cpu.get_flag(flags::ADD_SUB) {
                // After subtraction
                if cpu.get_flag(flags::HALF_CARRY) {
                    correction |= 0x06;
                }
                if carry {
                    correction |= 0x60;
                }
                cpu.a = cpu.a.wrapping_sub(correction);
            } else {
                // After addition
                if cpu.get_flag(flags::HALF_CARRY) || (cpu.a & 0x0F) > 9 {
                    correction |= 0x06;
                }
                if carry || cpu.a > 0x99 {
                    correction |= 0x60;
                    carry = true;
                }
                cpu.a = cpu.a.wrapping_add(correction);
            }

            cpu.set_flag(flags::CARRY, carry);
            cpu.set_flag(flags::HALF_CARRY, (correction & 0x06) != 0);
            cpu.set_sz_flags(cpu.a);
            cpu.set_parity_flag(cpu.a);
            4
        }
        5 => {
            // CPL
            cpu.a = !cpu.a;
            cpu.set_flag(flags::HALF_CARRY, true);
            cpu.set_flag(flags::ADD_SUB, true);
            cpu.set_flag(flags::X_FLAG, (cpu.a & 0x08) != 0);
            cpu.set_flag(flags::Y_FLAG, (cpu.a & 0x20) != 0);
            4
        }
        6 => {
            // SCF
            cpu.set_flag(flags::CARRY, true);
            cpu.set_flag(flags::HALF_CARRY, false);
            cpu.set_flag(flags::ADD_SUB, false);
            cpu.set_flag(flags::X_FLAG, (cpu.a & 0x08) != 0);
            cpu.set_flag(flags::Y_FLAG, (cpu.a & 0x20) != 0);
            4
        }
        7 => {
            // CCF
            let c = cpu.get_flag(flags::CARRY);
            cpu.set_flag(flags::HALF_CARRY, c);
            cpu.set_flag(flags::CARRY, !c);
            cpu.set_flag(flags::ADD_SUB, false);
            cpu.set_flag(flags::X_FLAG, (cpu.a & 0x08) != 0);
            cpu.set_flag(flags::Y_FLAG, (cpu.a & 0x20) != 0);
            4
        }
        _ => 4,
    }
}

fn execute_x3_ret_cc<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, y: u8) -> u8 {
    // RET cc
    if cpu.check_condition(y) {
        cpu.pc = cpu.pop();
        11
    } else {
        5
    }
}

fn execute_x3_pop_ret_exx<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, y: u8) -> u8 {
    let p = (y >> 1) & 0x03;
    let q = y & 0x01;
    if q == 0 {
        // POP rp2
        let val = cpu.pop();
        cpu.set_rp2(p, val);
        10
    } else {
        match p {
            0 => {
                // RET
                cpu.pc = cpu.pop();
                10
            }
            1 => {
                // EXX
                std::mem::swap(&mut cpu.b, &mut cpu.b_prime);
                std::mem::swap(&mut cpu.c, &mut cpu.c_prime);
                std::mem::swap(&mut cpu.d, &mut cpu.d_prime);
                std::mem::swap(&mut cpu.e, &mut cpu.e_prime);
                std::mem::swap(&mut cpu.h, &mut cpu.h_prime);
                std::mem::swap(&mut cpu.l, &mut cpu.l_prime);
                4
            }
            2 => {
                // JP HL
                cpu.pc = cpu.hl();
                4
            }
            3 => {
                // LD SP, HL
                cpu.sp = cpu.hl();
                6
            }
            _ => 4,
        }
    }
}

fn execute_x3_jp_cc<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, y: u8) -> u8 {
    // JP cc, nn
    let nn = cpu.fetch_word();
    if cpu.check_condition(y) {
        cpu.pc = nn;
    }
    10
}

fn execute_x3_jp_out_ex_di_ei<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    y: u8,
) -> u8 {
    match y {
        0 => {
            // JP nn
            cpu.pc = cpu.fetch_word();
            10
        }
        1 => cpu.execute_cb_prefix(),
        2 => {
            // OUT (n), A
            let n = cpu.fetch_byte();
            let port = (n as u16) | ((cpu.a as u16) << 8);
            cpu.write_port(port, cpu.a);
            11
        }
        3 => {
            // IN A, (n)
            let n = cpu.fetch_byte();
            let port = (n as u16) | ((cpu.a as u16) << 8);
            cpu.a = cpu.read_port(port);
            11
        }
        4 => {
            // EX (SP), HL
            let val = cpu.read_word(cpu.sp);
            cpu.memptr = val;
            cpu.write_word(cpu.sp, cpu.hl());
            cpu.memptr = val;
            cpu.set_hl(val);
            cpu.memptr = val;
            19
        }
        5 => {
            // EX DE, HL
            let de = cpu.de();
            let hl = cpu.hl();
            cpu.set_de(hl);
            cpu.set_hl(de);
            4
        }
        6 => {
            // DI
            cpu.iff1 = false;
            cpu.iff2 = false;
            4
        }
        7 => {
            // EI
            cpu.iff1 = true;
            cpu.iff2 = true;
            cpu.pending_ei = true;
            4
        }
        _ => 4,
    }
}

fn execute_x3_call_cc<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, y: u8) -> u8 {
    // CALL cc, nn
    let nn = cpu.fetch_word();
    if cpu.check_condition(y) {
        cpu.push(cpu.pc);
        cpu.pc = nn;
        17
    } else {
        10
    }
}

fn execute_x3_push_call_prefixes<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    y: u8,
) -> u8 {
    let p = (y >> 1) & 0x03;
    let q = y & 0x01;
    if q == 0 {
        // PUSH rp2
        let val = cpu.get_rp2(p);
        cpu.push(val);
        11
    } else {
        match p {
            0 => {
                // CALL nn
                let nn = cpu.fetch_word();
                cpu.push(cpu.pc);
                cpu.pc = nn;
                17
            }
            1 => cpu.execute_dd_prefix(),
            2 => cpu.execute_ed_prefix(),
            3 => cpu.execute_fd_prefix(),
            _ => 4,
        }
    }
}

fn execute_x3_alu_n<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, y: u8) -> u8 {
    // ALU A, n
    let n = cpu.fetch_byte();
    match y {
        0 => cpu.add_a(n, false),
        1 => cpu.add_a(n, true),
        2 => cpu.sub_a(n, false, true),
        3 => cpu.sub_a(n, true, true),
        4 => cpu.and_a(n),
        5 => cpu.xor_a(n),
        6 => cpu.or_a(n),
        7 => cpu.sub_a(n, false, false),
        _ => {}
    }
    7
}

fn execute_x3_rst<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, y: u8) -> u8 {
    // RST y*8
    cpu.push(cpu.pc);
    cpu.pc = (y as u16) * 8;
    11
}
