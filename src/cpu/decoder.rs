//! M68k Instruction Decoder
//!
//! Converts 16-bit opcodes into `Instruction` enums with addressing modes.

use super::addressing::AddressingMode;
use super::ops::{Instruction, ShiftCount};
use super::Size;

/// M68k Condition Codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Condition {
    True,
    False,
    High,
    LowOrSame,
    CarryClear,
    CarrySet,
    NotEqual,
    Equal,
    OverflowClear,
    OverflowSet,
    Plus,
    Minus,
    GreaterThanOrEqual,
    LessThan,
    GreaterThan,
    LessThanOrEqual,
}

impl Condition {
    /// Decode 4-bit condition code from opcode
    pub fn from_bits(bits: u8) -> Self {
        match bits & 0x0F {
            0b0000 => Condition::True,
            0b0001 => Condition::False,
            0b0010 => Condition::High,
            0b0011 => Condition::LowOrSame,
            0b0100 => Condition::CarryClear,
            0b0101 => Condition::CarrySet,
            0b0110 => Condition::NotEqual,
            0b0111 => Condition::Equal,
            0b1000 => Condition::OverflowClear,
            0b1001 => Condition::OverflowSet,
            0b1010 => Condition::Plus,
            0b1011 => Condition::Minus,
            0b1100 => Condition::GreaterThanOrEqual,
            0b1101 => Condition::LessThan,
            0b1110 => Condition::GreaterThan,
            0b1111 => Condition::LessThanOrEqual,
            _ => unreachable!(),
        }
    }

    /// Returns the mnemonic for this condition
    pub fn mnemonic(&self) -> &'static str {
        match self {
            Condition::True => "T",
            Condition::False => "F",
            Condition::High => "HI",
            Condition::LowOrSame => "LS",
            Condition::CarryClear => "CC", // or HS
            Condition::CarrySet => "CS",   // or LO
            Condition::NotEqual => "NE",
            Condition::Equal => "EQ",
            Condition::OverflowClear => "VC",
            Condition::OverflowSet => "VS",
            Condition::Plus => "PL",
            Condition::Minus => "MI",
            Condition::GreaterThanOrEqual => "GE",
            Condition::LessThan => "LT",
            Condition::GreaterThan => "GT",
            Condition::LessThanOrEqual => "LE",
        }
    }
}

pub fn decode(opcode: u16) -> Instruction {
    match (opcode >> 12) & 0x0F {
        0x0 => decode_0(opcode),
        0x1 => decode_move(opcode, Size::Byte),
        0x2 => decode_move(opcode, Size::Long),
        0x3 => decode_move(opcode, Size::Word),
        0x4 => decode_4(opcode),
        0x5 => decode_addq_subq_scc_dbcc(opcode),
        0x6 => decode_branch(opcode),
        0x7 => decode_moveq(opcode),
        0x8 => decode_or_div(opcode),
        0x9 => decode_sub_subx(opcode),
        0xB => decode_eor_cmp(opcode),
        0xC => decode_and_mul_abcd_exg(opcode),
        0xD => decode_add_addx(opcode),
        0xE => decode_shifts(opcode),
        _ => Instruction::Unimplemented { opcode },
    }
}

fn decode_0(opcode: u16) -> Instruction {
    let bit_op = (opcode >> 6) & 0x03;
    let mode = (opcode >> 3) & 0x07;
    let reg = (opcode & 0x07) as u8;

    if (opcode & 0x0100) != 0 {
        // BTST, BCHG, BCLR, BSET (dynamic)
        let bit_reg = ((opcode >> 9) & 0x07) as u8;
        let dst = AddressingMode::from_mode_reg(mode as u8, reg).unwrap();
        match bit_op {
            0 => Instruction::Btst {
                bit: super::ops::BitOp::Register(bit_reg),
                dst,
            },
            1 => Instruction::Bchg {
                bit: super::ops::BitOp::Register(bit_reg),
                dst,
            },
            2 => Instruction::Bclr {
                bit: super::ops::BitOp::Register(bit_reg),
                dst,
            },
            3 => Instruction::Bset {
                bit: super::ops::BitOp::Register(bit_reg),
                dst,
            },
            _ => unreachable!(),
        }
    } else {
        match bit_op {
            0 => {
                // Bitwise Immediate or Static Bit Op
                let size = match (opcode >> 6) & 0x03 {
                    0 => Size::Byte,
                    1 => Size::Word,
                    2 => Size::Long,
                    _ => return Instruction::Unimplemented { opcode },
                };
                let dst = AddressingMode::from_mode_reg(mode as u8, reg).unwrap();
                // BTST, BCHG, BCLR, BSET (static) handled by 0x0800 elsewhere?
                // This match is simplified
                Instruction::Unimplemented { opcode }
            }
            _ => Instruction::Unimplemented { opcode },
        }
    }
}

fn decode_move(opcode: u16, size: Size) -> Instruction {
    let src_mode = (opcode >> 3) & 0x07;
    let src_reg = opcode & 0x07;
    let dst_reg = (opcode >> 9) & 0x07;
    let dst_mode = (opcode >> 6) & 0x07;

    if let (Some(src), Some(dst)) = (
        AddressingMode::from_mode_reg(src_mode as u8, src_reg as u8),
        AddressingMode::from_mode_reg(dst_mode as u8, dst_reg as u8),
    ) {
        Instruction::Move { size, src, dst }
    } else {
        Instruction::Unimplemented { opcode }
    }
}

fn decode_4(opcode: u16) -> Instruction {
    match opcode & 0xFFC0 {
        0x4E70 => Instruction::Reset,
        0x4E71 => Instruction::Nop,
        0x4E72 => Instruction::Stop {
            imm: 0, // Should fetch next word
        },
        0x4E73 => Instruction::Rte,
        0x4E75 => Instruction::Rts,
        0x4E76 => Instruction::Trapv,
        0x4E77 => Instruction::Rtr,
        _ => {
            if (opcode & 0xFF00) == 0x4E00 {
                // JSR, JMP
                let mode = ((opcode >> 3) & 0x07) as u8;
                let reg = (opcode & 0x07) as u8;
                if let Some(dst) = AddressingMode::from_mode_reg(mode, reg) {
                    if (opcode & 0x0040) != 0 {
                        Instruction::Jmp { dst }
                    } else {
                        Instruction::Jsr { dst }
                    }
                } else {
                    Instruction::Unimplemented { opcode }
                }
            } else {
                Instruction::Unimplemented { opcode }
            }
        }
    }
}

fn decode_addq_subq_scc_dbcc(opcode: u16) -> Instruction {
    let data = ((opcode >> 9) & 0x07) as u8;
    let val = if data == 0 { 8 } else { data };
    let size_bits = (opcode >> 6) & 0x03;
    let mode = ((opcode >> 3) & 0x07) as u8;
    let reg = (opcode & 0x07) as u8;

    if size_bits == 0x3 {
        // Scc or DBcc
        Instruction::Unimplemented { opcode }
    } else {
        let size = Size::from_bits(size_bits as u8).unwrap();
        let dst = AddressingMode::from_mode_reg(mode, reg).unwrap();
        if (opcode & 0x0100) != 0 {
            Instruction::Subq { size, val, dst }
        } else {
            Instruction::Addq { size, val, dst }
        }
    }
}

fn decode_branch(opcode: u16) -> Instruction {
    let cond = ((opcode >> 8) & 0x0F) as u8;
    let disp = (opcode & 0xFF) as i8;
    Instruction::Bcc { cond, disp }
}

fn decode_moveq(opcode: u16) -> Instruction {
    let reg = ((opcode >> 9) & 0x07) as u8;
    let val = (opcode & 0xFF) as i8;
    Instruction::Moveq { reg, val }
}

fn decode_or_div(opcode: u16) -> Instruction {
    // Simplified
    Instruction::Unimplemented { opcode }
}

fn decode_sub_subx(opcode: u16) -> Instruction {
    // Simplified
    Instruction::Unimplemented { opcode }
}

fn decode_eor_cmp(opcode: u16) -> Instruction {
    // Simplified
    Instruction::Unimplemented { opcode }
}

fn decode_and_mul_abcd_exg(opcode: u16) -> Instruction {
    // Simplified
    Instruction::Unimplemented { opcode }
}

fn decode_add_addx(opcode: u16) -> Instruction {
    // Simplified
    Instruction::Unimplemented { opcode }
}

fn make_shift_instruction(
    op_type: u8,
    direction: bool,
    size: Size,
    dst: AddressingMode,
    count: ShiftCount,
) -> Instruction {
    match (op_type, direction) {
        (0b00, false) => Instruction::Asr { size, dst, count },
        (0b00, true) => Instruction::Asl { size, dst, count },
        (0b01, false) => Instruction::Lsr { size, dst, count },
        (0b01, true) => Instruction::Lsl { size, dst, count },
        (0b10, false) => Instruction::Roxr { size, dst, count },
        (0b10, true) => Instruction::Roxl { size, dst, count },
        (0b11, false) => Instruction::Ror { size, dst, count },
        (0b11, true) => Instruction::Rol { size, dst, count },
        _ => unreachable!(), // op_type is 2 bits
    }
}

fn decode_shifts(opcode: u16) -> Instruction {
    // ASL, ASR, LSL, LSR, ROL, ROR, ROXL, ROXR
    let count_or_reg = ((opcode >> 9) & 0x07) as u8;
    let direction = (opcode >> 8) & 0x01 != 0; // 0 = right, 1 = left
    let size_bits = (opcode >> 6) & 0x03;
    let ir = (opcode >> 5) & 0x01 != 0; // 0 = immediate, 1 = register
    let op_type = ((opcode >> 3) & 0x03) as u8;
    let reg = (opcode & 0x07) as u8;

    // Memory shifts (size = 0b11)
    if size_bits == 0b11 {
        let ea_mode = ((opcode >> 3) & 0x07) as u8;
        let ea_reg = (opcode & 0x07) as u8;
        if let Some(dst) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
            let count = ShiftCount::Immediate(1); // Memory shifts are always by 1
            return make_shift_instruction(op_type, direction, Size::Word, dst, count);
        }
    }

    // Register shifts
    if let Some(size) = Size::from_bits(size_bits as u8) {
        let count = if ir {
            ShiftCount::Register(count_or_reg)
        } else {
            let imm = if count_or_reg == 0 { 8 } else { count_or_reg };
            ShiftCount::Immediate(imm)
        };
        let dst = AddressingMode::DataRegister(reg);

        return make_shift_instruction(op_type, direction, size, dst, count);
    }

    Instruction::Unimplemented { opcode }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_move() {
        // MOVE.W D0, D1 -> 0x3200
        let instr = decode(0x3200);
        match instr {
            Instruction::Move { size, src, dst } => {
                assert_eq!(size, Size::Word);
                assert!(matches!(src, AddressingMode::DataRegister(0)));
                assert!(matches!(dst, AddressingMode::DataRegister(1)));
            }
            _ => panic!("Expected Move, got {:?}", instr),
        }
    }

    #[test]
    fn test_decode_shifts() {
        // ASL.W #1, D0 -> 0xE340
        // op_type=00, ir=0, count=1, size=01, reg=0, dir=1
        let instr = decode(0xE340);
        match instr {
            Instruction::Asl { size, dst, count } => {
                assert_eq!(size, Size::Word);
                assert!(matches!(dst, AddressingMode::DataRegister(0)));
                assert!(matches!(count, ShiftCount::Immediate(1)));
            }
            _ => panic!("Expected ASL, got {:?}", instr),
        }

        // LSR.L D1, D2 -> 0xE3AC
        // op_type=01, ir=1, reg_count=1, size=10, reg=2, dir=0
        let instr = decode(0xE3AC);
        match instr {
            Instruction::Lsr { size, dst, count } => {
                assert_eq!(size, Size::Long);
                assert!(matches!(dst, AddressingMode::DataRegister(2)));
                assert!(matches!(count, ShiftCount::Register(1)));
            }
            _ => panic!("Expected LSR, got {:?}", instr),
        }
    }
}
