//! M68k Instruction Decoder
//!
//! This module decodes M68k opcodes into instruction representations
//! that can be executed by the CPU.

pub use crate::cpu::instructions::{
    AddressingMode, ArithmeticInstruction, BitSource, BitsInstruction, Condition, DataInstruction,
    DecodeCacheEntry, Instruction, ShiftCount, Size, SystemInstruction,
};

/// Decode a single M68k instruction from an opcode
pub fn decode(opcode: u16) -> Instruction {
    decode_uncached(opcode)
}

fn decode_uncached(opcode: u16) -> Instruction {
    match (opcode >> 12) & 0x0F {
        0x0 => decode_group_0(opcode),
        0x1 => decode_move(opcode, Size::Byte),
        0x2 => decode_move(opcode, Size::Long),
        0x3 => decode_move(opcode, Size::Word),
        0x4 => decode_group_4(opcode),
        0x5 => decode_group_5(opcode),
        0x6 => decode_group_6(opcode),
        0x7 => decode_moveq(opcode),
        0x8 => decode_group_8(opcode),
        0x9 => decode_sub(opcode),
        0xA => decode_line_a(opcode),
        0xB => decode_group_b(opcode),
        0xC => decode_group_c(opcode),
        0xD => decode_add(opcode),
        0xE => decode_shifts(opcode),
        0xF => decode_line_f(opcode),
        _ => unreachable!(),
    }
}

// === Group decoders ===

fn decode_line_a(opcode: u16) -> Instruction {
    Instruction::System(SystemInstruction::LineA { opcode })
}

fn decode_line_f(opcode: u16) -> Instruction {
    Instruction::System(SystemInstruction::LineF { opcode })
}

fn decode_group_0(opcode: u16) -> Instruction {
    if (opcode & 0x0100) != 0 {
        decode_movep(opcode)
            .or_else(|| decode_bit_dynamic(opcode))
            .unwrap_or(Instruction::System(SystemInstruction::Unimplemented {
                opcode,
            }))
    } else {
        decode_static_bit(opcode)
            .or_else(|| decode_ccr_sr_immediate(opcode))
            .or_else(|| decode_immediate_alu(opcode))
            .unwrap_or(Instruction::System(SystemInstruction::Unimplemented {
                opcode,
            }))
    }
}

fn decode_movep(opcode: u16) -> Option<Instruction> {
    if opcode & 0x0138 == 0x0108 {
        let reg = ((opcode >> 9) & 0x07) as u8;
        let op = (opcode >> 6) & 0x07;
        let an = (opcode & 0x07) as u8;

        if (op & 0x04) != 0 {
            let size = if (op & 0x01) != 0 {
                Size::Long
            } else {
                Size::Word
            };
            let direction = (op & 0x02) != 0; // 0 = mem to reg, 1 = reg to mem
            return Some(Instruction::Data(DataInstruction::Movep {
                size,
                reg,
                an,
                direction,
            }));
        }
    }
    None
}

fn decode_bit_dynamic(opcode: u16) -> Option<Instruction> {
    if opcode & 0x0100 != 0 {
        // Bit manipulation with register
        let reg = ((opcode >> 9) & 0x07) as u8;
        let mode = ((opcode >> 3) & 0x07) as u8;
        let ea_reg = (opcode & 0x07) as u8;

        if let Some(dst) = AddressingMode::from_mode_reg(mode, ea_reg) {
            let op = (opcode >> 6) & 0x03;
            let bit = BitSource::Register(reg);

            return Some(match op {
                0b00 => Instruction::Bits(BitsInstruction::Btst { bit, dst }),
                0b01 => Instruction::Bits(BitsInstruction::Bchg { bit, dst }),
                0b10 => Instruction::Bits(BitsInstruction::Bclr { bit, dst }),
                0b11 => Instruction::Bits(BitsInstruction::Bset { bit, dst }),
                _ => unreachable!(),
            });
        }
    }
    None
}

fn decode_static_bit(opcode: u16) -> Option<Instruction> {
    let op = (opcode >> 9) & 0x07;
    // Static Bit Instructions (Op 4)
    if op == 0b100 {
        let mode = ((opcode >> 3) & 0x07) as u8;
        let reg = (opcode & 0x07) as u8;
        if let Some(dst) = AddressingMode::from_mode_reg(mode, reg) {
            let bit_op = (opcode >> 6) & 0x03;
            let bit = BitSource::Immediate;
            return Some(match bit_op {
                0b00 => Instruction::Bits(BitsInstruction::Btst { bit, dst }),
                0b01 => Instruction::Bits(BitsInstruction::Bchg { bit, dst }),
                0b10 => Instruction::Bits(BitsInstruction::Bclr { bit, dst }),
                0b11 => Instruction::Bits(BitsInstruction::Bset { bit, dst }),
                _ => unreachable!(),
            });
        }
    }
    None
}

fn decode_ccr_sr_immediate(opcode: u16) -> Option<Instruction> {
    let mode = ((opcode >> 3) & 0x07) as u8;
    let reg = (opcode & 0x07) as u8;

    // CCR/SR Immediate Operations - Special case when mode=7, reg=4 (immediate)
    if mode == 7 && reg == 4 {
        let op = (opcode >> 9) & 0x07;
        let size_bits = ((opcode >> 6) & 0x03) as u8;
        return Some(match (op, size_bits) {
            (0b000, 0b00) => Instruction::System(SystemInstruction::OriToCcr),
            (0b000, 0b01) => Instruction::System(SystemInstruction::OriToSr),
            (0b001, 0b00) => Instruction::System(SystemInstruction::AndiToCcr),
            (0b001, 0b01) => Instruction::System(SystemInstruction::AndiToSr),
            (0b101, 0b00) => Instruction::System(SystemInstruction::EoriToCcr),
            (0b101, 0b01) => Instruction::System(SystemInstruction::EoriToSr),
            _ => Instruction::System(SystemInstruction::Unimplemented { opcode }),
        });
    }
    None
}

fn decode_immediate_alu(opcode: u16) -> Option<Instruction> {
    // Immediate Instructions (ORI, ANDI, SUBI, ADDI, EORI, CMPI)
    let size_bits = ((opcode >> 6) & 0x03) as u8;
    if let Some(size) = Size::from_bits(size_bits) {
        let op = (opcode >> 9) & 0x07;
        let mode = ((opcode >> 3) & 0x07) as u8;
        let reg = (opcode & 0x07) as u8;
        if let Some(dst) = AddressingMode::from_mode_reg(mode, reg) {
            return Some(match op {
                0b000 => Instruction::Bits(BitsInstruction::OrI { size, dst }),
                0b001 => Instruction::Bits(BitsInstruction::AndI { size, dst }),
                0b010 => Instruction::Arithmetic(ArithmeticInstruction::SubI { size, dst }),
                0b011 => Instruction::Arithmetic(ArithmeticInstruction::AddI { size, dst }),
                // 0b100 is bit/CCR ops, handled elsewhere
                0b101 => Instruction::Bits(BitsInstruction::EorI { size, dst }),
                0b110 => Instruction::Arithmetic(ArithmeticInstruction::CmpI { size, dst }),
                _ => return None,
            });
        }
    }
    None
}

fn decode_move(opcode: u16, size: Size) -> Instruction {
    // MOVE instruction format:
    // Bits 15-12: Size (01=byte, 11=word, 10=long)
    // Bits 11-9: Destination register
    // Bits 8-6: Destination mode
    // Bits 5-3: Source mode
    // Bits 2-0: Source register

    let dst_reg = ((opcode >> 9) & 0x07) as u8;
    let dst_mode = ((opcode >> 6) & 0x07) as u8;
    let src_mode = ((opcode >> 3) & 0x07) as u8;
    let src_reg = (opcode & 0x07) as u8;

    let src = match AddressingMode::from_mode_reg(src_mode, src_reg) {
        Some(m) => m,
        None => {
            return Instruction::System(SystemInstruction::Unimplemented { opcode });
        }
    };

    // MOVEA has destination mode 001 (address register)
    if dst_mode == 0b001 {
        // MOVEA - size must be word or long
        if size == Size::Byte {
            return Instruction::System(SystemInstruction::Unimplemented { opcode });
        }
        return Instruction::Data(DataInstruction::MoveA { size, src, dst_reg });
    }

    let dst = match AddressingMode::from_mode_reg(dst_mode, dst_reg) {
        Some(m) => m,
        None => {
            return Instruction::System(SystemInstruction::Unimplemented { opcode });
        }
    };

    if !dst.is_alterable() {
        return Instruction::System(SystemInstruction::Unimplemented { opcode });
    }

    Instruction::Data(DataInstruction::Move { size, src, dst })
}

fn decode_group_4(opcode: u16) -> Instruction {
    decode_group_4_misc(opcode)
        .or_else(|| decode_group_4_control(opcode))
        .or_else(|| decode_group_4_movem(opcode))
        .or_else(|| decode_group_4_arithmetic(opcode))
        .unwrap_or(Instruction::System(SystemInstruction::Unimplemented {
            opcode,
        }))
}

fn decode_group_4_misc(opcode: u16) -> Option<Instruction> {
    let reg = (opcode & 0x07) as u8;

    // Check for specific instructions first
    match opcode & 0xFFF8 {
        0x4E70 => {
            return Some(match reg {
                0 => Instruction::System(SystemInstruction::Reset),
                1 => Instruction::System(SystemInstruction::Nop),
                2 => Instruction::System(SystemInstruction::Stop),
                3 => Instruction::System(SystemInstruction::Rte),
                5 => Instruction::System(SystemInstruction::Rts),
                6 => Instruction::System(SystemInstruction::TrapV),
                7 => Instruction::System(SystemInstruction::Rtr),
                _ => return None,
            })
        }
        0x4E50 => return Some(Instruction::System(SystemInstruction::Link { reg })),
        0x4E58 => return Some(Instruction::System(SystemInstruction::Unlk { reg })),
        0x4E60 => {
            return Some(Instruction::System(SystemInstruction::MoveUsp {
                reg,
                to_usp: true,
            }))
        }
        0x4E68 => {
            return Some(Instruction::System(SystemInstruction::MoveUsp {
                reg,
                to_usp: false,
            }))
        }
        0x4840 => return Some(Instruction::Data(DataInstruction::Swap { reg })),
        0x4880 => {
            return Some(Instruction::Data(DataInstruction::Ext {
                size: Size::Word,
                reg,
            }))
        }
        0x48C0 => {
            return Some(Instruction::Data(DataInstruction::Ext {
                size: Size::Long,
                reg,
            }))
        }

        _ => {}
    }

    // TRAP
    if opcode & 0xFFF0 == 0x4E40 {
        return Some(Instruction::System(SystemInstruction::Trap {
            vector: (opcode & 0x0F) as u8,
        }));
    }

    // ILLEGAL - 4AFC
    if opcode == 0x4AFC {
        return Some(Instruction::System(SystemInstruction::Illegal));
    }
    None
}

fn decode_group_4_control(opcode: u16) -> Option<Instruction> {
    let mode = ((opcode >> 3) & 0x07) as u8;
    let reg = (opcode & 0x07) as u8;

    // LEA
    if opcode & 0xF1C0 == 0x41C0 {
        let dst_reg = ((opcode >> 9) & 0x07) as u8;
        if let Some(src) = AddressingMode::from_mode_reg(mode, reg) {
            return Some(Instruction::Data(DataInstruction::Lea { src, dst_reg }));
        }
    }

    // PEA
    if opcode & 0xFFC0 == 0x4840 && mode != 0 {
        if let Some(src) = AddressingMode::from_mode_reg(mode, reg) {
            return Some(Instruction::Data(DataInstruction::Pea { src }));
        }
    }

    // JMP
    if opcode & 0xFFC0 == 0x4EC0 {
        if let Some(dst) = AddressingMode::from_mode_reg(mode, reg) {
            return Some(Instruction::System(SystemInstruction::Jmp { dst }));
        }
    }

    // JSR
    if opcode & 0xFFC0 == 0x4E80 {
        if let Some(dst) = AddressingMode::from_mode_reg(mode, reg) {
            return Some(Instruction::System(SystemInstruction::Jsr { dst }));
        }
    }

    // CHK - 0100 rrr 1s0 mmm xxx (s=0 word, s=1 long for 68020+)
    // 68000: only word size (bits 7-6 = 10)
    if opcode & 0xF1C0 == 0x4180 {
        let dst_reg = ((opcode >> 9) & 0x07) as u8;
        if let Some(src) = AddressingMode::from_mode_reg(mode, reg) {
            return Some(Instruction::Arithmetic(ArithmeticInstruction::Chk {
                src,
                dst_reg,
            }));
        }
    }
    None
}

fn decode_group_4_movem(opcode: u16) -> Option<Instruction> {
    // MOVEM - Register to Memory: 0100 1000 1s mmm rrr (s=0 word, s=1 long)
    //       - Memory to Register: 0100 1100 1s mmm rrr
    if opcode & 0xFB80 == 0x4880 {
        let to_memory = (opcode & 0x0400) == 0; // bit 10: 0=to mem, 1=from mem
        let size = if (opcode & 0x0040) != 0 {
            Size::Long
        } else {
            Size::Word
        };
        let mode = ((opcode >> 3) & 0x07) as u8;
        let reg = (opcode & 0x07) as u8;
        if let Some(ea) = AddressingMode::from_mode_reg(mode, reg) {
            // Mask is in extension word, but we'll read it during execution
            return Some(Instruction::Data(DataInstruction::Movem {
                size,
                direction: to_memory,
                mask: 0,
                ea,
            }));
        }
    }
    None
}

fn decode_group_4_arithmetic(opcode: u16) -> Option<Instruction> {
    let mode = ((opcode >> 3) & 0x07) as u8;
    let reg = (opcode & 0x07) as u8;

    // NBCD
    if opcode & 0xFFC0 == 0x4800 {
        if let Some(dst) = AddressingMode::from_mode_reg(mode, reg) {
            if dst.is_data_alterable() {
                return Some(Instruction::Arithmetic(ArithmeticInstruction::Nbcd { dst }));
            }
        }
    }

    // TAS - 0100 1010 11 mmm rrr (4AC0)
    if opcode & 0xFFC0 == 0x4AC0 {
        if let Some(dst) = AddressingMode::from_mode_reg(mode, reg) {
            return Some(Instruction::Bits(BitsInstruction::Tas { dst }));
        }
    }

    // CLR, NEG, NOT, TST
    let bits_11_8 = (opcode >> 8) & 0x0F;
    let bits_7_6 = (opcode >> 6) & 0x03;
    if let Some(size) = Size::from_bits(bits_7_6 as u8) {
        if let Some(dst) = AddressingMode::from_mode_reg(mode, reg) {
            match bits_11_8 {
                0x0 => {
                    return Some(Instruction::Arithmetic(ArithmeticInstruction::NegX {
                        size,
                        dst,
                    }))
                }
                0x2 => return Some(Instruction::Data(DataInstruction::Clr { size, dst })),
                0x4 => {
                    return Some(Instruction::Arithmetic(ArithmeticInstruction::Neg {
                        size,
                        dst,
                    }))
                }
                0x6 => return Some(Instruction::Bits(BitsInstruction::Not { size, dst })),
                0xA => {
                    return Some(Instruction::Arithmetic(ArithmeticInstruction::Tst {
                        size,
                        dst,
                    }))
                }
                _ => {}
            };
        }
    }

    // MOVE from SR
    if opcode & 0xFFC0 == 0x40C0 {
        if let Some(dst) = AddressingMode::from_mode_reg(mode, reg) {
            return Some(Instruction::System(SystemInstruction::MoveFromSr { dst }));
        }
    }

    // MOVE to CCR
    if opcode & 0xFFC0 == 0x44C0 {
        if let Some(src) = AddressingMode::from_mode_reg(mode, reg) {
            return Some(Instruction::System(SystemInstruction::MoveToCcr { src }));
        }
    }

    // MOVE to SR
    if opcode & 0xFFC0 == 0x46C0 {
        if let Some(src) = AddressingMode::from_mode_reg(mode, reg) {
            return Some(Instruction::System(SystemInstruction::MoveToSr { src }));
        }
    }
    None
}

fn decode_group_5(opcode: u16) -> Instruction {
    // ADDQ, SUBQ, Scc, DBcc

    let data = ((opcode >> 9) & 0x07) as u8;
    let data = if data == 0 { 8 } else { data };
    let size_bits = ((opcode >> 6) & 0x03) as u8;
    let mode = ((opcode >> 3) & 0x07) as u8;
    let reg = (opcode & 0x07) as u8;

    // DBcc
    if size_bits == 0b11 && mode == 0b001 {
        let condition = Condition::from_bits(((opcode >> 8) & 0x0F) as u8);
        return Instruction::System(SystemInstruction::DBcc { condition, reg });
    }

    // Scc
    if size_bits == 0b11 && mode != 0b001 {
        let condition = Condition::from_bits(((opcode >> 8) & 0x0F) as u8);
        if let Some(dst) = AddressingMode::from_mode_reg(mode, reg) {
            return Instruction::System(SystemInstruction::Scc { condition, dst });
        }
    }

    if let Some(size) = Size::from_bits(size_bits) {
        if let Some(dst) = AddressingMode::from_mode_reg(mode, reg) {
            // Check for illegal Byte operation on Address Register
            if size == Size::Byte && matches!(dst, AddressingMode::AddressRegister(_)) {
                return Instruction::System(SystemInstruction::Illegal);
            }

            let is_sub = (opcode >> 8) & 0x01 != 0;
            if is_sub {
                return Instruction::Arithmetic(ArithmeticInstruction::SubQ { size, dst, data });
            } else {
                return Instruction::Arithmetic(ArithmeticInstruction::AddQ { size, dst, data });
            }
        }
    }

    Instruction::System(SystemInstruction::Unimplemented { opcode })
}

fn decode_group_6(opcode: u16) -> Instruction {
    // Bcc, BRA, BSR

    let condition_bits = ((opcode >> 8) & 0x0F) as u8;
    let displacement_byte = (opcode & 0xFF) as i8;

    // Displacement of 0 means 16-bit displacement follows
    // Displacement of 0xFF means 32-bit displacement follows (68020+)
    let displacement = displacement_byte as i16;

    match condition_bits {
        0x0 => Instruction::System(SystemInstruction::Bra { displacement }),
        0x1 => Instruction::System(SystemInstruction::Bsr { displacement }),
        _ => {
            let condition = Condition::from_bits(condition_bits);
            Instruction::System(SystemInstruction::Bcc {
                condition,
                displacement,
            })
        }
    }
}

fn decode_moveq(opcode: u16) -> Instruction {
    // MOVEQ - Move Quick
    // Format: 0111 DDD 0 DDDDDDDD
    // D = destination register, D = 8-bit data

    if opcode & 0x0100 != 0 {
        return Instruction::System(SystemInstruction::Unimplemented { opcode });
    }

    let dst_reg = ((opcode >> 9) & 0x07) as u8;
    let data = (opcode & 0xFF) as i8;

    Instruction::Data(DataInstruction::MoveQ { dst_reg, data })
}

fn decode_group_8(opcode: u16) -> Instruction {
    // OR, DIV, SBCD

    let reg = ((opcode >> 9) & 0x07) as u8;
    let direction = (opcode >> 8) & 0x01 != 0;
    let size_bits = ((opcode >> 6) & 0x03) as u8;
    let ea_mode = ((opcode >> 3) & 0x07) as u8;
    let ea_reg = (opcode & 0x07) as u8;

    // SBCD
    // 1000 Rx 1 0000 m Ry
    if opcode & 0xF1F0 == 0x8100 {
        let memory_mode = (opcode & 0x0008) != 0;
        return Instruction::Arithmetic(ArithmeticInstruction::Sbcd {
            src_reg: ea_reg,
            dst_reg: reg,
            memory_mode,
        });
    }

    // DIVU
    if size_bits == 0b11 && !direction {
        if let Some(src) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
            return Instruction::Arithmetic(ArithmeticInstruction::DivU { src, dst_reg: reg });
        }
    }

    // DIVS
    if size_bits == 0b11 && direction {
        if let Some(src) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
            return Instruction::Arithmetic(ArithmeticInstruction::DivS { src, dst_reg: reg });
        }
    }

    // OR
    if let Some(size) = Size::from_bits(size_bits) {
        if let Some(ea) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
            let (src, dst) = if direction {
                (AddressingMode::DataRegister(reg), ea)
            } else {
                (ea, AddressingMode::DataRegister(reg))
            };
            return Instruction::Bits(BitsInstruction::Or {
                size,
                src,
                dst,
                direction,
            });
        }
    }

    Instruction::System(SystemInstruction::Unimplemented { opcode })
}

fn decode_sub(opcode: u16) -> Instruction {
    let reg = ((opcode >> 9) & 0x07) as u8;
    let opmode = ((opcode >> 6) & 0x07) as u8;
    let ea_mode = ((opcode >> 3) & 0x07) as u8;
    let ea_reg = (opcode & 0x07) as u8;

    match opmode {
        0..=2 => {
            // SUB <ea>, Dn
            let size = Size::from_bits(opmode).unwrap();
            if let Some(src) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
                return Instruction::Arithmetic(ArithmeticInstruction::Sub {
                    size,
                    src,
                    dst: AddressingMode::DataRegister(reg),
                    direction: false,
                });
            }
        }
        3 | 7 => {
            // SUBA <ea>, An
            let size = if opmode == 3 { Size::Word } else { Size::Long };
            if let Some(src) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
                return Instruction::Arithmetic(ArithmeticInstruction::SubA {
                    size,
                    src,
                    dst_reg: reg,
                });
            }
        }
        4..=6 => {
            // SUB Dn, <ea> or SUBX
            let size = Size::from_bits(opmode - 4).unwrap();

            // Check for SUBX: 1001 Rx 1 size 00 m Ry
            if (opcode & 0x0130) == 0x0100 {
                let memory_mode = (opcode & 0x0008) != 0;
                return Instruction::Arithmetic(ArithmeticInstruction::SubX {
                    size,
                    src_reg: ea_reg,
                    dst_reg: reg,
                    memory_mode,
                });
            }

            if ea_mode == 0b001 {
                // SUB Dn, An is illegal (use SUBA)
                return Instruction::System(SystemInstruction::Unimplemented { opcode });
            }
            if let Some(ea) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
                return Instruction::Arithmetic(ArithmeticInstruction::Sub {
                    size,
                    src: AddressingMode::DataRegister(reg),
                    dst: ea,
                    direction: true,
                });
            }
        }
        _ => {}
    }

    Instruction::System(SystemInstruction::Unimplemented { opcode })
}

fn decode_group_b(opcode: u16) -> Instruction {
    // CMP, CMPA, EOR, CMPM

    let reg = ((opcode >> 9) & 0x07) as u8;
    let opmode = (opcode >> 6) & 0x07;
    let ea_mode = ((opcode >> 3) & 0x07) as u8;
    let ea_reg = (opcode & 0x07) as u8;

    // CMPM
    if (opcode & 0x0138) == 0x0108 {
        let size_bits = ((opcode >> 6) & 0x03) as u8;
        if let Some(size) = Size::from_bits(size_bits) {
            return Instruction::Arithmetic(ArithmeticInstruction::CmpM {
                size,
                ax: reg,
                ay: ea_reg,
            });
        }
    }

    // CMPA
    if opmode == 0b011 || opmode == 0b111 {
        let size = if opmode == 0b011 {
            Size::Word
        } else {
            Size::Long
        };
        if let Some(src) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
            return Instruction::Arithmetic(ArithmeticInstruction::CmpA {
                size,
                src,
                dst_reg: reg,
            });
        }
    }

    // EOR (direction bit set, not CMPA)
    if opmode & 0x04 != 0 && opmode != 0b111 {
        let size_bits = (opmode & 0x03) as u8;
        if let Some(size) = Size::from_bits(size_bits) {
            if let Some(dst) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
                return Instruction::Bits(BitsInstruction::Eor {
                    size,
                    src_reg: reg,
                    dst,
                });
            }
        }
    }

    // CMP
    let size_bits = (opmode & 0x03) as u8;
    if let Some(size) = Size::from_bits(size_bits) {
        if let Some(src) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
            return Instruction::Arithmetic(ArithmeticInstruction::Cmp {
                size,
                src,
                dst_reg: reg,
            });
        }
    }

    Instruction::System(SystemInstruction::Unimplemented { opcode })
}

fn decode_group_c(opcode: u16) -> Instruction {
    // AND, MUL, ABCD, EXG

    let reg = ((opcode >> 9) & 0x07) as u8;
    let direction = (opcode >> 8) & 0x01 != 0;
    let size_bits = ((opcode >> 6) & 0x03) as u8;
    let ea_mode = ((opcode >> 3) & 0x07) as u8;
    let ea_reg = (opcode & 0x07) as u8;

    // ABCD
    // 1100 Rx 1 0000 m Ry
    if opcode & 0xF1F0 == 0xC100 {
        let memory_mode = (opcode & 0x0008) != 0;
        return Instruction::Arithmetic(ArithmeticInstruction::Abcd {
            src_reg: ea_reg,
            dst_reg: reg,
            memory_mode,
        });
    }

    // MULU
    if size_bits == 0b11 && !direction {
        if let Some(src) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
            return Instruction::Arithmetic(ArithmeticInstruction::MulU { src, dst_reg: reg });
        }
    }

    // MULS
    if size_bits == 0b11 && direction {
        if let Some(src) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
            return Instruction::Arithmetic(ArithmeticInstruction::MulS { src, dst_reg: reg });
        }
    }

    // EXG
    if opcode & 0x0130 == 0x0100 {
        let mode = ((opcode >> 3) & 0x1F) as u8;
        return Instruction::Data(DataInstruction::Exg {
            rx: reg,
            ry: ea_reg,
            mode,
        });
    }

    // AND
    if let Some(size) = Size::from_bits(size_bits) {
        if let Some(ea) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
            let (src, dst) = if direction {
                (AddressingMode::DataRegister(reg), ea)
            } else {
                (ea, AddressingMode::DataRegister(reg))
            };
            return Instruction::Bits(BitsInstruction::And {
                size,
                src,
                dst,
                direction,
            });
        }
    }

    Instruction::System(SystemInstruction::Unimplemented { opcode })
}

fn decode_add(opcode: u16) -> Instruction {
    let reg = ((opcode >> 9) & 0x07) as u8;
    let opmode = ((opcode >> 6) & 0x07) as u8;
    let ea_mode = ((opcode >> 3) & 0x07) as u8;
    let ea_reg = (opcode & 0x07) as u8;

    match opmode {
        0..=2 => {
            // ADD <ea>, Dn
            let size = Size::from_bits(opmode).unwrap();
            if let Some(src) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
                return Instruction::Arithmetic(ArithmeticInstruction::Add {
                    size,
                    src,
                    dst: AddressingMode::DataRegister(reg),
                    direction: false,
                });
            }
        }
        3 | 7 => {
            // ADDA <ea>, An
            let size = if opmode == 3 { Size::Word } else { Size::Long };
            if let Some(src) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
                return Instruction::Arithmetic(ArithmeticInstruction::AddA {
                    size,
                    src,
                    dst_reg: reg,
                });
            }
        }
        4..=6 => {
            // ADD Dn, <ea> or ADDX
            let size = Size::from_bits(opmode - 4).unwrap();

            // Check for ADDX: 1101 Rx 1 size 00 m Ry
            if (opcode & 0x0130) == 0x0100 {
                let memory_mode = (opcode & 0x0008) != 0;
                return Instruction::Arithmetic(ArithmeticInstruction::AddX {
                    size,
                    src_reg: ea_reg,
                    dst_reg: reg,
                    memory_mode,
                });
            }

            if ea_mode == 0b001 {
                // ADD Dn, An is illegal (use ADDA)
                return Instruction::System(SystemInstruction::Unimplemented { opcode });
            }
            if let Some(ea) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
                return Instruction::Arithmetic(ArithmeticInstruction::Add {
                    size,
                    src: AddressingMode::DataRegister(reg),
                    dst: ea,
                    direction: true,
                });
            }
        }
        _ => {}
    }

    Instruction::System(SystemInstruction::Unimplemented { opcode })
}

fn make_shift_instruction(
    op_type: u8,
    direction: bool,
    size: Size,
    dst: AddressingMode,
    count: ShiftCount,
) -> Instruction {
    match (op_type, direction) {
        (0b00, false) => Instruction::Bits(BitsInstruction::Asr { size, dst, count }),
        (0b00, true) => Instruction::Bits(BitsInstruction::Asl { size, dst, count }),
        (0b01, false) => Instruction::Bits(BitsInstruction::Lsr { size, dst, count }),
        (0b01, true) => Instruction::Bits(BitsInstruction::Lsl { size, dst, count }),
        (0b10, false) => Instruction::Bits(BitsInstruction::Roxr { size, dst, count }),
        (0b10, true) => Instruction::Bits(BitsInstruction::Roxl { size, dst, count }),
        (0b11, false) => Instruction::Bits(BitsInstruction::Ror { size, dst, count }),
        (0b11, true) => Instruction::Bits(BitsInstruction::Rol { size, dst, count }),
        _ => unreachable!(), // op_type is 2 bits
    }
}

fn decode_shifts(opcode: u16) -> Instruction {
    // ASL, ASR, LSL, LSR, ROL, ROR, ROXL, ROXR

    let count_or_reg = ((opcode >> 9) & 0x07) as u8;
    let direction = (opcode >> 8) & 0x01 != 0; // 0 = right, 1 = left
    let size_bits = ((opcode >> 6) & 0x03) as u8;
    let ir = (opcode >> 5) & 0x01 != 0; // 0 = immediate, 1 = register
    let op_type = ((opcode >> 3) & 0x03) as u8;
    let reg = (opcode & 0x07) as u8;

    // Memory shifts (size = 0b11)
    if size_bits == 0b11 {
        // For memory shifts, the type is encoded in bits 10-9 (part of what is count/reg in register shifts)
        let op_type = ((opcode >> 9) & 0x03) as u8;
        let ea_mode = ((opcode >> 3) & 0x07) as u8;
        let ea_reg = (opcode & 0x07) as u8;
        if let Some(dst) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
            // Memory shifts are always by 1.
            match (op_type, direction) {
                (0b00, true) => return Instruction::Bits(BitsInstruction::AslM { dst }),
                (0b00, false) => return Instruction::Bits(BitsInstruction::AsrM { dst }),
                _ => {
                    let count = ShiftCount::Immediate(1);
                    return make_shift_instruction(op_type, direction, Size::Word, dst, count);
                }
            }
        }
    }

    // Register shifts
    if let Some(size) = Size::from_bits(size_bits) {
        let count = if ir {
            ShiftCount::Register(count_or_reg)
        } else {
            let imm = if count_or_reg == 0 { 8 } else { count_or_reg };
            ShiftCount::Immediate(imm)
        };
        let dst = AddressingMode::DataRegister(reg);

        return make_shift_instruction(op_type, direction, size, dst, count);
    }

    Instruction::System(SystemInstruction::Unimplemented { opcode })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_nop() {
        assert_eq!(decode(0x4E71), Instruction::System(SystemInstruction::Nop));
    }

    #[test]
    fn test_decode_rts() {
        assert_eq!(decode(0x4E75), Instruction::System(SystemInstruction::Rts));
    }

    #[test]
    fn test_decode_move_l_d1_d0() {
        // MOVE.L D1, D0 = 0x2001
        let instr = decode(0x2001);
        assert_eq!(
            instr,
            Instruction::Data(DataInstruction::Move {
                size: Size::Long,
                src: AddressingMode::DataRegister(1),
                dst: AddressingMode::DataRegister(0),
            })
        );
    }

    #[test]
    fn test_decode_moveq() {
        // MOVEQ #42, D3
        let instr = decode(0x762A);
        assert_eq!(
            instr,
            Instruction::Data(DataInstruction::MoveQ {
                dst_reg: 3,
                data: 42,
            })
        );
    }

    #[test]
    fn test_decode_bra() {
        // BRA with 8-bit displacement
        let instr = decode(0x6010);
        assert_eq!(
            instr,
            Instruction::System(SystemInstruction::Bra { displacement: 16 })
        );
    }

    #[test]
    fn test_decode_beq() {
        // BEQ with 8-bit displacement
        let instr = decode(0x6708);
        assert_eq!(
            instr,
            Instruction::System(SystemInstruction::Bcc {
                condition: Condition::Equal,
                displacement: 8,
            })
        );
    }

    #[test]
    fn test_decode_addq() {
        // ADDQ.L #8, D0
        let instr = decode(0x5080);
        assert_eq!(
            instr,
            Instruction::Arithmetic(ArithmeticInstruction::AddQ {
                size: Size::Long,
                dst: AddressingMode::DataRegister(0),
                data: 8,
            })
        );
    }

    #[test]
    fn test_addressing_mode_display() {
        assert_eq!(format!("{}", AddressingMode::DataRegister(3)), "D3");
        assert_eq!(format!("{}", AddressingMode::AddressIndirect(5)), "(A5)");
        assert_eq!(
            format!("{}", AddressingMode::AddressPostIncrement(2)),
            "(A2)+"
        );
        assert_eq!(
            format!("{}", AddressingMode::AddressPreDecrement(7)),
            "-(A7)"
        );
    }

    #[test]
    fn test_decode_div() {
        // DIVU.W D1, D0
        let instr_divu = decode(0x80C1);
        assert_eq!(
            instr_divu,
            Instruction::Arithmetic(ArithmeticInstruction::DivU {
                src: AddressingMode::DataRegister(1),
                dst_reg: 0,
            })
        );

        // DIVS.W D1, D0
        let instr_divs = decode(0x81C1);
        assert_eq!(
            instr_divs,
            Instruction::Arithmetic(ArithmeticInstruction::DivS {
                src: AddressingMode::DataRegister(1),
                dst_reg: 0,
            })
        );
    }

    #[test]
    fn test_decode_nbcd() {
        // Valid NBCD modes
        assert_eq!(
            decode(0x4800),
            Instruction::Arithmetic(ArithmeticInstruction::Nbcd {
                dst: AddressingMode::DataRegister(0)
            })
        );
        assert_eq!(
            decode(0x4810),
            Instruction::Arithmetic(ArithmeticInstruction::Nbcd {
                dst: AddressingMode::AddressIndirect(0)
            })
        );
        assert_eq!(
            decode(0x4818),
            Instruction::Arithmetic(ArithmeticInstruction::Nbcd {
                dst: AddressingMode::AddressPostIncrement(0)
            })
        );
        assert_eq!(
            decode(0x4820),
            Instruction::Arithmetic(ArithmeticInstruction::Nbcd {
                dst: AddressingMode::AddressPreDecrement(0)
            })
        );
        assert_eq!(
            decode(0x4828),
            Instruction::Arithmetic(ArithmeticInstruction::Nbcd {
                dst: AddressingMode::AddressDisplacement(0)
            })
        );
        assert_eq!(
            decode(0x4830),
            Instruction::Arithmetic(ArithmeticInstruction::Nbcd {
                dst: AddressingMode::AddressIndex(0)
            })
        );
        assert_eq!(
            decode(0x4838),
            Instruction::Arithmetic(ArithmeticInstruction::Nbcd {
                dst: AddressingMode::AbsoluteShort
            })
        );
        assert_eq!(
            decode(0x4839),
            Instruction::Arithmetic(ArithmeticInstruction::Nbcd {
                dst: AddressingMode::AbsoluteLong
            })
        );

        // Invalid NBCD modes
        // NBCD A0 (0x4808)
        assert_eq!(
            decode(0x4808),
            Instruction::System(SystemInstruction::Unimplemented { opcode: 0x4808 })
        );
        // NBCD #<data> (0x483C)
        assert_eq!(
            decode(0x483C),
            Instruction::System(SystemInstruction::Unimplemented { opcode: 0x483C })
        );
        // NBCD d16(PC) (0x483A)
        assert_eq!(
            decode(0x483A),
            Instruction::System(SystemInstruction::Unimplemented { opcode: 0x483A })
        );
        // NBCD d8(PC,Xn) (0x483B)
        assert_eq!(
            decode(0x483B),
            Instruction::System(SystemInstruction::Unimplemented { opcode: 0x483B })
        );
    }

    #[test]
    fn test_decode_groups_dispatch() {
        // Group 0: ORI.B #<data>, D0 (0x0000)
        assert!(matches!(
            decode(0x0000),
            Instruction::Bits(BitsInstruction::OrI { .. })
        ));
        // Group 1: MOVE.B D0, D1 (0x1200)
        assert!(matches!(
            decode(0x1200),
            Instruction::Data(DataInstruction::Move {
                size: Size::Byte,
                ..
            })
        ));
        // Group 2: MOVE.L D0, D1 (0x2200)
        assert!(matches!(
            decode(0x2200),
            Instruction::Data(DataInstruction::Move {
                size: Size::Long,
                ..
            })
        ));
        // Group 3: MOVE.W D0, D1 (0x3200)
        assert!(matches!(
            decode(0x3200),
            Instruction::Data(DataInstruction::Move {
                size: Size::Word,
                ..
            })
        ));
        // Group 4: CLR.W D0 (0x4240)
        assert!(matches!(
            decode(0x4240),
            Instruction::Data(DataInstruction::Clr { .. })
        ));
        // Group 5: ADDQ.W #1, D0 (0x5240)
        assert!(matches!(
            decode(0x5240),
            Instruction::Arithmetic(ArithmeticInstruction::AddQ { .. })
        ));
        // Group 6: BRA <disp> (0x6000)
        assert!(matches!(
            decode(0x6000),
            Instruction::System(SystemInstruction::Bra { .. })
        ));
        // Group 7: MOVEQ #0, D0 (0x7000)
        assert!(matches!(
            decode(0x7000),
            Instruction::Data(DataInstruction::MoveQ { .. })
        ));
        // Group 8: DIVU.W D1, D0 (0x80C1)
        assert!(matches!(
            decode(0x80C1),
            Instruction::Arithmetic(ArithmeticInstruction::DivU { .. })
        ));
        // Group 9: SUB.W D0, D1 (0x9240)
        assert!(matches!(
            decode(0x9240),
            Instruction::Arithmetic(ArithmeticInstruction::Sub { .. })
        ));
        // Group A: Line A (0xA000)
        assert!(matches!(
            decode(0xA000),
            Instruction::System(SystemInstruction::LineA { .. })
        ));
        // Group B: CMP.W D0, D1 (0xB240)
        assert!(matches!(
            decode(0xB240),
            Instruction::Arithmetic(ArithmeticInstruction::Cmp { .. })
        ));
        // Group C: AND.W D0, D1 (0xC240)
        assert!(matches!(
            decode(0xC240),
            Instruction::Bits(BitsInstruction::And { .. })
        ));
        // Group D: ADD.W D0, D1 (0xD240)
        assert!(matches!(
            decode(0xD240),
            Instruction::Arithmetic(ArithmeticInstruction::Add { .. })
        ));
        // Group E: ASL.W #1, D0 (0xE340)
        assert!(matches!(
            decode(0xE340),
            Instruction::Bits(BitsInstruction::Asl { .. })
        ));
        // Group F: Line F (0xF000)
        assert!(matches!(
            decode(0xF000),
            Instruction::System(SystemInstruction::LineF { .. })
        ));
    }

    #[test]
    fn test_decode_bit_dynamic() {
        // BTST D1, D0
        // Opcode: 0000 001 1 00 000 000 (0x0300)
        assert_eq!(
            decode(0x0300),
            Instruction::Bits(BitsInstruction::Btst {
                bit: BitSource::Register(1),
                dst: AddressingMode::DataRegister(0),
            })
        );

        // BCHG D2, (A0)
        // Opcode: 0000 010 1 01 010 000 (0x0550)
        assert_eq!(
            decode(0x0550),
            Instruction::Bits(BitsInstruction::Bchg {
                bit: BitSource::Register(2),
                dst: AddressingMode::AddressIndirect(0),
            })
        );

        // BCLR D3, (A1)+
        // Opcode: 0000 011 1 10 011 001 (0x0799)
        assert_eq!(
            decode(0x0799),
            Instruction::Bits(BitsInstruction::Bclr {
                bit: BitSource::Register(3),
                dst: AddressingMode::AddressPostIncrement(1),
            })
        );

        // BSET D4, -(A2)
        // Opcode: 0000 100 1 11 100 010 (0x09E2)
        assert_eq!(
            decode(0x09E2),
            Instruction::Bits(BitsInstruction::Bset {
                bit: BitSource::Register(4),
                dst: AddressingMode::AddressPreDecrement(2),
            })
        );

        // BTST D0, d16(A3)
        // Opcode: 0000 000 1 00 101 011 (0x012B)
        assert_eq!(
            decode(0x012B),
            Instruction::Bits(BitsInstruction::Btst {
                bit: BitSource::Register(0),
                dst: AddressingMode::AddressDisplacement(3),
            })
        );
    }
}
