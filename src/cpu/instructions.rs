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

// ---------------------------------------------------------------------------
// Disassembly formatting: standard Motorola syntax, or GNU (AT&T) syntax.
//
// The decoder only captures the opcode word, so operand *values* (immediates,
// absolute addresses, displacements) are reconstructed here from the raw
// instruction words supplied by the caller via `DisAsm`.
// ---------------------------------------------------------------------------

fn dis_dreg(n: u8, gnu: bool) -> String {
    if gnu {
        format!("%d{n}")
    } else {
        format!("D{n}")
    }
}

fn dis_areg(n: u8, gnu: bool) -> String {
    if gnu {
        format!("%a{n}")
    } else {
        format!("A{n}")
    }
}

fn dis_special(name: &str, gnu: bool) -> String {
    if gnu {
        format!("%{}", name.to_lowercase())
    } else {
        name.to_string()
    }
}

fn dis_size(size: Size) -> &'static str {
    match size {
        Size::Byte => ".B",
        Size::Word => ".W",
        Size::Long => ".L",
    }
}

fn dis_imm(v: i64, gnu: bool) -> String {
    match (v < 0, gnu) {
        (true, true) => format!("#-0x{:x}", -v),
        (true, false) => format!("#-${:X}", -v),
        (false, true) => format!("#0x{v:x}"),
        (false, false) => format!("#${v:X}"),
    }
}

fn dis_hex_signed(v: i32, gnu: bool) -> String {
    let m = (v as i64).unsigned_abs();
    match (v < 0, gnu) {
        (true, true) => format!("-0x{m:x}"),
        (true, false) => format!("-${m:X}"),
        (false, true) => format!("0x{m:x}"),
        (false, false) => format!("${m:X}"),
    }
}

/// Format a MOVEM register mask, e.g. "D0-D3/A0/A2".
fn dis_reglist(mask: u16, predec: bool, gnu: bool) -> String {
    let mut regs = [false; 16];
    for (i, present) in regs.iter_mut().enumerate() {
        // -(An) reverses the bit order (A7..D0); otherwise D0..A7.
        let bit = if predec { 15 - i } else { i };
        *present = (mask >> bit) & 1 != 0;
    }
    let name = |i: usize| {
        if i < 8 {
            dis_dreg(i as u8, gnu)
        } else {
            dis_areg((i - 8) as u8, gnu)
        }
    };
    let mut parts: Vec<String> = Vec::new();
    let mut i = 0;
    while i < 16 {
        if !regs[i] {
            i += 1;
            continue;
        }
        let file_end = if i < 8 { 8 } else { 16 };
        let start = i;
        while i + 1 < file_end && regs[i + 1] {
            i += 1;
        }
        if i == start {
            parts.push(name(start));
        } else {
            parts.push(format!("{}-{}", name(start), name(i)));
        }
        i += 1;
    }
    parts.join("/")
}

/// Assemble "MNEMONIC.sz  op1,op2" for the selected syntax.
fn dis_line(gnu: bool, mnem: &str, size: Option<Size>, ops: &[String]) -> String {
    let sz = size.map(dis_size).unwrap_or("");
    let head = if gnu {
        format!("{}{}", mnem.to_lowercase(), sz.to_lowercase())
    } else {
        format!("{mnem}{sz}")
    };
    if ops.is_empty() {
        head
    } else {
        format!("{head:<8}{}", ops.join(","))
    }
}

/// Cursor over an instruction's raw words. Formats operands, consuming
/// extension words in encoding order so immediates/addresses show real values.
struct DisAsm<'a> {
    words: &'a [u16],
    /// Index of the next unread word (1 = first extension word).
    idx: usize,
    pc: u32,
    gnu: bool,
}

impl<'a> DisAsm<'a> {
    fn new(words: &'a [u16], pc: u32, gnu: bool) -> Self {
        Self {
            words,
            idx: 1,
            pc,
            gnu,
        }
    }

    fn word_u(&mut self) -> u32 {
        let v = self.words.get(self.idx).copied().unwrap_or(0) as u32;
        self.idx += 1;
        v
    }

    fn word_s(&mut self) -> i32 {
        let v = self.words.get(self.idx).copied().unwrap_or(0) as i16 as i32;
        self.idx += 1;
        v
    }

    fn long(&mut self) -> u32 {
        let hi = self.word_u();
        let lo = self.word_u();
        (hi << 16) | lo
    }

    /// Address of the extension word about to be read (for PC-relative modes).
    fn cur_addr(&self) -> u32 {
        self.pc.wrapping_add(2 * self.idx as u32)
    }

    /// Decode a brief extension word: (displacement, "Xn.sz").
    fn brief_index(&mut self) -> (i32, String) {
        let ext = self.word_u();
        let disp = (ext as u8) as i8 as i32;
        let is_addr = (ext & 0x8000) != 0;
        let reg = ((ext >> 12) & 0x7) as u8;
        let long = (ext & 0x0800) != 0;
        let rname = if is_addr {
            dis_areg(reg, self.gnu)
        } else {
            dis_dreg(reg, self.gnu)
        };
        let sz = match (long, self.gnu) {
            (true, true) => ".l",
            (true, false) => ".L",
            (false, true) => ".w",
            (false, false) => ".W",
        };
        (disp, format!("{rname}{sz}"))
    }

    fn imm(&mut self, size: Size) -> String {
        let v: i64 = match size {
            Size::Byte => (self.word_u() & 0xFF) as i64,
            Size::Word => self.word_s() as i64,
            Size::Long => self.long() as i32 as i64,
        };
        dis_imm(v, self.gnu)
    }

    fn ea(&mut self, m: &AddressingMode, size: Size) -> String {
        let gnu = self.gnu;
        match m {
            AddressingMode::DataRegister(r) => dis_dreg(*r, gnu),
            AddressingMode::AddressRegister(r) => dis_areg(*r, gnu),
            AddressingMode::AddressIndirect(r) => format!("({})", dis_areg(*r, gnu)),
            AddressingMode::AddressPostIncrement(r) => format!("({})+", dis_areg(*r, gnu)),
            AddressingMode::AddressPreDecrement(r) => format!("-({})", dis_areg(*r, gnu)),
            AddressingMode::AddressDisplacement(r) => {
                let d = self.word_s();
                format!("{}({})", dis_hex_signed(d, gnu), dis_areg(*r, gnu))
            }
            AddressingMode::AddressIndex(r) => {
                let (disp, idx) = self.brief_index();
                format!(
                    "{}({},{})",
                    dis_hex_signed(disp, gnu),
                    dis_areg(*r, gnu),
                    idx
                )
            }
            AddressingMode::AbsoluteShort => {
                let a = self.word_s() as u32; // sign-extended to 32 bits
                let body = if gnu {
                    format!("0x{a:x}")
                } else {
                    format!("${:X}", a & 0xFFFF)
                };
                format!("{}{}", body, if gnu { ".w" } else { ".W" })
            }
            AddressingMode::AbsoluteLong => {
                let a = self.long();
                let body = if gnu {
                    format!("0x{a:x}")
                } else {
                    format!("${a:06X}")
                };
                format!("{}{}", body, if gnu { ".l" } else { ".L" })
            }
            AddressingMode::PcDisplacement => {
                let base = self.cur_addr();
                let d = self.word_s();
                let t = base.wrapping_add(d as u32);
                if gnu {
                    format!("0x{t:x}(%pc)")
                } else {
                    format!("${t:06X}(PC)")
                }
            }
            AddressingMode::PcIndex => {
                let base = self.cur_addr();
                let (disp, idx) = self.brief_index();
                let t = base.wrapping_add(disp as u32);
                if gnu {
                    format!("0x{t:x}(%pc,{idx})")
                } else {
                    format!("${t:06X}(PC,{idx})")
                }
            }
            AddressingMode::Immediate => self.imm(size),
        }
    }

    /// Branch target: the byte form carries the displacement in the opcode; a
    /// zero byte selects the 16-bit form (displacement in the next word).
    fn target(&mut self, byte_disp: i16) -> String {
        let disp = if byte_disp != 0 {
            byte_disp as i32
        } else {
            self.word_s()
        };
        let t = self.pc.wrapping_add(2).wrapping_add(disp as u32);
        if self.gnu {
            format!("0x{t:x}")
        } else {
            format!("${t:06X}")
        }
    }
}

impl Instruction {
    /// Disassemble using the raw instruction words (`words[0]` = opcode) so
    /// operand values are shown. `gnu` selects GNU/AT&T syntax over Motorola.
    pub fn format_with_words(&self, pc: u32, words: &[u16], gnu: bool) -> String {
        let mut d = DisAsm::new(words, pc, gnu);
        match self {
            Instruction::Data(x) => x.format(&mut d),
            Instruction::Arithmetic(x) => x.format(&mut d),
            Instruction::Bits(x) => x.format(&mut d),
            Instruction::System(x) => x.format(&mut d),
        }
    }

    /// Disassemble without extension words (registers/branches show correctly;
    /// operands needing extension words render as 0).
    pub fn format(&self, pc: u32, gnu: bool) -> String {
        self.format_with_words(pc, &[], gnu)
    }
}

impl DataInstruction {
    fn format(&self, d: &mut DisAsm) -> String {
        use DataInstruction::*;
        let gnu = d.gnu;
        match self {
            Move { size, src, dst } => {
                let s = d.ea(src, *size);
                let t = d.ea(dst, *size);
                dis_line(gnu, "MOVE", Some(*size), &[s, t])
            }
            MoveA { size, src, dst_reg } => {
                let s = d.ea(src, *size);
                dis_line(gnu, "MOVEA", Some(*size), &[s, dis_areg(*dst_reg, gnu)])
            }
            MoveQ { dst_reg, data } => dis_line(
                gnu,
                "MOVEQ",
                None,
                &[dis_imm(*data as i64, gnu), dis_dreg(*dst_reg, gnu)],
            ),
            Lea { src, dst_reg } => {
                let s = d.ea(src, Size::Long);
                dis_line(gnu, "LEA", None, &[s, dis_areg(*dst_reg, gnu)])
            }
            Pea { src } => {
                let s = d.ea(src, Size::Long);
                dis_line(gnu, "PEA", None, &[s])
            }
            Clr { size, dst } => {
                let t = d.ea(dst, *size);
                dis_line(gnu, "CLR", Some(*size), &[t])
            }
            Exg { rx, ry, mode } => {
                let (a, b) = match mode {
                    0x08 => (dis_dreg(*rx, gnu), dis_dreg(*ry, gnu)),
                    0x09 => (dis_areg(*rx, gnu), dis_areg(*ry, gnu)),
                    _ => (dis_dreg(*rx, gnu), dis_areg(*ry, gnu)),
                };
                dis_line(gnu, "EXG", None, &[a, b])
            }
            Movep {
                size,
                reg,
                an,
                direction,
            } => {
                let disp = d.word_s();
                let dn = dis_dreg(*reg, gnu);
                let ea = format!("{}({})", dis_hex_signed(disp, gnu), dis_areg(*an, gnu));
                let ops = if *direction {
                    vec![dn, ea]
                } else {
                    vec![ea, dn]
                };
                dis_line(gnu, "MOVEP", Some(*size), &ops)
            }
            Movem {
                size,
                direction,
                mask,
                ea,
            } => {
                // The register mask lives in the first extension word (decode
                // always stores mask 0); consume it before the EA reads its
                // own extension words. Fall back to the instruction's mask so
                // callers without raw words still get a list.
                let ext_mask = d.word_u() as u16;
                let mask = if *mask != 0 { *mask } else { ext_mask };
                let predec = matches!(ea, AddressingMode::AddressPreDecrement(_));
                let list = dis_reglist(mask, predec, gnu);
                let e = d.ea(ea, *size);
                // direction=true is registers -> memory.
                let ops = if *direction {
                    vec![list, e]
                } else {
                    vec![e, list]
                };
                dis_line(gnu, "MOVEM", Some(*size), &ops)
            }
            Swap { reg } => dis_line(gnu, "SWAP", None, &[dis_dreg(*reg, gnu)]),
            Ext { size, reg } => dis_line(gnu, "EXT", Some(*size), &[dis_dreg(*reg, gnu)]),
        }
    }
}

impl ArithmeticInstruction {
    fn format(&self, d: &mut DisAsm) -> String {
        use ArithmeticInstruction::*;
        let gnu = d.gnu;
        let bcd = |sr: u8, dr: u8, mem: bool| {
            if mem {
                vec![
                    format!("-({})", dis_areg(sr, gnu)),
                    format!("-({})", dis_areg(dr, gnu)),
                ]
            } else {
                vec![dis_dreg(sr, gnu), dis_dreg(dr, gnu)]
            }
        };
        match self {
            Add { size, src, dst, .. } => {
                let s = d.ea(src, *size);
                let t = d.ea(dst, *size);
                dis_line(gnu, "ADD", Some(*size), &[s, t])
            }
            Sub { size, src, dst, .. } => {
                let s = d.ea(src, *size);
                let t = d.ea(dst, *size);
                dis_line(gnu, "SUB", Some(*size), &[s, t])
            }
            AddA { size, src, dst_reg } => {
                let s = d.ea(src, *size);
                dis_line(gnu, "ADDA", Some(*size), &[s, dis_areg(*dst_reg, gnu)])
            }
            SubA { size, src, dst_reg } => {
                let s = d.ea(src, *size);
                dis_line(gnu, "SUBA", Some(*size), &[s, dis_areg(*dst_reg, gnu)])
            }
            AddI { size, dst } => {
                let imm = d.imm(*size);
                let t = d.ea(dst, *size);
                dis_line(gnu, "ADDI", Some(*size), &[imm, t])
            }
            SubI { size, dst } => {
                let imm = d.imm(*size);
                let t = d.ea(dst, *size);
                dis_line(gnu, "SUBI", Some(*size), &[imm, t])
            }
            AddQ { size, dst, data } => {
                let t = d.ea(dst, *size);
                dis_line(gnu, "ADDQ", Some(*size), &[dis_imm(*data as i64, gnu), t])
            }
            SubQ { size, dst, data } => {
                let t = d.ea(dst, *size);
                dis_line(gnu, "SUBQ", Some(*size), &[dis_imm(*data as i64, gnu), t])
            }
            MulU { src, dst_reg } => {
                let s = d.ea(src, Size::Word);
                dis_line(gnu, "MULU", None, &[s, dis_dreg(*dst_reg, gnu)])
            }
            MulS { src, dst_reg } => {
                let s = d.ea(src, Size::Word);
                dis_line(gnu, "MULS", None, &[s, dis_dreg(*dst_reg, gnu)])
            }
            DivU { src, dst_reg } => {
                let s = d.ea(src, Size::Word);
                dis_line(gnu, "DIVU", None, &[s, dis_dreg(*dst_reg, gnu)])
            }
            DivS { src, dst_reg } => {
                let s = d.ea(src, Size::Word);
                dis_line(gnu, "DIVS", None, &[s, dis_dreg(*dst_reg, gnu)])
            }
            Neg { size, dst } => {
                let t = d.ea(dst, *size);
                dis_line(gnu, "NEG", Some(*size), &[t])
            }
            NegX { size, dst } => {
                let t = d.ea(dst, *size);
                dis_line(gnu, "NEGX", Some(*size), &[t])
            }
            Nbcd { dst } => {
                let t = d.ea(dst, Size::Byte);
                dis_line(gnu, "NBCD", None, &[t])
            }
            Abcd {
                src_reg,
                dst_reg,
                memory_mode,
            } => dis_line(gnu, "ABCD", None, &bcd(*src_reg, *dst_reg, *memory_mode)),
            Sbcd {
                src_reg,
                dst_reg,
                memory_mode,
            } => dis_line(gnu, "SBCD", None, &bcd(*src_reg, *dst_reg, *memory_mode)),
            AddX {
                size,
                src_reg,
                dst_reg,
                memory_mode,
            } => dis_line(
                gnu,
                "ADDX",
                Some(*size),
                &bcd(*src_reg, *dst_reg, *memory_mode),
            ),
            SubX {
                size,
                src_reg,
                dst_reg,
                memory_mode,
            } => dis_line(
                gnu,
                "SUBX",
                Some(*size),
                &bcd(*src_reg, *dst_reg, *memory_mode),
            ),
            Chk { src, dst_reg } => {
                let s = d.ea(src, Size::Word);
                dis_line(gnu, "CHK", None, &[s, dis_dreg(*dst_reg, gnu)])
            }
            Cmp { size, src, dst_reg } => {
                let s = d.ea(src, *size);
                dis_line(gnu, "CMP", Some(*size), &[s, dis_dreg(*dst_reg, gnu)])
            }
            CmpA { size, src, dst_reg } => {
                let s = d.ea(src, *size);
                dis_line(gnu, "CMPA", Some(*size), &[s, dis_areg(*dst_reg, gnu)])
            }
            CmpI { size, dst } => {
                let imm = d.imm(*size);
                let t = d.ea(dst, *size);
                dis_line(gnu, "CMPI", Some(*size), &[imm, t])
            }
            CmpM { size, ax, ay } => dis_line(
                gnu,
                "CMPM",
                Some(*size),
                &[
                    format!("({})+", dis_areg(*ay, gnu)),
                    format!("({})+", dis_areg(*ax, gnu)),
                ],
            ),
            Tst { size, dst } => {
                let t = d.ea(dst, *size);
                dis_line(gnu, "TST", Some(*size), &[t])
            }
        }
    }
}

impl BitsInstruction {
    fn format(&self, d: &mut DisAsm) -> String {
        use BitsInstruction::*;
        let gnu = d.gnu;
        match self {
            And { size, src, dst, .. } => {
                let s = d.ea(src, *size);
                let t = d.ea(dst, *size);
                dis_line(gnu, "AND", Some(*size), &[s, t])
            }
            Or { size, src, dst, .. } => {
                let s = d.ea(src, *size);
                let t = d.ea(dst, *size);
                dis_line(gnu, "OR", Some(*size), &[s, t])
            }
            AndI { size, dst } => {
                let imm = d.imm(*size);
                let t = d.ea(dst, *size);
                dis_line(gnu, "ANDI", Some(*size), &[imm, t])
            }
            OrI { size, dst } => {
                let imm = d.imm(*size);
                let t = d.ea(dst, *size);
                dis_line(gnu, "ORI", Some(*size), &[imm, t])
            }
            EorI { size, dst } => {
                let imm = d.imm(*size);
                let t = d.ea(dst, *size);
                dis_line(gnu, "EORI", Some(*size), &[imm, t])
            }
            Eor { size, src_reg, dst } => {
                let t = d.ea(dst, *size);
                dis_line(gnu, "EOR", Some(*size), &[dis_dreg(*src_reg, gnu), t])
            }
            Not { size, dst } => {
                let t = d.ea(dst, *size);
                dis_line(gnu, "NOT", Some(*size), &[t])
            }
            Lsl { size, dst, count } => shift(d, "LSL", *size, dst, count),
            Lsr { size, dst, count } => shift(d, "LSR", *size, dst, count),
            Asl { size, dst, count } => shift(d, "ASL", *size, dst, count),
            Asr { size, dst, count } => shift(d, "ASR", *size, dst, count),
            Rol { size, dst, count } => shift(d, "ROL", *size, dst, count),
            Ror { size, dst, count } => shift(d, "ROR", *size, dst, count),
            Roxl { size, dst, count } => shift(d, "ROXL", *size, dst, count),
            Roxr { size, dst, count } => shift(d, "ROXR", *size, dst, count),
            AslM { dst } => {
                let t = d.ea(dst, Size::Word);
                dis_line(gnu, "ASL", None, &[t])
            }
            AsrM { dst } => {
                let t = d.ea(dst, Size::Word);
                dis_line(gnu, "ASR", None, &[t])
            }
            Btst { bit, dst } => bitop(d, "BTST", bit, dst),
            Bset { bit, dst } => bitop(d, "BSET", bit, dst),
            Bclr { bit, dst } => bitop(d, "BCLR", bit, dst),
            Bchg { bit, dst } => bitop(d, "BCHG", bit, dst),
            Tas { dst } => {
                let t = d.ea(dst, Size::Byte);
                dis_line(gnu, "TAS", None, &[t])
            }
        }
    }
}

fn shift(
    d: &mut DisAsm,
    mnem: &str,
    size: Size,
    dst: &AddressingMode,
    count: &ShiftCount,
) -> String {
    let gnu = d.gnu;
    let c = match count {
        ShiftCount::Immediate(n) => dis_imm(*n as i64, gnu),
        ShiftCount::Register(r) => dis_dreg(*r, gnu),
    };
    let t = d.ea(dst, size);
    dis_line(gnu, mnem, Some(size), &[c, t])
}

fn bitop(d: &mut DisAsm, mnem: &str, bit: &BitSource, dst: &AddressingMode) -> String {
    let gnu = d.gnu;
    // An immediate bit number is the first extension word.
    let src = match bit {
        BitSource::Immediate => dis_imm((d.word_u() & 0xFF) as i64, gnu),
        BitSource::Register(r) => dis_dreg(*r, gnu),
    };
    let t = d.ea(dst, Size::Byte);
    dis_line(gnu, mnem, None, &[src, t])
}

impl SystemInstruction {
    fn format(&self, d: &mut DisAsm) -> String {
        use SystemInstruction::*;
        let gnu = d.gnu;
        match self {
            Bra { displacement } => {
                let t = d.target(*displacement);
                dis_line(gnu, "BRA", None, &[t])
            }
            Bsr { displacement } => {
                let t = d.target(*displacement);
                dis_line(gnu, "BSR", None, &[t])
            }
            Bcc {
                condition,
                displacement,
            } => {
                let t = d.target(*displacement);
                dis_line(gnu, &format!("B{}", condition.mnemonic()), None, &[t])
            }
            Scc { condition, dst } => {
                let t = d.ea(dst, Size::Byte);
                dis_line(gnu, &format!("S{}", condition.mnemonic()), None, &[t])
            }
            DBcc { condition, reg } => {
                let t = d.target(0);
                dis_line(
                    gnu,
                    &format!("DB{}", condition.mnemonic()),
                    None,
                    &[dis_dreg(*reg, gnu), t],
                )
            }
            Jmp { dst } => {
                let t = d.ea(dst, Size::Long);
                dis_line(gnu, "JMP", None, &[t])
            }
            Jsr { dst } => {
                let t = d.ea(dst, Size::Long);
                dis_line(gnu, "JSR", None, &[t])
            }
            Rts => dis_line(gnu, "RTS", None, &[]),
            Rte => dis_line(gnu, "RTE", None, &[]),
            Rtr => dis_line(gnu, "RTR", None, &[]),
            Nop => dis_line(gnu, "NOP", None, &[]),
            Reset => dis_line(gnu, "RESET", None, &[]),
            Stop => {
                let v = d.word_u();
                dis_line(gnu, "STOP", None, &[dis_imm(v as i64, gnu)])
            }
            TrapV => dis_line(gnu, "TRAPV", None, &[]),
            Illegal => dis_line(gnu, "ILLEGAL", None, &[]),
            MoveUsp { reg, to_usp } => {
                let an = dis_areg(*reg, gnu);
                let usp = dis_special("USP", gnu);
                let ops = if *to_usp {
                    vec![an, usp]
                } else {
                    vec![usp, an]
                };
                dis_line(gnu, "MOVE", None, &ops)
            }
            Trap { vector } => dis_line(gnu, "TRAP", None, &[dis_imm(*vector as i64, gnu)]),
            Link { reg } => {
                let disp = d.word_s();
                dis_line(
                    gnu,
                    "LINK",
                    None,
                    &[dis_areg(*reg, gnu), dis_imm(disp as i64, gnu)],
                )
            }
            Unlk { reg } => dis_line(gnu, "UNLK", None, &[dis_areg(*reg, gnu)]),
            MoveToSr { src } => {
                let s = d.ea(src, Size::Word);
                dis_line(gnu, "MOVE", None, &[s, dis_special("SR", gnu)])
            }
            MoveFromSr { dst } => {
                let t = d.ea(dst, Size::Word);
                dis_line(gnu, "MOVE", None, &[dis_special("SR", gnu), t])
            }
            MoveToCcr { src } => {
                let s = d.ea(src, Size::Word);
                dis_line(gnu, "MOVE", None, &[s, dis_special("CCR", gnu)])
            }
            AndiToCcr => {
                let imm = d.imm(Size::Byte);
                dis_line(gnu, "ANDI", None, &[imm, dis_special("CCR", gnu)])
            }
            AndiToSr => {
                let imm = d.imm(Size::Word);
                dis_line(gnu, "ANDI", None, &[imm, dis_special("SR", gnu)])
            }
            OriToCcr => {
                let imm = d.imm(Size::Byte);
                dis_line(gnu, "ORI", None, &[imm, dis_special("CCR", gnu)])
            }
            OriToSr => {
                let imm = d.imm(Size::Word);
                dis_line(gnu, "ORI", None, &[imm, dis_special("SR", gnu)])
            }
            EoriToCcr => {
                let imm = d.imm(Size::Byte);
                dis_line(gnu, "EORI", None, &[imm, dis_special("CCR", gnu)])
            }
            EoriToSr => {
                let imm = d.imm(Size::Word);
                dis_line(gnu, "EORI", None, &[imm, dis_special("SR", gnu)])
            }
            LineA { opcode } | LineF { opcode } | Unimplemented { opcode } => {
                if gnu {
                    format!(".short  0x{opcode:x}")
                } else {
                    format!("DC.W    ${opcode:04X}")
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disassemble_motorola_and_gnu() {
        let mv = Instruction::Data(DataInstruction::Move {
            size: Size::Word,
            src: AddressingMode::DataRegister(0),
            dst: AddressingMode::AddressIndirect(1),
        });
        assert_eq!(mv.format(0x1000, false), "MOVE.W  D0,(A1)");
        assert_eq!(mv.format(0x1000, true), "move.w  %d0,(%a1)");

        let mq = Instruction::Data(DataInstruction::MoveQ {
            dst_reg: 3,
            data: 0x34,
        });
        assert_eq!(mq.format(0, false), "MOVEQ   #$34,D3");
        assert_eq!(mq.format(0, true), "moveq   #0x34,%d3");

        // BEQ: target = pc + 2 + disp = 0x1000 + 2 + 0x20 = 0x1022.
        let beq = Instruction::System(SystemInstruction::Bcc {
            condition: Condition::Equal,
            displacement: 0x20,
        });
        assert_eq!(beq.format(0x1000, false), "BEQ     $001022");
        assert_eq!(beq.format(0x1000, true), "beq     0x1022");

        let rts = Instruction::System(SystemInstruction::Rts);
        assert_eq!(rts.format(0, false), "RTS");
        assert_eq!(rts.format(0, true), "rts");

        // MOVEM.L D0-D2/A6,-(A7): direction=true is registers->memory. In
        // predec order the mask bits run A7..D0, so D0-D2/A6 is 0xE002.
        let movem = Instruction::Data(DataInstruction::Movem {
            size: Size::Long,
            direction: true,
            mask: 0xE002,
            ea: AddressingMode::AddressPreDecrement(7),
        });
        assert_eq!(movem.format(0, false), "MOVEM.L D0-D2/A6,-(A7)");

        // Memory->registers with the mask taken from the extension word
        // (decode always stores mask 0): MOVEM.W (A0)+,D0-D1.
        let movem_load = Instruction::Data(DataInstruction::Movem {
            size: Size::Word,
            direction: false,
            mask: 0,
            ea: AddressingMode::AddressPostIncrement(0),
        });
        let words = [0x4C98, 0x0003, 0, 0, 0];
        assert_eq!(
            movem_load.format_with_words(0, &words, false),
            "MOVEM.W (A0)+,D0-D1"
        );
    }

    #[test]
    fn test_disassemble_with_extension_words() {
        // MOVE.W #$1234,$FF0000.L  — immediate word then absolute-long address.
        let mv = Instruction::Data(DataInstruction::Move {
            size: Size::Word,
            src: AddressingMode::Immediate,
            dst: AddressingMode::AbsoluteLong,
        });
        let words = [0x33FC, 0x1234, 0x00FF, 0x0000, 0];
        assert_eq!(
            mv.format_with_words(0, &words, false),
            "MOVE.W  #$1234,$FF0000.L"
        );
        assert_eq!(
            mv.format_with_words(0, &words, true),
            "move.w  #0x1234,0xff0000.l"
        );

        // ADDI.L #$12345678,D0 — the long immediate precedes the (register) dst.
        let addi = Instruction::Arithmetic(ArithmeticInstruction::AddI {
            size: Size::Long,
            dst: AddressingMode::DataRegister(0),
        });
        let words = [0x0680, 0x1234, 0x5678, 0, 0];
        assert_eq!(
            addi.format_with_words(0, &words, false),
            "ADDI.L  #$12345678,D0"
        );

        // LEA $10(A5),A0 — displacement extension word.
        let lea = Instruction::Data(DataInstruction::Lea {
            src: AddressingMode::AddressDisplacement(5),
            dst_reg: 0,
        });
        let words = [0x41ED, 0x0010, 0, 0, 0];
        assert_eq!(
            lea.format_with_words(0, &words, false),
            "LEA     $10(A5),A0"
        );

        // Word-form BNE: byte displacement 0 pulls the 16-bit displacement from
        // the next word (0x0100). Target = 0x1000 + 2 + 0x100 = 0x1102.
        let bne = Instruction::System(SystemInstruction::Bcc {
            condition: Condition::NotEqual,
            displacement: 0,
        });
        let words = [0x6600, 0x0100, 0, 0, 0];
        assert_eq!(
            bne.format_with_words(0x1000, &words, false),
            "BNE     $001102"
        );
    }

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
        assert_eq!(
            AddressingMode::from_mode_reg(0, 1),
            Some(AddressingMode::DataRegister(1))
        );
        assert_eq!(
            AddressingMode::from_mode_reg(1, 2),
            Some(AddressingMode::AddressRegister(2))
        );
        assert_eq!(
            AddressingMode::from_mode_reg(2, 3),
            Some(AddressingMode::AddressIndirect(3))
        );
        assert_eq!(
            AddressingMode::from_mode_reg(3, 4),
            Some(AddressingMode::AddressPostIncrement(4))
        );
        assert_eq!(
            AddressingMode::from_mode_reg(4, 5),
            Some(AddressingMode::AddressPreDecrement(5))
        );
        assert_eq!(
            AddressingMode::from_mode_reg(5, 6),
            Some(AddressingMode::AddressDisplacement(6))
        );
        assert_eq!(
            AddressingMode::from_mode_reg(6, 7),
            Some(AddressingMode::AddressIndex(7))
        );
        assert_eq!(
            AddressingMode::from_mode_reg(7, 0),
            Some(AddressingMode::AbsoluteShort)
        );
        assert_eq!(
            AddressingMode::from_mode_reg(7, 1),
            Some(AddressingMode::AbsoluteLong)
        );
        assert_eq!(
            AddressingMode::from_mode_reg(7, 2),
            Some(AddressingMode::PcDisplacement)
        );
        assert_eq!(
            AddressingMode::from_mode_reg(7, 3),
            Some(AddressingMode::PcIndex)
        );
        assert_eq!(
            AddressingMode::from_mode_reg(7, 4),
            Some(AddressingMode::Immediate)
        );
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
        assert_eq!(
            AddressingMode::DataRegister(0).extension_words(Size::Long),
            0
        );
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
