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

#[cfg(test)]
mod length_tests {
    use super::*;

    #[test]
    fn test_instruction_length() {
        // NOP - 1 word
        let instr = Instruction::System(SystemInstruction::Nop);
        assert_eq!(instr.length_words(), 1);

        // MOVE.L D0, D1 - 1 word
        let instr = Instruction::Data(DataInstruction::Move {
            size: Size::Long,
            src: AddressingMode::DataRegister(0),
            dst: AddressingMode::DataRegister(1),
        });
        assert_eq!(instr.length_words(), 1);

        // MOVE.W #$1234, D0 - 2 words (1 opcode + 1 immediate)
        let instr = Instruction::Data(DataInstruction::Move {
            size: Size::Word,
            src: AddressingMode::Immediate,
            dst: AddressingMode::DataRegister(0),
        });
        assert_eq!(instr.length_words(), 2);

        // MOVE.L #$12345678, D0 - 3 words (1 opcode + 2 immediate)
        let instr = Instruction::Data(DataInstruction::Move {
            size: Size::Long,
            src: AddressingMode::Immediate,
            dst: AddressingMode::DataRegister(0),
        });
        assert_eq!(instr.length_words(), 3);

        // ADD.L (A0)+, -(A1) - 1 word (no extension words for these modes)
        let instr = Instruction::Arithmetic(ArithmeticInstruction::Add {
            size: Size::Long,
            src: AddressingMode::AddressPostIncrement(0),
            dst: AddressingMode::AddressPreDecrement(1),
            direction: false,
        });
        assert_eq!(instr.length_words(), 1);

        // JMP (xxx).L - 3 words (1 opcode + 2 extension)
        let instr = Instruction::System(SystemInstruction::Jmp {
            dst: AddressingMode::AbsoluteLong,
        });
        assert_eq!(instr.length_words(), 3);

        // BRA with 8-bit displacement (0) - 2 words (1 opcode + 1 extension)
        let instr = Instruction::System(SystemInstruction::Bra {
            displacement: 0,
        });
        assert_eq!(instr.length_words(), 2);

        // BRA with 8-bit displacement (non-zero) - 1 word
        let instr = Instruction::System(SystemInstruction::Bra {
            displacement: 4,
        });
        assert_eq!(instr.length_words(), 1);
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

    /// Returns true if this mode is "Alterable" (excluding PC-relative and Immediate)
    pub fn is_alterable(&self) -> bool {
        !matches!(
            self,
            AddressingMode::PcDisplacement | AddressingMode::PcIndex | AddressingMode::Immediate
        )
    }

    /// Returns true if this mode is "Data Alterable" (Alterable and not Address Register Direct)
    pub fn is_data_alterable(&self) -> bool {
        self.is_alterable() && !matches!(self, AddressingMode::AddressRegister(_))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instruction {
    Data(DataInstruction),
    Arithmetic(ArithmeticInstruction),
    Bits(BitsInstruction),
    System(SystemInstruction),
}


impl Instruction {
    /// Returns the length of the instruction in 16-bit words.
    pub fn length_words(&self) -> u32 {
        1 + match self {
            Instruction::Data(instr) => instr.extension_words(),
            Instruction::Arithmetic(instr) => instr.extension_words(),
            Instruction::Bits(instr) => instr.extension_words(),
            Instruction::System(instr) => instr.extension_words(),
        }
    }
}

impl DataInstruction {
    pub fn extension_words(&self) -> u32 {
        match self {
            Self::Move { size, src, dst } => src.extension_words(*size) + dst.extension_words(*size),
            Self::MoveA { size, src, .. } => src.extension_words(*size),
            Self::MoveQ { .. } => 0,
            Self::Lea { src, .. } => src.extension_words(Size::Long),
            Self::Pea { src } => src.extension_words(Size::Long),
            Self::Clr { size, dst } => dst.extension_words(*size),
            Self::Exg { .. } => 0,
            Self::Movep { .. } => 1, // d16
            Self::Movem { size, ea, .. } => 1 + ea.extension_words(*size), // 1 word for mask
            Self::Swap { .. } => 0,
            Self::Ext { .. } => 0,
        }
    }
}

impl ArithmeticInstruction {
    pub fn extension_words(&self) -> u32 {
        match self {
            Self::Add { size, src, dst, .. } => src.extension_words(*size) + dst.extension_words(*size),
            Self::AddA { size, src, .. } => src.extension_words(*size),
            Self::AddI { size, dst } => (if *size == Size::Long { 2 } else { 1 }) + dst.extension_words(*size),
            Self::AddQ { size, dst, .. } => dst.extension_words(*size),
            Self::Sub { size, src, dst, .. } => src.extension_words(*size) + dst.extension_words(*size),
            Self::SubA { size, src, .. } => src.extension_words(*size),
            Self::SubI { size, dst } => (if *size == Size::Long { 2 } else { 1 }) + dst.extension_words(*size),
            Self::SubQ { size, dst, .. } => dst.extension_words(*size),
            Self::MulU { src, .. } => src.extension_words(Size::Word),
            Self::MulS { src, .. } => src.extension_words(Size::Word),
            Self::DivU { src, .. } => src.extension_words(Size::Word),
            Self::DivS { src, .. } => src.extension_words(Size::Word),
            Self::Neg { size, dst } => dst.extension_words(*size),
            Self::Abcd { .. } => 0,
            Self::Sbcd { .. } => 0,
            Self::Nbcd { dst } => dst.extension_words(Size::Byte),
            Self::AddX { .. } => 0,
            Self::SubX { .. } => 0,
            Self::NegX { size, dst } => dst.extension_words(*size),
            Self::Chk { src, .. } => src.extension_words(Size::Word),
            Self::Cmp { size, src, .. } => src.extension_words(*size),
            Self::CmpA { size, src, .. } => src.extension_words(*size),
            Self::CmpI { size, dst } => (if *size == Size::Long { 2 } else { 1 }) + dst.extension_words(*size),
            Self::CmpM { .. } => 0,
            Self::Tst { size, dst } => dst.extension_words(*size),
        }
    }
}

impl BitsInstruction {
    pub fn extension_words(&self) -> u32 {
        match self {
            Self::And { size, src, dst, .. } => src.extension_words(*size) + dst.extension_words(*size),
            Self::AndI { size, dst } => (if *size == Size::Long { 2 } else { 1 }) + dst.extension_words(*size),
            Self::Or { size, src, dst, .. } => src.extension_words(*size) + dst.extension_words(*size),
            Self::OrI { size, dst } => (if *size == Size::Long { 2 } else { 1 }) + dst.extension_words(*size),
            Self::Eor { size, dst, .. } => dst.extension_words(*size),
            Self::EorI { size, dst } => (if *size == Size::Long { 2 } else { 1 }) + dst.extension_words(*size),
            Self::Not { size, dst } => dst.extension_words(*size),
            Self::Lsl { size, dst, .. } => dst.extension_words(*size),
            Self::Lsr { size, dst, .. } => dst.extension_words(*size),
            Self::Asl { size, dst, .. } => dst.extension_words(*size),
            Self::AslM { dst } => dst.extension_words(Size::Word),
            Self::Asr { size, dst, .. } => dst.extension_words(*size),
            Self::AsrM { dst } => dst.extension_words(Size::Word),
            Self::Rol { size, dst, .. } => dst.extension_words(*size),
            Self::Ror { size, dst, .. } => dst.extension_words(*size),
            Self::Roxl { size, dst, .. } => dst.extension_words(*size),
            Self::Roxr { size, dst, .. } => dst.extension_words(*size),
            Self::Btst { bit, dst } => (if matches!(bit, BitSource::Immediate) { 1 } else { 0 }) + dst.extension_words(Size::Byte),
            Self::Bset { bit, dst } => (if matches!(bit, BitSource::Immediate) { 1 } else { 0 }) + dst.extension_words(Size::Byte),
            Self::Bclr { bit, dst } => (if matches!(bit, BitSource::Immediate) { 1 } else { 0 }) + dst.extension_words(Size::Byte),
            Self::Bchg { bit, dst } => (if matches!(bit, BitSource::Immediate) { 1 } else { 0 }) + dst.extension_words(Size::Byte),
            Self::Tas { dst } => dst.extension_words(Size::Byte),
        }
    }
}

impl SystemInstruction {
    pub fn extension_words(&self) -> u32 {
        match self {
            Self::Bra { displacement } => if *displacement == 0 { 1 } else { 0 }, // 8-bit displacement is 0, extension word follows
            Self::Bsr { displacement } => if *displacement == 0 { 1 } else { 0 },
            Self::Bcc { displacement, .. } => if *displacement == 0 { 1 } else { 0 },
            Self::Scc { dst, .. } => dst.extension_words(Size::Byte),
            Self::DBcc { .. } => 1, // d16
            Self::Jmp { dst } => dst.extension_words(Size::Long),
            Self::Jsr { dst } => dst.extension_words(Size::Long),
            Self::Rts | Self::Rte | Self::Rtr | Self::Nop | Self::Reset | Self::TrapV => 0,
            Self::Stop => 1, // 1 word for immediate SR value
            Self::MoveUsp { .. } => 0,
            Self::Trap { .. } => 0,
            Self::Link { .. } => 1, // d16
            Self::Unlk { .. } => 0,
            Self::MoveToSr { src } => src.extension_words(Size::Word),
            Self::MoveFromSr { dst } => dst.extension_words(Size::Word),
            Self::MoveToCcr { src } => src.extension_words(Size::Word),
            Self::AndiToCcr | Self::AndiToSr | Self::OriToCcr | Self::OriToSr | Self::EoriToCcr | Self::EoriToSr => 1, // #<data>
            Self::Illegal | Self::LineA { .. } | Self::LineF { .. } | Self::Unimplemented { .. } => 0,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataInstruction {
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
    Movem {
        size: Size,
        direction: bool,
        mask: u16,
        ea: AddressingMode,
    },
    Swap {
        reg: u8,
    },
    Ext {
        size: Size,
        reg: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArithmeticInstruction {
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitsInstruction {
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
    AslM {
        dst: AddressingMode,
    },
    Asr {
        size: Size,
        dst: AddressingMode,
        count: ShiftCount,
    },
    AsrM {
        dst: AddressingMode,
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
    Tas {
        dst: AddressingMode,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemInstruction {
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

/// Cache entry for decoded instructions
#[derive(Debug, Clone, Copy)]
#[repr(align(16))]
pub struct DecodeCacheEntry {
    pub pc: u32,
    pub instruction: Instruction,
}

impl Default for DecodeCacheEntry {
    fn default() -> Self {
        Self {
            pc: u32::MAX, // Invalid PC
            instruction: Instruction::System(SystemInstruction::Nop),
        }
    }
}
