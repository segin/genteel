use crate::memory::{IoInterface, MemoryInterface};
use crate::z80::{flags, Z80};

pub trait EdOps {
    fn execute_ed_prefix(&mut self) -> u8;
}

impl<M: MemoryInterface, I: IoInterface> EdOps for Z80<M, I> {
    fn execute_ed_prefix(&mut self) -> u8 {
        let opcode = self.fetch_byte();
        let x = (opcode >> 6) & 0x03;
        let y = (opcode >> 3) & 0x07;
        let z = opcode & 0x07;
        let p = (y >> 1) & 0x03;
        let q = y & 0x01;

        match x {
            1 => dispatch_z!(
                z,
                execute_ed_in_r_c(self, y),
                execute_ed_out_c_r(self, y),
                execute_ed_sbc_adc_hl(self, p, q),
                execute_ed_ld_rp_nn(self, p, q),
                execute_ed_neg(self),
                execute_ed_retn_reti(self, q),
                execute_ed_im(self, y),
                execute_ed_misc(self, y)
            ),
            2 => execute_ed_block(self, y, z),
            _ => 8, // NONI / NOP
        }
    }
}

fn execute_ed_in_r_c<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, y: u8) -> u8 {
    // IN r, (C)
    let port = cpu.bc();
    let val = cpu.read_port(port);
    if y != 6 {
        cpu.set_reg(y, val);
    }
    cpu.set_sz_flags(val);
    cpu.set_parity_flag(val);
    cpu.set_flag(flags::HALF_CARRY, false);
    cpu.set_flag(flags::ADD_SUB, false);
    12
}

fn execute_ed_out_c_r<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, y: u8) -> u8 {
    // OUT (C), r
    let port = cpu.bc();
    let val = if y == 6 { 0 } else { cpu.get_reg(y) };
    cpu.write_port(port, val);
    12
}

fn execute_ed_neg<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>) -> u8 {
    // NEG
    let a = cpu.a;
    cpu.a = 0;
    cpu.sub_a(a, false, true);
    8
}

fn execute_ed_retn_reti<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, q: u8) -> u8 {
    if q == 0 {
        // RETN
        cpu.iff1 = cpu.iff2;
        cpu.pc = cpu.pop();
        14
    } else {
        // RETI
        cpu.pc = cpu.pop();
        14
    }
}

fn execute_ed_im<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, y: u8) -> u8 {
    // IM y
    cpu.im = match y & 0x03 {
        0 | 1 => 0,
        2 => 1,
        3 => 2,
        _ => 0,
    };
    8
}

fn execute_ed_block<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, y: u8, z: u8) -> u8 {
    // Block instructions
    if y >= 4 {
        dispatch_z!(
            z,
            execute_ldi_ldd(cpu, y),
            execute_cpi_cpd(cpu, y),
            execute_ini_ind(cpu, y),
            execute_outi_outd(cpu, y),
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
    p: u8,
    q: u8,
) -> u8 {
    if q == 0 {
        // SBC HL, rp
        let hl = cpu.hl() as u32;
        let rp = cpu.get_rp(p) as u32;
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
        let rp = cpu.get_rp(p) as u32;
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
    p: u8,
    q: u8,
) -> u8 {
    let nn = cpu.fetch_word();
    if q == 0 {
        // LD (nn), rp
        cpu.write_word(nn, cpu.get_rp(p));
    } else {
        // LD rp, (nn)
        let val = cpu.read_word(nn);
        cpu.set_rp(p, val);
    }
    cpu.memptr = nn.wrapping_add(1);
    20
}

fn execute_ed_misc<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, y: u8) -> u8 {
    match y {
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

fn execute_ldi_ldd<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, y: u8) -> u8 {
    let hl = cpu.hl();
    let de = cpu.de();
    let val = cpu.read_byte(hl);
    cpu.write_byte(de, val);

    let bc = cpu.bc().wrapping_sub(1);
    cpu.set_bc(bc);

    let (new_hl, new_de) = if (y & 1) == 0 {
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
    if y >= 6 && bc != 0 {
        cpu.pc = cpu.pc.wrapping_sub(2);
        cpu.memptr = cpu.pc.wrapping_add(1);
        21
    } else {
        16
    }
}

fn execute_cpi_cpd<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, y: u8) -> u8 {
    let hl = cpu.hl();
    let val = cpu.read_byte(hl);
    let result = cpu.a.wrapping_sub(val);

    let bc = cpu.bc().wrapping_sub(1);
    cpu.set_bc(bc);

    let new_hl = if (y & 1) == 0 {
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

    cpu.memptr = cpu.memptr.wrapping_add(1);

    // CPIR/CPDR
    if y >= 6 && bc != 0 && result != 0 {
        cpu.pc = cpu.pc.wrapping_sub(2);
        cpu.memptr = cpu.pc.wrapping_add(1);
        21
    } else {
        16
    }
}

fn execute_ini_ind<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, y: u8) -> u8 {
    // INI (y=4), IND (y=5), INIR (y=6), INDR (y=7)

    let port = cpu.bc();
    let hl = cpu.hl();

    let io_val = cpu.read_port(port);
    cpu.write_byte(hl, io_val);

    let b = cpu.b.wrapping_sub(1);
    cpu.b = b;

    let new_hl = if (y & 1) == 0 {
        hl.wrapping_add(1)
    } else {
        hl.wrapping_sub(1)
    };
    cpu.set_hl(new_hl);

    // Flags:
    // Z: set if B=0
    // N: Set (bit 7 of internal calculation?) -> Z80 manual says N is Set.
    cpu.set_flag(flags::ZERO, b == 0);
    cpu.set_flag(flags::ADD_SUB, true);

    // Repeat logic for INIR/INDR (y>=6)
    if y >= 6 && b != 0 {
        cpu.pc = cpu.pc.wrapping_sub(2);
        21
    } else {
        16
    }
}

fn execute_outi_outd<M: MemoryInterface, I: IoInterface>(cpu: &mut Z80<M, I>, y: u8) -> u8 {
    // OUTI (y=4), OUTD (y=5), OTIR (y=6), OTDR (y=7)

    let hl = cpu.hl();
    let val = cpu.read_byte(hl);

    let port = cpu.bc();
    cpu.write_port(port, val);

    let b = cpu.b.wrapping_sub(1);
    cpu.b = b;

    let new_hl = if (y & 1) == 0 {
        hl.wrapping_add(1)
    } else {
        hl.wrapping_sub(1)
    };
    cpu.set_hl(new_hl);

    cpu.set_flag(flags::ZERO, b == 0);
    cpu.set_flag(flags::ADD_SUB, true);

    if y >= 6 && b != 0 {
        cpu.pc = cpu.pc.wrapping_sub(2);
        21
    } else {
        16
    }
}
