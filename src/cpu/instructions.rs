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
        let extension = match self {
            Instruction::Data(data) => data.extension_words(),
            Instruction::Arithmetic(arith) => arith.extension_words(),
            Instruction::Bits(bits) => bits.extension_words(),
            Instruction::System(sys) => sys.extension_words(),
        };
        1 + extension
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

impl DataInstruction {
    pub fn extension_words(&self) -> u32 {
        match self {
            DataInstruction::Move { size, src, dst } => {
                src.extension_words(*size) + dst.extension_words(*size)
            }
            DataInstruction::MoveA { size, src, .. } => src.extension_words(*size),
            DataInstruction::MoveQ { .. }
            | DataInstruction::Swap { .. }
            | DataInstruction::Ext { .. }
            | DataInstruction::Exg { .. } => 0,
            DataInstruction::Lea { src, .. } | DataInstruction::Pea { src } => {
                src.extension_words(Size::Long)
            }
            DataInstruction::Clr { size, dst } => dst.extension_words(*size),
            DataInstruction::Movep { .. } => {
                1 // displacement
            }
            DataInstruction::Movem { ea, .. } => {
                1 + ea.extension_words(Size::Word) // mask + ea
            }
        }
    }
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

impl ArithmeticInstruction {
    pub fn extension_words(&self) -> u32 {
        match self {
            ArithmeticInstruction::Add { size, src, dst, .. }
            | ArithmeticInstruction::Sub { size, src, dst, .. } => {
                src.extension_words(*size) + dst.extension_words(*size)
            }
            ArithmeticInstruction::AddA { size, src, .. }
            | ArithmeticInstruction::SubA { size, src, .. }
            | ArithmeticInstruction::CmpA { size, src, .. } => src.extension_words(*size),
            ArithmeticInstruction::AddI { size, dst }
            | ArithmeticInstruction::SubI { size, dst }
            | ArithmeticInstruction::CmpI { size, dst } => {
                AddressingMode::Immediate.extension_words(*size) + dst.extension_words(*size)
            }
            ArithmeticInstruction::AddQ { size, dst, .. }
            | ArithmeticInstruction::SubQ { size, dst, .. }
            | ArithmeticInstruction::Neg { size, dst }
            | ArithmeticInstruction::NegX { size, dst }
            | ArithmeticInstruction::Tst { size, dst } => dst.extension_words(*size),
            ArithmeticInstruction::MulU { src, .. }
            | ArithmeticInstruction::MulS { src, .. }
            | ArithmeticInstruction::DivU { src, .. }
            | ArithmeticInstruction::DivS { src, .. }
            | ArithmeticInstruction::Chk { src, .. } => src.extension_words(Size::Word),
            ArithmeticInstruction::Cmp { size, src, .. } => src.extension_words(*size),
            ArithmeticInstruction::Abcd { .. }
            | ArithmeticInstruction::Sbcd { .. }
            | ArithmeticInstruction::AddX { .. }
            | ArithmeticInstruction::SubX { .. }
            | ArithmeticInstruction::CmpM { .. } => 0,
            ArithmeticInstruction::Nbcd { dst } => dst.extension_words(Size::Byte),
        }
    }
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

impl BitsInstruction {
    pub fn extension_words(&self) -> u32 {
        match self {
            BitsInstruction::And { size, src, dst, .. }
            | BitsInstruction::Or { size, src, dst, .. } => {
                src.extension_words(*size) + dst.extension_words(*size)
            }
            BitsInstruction::AndI { size, dst }
            | BitsInstruction::OrI { size, dst }
            | BitsInstruction::EorI { size, dst } => {
                AddressingMode::Immediate.extension_words(*size) + dst.extension_words(*size)
            }
            BitsInstruction::Eor { size, dst, .. } => dst.extension_words(*size),
            BitsInstruction::Not { size, dst } => dst.extension_words(*size),
            BitsInstruction::Lsl { size, dst, .. }
            | BitsInstruction::Lsr { size, dst, .. }
            | BitsInstruction::Asl { size, dst, .. }
            | BitsInstruction::Asr { size, dst, .. }
            | BitsInstruction::Rol { size, dst, .. }
            | BitsInstruction::Ror { size, dst, .. }
            | BitsInstruction::Roxl { size, dst, .. }
            | BitsInstruction::Roxr { size, dst, .. } => dst.extension_words(*size),
            BitsInstruction::AslM { dst }
            | BitsInstruction::AsrM { dst }
            | BitsInstruction::Tas { dst } => dst.extension_words(Size::Byte),
            BitsInstruction::Btst { bit, dst }
            | BitsInstruction::Bset { bit, dst }
            | BitsInstruction::Bclr { bit, dst }
            | BitsInstruction::Bchg { bit, dst } => {
                let bit_ext = if let BitSource::Immediate = bit { 1 } else { 0 };
                bit_ext + dst.extension_words(Size::Byte)
            }
        }
    }
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

impl SystemInstruction {
    pub fn extension_words(&self) -> u32 {
        match self {
            SystemInstruction::Bra { displacement } | SystemInstruction::Bsr { displacement } => {
                if *displacement == 0 {
                    1
                } else {
                    0
                }
            }
            SystemInstruction::Bcc { displacement, .. } => {
                if *displacement == 0 {
                    1
                } else {
                    0
                }
            }
            SystemInstruction::Scc { dst, .. } => dst.extension_words(Size::Byte),
            SystemInstruction::DBcc { .. }
            | SystemInstruction::Link { .. }
            | SystemInstruction::Stop => 1,
            SystemInstruction::Jmp { dst } | SystemInstruction::Jsr { dst } => {
                dst.extension_words(Size::Long)
            }
            SystemInstruction::MoveToSr { src } | SystemInstruction::MoveToCcr { src } => {
                src.extension_words(Size::Word)
            }
            SystemInstruction::MoveFromSr { dst } => dst.extension_words(Size::Word),
            SystemInstruction::AndiToCcr
            | SystemInstruction::AndiToSr
            | SystemInstruction::OriToCcr
            | SystemInstruction::OriToSr
            | SystemInstruction::EoriToCcr
            | SystemInstruction::EoriToSr => 1,
            _ => 0,
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_bytes() {
        assert_eq!(Size::Byte.bytes(), 1);
        assert_eq!(Size::Word.bytes(), 2);
        assert_eq!(Size::Long.bytes(), 4);
    }

    #[test]
    fn test_size_mask() {
        assert_eq!(Size::Byte.mask(), 0xFF);
        assert_eq!(Size::Word.mask(), 0xFFFF);
        assert_eq!(Size::Long.mask(), 0xFFFFFFFF);
    }

    #[test]
    fn test_size_apply() {
        assert_eq!(Size::Byte.apply(0x12345678, 0xAAAAAAAA), 0x123456AA);
        assert_eq!(Size::Word.apply(0x12345678, 0xAAAAAAAA), 0x1234AAAA);
        assert_eq!(Size::Long.apply(0x12345678, 0xAAAAAAAA), 0xAAAAAAAA);
    }

    #[test]
    fn test_size_sign_bit() {
        assert_eq!(Size::Byte.sign_bit(), 0x80);
        assert_eq!(Size::Word.sign_bit(), 0x8000);
        assert_eq!(Size::Long.sign_bit(), 0x80000000);
    }

    #[test]
    fn test_size_is_negative() {
        assert!(Size::Byte.is_negative(0x80));
        assert!(!Size::Byte.is_negative(0x7F));
        assert!(Size::Word.is_negative(0x8000));
        assert!(!Size::Word.is_negative(0x7FFF));
        assert!(Size::Long.is_negative(0x80000000));
        assert!(!Size::Long.is_negative(0x7FFFFFFF));
    }

    #[test]
    fn test_size_bits() {
        assert_eq!(Size::Byte.bits(), 8);
        assert_eq!(Size::Word.bits(), 16);
        assert_eq!(Size::Long.bits(), 32);
    }

    #[test]
    fn test_size_from_bits() {
        assert_eq!(Size::from_bits(0b00), Some(Size::Byte));
        assert_eq!(Size::from_bits(0b01), Some(Size::Word));
        assert_eq!(Size::from_bits(0b10), Some(Size::Long));
        assert_eq!(Size::from_bits(0b11), None);
    }

    #[test]
    fn test_size_from_move_bits() {
        assert_eq!(Size::from_move_bits(0b01), Some(Size::Byte));
        assert_eq!(Size::from_move_bits(0b11), Some(Size::Word));
        assert_eq!(Size::from_move_bits(0b10), Some(Size::Long));
        assert_eq!(Size::from_move_bits(0b00), None);
    }

    #[test]
    fn test_size_display() {
        assert_eq!(format!("{}", Size::Byte), ".B");
        assert_eq!(format!("{}", Size::Word), ".W");
        assert_eq!(format!("{}", Size::Long), ".L");
    }

    #[test]
    fn test_addressing_mode_from_mode_reg() {
        assert_eq!(AddressingMode::from_mode_reg(0, 1), Some(AddressingMode::DataRegister(1)));
        assert_eq!(AddressingMode::from_mode_reg(1, 2), Some(AddressingMode::AddressRegister(2)));
        assert_eq!(AddressingMode::from_mode_reg(2, 3), Some(AddressingMode::AddressIndirect(3)));
        assert_eq!(AddressingMode::from_mode_reg(3, 4), Some(AddressingMode::AddressPostIncrement(4)));
        assert_eq!(AddressingMode::from_mode_reg(4, 5), Some(AddressingMode::AddressPreDecrement(5)));
        assert_eq!(AddressingMode::from_mode_reg(5, 6), Some(AddressingMode::AddressDisplacement(6)));
        assert_eq!(AddressingMode::from_mode_reg(6, 7), Some(AddressingMode::AddressIndex(7)));
        assert_eq!(AddressingMode::from_mode_reg(7, 0), Some(AddressingMode::AbsoluteShort));
        assert_eq!(AddressingMode::from_mode_reg(7, 1), Some(AddressingMode::AbsoluteLong));
        assert_eq!(AddressingMode::from_mode_reg(7, 2), Some(AddressingMode::PcDisplacement));
        assert_eq!(AddressingMode::from_mode_reg(7, 3), Some(AddressingMode::PcIndex));
        assert_eq!(AddressingMode::from_mode_reg(7, 4), Some(AddressingMode::Immediate));
        assert_eq!(AddressingMode::from_mode_reg(7, 5), None);
    }

    #[test]
    fn test_addressing_mode_is_alterable() {
        assert!(AddressingMode::DataRegister(0).is_alterable());
        assert!(AddressingMode::AddressRegister(0).is_alterable());
        assert!(AddressingMode::AddressIndirect(0).is_alterable());
        assert!(!AddressingMode::PcDisplacement.is_alterable());
        assert!(!AddressingMode::PcIndex.is_alterable());
        assert!(!AddressingMode::Immediate.is_alterable());
    }

    #[test]
    fn test_addressing_mode_is_data_alterable() {
        assert!(AddressingMode::DataRegister(0).is_data_alterable());
        assert!(!AddressingMode::AddressRegister(0).is_data_alterable());
        assert!(AddressingMode::AddressIndirect(0).is_data_alterable());
    }

    #[test]
    fn test_addressing_mode_extension_words() {
        assert_eq!(AddressingMode::DataRegister(0).extension_words(Size::Long), 0);
        assert_eq!(AddressingMode::AbsoluteShort.extension_words(Size::Long), 1);
        assert_eq!(AddressingMode::AbsoluteLong.extension_words(Size::Word), 2);
        assert_eq!(AddressingMode::Immediate.extension_words(Size::Byte), 1);
        assert_eq!(AddressingMode::Immediate.extension_words(Size::Word), 1);
        assert_eq!(AddressingMode::Immediate.extension_words(Size::Long), 2);
    }

    #[test]
    fn test_addressing_mode_display() {
        assert_eq!(format!("{}", AddressingMode::DataRegister(1)), "D1");
        assert_eq!(format!("{}", AddressingMode::PcDisplacement), "d16(PC)");
        assert_eq!(format!("{}", AddressingMode::Immediate), "#<data>");
    }

    #[test]
    fn test_condition_from_bits() {
        assert_eq!(Condition::from_bits(0x0), Condition::True);
        assert_eq!(Condition::from_bits(0x7), Condition::Equal);
        assert_eq!(Condition::from_bits(0xF), Condition::LessOrEqual);
        // Test with higher bits set (should be masked)
        assert_eq!(Condition::from_bits(0x10), Condition::True);
    }

    #[test]
    fn test_condition_mnemonic() {
        assert_eq!(Condition::True.mnemonic(), "T");
        assert_eq!(Condition::Equal.mnemonic(), "EQ");
        assert_eq!(Condition::LessOrEqual.mnemonic(), "LE");
    }
}
