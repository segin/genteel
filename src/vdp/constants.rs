// VDP Control Codes (bits 0-3)
pub const VRAM_READ: u8 = 0x00;
pub const VRAM_WRITE: u8 = 0x01;
pub const CRAM_WRITE: u8 = 0x03;
pub const VSRAM_READ: u8 = 0x04;
pub const VSRAM_WRITE: u8 = 0x05;
pub const CRAM_READ: u8 = 0x08;

// Register indices
pub const REG_MODE1: usize = 0;
pub const REG_MODE2: usize = 1;
pub const REG_PLANE_A: usize = 2;
pub const REG_PLANE_B: usize = 4;
pub const REG_SPRITE_TABLE: usize = 5;
pub const REG_BG_COLOR: usize = 7;
pub const REG_MODE3: usize = 11;
pub const REG_MODE4: usize = 12;
pub const REG_HSCROLL: usize = 13;
pub const REG_AUTO_INC: usize = 15;
pub const REG_DMA_LEN_LO: usize = 19;
pub const REG_DMA_LEN_HI: usize = 20;
pub const REG_DMA_SRC_LO: usize = 21;
pub const REG_DMA_SRC_MID: usize = 22;
pub const REG_DMA_SRC_HI: usize = 23;

// Mode bits
pub const MODE1_HINT_ENABLE: u8 = 0x10;
pub const MODE2_V30_MODE: u8 = 0x08;
pub const MODE2_DMA_ENABLE: u8 = 0x10;
pub const MODE2_VINT_ENABLE: u8 = 0x20;
pub const MODE2_DISPLAY_ENABLE: u8 = 0x40;
pub const MODE4_H40_MODE: u8 = 0x81; // H40 mode check mask

// DMA Modes
pub const DMA_MODE_MASK: u8 = 0xC0;
pub const DMA_MODE_FILL: u8 = 0x80;
pub const DMA_MODE_COPY: u8 = 0xC0;

// Status bits
pub const STATUS_VBLANK: u16 = 0x0008;
pub const STATUS_VINT_PENDING: u16 = 0x0080;

pub const NUM_REGISTERS: usize = 24;
