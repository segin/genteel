use crate::memory::{IoInterface, MemoryInterface};
use crate::z80::{flags, OpParams, Z80};

pub trait EdOps {
    fn execute_ed_prefix(&mut self) -> u8;
}

impl<M: MemoryInterface, I: IoInterface> EdOps for Z80<M, I> {
    fn execute_ed_prefix(&mut self) -> u8 {
        let opcode = self.fetch_opcode();
        let params = OpParams::decode(opcode);

        match params.x {
            1 => dispatch_z!(
                params.z,
                execute_ed_in_r_c(self, &params),
                execute_ed_out_c_r(self, &params),
                execute_ed_sbc_adc_hl(self, &params),
                execute_ed_ld_rp_nn(self, &params),
                execute_ed_neg(self),
                execute_ed_retn_reti(self, &params),
                execute_ed_im(self, &params),
                execute_ed_misc(self, &params)
            ),
            2 => execute_ed_block(self, &params),
            _ => 8, // NONI / NOP
        }
    }
}

fn execute_ed_in_r_c<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    params: &OpParams,
) -> u8 {
    // IN r, (C)
    let port = cpu.bc();
    let val = cpu.read_port(port);
    cpu.memptr = port.wrapping_add(1);
    if params.y != 6 {
        cpu.set_reg(params.y, val);
    }
    cpu.set_sz_flags(val);
    cpu.set_parity_flag(val);
    cpu.set_flag(flags::HALF_CARRY, false);
    cpu.set_flag(flags::ADD_SUB, false);
    12
}

fn execute_ed_out_c_r<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    params: &OpParams,
) -> u8 {
    // OUT (C), r
    let port = cpu.bc();
    let val = if params.y == 6 {
        0
    } else {
        cpu.get_reg(params.y)
    };
    cpu.write_port(port, val);
    cpu.memptr = port.wrapping_add(1);
    12
}

fn execute_ed_neg<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>) -> u8 {
    // NEG
    let a = cpu.a;
    cpu.a = 0;
    cpu.sub_a(a, false, true);
    8
}

fn execute_ed_retn_reti<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    params: &OpParams,
) -> u8 {
    /* RETN and RETI (and their mirrors) both copy IFF2 to IFF1. */
    let _ = params;
    cpu.iff1 = cpu.iff2;
    cpu.pc = cpu.pop();
    cpu.memptr = cpu.pc;
    14
}

fn execute_ed_im<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, params: &OpParams) -> u8 {
    // IM params.y
    cpu.im = match params.y & 0x03 {
        0 | 1 => 0,
        2 => 1,
        3 => 2,
        _ => 0,
    };
    8
}

fn execute_ed_block<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    params: &OpParams,
) -> u8 {
    // Block instructions
    if params.y >= 4 {
        dispatch_z!(
            params.z,
            execute_ldi_ldd(cpu, params),
            execute_cpi_cpd(cpu, params),
            execute_ini_ind(cpu, params),
            execute_outi_outd(cpu, params),
            8, // 4
            8, // 5
            8, // 6
            8  // 7
        )
    } else {
        8 // Invalid
    }
}

fn execute_ed_sbc_adc_hl<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    params: &OpParams,
) -> u8 {
    cpu.memptr = cpu.hl().wrapping_add(1);
    if params.q == 0 {
        // SBC HL, rp
        let hl = cpu.hl() as u32;
        let rp = cpu.get_rp(params.p) as u32;
        let c = if cpu.get_flag(flags::CARRY) { 1u32 } else { 0 };
        let result = hl.wrapping_sub(rp).wrapping_sub(c);

        cpu.set_flag(flags::CARRY, result > 0xFFFF);
        cpu.set_flag(flags::ADD_SUB, true);
        cpu.set_flag(flags::ZERO, (result & 0xFFFF) == 0);
        cpu.set_flag(flags::SIGN, (result & 0x8000) != 0);
        // Half borrow: (HL & 0xFFF) - (RP & 0xFFF) - C < 0
        let h_check = (hl & 0xFFF).wrapping_sub(rp & 0xFFF).wrapping_sub(c);
        cpu.set_flag(flags::HALF_CARRY, h_check > 0xFFF);
        // P/V: Overflow
        let overflow = ((hl ^ rp) & (hl ^ result) & 0x8000) != 0;
        cpu.set_flag(flags::PARITY, overflow);

        // X/Y from High Byte
        let h_res = (result >> 8) as u8;
        cpu.set_flag(flags::X_FLAG, (h_res & 0x08) != 0);
        cpu.set_flag(flags::Y_FLAG, (h_res & 0x20) != 0);

        cpu.set_hl(result as u16);
        15
    } else {
        // ADC HL, rp
        let hl = cpu.hl() as u32;
        let rp = cpu.get_rp(params.p) as u32;
        let c = if cpu.get_flag(flags::CARRY) { 1u32 } else { 0 };
        let result = hl + rp + c;

        cpu.set_flag(flags::CARRY, result > 0xFFFF);
        cpu.set_flag(flags::ADD_SUB, false);
        cpu.set_flag(flags::ZERO, (result & 0xFFFF) == 0);
        cpu.set_flag(flags::SIGN, (result & 0x8000) != 0);
        // Half carry: Carry from bit 11
        cpu.set_flag(flags::HALF_CARRY, ((hl & 0xFFF) + (rp & 0xFFF) + c) > 0xFFF);
        // P/V: Overflow
        let overflow = (!(hl ^ rp) & (hl ^ result) & 0x8000) != 0;
        cpu.set_flag(flags::PARITY, overflow);

        // X/Y from High Byte
        let h_res = (result >> 8) as u8;
        cpu.set_flag(flags::X_FLAG, (h_res & 0x08) != 0);
        cpu.set_flag(flags::Y_FLAG, (h_res & 0x20) != 0);

        cpu.set_hl(result as u16);
        15
    }
}

fn execute_ed_ld_rp_nn<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    params: &OpParams,
) -> u8 {
    let nn = cpu.fetch_word();
    if params.q == 0 {
        // LD (nn), rp
        cpu.write_word(nn, cpu.get_rp(params.p));
    } else {
        // LD rp, (nn)
        let val = cpu.read_word(nn);
        cpu.set_rp(params.p, val);
    }
    cpu.memptr = nn.wrapping_add(1);
    20
}

fn execute_ed_misc<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    params: &OpParams,
) -> u8 {
    match params.y {
        0 => {
            // LD I, A
            cpu.i = cpu.a;
            9
        }
        1 => {
            // LD R, A
            cpu.r = cpu.a;
            9
        }
        2 => {
            // LD A, I
            cpu.a = cpu.i;
            cpu.set_sz_flags(cpu.a);
            cpu.set_flag(flags::PARITY, cpu.iff2);
            cpu.set_flag(flags::HALF_CARRY, false);
            cpu.set_flag(flags::ADD_SUB, false);
            9
        }
        3 => {
            // LD A, R
            cpu.a = cpu.r;
            cpu.set_sz_flags(cpu.a);
            cpu.set_flag(flags::PARITY, cpu.iff2);
            cpu.set_flag(flags::HALF_CARRY, false);
            cpu.set_flag(flags::ADD_SUB, false);
            9
        }
        4 => {
            // RRD
            let hl = cpu.hl();
            cpu.memptr = hl.wrapping_add(1);
            let m = cpu.read_byte(hl);
            let new_m = (cpu.a << 4) | (m >> 4);
            cpu.a = (cpu.a & 0xF0) | (m & 0x0F);
            cpu.write_byte(hl, new_m);
            cpu.set_sz_flags(cpu.a);
            cpu.set_parity_flag(cpu.a);
            cpu.set_flag(flags::HALF_CARRY, false);
            cpu.set_flag(flags::ADD_SUB, false);
            18
        }
        5 => {
            // RLD
            let hl = cpu.hl();
            cpu.memptr = hl.wrapping_add(1);
            let m = cpu.read_byte(hl);
            let new_m = (m << 4) | (cpu.a & 0x0F);
            cpu.a = (cpu.a & 0xF0) | (m >> 4);
            cpu.write_byte(hl, new_m);
            cpu.set_sz_flags(cpu.a);
            cpu.set_parity_flag(cpu.a);
            cpu.set_flag(flags::HALF_CARRY, false);
            cpu.set_flag(flags::ADD_SUB, false);
            18
        }
        _ => 8,
    }
}

fn execute_ldi_ldd<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    params: &OpParams,
) -> u8 {
    let hl = cpu.hl();
    let de = cpu.de();
    let val = cpu.read_byte(hl);
    cpu.write_byte(de, val);

    let bc = cpu.bc().wrapping_sub(1);
    cpu.set_bc(bc);

    let (new_hl, new_de) = if (params.y & 1) == 0 {
        (hl.wrapping_add(1), de.wrapping_add(1)) // LDI
    } else {
        (hl.wrapping_sub(1), de.wrapping_sub(1)) // LDD
    };

    cpu.set_hl(new_hl);
    cpu.set_de(new_de);

    let n_val = val.wrapping_add(cpu.a);
    cpu.set_flag(flags::Y_FLAG, (n_val & 0x02) != 0);
    cpu.set_flag(flags::X_FLAG, (n_val & 0x08) != 0);
    cpu.set_flag(flags::PARITY, bc != 0);
    cpu.set_flag(flags::HALF_CARRY, false);
    cpu.set_flag(flags::ADD_SUB, false);

    // LDIR/LDDR
    if params.y >= 6 && bc != 0 {
        cpu.pc = cpu.pc.wrapping_sub(2);
        cpu.memptr = cpu.pc.wrapping_add(1);
        block_repeat_xy(cpu);
        21
    } else {
        16
    }
}

fn execute_cpi_cpd<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    params: &OpParams,
) -> u8 {
    let hl = cpu.hl();
    let val = cpu.read_byte(hl);
    let result = cpu.a.wrapping_sub(val);

    let bc = cpu.bc().wrapping_sub(1);
    cpu.set_bc(bc);

    let new_hl = if (params.y & 1) == 0 {
        hl.wrapping_add(1) // CPI
    } else {
        hl.wrapping_sub(1) // CPD
    };

    cpu.set_hl(new_hl);

    let h = (cpu.a & 0x0F) < (val & 0x0F);
    cpu.set_flag(flags::ZERO, result == 0);
    cpu.set_flag(flags::SIGN, (result & 0x80) != 0);
    cpu.set_flag(flags::HALF_CARRY, h);
    cpu.set_flag(flags::PARITY, bc != 0);
    cpu.set_flag(flags::ADD_SUB, true);

    // CPI/CPD X/Y flags: based on A - val - H
    let mut x_val = cpu.a.wrapping_sub(val);
    if h {
        x_val = x_val.wrapping_sub(1);
    }
    cpu.set_flag(flags::Y_FLAG, (x_val & 0x02) != 0);
    cpu.set_flag(flags::X_FLAG, (x_val & 0x08) != 0);

    cpu.memptr = if (params.y & 1) == 0 {
        cpu.memptr.wrapping_add(1)
    } else {
        cpu.memptr.wrapping_sub(1)
    };

    // CPIR/CPDR
    if params.y >= 6 && bc != 0 && result != 0 {
        cpu.pc = cpu.pc.wrapping_sub(2);
        cpu.memptr = cpu.pc.wrapping_add(1);
        block_repeat_xy(cpu);
        21
    } else {
        16
    }
}

/// While a block instruction repeats, X/Y are taken from bits 11 and 13 of
/// the rewound PC (the instruction address).
fn block_repeat_xy<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>) {
    let pch = (cpu.pc >> 8) as u8;
    let f = (cpu.f & !(flags::X_FLAG | flags::Y_FLAG)) | (pch & (flags::X_FLAG | flags::Y_FLAG));
    cpu.set_f(f);
}

fn parity_even(v: u8) -> bool {
    v.count_ones().is_multiple_of(2)
}

/// Flags for INI/IND/OUTI/OUTD and, when `repeat` is set, the extra
/// adjustments made by the repeat cycle of INIR/INDR/OTIR/OTDR.
///
/// `val` is the byte transferred, `k` the 8-bit addend (C±1 for input,
/// the updated L for output); `cpu.b` already holds the decremented B.
fn block_io_flags<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    val: u8,
    k: u8,
    repeat: bool,
) {
    let b = cpu.b;
    let sum = val as u16 + k as u16;
    let carry = sum > 0xFF;

    let mut f = b & (flags::SIGN | flags::Y_FLAG | flags::X_FLAG);
    if b == 0 {
        f |= flags::ZERO;
    }
    if val & 0x80 != 0 {
        f |= flags::ADD_SUB;
    }
    if carry {
        f |= flags::HALF_CARRY | flags::CARRY;
    }
    let mut pv = parity_even(((sum as u8) & 7) ^ b);

    if repeat {
        let pch = (cpu.pc >> 8) as u8;
        f = (f & !(flags::X_FLAG | flags::Y_FLAG)) | (pch & (flags::X_FLAG | flags::Y_FLAG));
        if carry {
            f &= !flags::HALF_CARRY;
            if val & 0x80 != 0 {
                pv ^= !parity_even(b.wrapping_sub(1) & 7);
                if b & 0x0F == 0x00 {
                    f |= flags::HALF_CARRY;
                }
            } else {
                pv ^= !parity_even(b.wrapping_add(1) & 7);
                if b & 0x0F == 0x0F {
                    f |= flags::HALF_CARRY;
                }
            }
        } else {
            pv ^= !parity_even(b & 7);
        }
    }
    if pv {
        f |= flags::PARITY;
    }
    cpu.set_f(f);
}

fn execute_ini_ind<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    params: &OpParams,
) -> u8 {
    // INI (params.y=4), IND (params.y=5), INIR (params.y=6), INDR (params.y=7)

    let port = cpu.bc();
    let hl = cpu.hl();
    let dec = (params.y & 1) != 0;

    let io_val = cpu.read_port(port);
    cpu.write_byte(hl, io_val);
    cpu.memptr = if dec {
        port.wrapping_sub(1)
    } else {
        port.wrapping_add(1)
    };

    cpu.b = cpu.b.wrapping_sub(1);
    cpu.set_hl(if dec {
        hl.wrapping_sub(1)
    } else {
        hl.wrapping_add(1)
    });

    let k = if dec {
        cpu.c.wrapping_sub(1)
    } else {
        cpu.c.wrapping_add(1)
    };

    // Repeat logic for INIR/INDR (params.y>=6)
    if params.y >= 6 && cpu.b != 0 {
        cpu.pc = cpu.pc.wrapping_sub(2);
        cpu.memptr = cpu.pc.wrapping_add(1);
        block_io_flags(cpu, io_val, k, true);
        21
    } else {
        block_io_flags(cpu, io_val, k, false);
        16
    }
}

fn execute_outi_outd<M: MemoryInterface, I: IoInterface>(
    cpu: &mut Z80<M, I>,
    params: &OpParams,
) -> u8 {
    // OUTI (params.y=4), OUTD (params.y=5), OTIR (params.y=6), OTDR (params.y=7)

    let hl = cpu.hl();
    let dec = (params.y & 1) != 0;
    let val = cpu.read_byte(hl);

    /* B is decremented before it is placed on the upper address lines. */
    cpu.b = cpu.b.wrapping_sub(1);
    let port = cpu.bc();
    cpu.write_port(port, val);
    cpu.memptr = if dec {
        port.wrapping_sub(1)
    } else {
        port.wrapping_add(1)
    };

    cpu.set_hl(if dec {
        hl.wrapping_sub(1)
    } else {
        hl.wrapping_add(1)
    });
    let k = cpu.l;

    if params.y >= 6 && cpu.b != 0 {
        cpu.pc = cpu.pc.wrapping_sub(2);
        cpu.memptr = cpu.pc.wrapping_add(1);
        block_io_flags(cpu, val, k, true);
        21
    } else {
        block_io_flags(cpu, val, k, false);
        16
    }
}
