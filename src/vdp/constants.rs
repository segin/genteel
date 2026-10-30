// VDP Control Codes (bits 0-3)
pub const VRAM_READ: u8 = 0x00;
pub const VRAM_WRITE: u8 = 0x01;
pub const CRAM_WRITE: u8 = 0x03;
pub const VSRAM_READ: u8 = 0x04;
pub const VSRAM_WRITE: u8 = 0x05;
pub const CRAM_READ: u8 = 0x08;
/// Undocumented 8-bit VRAM read (returns the byte at address ^ 1).
pub const VRAM_READ_8BIT: u8 = 0x0C;

/// Implemented CRAM colour bits (`0000BBB0GGG0RRR0`).
pub const CRAM_COLOR_MASK: u16 = 0x0EEE;
/// VSRAM holds 40 entries (0x50 bytes) of 11 bits each.
pub const VSRAM_SIZE: usize = 0x50;
pub const VSRAM_ENTRY_MASK: u16 = 0x07FF;

// Register indices
pub const REG_MODE1: usize = 0;
pub const REG_MODE2: usize = 1;
pub const REG_PLANE_A: usize = 2;
pub const REG_WINDOW: usize = 3;
pub const REG_PLANE_B: usize = 4;
pub const REG_SPRITE_TABLE: usize = 5;
pub const REG_SPRITE_PATTERN: usize = 6;
pub const REG_BG_COLOR: usize = 7;
pub const REG_H_INT_COUNTER: usize = 10;
pub const REG_MODE3: usize = 11;
pub const REG_MODE4: usize = 12;
pub const REG_HSCROLL: usize = 13;
pub const REG_AUTO_INC: usize = 15;
pub const REG_PLANE_SIZE: usize = 16;
pub const REG_WINDOW_H_POS: usize = 17;
pub const REG_WINDOW_V_POS: usize = 18;
pub const REG_DMA_LEN_LO: usize = 19;
pub const REG_DMA_LEN_HI: usize = 20;
pub const REG_DMA_SRC_LO: usize = 21;
pub const REG_DMA_SRC_MID: usize = 22;
pub const REG_DMA_SRC_HI: usize = 23;

// Mode bits
pub const MODE1_HINT_ENABLE: u8 = 0x10;
/// Register 0 bit 1 (M3): freeze the HV counter.
pub const MODE1_HV_LATCH: u8 = 0x02;
pub const MODE2_V30_MODE: u8 = 0x08;
pub const MODE2_DMA_ENABLE: u8 = 0x10;
pub const MODE2_VINT_ENABLE: u8 = 0x20;
pub const MODE2_DISPLAY_ENABLE: u8 = 0x40;
/// Register 12 bit 0 (RS1) selects H40 mode.
pub const MODE4_RS1: u8 = 0x01;
/// Register 12 bit 1 (LSM0): interlace enable.
pub const MODE4_LSM0: u8 = 0x02;
/// Register 12 bit 2 (LSM1): with LSM0, double-resolution interlace.
pub const MODE4_LSM1: u8 = 0x04;

// DMA Modes
pub const DMA_MODE_MASK: u8 = 0xC0;
pub const DMA_MODE_FILL: u8 = 0x80;
pub const DMA_MODE_COPY: u8 = 0xC0;

// Status bits
pub const STATUS_PAL: u16 = 0x0001;
pub const STATUS_DMA: u16 = 0x0002;
pub const STATUS_HBLANK: u16 = 0x0004;
pub const STATUS_VBLANK: u16 = 0x0008;
pub const STATUS_ODD_FRAME: u16 = 0x0010;
/// Sprite-sprite pixel collision (bit 5, SCOL).
pub const STATUS_COLLISION: u16 = 0x0020;
/// Sprite overflow: more than 20 (H40) / 16 (H32) sprites on a line (bit 6, SOVR).
pub const STATUS_SOVR: u16 = 0x0040;
pub const STATUS_VINT_PENDING: u16 = 0x0080;
pub const STATUS_FIFO_FULL: u16 = 0x0100;
pub const STATUS_FIFO_EMPTY: u16 = 0x0200;

pub const NUM_REGISTERS: usize = 24;
