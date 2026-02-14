//! M68k Instruction Decoder
//!
//! This module decodes M68k opcodes into instruction representations
//! that can be executed by the CPU.

use std::fmt;

/// Size specifier for M68k instructions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    Byte, // .B - 8 bits
    Word, // .W - 16 bits
    Long, // .L - 32 bits
}

impl Size {
    /// Decode size from the common 2-bit field (bits 7-6)
    pub fn from_bits(bits: u8) -> Option<Self> {
        match bits & 0x03 {
            0b00 => Some(Size::Byte),
            0b01 => Some(Size::Word),
            0b10 => Some(Size::Long),
            _ => None, // 0b11 is typically invalid or used for address register
        }
    }

    /// Decode size from move instruction size field (bits 13-12)
    pub fn from_move_bits(bits: u8) -> Option<Self> {
        match bits & 0x03 {
            0b01 => Some(Size::Byte),
            0b11 => Some(Size::Word),
            0b10 => Some(Size::Long),
            _ => None, // 0b00 is invalid for MOVE
        }
    }

    /// Returns the size in bytes
    pub fn bytes(self) -> u32 {
        match self {
            Size::Byte => 1,
            Size::Word => 2,
            Size::Long => 4,
        }
    }

    /// Returns the bitmask for this size
    pub fn mask(self) -> u32 {
        match self {
            Size::Byte => 0xFF,
            Size::Word => 0xFFFF,
            Size::Long => 0xFFFFFFFF,
        }
    }

    /// Apply this size to a 32-bit value (keeping higher bits of old value)
    pub fn apply(self, old: u32, new: u32) -> u32 {
        let mask = self.mask();
        (old & !mask) | (new & mask)
    }

    /// Check if a value is negative for this size
    pub fn is_negative(self, val: u32) -> bool {
        (val & self.sign_bit()) != 0
    }

    /// Returns the sign bit (MSB) for this size
    pub fn sign_bit(self) -> u32 {
        match self {
            Size::Byte => 0x80,
            Size::Word => 0x8000,
            Size::Long => 0x80000000,
        }
    }

    /// Returns the number of bits for this size
    pub fn bits(self) -> u32 {
        match self {
            Size::Byte => 8,
            Size::Word => 16,
            Size::Long => 32,
        }
    }
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Size::Byte => write!(f, ".B"),
            Size::Word => write!(f, ".W"),
            Size::Long => write!(f, ".L"),
        }
    }
}

/// M68k Addressing Mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressingMode {
    /// Dn - Data Register Direct
    DataRegister(u8),
    /// An - Address Register Direct
    AddressRegister(u8),
    /// (An) - Address Register Indirect
    AddressIndirect(u8),
    /// (An)+ - Address Register Indirect with Postincrement
    AddressPostIncrement(u8),
    /// -(An) - Address Register Indirect with Predecrement
    AddressPreDecrement(u8),
    /// d16(An) - Address Register Indirect with Displacement
    AddressDisplacement(u8),
    /// d8(An,Xn) - Address Register Indirect with Index
    AddressIndex(u8),
    /// (xxx).W - Absolute Short
    AbsoluteShort,
    /// (xxx).L - Absolute Long
    AbsoluteLong,
    /// d16(PC) - Program Counter with Displacement
    PcDisplacement,
    /// d8(PC,Xn) - Program Counter with Index
    PcIndex,
    /// #<data> - Immediate
    Immediate,
}

impl AddressingMode {
    /// Decode addressing mode from mode (3 bits) and register (3 bits) fields
    pub fn from_mode_reg(mode: u8, reg: u8) -> Option<Self> {
        match mode & 0x07 {
            0b000 => Some(AddressingMode::DataRegister(reg & 0x07)),
            0b001 => Some(AddressingMode::AddressRegister(reg & 0x07)),
            0b010 => Some(AddressingMode::AddressIndirect(reg & 0x07)),
            0b011 => Some(AddressingMode::AddressPostIncrement(reg & 0x07)),
            0b100 => Some(AddressingMode::AddressPreDecrement(reg & 0x07)),
            0b101 => Some(AddressingMode::AddressDisplacement(reg & 0x07)),
            0b110 => Some(AddressingMode::AddressIndex(reg & 0x07)),
            0b111 => match reg & 0x07 {
                0b000 => Some(AddressingMode::AbsoluteShort),
                0b001 => Some(AddressingMode::AbsoluteLong),
                0b010 => Some(AddressingMode::PcDisplacement),
                0b011 => Some(AddressingMode::PcIndex),
                0b100 => Some(AddressingMode::Immediate),
                _ => None,
            },
            _ => None,
        }
    }

    /// Returns true if this mode is valid as a destination
    pub fn is_valid_destination(&self) -> bool {
        !matches!(
            self,
            AddressingMode::PcDisplacement | AddressingMode::PcIndex | AddressingMode::Immediate
        )
    }

    /// Returns the number of extension words needed for this addressing mode
    pub fn extension_words(&self, size: Size) -> u32 {
        match self {
            AddressingMode::DataRegister(_) | AddressingMode::AddressRegister(_) => 0,
            AddressingMode::AddressIndirect(_)
            | AddressingMode::AddressPostIncrement(_)
            | AddressingMode::AddressPreDecrement(_) => 0,
            AddressingMode::AddressDisplacement(_) | AddressingMode::PcDisplacement => 1,
            AddressingMode::AddressIndex(_) | AddressingMode::PcIndex => 1,
            AddressingMode::AbsoluteShort => 1,
            AddressingMode::AbsoluteLong => 2,
            AddressingMode::Immediate => match size {
                Size::Byte | Size::Word => 1,
                Size::Long => 2,
            },
        }
    }
}

impl fmt::Display for AddressingMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AddressingMode::DataRegister(r) => write!(f, "D{}", r),
            AddressingMode::AddressRegister(r) => write!(f, "A{}", r),
            AddressingMode::AddressIndirect(r) => write!(f, "(A{})", r),
            AddressingMode::AddressPostIncrement(r) => write!(f, "(A{})+", r),
            AddressingMode::AddressPreDecrement(r) => write!(f, "-(A{})", r),
            AddressingMode::AddressDisplacement(r) => write!(f, "d16(A{})", r),
            AddressingMode::AddressIndex(r) => write!(f, "d8(A{},Xn)", r),
            AddressingMode::AbsoluteShort => write!(f, "(xxx).W"),
            AddressingMode::AbsoluteLong => write!(f, "(xxx).L"),
            AddressingMode::PcDisplacement => write!(f, "d16(PC)"),
            AddressingMode::PcIndex => write!(f, "d8(PC,Xn)"),
            AddressingMode::Immediate => write!(f, "#<data>"),
        }
    }
}

/// Condition codes for Bcc/Scc/DBcc instructions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Condition {
    True,           // T  - Always true
    False,          // F  - Always false
    High,           // HI - Higher (unsigned)
    LowOrSame,      // LS - Lower or Same (unsigned)
    CarryClear,     // CC - Carry Clear (HI for unsigned)
    CarrySet,       // CS - Carry Set (LO for unsigned)
    NotEqual,       // NE - Not Equal
    Equal,          // EQ - Equal
    OverflowClear,  // VC - Overflow Clear
    OverflowSet,    // VS - Overflow Set
    Plus,           // PL - Plus (positive)
    Minus,          // MI - Minus (negative)
    GreaterOrEqual, // GE - Greater or Equal (signed)
    LessThan,       // LT - Less Than (signed)
    GreaterThan,    // GT - Greater Than (signed)
    LessOrEqual,    // LE - Less or Equal (signed)
}

impl Condition {
    /// Decode condition from 4-bit field
    pub fn from_bits(bits: u8) -> Self {
        match bits & 0x0F {
            0x0 => Condition::True,
            0x1 => Condition::False,
            0x2 => Condition::High,
            0x3 => Condition::LowOrSame,
            0x4 => Condition::CarryClear,
            0x5 => Condition::CarrySet,
            0x6 => Condition::NotEqual,
            0x7 => Condition::Equal,
            0x8 => Condition::OverflowClear,
            0x9 => Condition::OverflowSet,
            0xA => Condition::Plus,
            0xB => Condition::Minus,
            0xC => Condition::GreaterOrEqual,
            0xD => Condition::LessThan,
            0xE => Condition::GreaterThan,
            0xF => Condition::LessOrEqual,
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
            Condition::CarryClear => "CC",
            Condition::CarrySet => "CS",
            Condition::NotEqual => "NE",
            Condition::Equal => "EQ",
            Condition::OverflowClear => "VC",
            Condition::OverflowSet => "VS",
            Condition::Plus => "PL",
            Condition::Minus => "MI",
            Condition::GreaterOrEqual => "GE",
            Condition::LessThan => "LT",
            Condition::GreaterThan => "GT",
            Condition::LessOrEqual => "LE",
        }
    }
}

/// Decoded M68k instruction
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    // Data Movement
    Move {
        size: Size,
        src: AddressingMode,
        dst: AddressingMode,
    },
    MoveA {
        size: Size,
        src: AddressingMode,
        dst_reg: u8,
    },
    MoveQ {
        dst_reg: u8,
        data: i8,
    },
    Lea {
        src: AddressingMode,
        dst_reg: u8,
    },
    Pea {
        src: AddressingMode,
    },
    Clr {
        size: Size,
        dst: AddressingMode,
    },
    Exg {
        rx: u8,
        ry: u8,
        mode: u8,
    },
    Movep {
        size: Size,
        reg: u8,
        an: u8,
        direction: bool,
    },

    // Arithmetic
    Add {
        size: Size,
        src: AddressingMode,
        dst: AddressingMode,
        direction: bool,
    },
    AddA {
        size: Size,
        src: AddressingMode,
        dst_reg: u8,
    },
    AddI {
        size: Size,
        dst: AddressingMode,
    },
    AddQ {
        size: Size,
        dst: AddressingMode,
        data: u8,
    },
    Sub {
        size: Size,
        src: AddressingMode,
        dst: AddressingMode,
        direction: bool,
    },
    SubA {
        size: Size,
        src: AddressingMode,
        dst_reg: u8,
    },
    SubI {
        size: Size,
        dst: AddressingMode,
    },
    SubQ {
        size: Size,
        dst: AddressingMode,
        data: u8,
    },
    MulU {
        src: AddressingMode,
        dst_reg: u8,
    },
    MulS {
        src: AddressingMode,
        dst_reg: u8,
    },
    DivU {
        src: AddressingMode,
        dst_reg: u8,
    },
    DivS {
        src: AddressingMode,
        dst_reg: u8,
    },
    Neg {
        size: Size,
        dst: AddressingMode,
    },
    Ext {
        size: Size,
        reg: u8,
    },
    Abcd {
        src_reg: u8,
        dst_reg: u8,
        memory_mode: bool,
    },
    Sbcd {
        src_reg: u8,
        dst_reg: u8,
        memory_mode: bool,
    },
    Nbcd {
        dst: AddressingMode,
    },
    AddX {
        size: Size,
        src_reg: u8,
        dst_reg: u8,
        memory_mode: bool,
    },
    SubX {
        size: Size,
        src_reg: u8,
        dst_reg: u8,
        memory_mode: bool,
    },
    NegX {
        size: Size,
        dst: AddressingMode,
    },
    Chk {
        src: AddressingMode,
        dst_reg: u8,
    },
    Tas {
        dst: AddressingMode,
    },
    Movem {
        size: Size,
        direction: bool,
        mask: u16,
        ea: AddressingMode,
    },

    // Logical
    And {
        size: Size,
        src: AddressingMode,
        dst: AddressingMode,
        direction: bool,
    },
    AndI {
        size: Size,
        dst: AddressingMode,
    },
    Or {
        size: Size,
        src: AddressingMode,
        dst: AddressingMode,
        direction: bool,
    },
    OrI {
        size: Size,
        dst: AddressingMode,
    },
    Eor {
        size: Size,
        src_reg: u8,
        dst: AddressingMode,
    },
    EorI {
        size: Size,
        dst: AddressingMode,
    },
    Not {
        size: Size,
        dst: AddressingMode,
    },

    // Shifts and Rotates
    Lsl {
        size: Size,
        dst: AddressingMode,
        count: ShiftCount,
    },
    Lsr {
        size: Size,
        dst: AddressingMode,
        count: ShiftCount,
    },
    Asl {
        size: Size,
        dst: AddressingMode,
        count: ShiftCount,
    },
    Asr {
        size: Size,
        dst: AddressingMode,
        count: ShiftCount,
    },
    Rol {
        size: Size,
        dst: AddressingMode,
        count: ShiftCount,
    },
    Ror {
        size: Size,
        dst: AddressingMode,
        count: ShiftCount,
    },
    Roxl {
        size: Size,
        dst: AddressingMode,
        count: ShiftCount,
    },
    Roxr {
        size: Size,
        dst: AddressingMode,
        count: ShiftCount,
    },

    // Bit Manipulation
    Btst {
        bit: BitSource,
        dst: AddressingMode,
    },
    Bset {
        bit: BitSource,
        dst: AddressingMode,
    },
    Bclr {
        bit: BitSource,
        dst: AddressingMode,
    },
    Bchg {
        bit: BitSource,
        dst: AddressingMode,
    },

    // Compare and Test
    Cmp {
        size: Size,
        src: AddressingMode,
        dst_reg: u8,
    },
    CmpA {
        size: Size,
        src: AddressingMode,
        dst_reg: u8,
    },
    CmpI {
        size: Size,
        dst: AddressingMode,
    },
    CmpM {
        size: Size,
        ax: u8,
        ay: u8,
    },
    Tst {
        size: Size,
        dst: AddressingMode,
    },

    // Branch and Jump
    Bra {
        displacement: i16,
    },
    Bsr {
        displacement: i16,
    },
    Bcc {
        condition: Condition,
        displacement: i16,
    },
    Scc {
        condition: Condition,
        dst: AddressingMode,
    },
    DBcc {
        condition: Condition,
        reg: u8,
    },
    Jmp {
        dst: AddressingMode,
    },
    Jsr {
        dst: AddressingMode,
    },
    Rts,
    Rte,
    Rtr,

    // Misc
    Nop,
    Reset,
    Stop,
    MoveUsp {
        reg: u8,
        to_usp: bool,
    },
    Trap {
        vector: u8,
    },
    TrapV,
    Link {
        reg: u8,
    },
    Unlk {
        reg: u8,
    },
    Swap {
        reg: u8,
    },

    // Status Register
    MoveToSr {
        src: AddressingMode,
    },
    MoveFromSr {
        dst: AddressingMode,
    },
    MoveToCcr {
        src: AddressingMode,
    },
    AndiToCcr,
    AndiToSr,
    OriToCcr,
    OriToSr,
    EoriToCcr,
    EoriToSr,

    // Illegal/Unimplemented
    Illegal,
    LineA {
        opcode: u16,
    },
    LineF {
        opcode: u16,
    },
    Unimplemented {
        opcode: u16,
    },
}

/// Shift count source for shift instructions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShiftCount {
    Immediate(u8), // 1-8 (0 encodes 8)
    Register(u8),  // Value in Dn
}

/// Bit source for bit manipulation instructions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitSource {
    Immediate,    // Bit number in extension word
    Register(u8), // Bit number in Dn
}

/// Decode a single M68k instruction from an opcode
pub fn decode(opcode: u16) -> Instruction {
    let group = ((opcode >> 12) & 0x0F) as usize;
    GROUP_DECODERS[group](opcode)
}

// === Group decoders ===

type DecoderFn = fn(u16) -> Instruction;

const GROUP_DECODERS: [DecoderFn; 16] = [
    decode_group_0,
    decode_move_byte,
    decode_move_long,
    decode_move_word,
    decode_group_4,
    decode_group_5,
    decode_group_6,
    decode_moveq,
    decode_group_8,
    decode_sub,
    decode_line_a,
    decode_group_b,
    decode_group_c,
    decode_add,
    decode_shifts,
    decode_line_f,
];

fn decode_line_a(opcode: u16) -> Instruction {
    Instruction::LineA { opcode }
}

fn decode_line_f(opcode: u16) -> Instruction {
    Instruction::LineF { opcode }
}

fn decode_group_0(opcode: u16) -> Instruction {
    decode_movep(opcode)
        .or_else(|| decode_bit_dynamic(opcode))
        .or_else(|| decode_immediate_and_static_bit(opcode))
        .unwrap_or(Instruction::Unimplemented { opcode })
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
            return Some(Instruction::Movep {
                size,
                reg,
                an,
                direction,
            });
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
                0b00 => Instruction::Btst { bit, dst },
                0b01 => Instruction::Bchg { bit, dst },
                0b10 => Instruction::Bclr { bit, dst },
                0b11 => Instruction::Bset { bit, dst },
                _ => unreachable!(),
            });
        }
    }
    None
}

fn decode_immediate_and_static_bit(opcode: u16) -> Option<Instruction> {
    // Check for immediate operations and Static Bit Ops
    let bit8 = (opcode >> 8) & 0x01;
    if bit8 == 0 {
        let op = (opcode >> 9) & 0x07;
        let mode = ((opcode >> 3) & 0x07) as u8;
        let reg = (opcode & 0x07) as u8;

        // Static Bit Instructions (Op 4)
        if op == 0b100 {
            if let Some(dst) = AddressingMode::from_mode_reg(mode, reg) {
                let bit_op = (opcode >> 6) & 0x03;
                let bit = BitSource::Immediate;
                return Some(match bit_op {
                    0b00 => Instruction::Btst { bit, dst },
                    0b01 => Instruction::Bchg { bit, dst },
                    0b10 => Instruction::Bclr { bit, dst },
                    0b11 => Instruction::Bset { bit, dst },
                    _ => unreachable!(),
                });
            }
        }

        // CCR/SR Immediate Operations - Special case when mode=7, reg=4 (immediate)
        // 0000 000 0 00 111 100 = ORI to CCR (003C)
        // 0000 000 0 01 111 100 = ORI to SR  (007C)
        // 0000 001 0 00 111 100 = ANDI to CCR (023C)
        // 0000 001 0 01 111 100 = ANDI to SR  (027C)
        // 0000 101 0 00 111 100 = EORI to CCR (0A3C)
        // 0000 101 0 01 111 100 = EORI to SR  (0A7C)
        if mode == 7 && reg == 4 {
            let size_bits = ((opcode >> 6) & 0x03) as u8;
            return Some(match (op, size_bits) {
                (0b000, 0b00) => Instruction::OriToCcr,
                (0b000, 0b01) => Instruction::OriToSr,
                (0b001, 0b00) => Instruction::AndiToCcr,
                (0b001, 0b01) => Instruction::AndiToSr,
                (0b101, 0b00) => Instruction::EoriToCcr,
                (0b101, 0b01) => Instruction::EoriToSr,
                _ => Instruction::Unimplemented { opcode },
            });
        }

        // Immediate Instructions (ORI, ANDI, SUBI, ADDI, EORI, CMPI)
        let size_bits = ((opcode >> 6) & 0x03) as u8;
        if let Some(size) = Size::from_bits(size_bits) {
            if let Some(dst) = AddressingMode::from_mode_reg(mode, reg) {
                return Some(match op {
                    0b000 => Instruction::OrI { size, dst },
                    0b001 => Instruction::AndI { size, dst },
                    0b010 => Instruction::SubI { size, dst },
                    0b011 => Instruction::AddI { size, dst },
                    0b100 => Instruction::Unimplemented { opcode }, // Handled above
                    0b101 => Instruction::EorI { size, dst },
                    0b110 => Instruction::CmpI { size, dst },
                    _ => Instruction::Unimplemented { opcode },
                });
            }
        }
    }
    None
}
fn decode_move_byte(opcode: u16) -> Instruction {
    decode_move(opcode, Size::Byte)
}

fn decode_move_long(opcode: u16) -> Instruction {
    decode_move(opcode, Size::Long)
}

fn decode_move_word(opcode: u16) -> Instruction {
    decode_move(opcode, Size::Word)
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
        None => return Instruction::Unimplemented { opcode },
    };

    // MOVEA has destination mode 001 (address register)
    if dst_mode == 0b001 {
        // MOVEA - size must be word or long
        if size == Size::Byte {
            return Instruction::Unimplemented { opcode };
        }
        return Instruction::MoveA { size, src, dst_reg };
    }

    let dst = match AddressingMode::from_mode_reg(dst_mode, dst_reg) {
        Some(m) => m,
        None => return Instruction::Unimplemented { opcode },
    };

    if !dst.is_valid_destination() {
        return Instruction::Unimplemented { opcode };
    }

    Instruction::Move { size, src, dst }
}

fn decode_group_4(opcode: u16) -> Instruction {
    decode_group_4_misc(opcode)
        .or_else(|| decode_group_4_control(opcode))
        .or_else(|| decode_group_4_movem(opcode))
        .or_else(|| decode_group_4_arithmetic(opcode))
        .unwrap_or(Instruction::Unimplemented { opcode })
}

fn decode_group_4_misc(opcode: u16) -> Option<Instruction> {
    let reg = (opcode & 0x07) as u8;

    // Check for specific instructions first
    match opcode & 0xFFF8 {
        0x4E70 => {
            return Some(match reg {
                0 => Instruction::Reset,
                1 => Instruction::Nop,
                2 => Instruction::Stop,
                3 => Instruction::Rte,
                5 => Instruction::Rts,
                6 => Instruction::TrapV,
                7 => Instruction::Rtr,
                _ => return None,
            })
        }
        0x4E50 => return Some(Instruction::Link { reg }),
        0x4E58 => return Some(Instruction::Unlk { reg }),
        0x4E60 => return Some(Instruction::MoveUsp { reg, to_usp: true }),
        0x4E68 => return Some(Instruction::MoveUsp { reg, to_usp: false }),
        0x4840 => return Some(Instruction::Swap { reg }),
        0x4880 => {
            return Some(Instruction::Ext {
                size: Size::Word,
                reg,
            })
        }
        0x48C0 => {
            return Some(Instruction::Ext {
                size: Size::Long,
                reg,
            })
        }

        _ => {}
    }

    // TRAP
    if opcode & 0xFFF0 == 0x4E40 {
        return Some(Instruction::Trap {
            vector: (opcode & 0x0F) as u8,
        });
    }

    // ILLEGAL - 4AFC
    if opcode == 0x4AFC {
        return Some(Instruction::Illegal);
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
            return Some(Instruction::Lea { src, dst_reg });
        }
    }

    // PEA
    if opcode & 0xFFC0 == 0x4840 && mode != 0 {
        if let Some(src) = AddressingMode::from_mode_reg(mode, reg) {
            return Some(Instruction::Pea { src });
        }
    }

    // JMP
    if opcode & 0xFFC0 == 0x4EC0 {
        if let Some(dst) = AddressingMode::from_mode_reg(mode, reg) {
            return Some(Instruction::Jmp { dst });
        }
    }

    // JSR
    if opcode & 0xFFC0 == 0x4E80 {
        if let Some(dst) = AddressingMode::from_mode_reg(mode, reg) {
            return Some(Instruction::Jsr { dst });
        }
    }

    // CHK - 0100 rrr 1s0 mmm xxx (s=0 word, s=1 long for 68020+)
    // 68000: only word size (bits 7-6 = 10)
    if opcode & 0xF1C0 == 0x4180 {
        let dst_reg = ((opcode >> 9) & 0x07) as u8;
        if let Some(src) = AddressingMode::from_mode_reg(mode, reg) {
            return Some(Instruction::Chk { src, dst_reg });
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
            return Some(Instruction::Movem {
                size,
                direction: to_memory,
                mask: 0,
                ea,
            });
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
            if dst.is_valid_destination()
                && matches!(
                    dst,
                    AddressingMode::DataRegister(_)
                        | AddressingMode::AddressPreDecrement(_)
                        | AddressingMode::AddressDisplacement(_)
                        | AddressingMode::AddressIndex(_)
                        | AddressingMode::AbsoluteShort
                        | AddressingMode::AbsoluteLong
                )
            {
                return Some(Instruction::Nbcd { dst });
            }
            // Nbcd requires data alterable. is_valid_destination checks mostly immediate logic.
            // Check manual: NBCD <ea>. <ea> is Data Alterable.
            // DataRegister, (An), (An)+, -(An), d(An), d(An,xi), xxx.W, xxx.L.
            // An direct is NOT data alterable.
            // My AddressingMode check is approximate.
            // I'll assume valid destination for now if not An.
            if !matches!(
                dst,
                AddressingMode::AddressRegister(_)
                    | AddressingMode::Immediate
                    | AddressingMode::PcDisplacement
                    | AddressingMode::PcIndex
            ) {
                return Some(Instruction::Nbcd { dst });
            }
        }
    }

    // TAS - 0100 1010 11 mmm rrr (4AC0)
    if opcode & 0xFFC0 == 0x4AC0 {
        if let Some(dst) = AddressingMode::from_mode_reg(mode, reg) {
            return Some(Instruction::Tas { dst });
        }
    }

    // CLR, NEG, NOT, TST
    let bits_11_8 = (opcode >> 8) & 0x0F;
    let bits_7_6 = (opcode >> 6) & 0x03;
    if let Some(size) = Size::from_bits(bits_7_6 as u8) {
        if let Some(dst) = AddressingMode::from_mode_reg(mode, reg) {
            match bits_11_8 {
                0x0 => return Some(Instruction::NegX { size, dst }),
                0x2 => return Some(Instruction::Clr { size, dst }),
                0x4 => return Some(Instruction::Neg { size, dst }),
                0x6 => return Some(Instruction::Not { size, dst }),
                0xA => return Some(Instruction::Tst { size, dst }),
                _ => {}
            };
        }
    }

    // MOVE from SR
    if opcode & 0xFFC0 == 0x40C0 {
        if let Some(dst) = AddressingMode::from_mode_reg(mode, reg) {
            return Some(Instruction::MoveFromSr { dst });
        }
    }

    // MOVE to CCR
    if opcode & 0xFFC0 == 0x44C0 {
        if let Some(src) = AddressingMode::from_mode_reg(mode, reg) {
            return Some(Instruction::MoveToCcr { src });
        }
    }

    // MOVE to SR
    if opcode & 0xFFC0 == 0x46C0 {
        if let Some(src) = AddressingMode::from_mode_reg(mode, reg) {
            return Some(Instruction::MoveToSr { src });
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
        return Instruction::DBcc { condition, reg };
    }

    // Scc
    if size_bits == 0b11 && mode != 0b001 {
        let condition = Condition::from_bits(((opcode >> 8) & 0x0F) as u8);
        if let Some(dst) = AddressingMode::from_mode_reg(mode, reg) {
            return Instruction::Scc { condition, dst };
        }
    }

    if let Some(size) = Size::from_bits(size_bits) {
        if let Some(dst) = AddressingMode::from_mode_reg(mode, reg) {
            // Check for illegal Byte operation on Address Register
            if size == Size::Byte && matches!(dst, AddressingMode::AddressRegister(_)) {
                return Instruction::Illegal;
            }

            let is_sub = (opcode >> 8) & 0x01 != 0;
            if is_sub {
                return Instruction::SubQ { size, dst, data };
            } else {
                return Instruction::AddQ { size, dst, data };
            }
        }
    }

    Instruction::Unimplemented { opcode }
}

fn decode_group_6(opcode: u16) -> Instruction {
    // Bcc, BRA, BSR

    let condition_bits = ((opcode >> 8) & 0x0F) as u8;
    let displacement_byte = (opcode & 0xFF) as i8;

    // Displacement of 0 means 16-bit displacement follows
    // Displacement of 0xFF means 32-bit displacement follows (68020+)
    let displacement = displacement_byte as i16;

    match condition_bits {
        0x0 => Instruction::Bra { displacement },
        0x1 => Instruction::Bsr { displacement },
        _ => {
            let condition = Condition::from_bits(condition_bits);
            Instruction::Bcc {
                condition,
                displacement,
            }
        }
    }
}

fn decode_moveq(opcode: u16) -> Instruction {
    // MOVEQ - Move Quick
    // Format: 0111 DDD 0 DDDDDDDD
    // D = destination register, D = 8-bit data

    if opcode & 0x0100 != 0 {
        return Instruction::Unimplemented { opcode };
    }

    let dst_reg = ((opcode >> 9) & 0x07) as u8;
    let data = (opcode & 0xFF) as i8;

    Instruction::MoveQ { dst_reg, data }
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
        return Instruction::Sbcd {
            src_reg: ea_reg,
            dst_reg: reg,
            memory_mode,
        };
    }

    // DIVU
    if size_bits == 0b11 && !direction {
        if let Some(src) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
            return Instruction::DivU { src, dst_reg: reg };
        }
    }

    // DIVS
    if size_bits == 0b11 && direction {
        if let Some(src) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
            return Instruction::DivS { src, dst_reg: reg };
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
            return Instruction::Or {
                size,
                src,
                dst,
                direction,
            };
        }
    }

    Instruction::Unimplemented { opcode }
}

fn decode_sub(opcode: u16) -> Instruction {
    let reg = ((opcode >> 9) & 0x07) as u8;
    let direction = (opcode >> 8) & 0x01 != 0;
    let size_bits = ((opcode >> 6) & 0x03) as u8;
    let ea_mode = ((opcode >> 3) & 0x07) as u8;
    let ea_reg = (opcode & 0x07) as u8;

    // SUBX
    if direction && (opcode & 0x30) == 0 {
        if let Some(size) = Size::from_bits(size_bits) {
            let memory_mode = (opcode & 0x08) != 0;
            return Instruction::SubX {
                size,
                src_reg: ea_reg,
                dst_reg: reg,
                memory_mode,
            };
        }
    }

    // SUBA
    if size_bits == 0b11 {
        let size = if direction { Size::Long } else { Size::Word };
        if let Some(src) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
            return Instruction::SubA {
                size,
                src,
                dst_reg: reg,
            };
        }
    }

    // SUB
    if let Some(size) = Size::from_bits(size_bits) {
        if let Some(ea) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
            let (src, dst) = if direction {
                (AddressingMode::DataRegister(reg), ea)
            } else {
                (ea, AddressingMode::DataRegister(reg))
            };
            return Instruction::Sub {
                size,
                src,
                dst,
                direction,
            };
        }
    }

    Instruction::Unimplemented { opcode }
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
            return Instruction::CmpM {
                size,
                ax: reg,
                ay: ea_reg,
            };
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
            return Instruction::CmpA {
                size,
                src,
                dst_reg: reg,
            };
        }
    }

    // EOR (direction bit set, not CMPA)
    if opmode & 0x04 != 0 && opmode != 0b111 {
        let size_bits = (opmode & 0x03) as u8;
        if let Some(size) = Size::from_bits(size_bits) {
            if let Some(dst) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
                return Instruction::Eor {
                    size,
                    src_reg: reg,
                    dst,
                };
            }
        }
    }

    // CMP
    let size_bits = (opmode & 0x03) as u8;
    if let Some(size) = Size::from_bits(size_bits) {
        if let Some(src) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
            return Instruction::Cmp {
                size,
                src,
                dst_reg: reg,
            };
        }
    }

    Instruction::Unimplemented { opcode }
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
        return Instruction::Abcd {
            src_reg: ea_reg,
            dst_reg: reg,
            memory_mode,
        };
    }

    // MULU
    if size_bits == 0b11 && !direction {
        if let Some(src) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
            return Instruction::MulU { src, dst_reg: reg };
        }
    }

    // MULS
    if size_bits == 0b11 && direction {
        if let Some(src) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
            return Instruction::MulS { src, dst_reg: reg };
        }
    }

    // EXG
    if opcode & 0x0130 == 0x0100 {
        let mode = ((opcode >> 3) & 0x1F) as u8;
        return Instruction::Exg {
            rx: reg,
            ry: ea_reg,
            mode,
        };
    }

    // AND
    if let Some(size) = Size::from_bits(size_bits) {
        if let Some(ea) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
            let (src, dst) = if direction {
                (AddressingMode::DataRegister(reg), ea)
            } else {
                (ea, AddressingMode::DataRegister(reg))
            };
            return Instruction::And {
                size,
                src,
                dst,
                direction,
            };
        }
    }

    Instruction::Unimplemented { opcode }
}

fn decode_add(opcode: u16) -> Instruction {
    let reg = ((opcode >> 9) & 0x07) as u8;
    let direction = (opcode >> 8) & 0x01 != 0;
    let size_bits = ((opcode >> 6) & 0x03) as u8;
    let ea_mode = ((opcode >> 3) & 0x07) as u8;
    let ea_reg = (opcode & 0x07) as u8;

    // ADDX
    if direction && (opcode & 0x30) == 0 {
        if let Some(size) = Size::from_bits(size_bits) {
            let memory_mode = (opcode & 0x08) != 0;
            return Instruction::AddX {
                size,
                src_reg: ea_reg,
                dst_reg: reg,
                memory_mode,
            };
        }
    }

    // ADDA
    if size_bits == 0b11 {
        let size = if direction { Size::Long } else { Size::Word };
        if let Some(src) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
            return Instruction::AddA {
                size,
                src,
                dst_reg: reg,
            };
        }
    }

    // ADD
    if let Some(size) = Size::from_bits(size_bits) {
        if let Some(ea) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
            let (src, dst) = if direction {
                (AddressingMode::DataRegister(reg), ea)
            } else {
                (ea, AddressingMode::DataRegister(reg))
            };
            return Instruction::Add {
                size,
                src,
                dst,
                direction,
            };
        }
    }

    Instruction::Unimplemented { opcode }
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
        let ea_mode = ((opcode >> 3) & 0x07) as u8;
        let ea_reg = (opcode & 0x07) as u8;
        if let Some(dst) = AddressingMode::from_mode_reg(ea_mode, ea_reg) {
            let count = ShiftCount::Immediate(1); // Memory shifts are always by 1
            return match (op_type, direction) {
                (0b00, false) => Instruction::Asr {
                    size: Size::Word,
                    dst,
                    count,
                },
                (0b00, true) => Instruction::Asl {
                    size: Size::Word,
                    dst,
                    count,
                },
                (0b01, false) => Instruction::Lsr {
                    size: Size::Word,
                    dst,
                    count,
                },
                (0b01, true) => Instruction::Lsl {
                    size: Size::Word,
                    dst,
                    count,
                },
                (0b10, false) => Instruction::Roxr {
                    size: Size::Word,
                    dst,
                    count,
                },
                (0b10, true) => Instruction::Roxl {
                    size: Size::Word,
                    dst,
                    count,
                },
                (0b11, false) => Instruction::Ror {
                    size: Size::Word,
                    dst,
                    count,
                },
                (0b11, true) => Instruction::Rol {
                    size: Size::Word,
                    dst,
                    count,
                },
                _ => Instruction::Unimplemented { opcode },
            };
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

        return match (op_type, direction) {
            (0b00, false) => Instruction::Asr { size, dst, count },
            (0b00, true) => Instruction::Asl { size, dst, count },
            (0b01, false) => Instruction::Lsr { size, dst, count },
            (0b01, true) => Instruction::Lsl { size, dst, count },
            (0b10, false) => Instruction::Roxr { size, dst, count },
            (0b10, true) => Instruction::Roxl { size, dst, count },
            (0b11, false) => Instruction::Ror { size, dst, count },
            (0b11, true) => Instruction::Rol { size, dst, count },
            _ => Instruction::Unimplemented { opcode },
        };
    }

    Instruction::Unimplemented { opcode }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_nop() {
        assert_eq!(decode(0x4E71), Instruction::Nop);
    }

    #[test]
    fn test_decode_rts() {
        assert_eq!(decode(0x4E75), Instruction::Rts);
    }

    #[test]
    fn test_decode_move_l_d1_d0() {
        // MOVE.L D1, D0 = 0x2001
        let instr = decode(0x2001);
        assert_eq!(
            instr,
            Instruction::Move {
                size: Size::Long,
                src: AddressingMode::DataRegister(1),
                dst: AddressingMode::DataRegister(0),
            }
        );
    }

    #[test]
    fn test_decode_moveq() {
        // MOVEQ #42, D3
        let instr = decode(0x762A);
        assert_eq!(
            instr,
            Instruction::MoveQ {
                dst_reg: 3,
                data: 42,
            }
        );
    }

    #[test]
    fn test_decode_bra() {
        // BRA with 8-bit displacement
        let instr = decode(0x6010);
        assert_eq!(instr, Instruction::Bra { displacement: 16 });
    }

    #[test]
    fn test_decode_beq() {
        // BEQ with 8-bit displacement
        let instr = decode(0x6708);
        assert_eq!(
            instr,
            Instruction::Bcc {
                condition: Condition::Equal,
                displacement: 8,
            }
        );
    }

    #[test]
    fn test_decode_addq() {
        // ADDQ.L #8, D0
        let instr = decode(0x5080);
        assert_eq!(
            instr,
            Instruction::AddQ {
                size: Size::Long,
                dst: AddressingMode::DataRegister(0),
                data: 8,
            }
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
            Instruction::DivU {
                src: AddressingMode::DataRegister(1),
                dst_reg: 0,
            }
        );

        // DIVS.W D1, D0
        let instr_divs = decode(0x81C1);
        assert_eq!(
            instr_divs,
            Instruction::DivS {
                src: AddressingMode::DataRegister(1),
                dst_reg: 0,
            }
        );
    }

    #[test]
    fn test_decode_nbcd() {
        // Valid NBCD modes
        assert_eq!(
            decode(0x4800),
            Instruction::Nbcd {
                dst: AddressingMode::DataRegister(0)
            }
        );
        assert_eq!(
            decode(0x4810),
            Instruction::Nbcd {
                dst: AddressingMode::AddressIndirect(0)
            }
        );
        assert_eq!(
            decode(0x4818),
            Instruction::Nbcd {
                dst: AddressingMode::AddressPostIncrement(0)
            }
        );
        assert_eq!(
            decode(0x4820),
            Instruction::Nbcd {
                dst: AddressingMode::AddressPreDecrement(0)
            }
        );
        assert_eq!(
            decode(0x4828),
            Instruction::Nbcd {
                dst: AddressingMode::AddressDisplacement(0)
            }
        );
        assert_eq!(
            decode(0x4830),
            Instruction::Nbcd {
                dst: AddressingMode::AddressIndex(0)
            }
        );
        assert_eq!(
            decode(0x4838),
            Instruction::Nbcd {
                dst: AddressingMode::AbsoluteShort
            }
        );
        assert_eq!(
            decode(0x4839),
            Instruction::Nbcd {
                dst: AddressingMode::AbsoluteLong
            }
        );

        // Invalid NBCD modes
        // NBCD A0 (0x4808)
        assert_eq!(
            decode(0x4808),
            Instruction::Unimplemented { opcode: 0x4808 }
        );
        // NBCD #<data> (0x483C)
        assert_eq!(
            decode(0x483C),
            Instruction::Unimplemented { opcode: 0x483C }
        );
        // NBCD d16(PC) (0x483A)
        assert_eq!(
            decode(0x483A),
            Instruction::Unimplemented { opcode: 0x483A }
        );
        // NBCD d8(PC,Xn) (0x483B)
        assert_eq!(
            decode(0x483B),
            Instruction::Unimplemented { opcode: 0x483B }
        );
    }
}
