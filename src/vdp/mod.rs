//! Sega Genesis Video Display Processor (VDP)
//!
//! The VDP is responsible for generating the video output. It contains:
//! - 64KB of VRAM for tile patterns and nametables
//! - 128 bytes of CRAM for the color palette (64 colors)
//! - 80 bytes of VSRAM for vertical scroll values
//! - 24 internal registers for configuration
//!
//! ## VDP Ports (Memory-Mapped)
//!
//! | Address          | Description                    |
//! |:-----------------|:-------------------------------|
//! | 0xC00000-0xC00003| Data Port (read/write VRAM)    |
//! | 0xC00004-0xC00007| Control Port (commands/status) |
//! | 0xC00008-0xC0000F| H/V Counter                    |
use crate::debugger::Debuggable;
use serde_json::{json, Value};

// VDP Control Codes (bits 0-3)
const VRAM_READ: u8 = 0x00;
const VRAM_WRITE: u8 = 0x01;
const CRAM_WRITE: u8 = 0x03;
const VSRAM_READ: u8 = 0x04;
const VSRAM_WRITE: u8 = 0x05;
const CRAM_READ: u8 = 0x08;

struct SpriteAttributes {
    v_pos: u16,
    h_pos: u16,
    h_size: u8, // tiles
    v_size: u8, // tiles
    link: u8,
    priority: bool,
    palette: u8,
    v_flip: bool,
    h_flip: bool,
    base_tile: u16,
}

struct SpriteIterator<'a> {
    vram: &'a [u8; 0x10000],
    next_idx: u8,
    count: usize,
    max_sprites: usize,
    sat_base: usize,
}

impl<'a> Iterator for SpriteIterator<'a> {
    type Item = SpriteAttributes;

    fn next(&mut self) -> Option<Self::Item> {
        if self.count >= self.max_sprites {
            return None;
        }

        // Check SAT boundary
        if self.sat_base + (self.next_idx as usize * 8) + 8 > 0x10000 {
            return None;
        }

        let addr = self.sat_base + (self.next_idx as usize * 8);

        let cur_v = (((self.vram[addr] as u16) << 8) | (self.vram[addr + 1] as u16)) & 0x03FF;
        let v_pos = cur_v.wrapping_sub(128);

        let size = self.vram[addr + 2];
        let h_size = ((size >> 2) & 0x03) + 1;
        let v_size = (size & 0x03) + 1;

        let link = self.vram[addr + 3] & 0x7F;

        let attr_word = ((self.vram[addr + 4] as u16) << 8) | (self.vram[addr + 5] as u16);
        let priority = (attr_word & 0x8000) != 0;
        let palette = ((attr_word >> 13) & 0x03) as u8;
        let v_flip = (attr_word & 0x1000) != 0;
        let h_flip = (attr_word & 0x0800) != 0;
        let base_tile = attr_word & 0x07FF;

        let cur_h = (((self.vram[addr + 6] as u16) << 8) | (self.vram[addr + 7] as u16)) & 0x03FF;
        let h_pos = cur_h.wrapping_sub(128);

        let attr = SpriteAttributes {
            v_pos,
            h_pos,
            h_size,
            v_size,
            link,
            priority,
            palette,
            v_flip,
            h_flip,
            base_tile,
        };

        self.count += 1;
        self.next_idx = link;

        if link == 0 {
            self.count = self.max_sprites; // Stop after this one
        }

        Some(attr)
    }
}

// Control Port Constants
const CTRL_CODE_LOW_MASK: u8 = 0x03; // CD1-CD0
const CTRL_CODE_HIGH_MASK: u16 = 0x3C; // CD5-CD2 (after shift)
const CTRL_ADDR_LO_MASK: u16 = 0x3FFF; // A13-A0
const CTRL_ADDR_HI_MASK: u16 = 0x03; // A15-A14 (in value)
const CTRL_DMA_BIT: u8 = 0x20; // DMA enable bit in control code

const CTRL_CODE_LOW_SHIFT: u16 = 14;
const CTRL_CODE_HIGH_SHIFT: u16 = 2;
const CTRL_ADDR_HI_SHIFT: u16 = 14;

// VDP Registers
const REG_MODE1: usize = 0;
const REG_MODE2: usize = 1;
const REG_PLANE_A: usize = 2;
// const REG_WINDOW: usize = 3;
const REG_PLANE_B: usize = 4;
const REG_SPRITE_TABLE: usize = 5;
// const REG_SPRITE_PATTERN: usize = 6;
const REG_BG_COLOR: usize = 7;
// const REG_H_INT: usize = 10;
// const REG_MODE3: usize = 11;
const REG_MODE4: usize = 12;
const REG_HSCROLL: usize = 13;
const REG_AUTO_INC: usize = 15;
const REG_PLANE_SIZE: usize = 16;
// const REG_WINDOW_H: usize = 17;
// const REG_WINDOW_V: usize = 18;
const REG_DMA_LEN_LO: usize = 19;
const REG_DMA_LEN_HI: usize = 20;
const REG_DMA_SRC_LO: usize = 21;
const REG_DMA_SRC_MID: usize = 22;
const REG_DMA_SRC_HI: usize = 23;

// Register Flags
const MODE1_HINT_ENABLE: u8 = 0x10;

const MODE2_V30_MODE: u8 = 0x08;
const MODE2_DMA_ENABLE: u8 = 0x10;
const MODE2_VINT_ENABLE: u8 = 0x20;
const MODE2_DISPLAY_ENABLE: u8 = 0x40;

const MODE4_H40_MODE: u8 = 0x81; // H40 mode check mask

// DMA Modes (Reg 23)
const DMA_MODE_MASK: u8 = 0xC0;
const DMA_MODE_FILL: u8 = 0x80;
const DMA_MODE_COPY: u8 = 0xC0;
const DMA_TYPE_BIT: u8 = 0x80; // 0=Transfer, 1=Fill/Copy

// Status Register Flags
const STATUS_VBLANK: u16 = 0x0008;
const STATUS_VINT_PENDING: u16 = 0x0080;

const REG_WRITE_TAG: u16 = 0x8000; // Value indicating register write
const REG_WRITE_MASK: u16 = 0xC000; // Mask to check register write tag
const REG_IDX_MASK: u16 = 0x1F; // Register index mask (5 bits)
const REG_DATA_MASK: u16 = 0xFF; // Register data mask (8 bits)
const REG_IDX_SHIFT: u16 = 8;
const NUM_REGISTERS: usize = 24;

/// Video Display Processor (VDP)
#[derive(Debug)]
pub struct Vdp {
    /// Video RAM (64KB) - stores tile patterns and nametables
    pub vram: [u8; 0x10000],

    /// Color RAM (128 bytes) - 64 colors, 2 bytes each (9-bit color)
    /// Format: ----BBB-GGG-RRR- (each component 0-7)
    pub cram: [u8; 128],

    /// Cached RGB565 colors for faster lookup
    pub cram_cache: [u16; 64],

    /// Vertical Scroll RAM (80 bytes) - 40 columns × 2 bytes
    pub vsram: [u8; 80],

    /// VDP Registers (24 registers, but only first 24 are meaningful)
    pub registers: [u8; NUM_REGISTERS],

    /// Control port state
    control_pending: bool,
    control_code: u8,
    control_address: u16,

    /// DMA state
    pub dma_pending: bool,

    /// Status register
    status: u16,

    /// Horizontal and vertical counters
    h_counter: u16,
    v_counter: u16,

    /// Internal line counter for HINT
    pub line_counter: u8,

    /// Last data value written (for VRAM fill DMA)
    pub last_data_write: u16,

    /// V30 offset for NTSC rolling effect
    pub v30_offset: u16,
    pub is_pal: bool,

    /// Framebuffer (320x240 RGB565)
    pub framebuffer: Vec<u16>,
}

impl Default for Vdp {
    fn default() -> Self {
        Self::new()
    }
}

impl Vdp {
    pub fn new() -> Self {
        Vdp {
            vram: [0; 0x10000],
            cram: [0; 128],
            cram_cache: [0; 64],
            vsram: [0; 80],
            registers: [0; NUM_REGISTERS],
            control_pending: false,
            control_code: 0,
            control_address: 0,
            dma_pending: false,
            status: 0x3600, // FIFO empty
            h_counter: 0,
            v_counter: 0,
            line_counter: 0,
            last_data_write: 0,
            v30_offset: 0,
            is_pal: false,
            framebuffer: vec![0; 320 * 240],
        }
    }

    /// Set the region (PAL=true, NTSC=false)
    pub fn set_region(&mut self, is_pal: bool) {
        self.is_pal = is_pal;
    }

    pub fn write_data_bulk(&mut self, data: &[u8]) {
        self.control_pending = false;

        // Optimized VRAM write for standard increment
        if (self.control_code & 0x0F) == VRAM_WRITE && self.auto_increment() == 2 {
            let mut addr = self.control_address as usize;
            for chunk in data.chunks_exact(2) {
                if addr < 0x10000 {
                    // Optimized direct write:
                    // 1. Data comes in big-endian (chunk[0]=High, chunk[1]=Low).
                    // 2. Standard VRAM write puts High at `addr` and Low at `addr ^ 1`.
                    // 3. Since auto-increment is 2, address parity is preserved between writes.
                    self.vram[addr] = chunk[0];
                    self.vram[addr ^ 1] = chunk[1];
                }
                addr = (addr + 2) & 0xFFFF;
            }
            self.control_address = addr as u16;

            // Update last_data_write
            if data.len() >= 2 {
                let last_idx = data.len() - 2;
                self.last_data_write = ((data[last_idx] as u16) << 8) | (data[last_idx + 1] as u16);
            }
            return;
        }

        // Fallback
        for chunk in data.chunks_exact(2) {
            let val = ((chunk[0] as u16) << 8) | (chunk[1] as u16);
            self.write_data(val);
        }
    }

    pub fn write_data(&mut self, value: u16) {
        self.control_pending = false;
        self.last_data_write = value;

        // DMA Fill (Mode 2) check
        // Enabled (Reg 1 bit 4) AND Mode 2 (Reg 23 bits 7,6 = 1,0) AND DMA Pending (CD5=1)
        if (self.registers[REG_MODE2] & MODE2_DMA_ENABLE) != 0 
            && self.is_dma_fill() 
            && self.dma_pending 
        {
            self.execute_dma();
            self.dma_pending = false;
            return;
        }

        match self.control_code & 0x0F {
            VRAM_WRITE => {
                // Write VRAM
                let addr = self.control_address as usize;
                if addr < 0x10000 {
                    // Byte swap needed? VRAM is accessed as bytes usually
                    self.vram[addr] = (value >> 8) as u8;
                    self.vram[addr ^ 1] = (value & 0xFF) as u8;
                }
            }
            CRAM_WRITE => {
                // Write CRAM
                let mut val = value;
                if (self.control_address & 0x01) != 0 {
                    val = val.rotate_left(8);
                }
                let addr = (self.control_address & 0x7E) as usize;
                // Pack 9-bit color to RGB565
                let r = (val & 0xE) << 1; // 3 bits -> 4 bits
                let g = (val & 0xE0) >> 3; // 3 bits -> 4 bits
                let b = (val & 0xE00) >> 7; // 3 bits -> 4 bits
                                            // Expand to 5/6/5
                let r5 = (r << 1) | (r >> 3);
                let g6 = (g << 2) | (g >> 2);
                let b5 = (b << 1) | (b >> 3);
                self.cram_cache[addr >> 1] = (r5 << 11) | (g6 << 5) | b5;

                self.cram[addr] = (val & 0xFF) as u8;
                self.cram[addr + 1] = (val >> 8) as u8;
            }
            VSRAM_WRITE => {
                // Write VSRAM
                let addr = (self.control_address & 0x7F) as usize;
                if addr < 80 {
                    self.vsram[addr] = (value >> 8) as u8;
                    self.vsram[addr + 1] = (value & 0xFF) as u8;
                }
            }
            _ => {}
        }

        self.control_address = self
            .control_address
            .wrapping_add(self.auto_increment() as u16);
    }

    pub fn read_data(&mut self) -> u16 {
        self.control_pending = false;
        match self.control_code & 0x0F {
            VRAM_READ => {
                // Read VRAM
                let addr = self.control_address as usize;
                let val = if addr < 0x10000 {
                    ((self.vram[addr] as u16) << 8) | (self.vram[addr ^ 1] as u16)
                } else {
                    0
                };
                self.control_address = self
                    .control_address
                    .wrapping_add(self.auto_increment() as u16);
                val
            }
            CRAM_READ => {
                // Read CRAM
                let addr = (self.control_address & 0x7F) as usize;
                ((self.cram[addr + 1] as u16) << 8) | (self.cram[addr] as u16)
            }
            VSRAM_READ => {
                // Read VSRAM
                let addr = (self.control_address & 0x7F) as usize;
                if addr < 80 {
                    ((self.vsram[addr] as u16) << 8) | (self.vsram[addr + 1] as u16)
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    pub fn write_control(&mut self, value: u16) {
        if self.control_pending {
            // Second word of command
            let high = ((value >> CTRL_CODE_HIGH_SHIFT) & CTRL_CODE_HIGH_MASK) as u8;
            self.control_code = (self.control_code & CTRL_CODE_LOW_MASK) | high;
            self.control_address = (self.control_address & CTRL_ADDR_LO_MASK)
                | ((value & CTRL_ADDR_HI_MASK) << CTRL_ADDR_HI_SHIFT);
            self.control_pending = false;

            // DMA initiation check
            if (self.control_code & CTRL_DMA_BIT) != 0 {
                // DMA requested
                self.dma_pending = true;
            }
        } else if (value & REG_WRITE_MASK) == REG_WRITE_TAG {
            // Register write
            let reg = ((value >> REG_IDX_SHIFT) & REG_IDX_MASK) as usize;
            let val = (value & REG_DATA_MASK) as u8;
            if reg < NUM_REGISTERS {
                self.registers[reg] = val;
            }
        } else {
            // First word of command
            self.control_code =
                ((value >> CTRL_CODE_LOW_SHIFT) & (CTRL_CODE_LOW_MASK as u16)) as u8;
            self.control_address = value & CTRL_ADDR_LO_MASK;
            self.control_pending = true;
        }
    }

    pub fn read_status(&self) -> u16 {
        self.status
    }

    /// Reset VDP state
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    #[cfg(test)]
    pub fn get_control_code(&self) -> u8 {
        self.control_code
    }

    #[cfg(test)]
    pub fn get_control_address(&self) -> u16 {
        self.control_address
    }

    #[cfg(test)]
    pub fn get_cram_color_pub(&self, palette: u8, color: u8) -> u16 {
        self.get_cram_color(palette, color)
    }

    // Helper methods
    fn auto_increment(&self) -> u8 {
        self.registers[REG_AUTO_INC]
    }

    pub fn mode1(&self) -> u8 {
        self.registers[REG_MODE1]
    }

    pub fn mode2(&self) -> u8 {
        self.registers[REG_MODE2]
    }

    pub fn h40_mode(&self) -> bool {
        (self.registers[REG_MODE4] & MODE4_H40_MODE) == MODE4_H40_MODE
    }

    pub fn screen_width(&self) -> u16 {
        if self.h40_mode() {
            320
        } else {
            256
        }
    }

    pub fn screen_height(&self) -> u16 {
        if self.v30_mode() {
            240
        } else {
            224
        }
    }

    pub fn v30_mode(&self) -> bool {
        (self.registers[REG_MODE2] & MODE2_V30_MODE) != 0
    }

    pub fn display_enabled(&self) -> bool {
        (self.registers[REG_MODE2] & MODE2_DISPLAY_ENABLE) != 0
    }

    pub fn dma_enabled(&self) -> bool {
        (self.registers[REG_MODE2] & MODE2_DMA_ENABLE) != 0
    }

    pub fn dma_mode(&self) -> u8 {
        // Bit 4 of Reg 23 determines mode (0=memory, 1=vram fill/copy)
        // Bit 5 is unused? Actually bit 6 and 7 of reg 23 are type.
        // Simplified: Reg 23
        self.registers[REG_DMA_SRC_HI]
    }

    pub fn dma_source(&self) -> u32 {
        ((self.registers[REG_DMA_SRC_HI] as u32) << 17)
            | ((self.registers[REG_DMA_SRC_MID] as u32) << 9)
            | ((self.registers[REG_DMA_SRC_LO] as u32) << 1)
    }

    pub fn dma_length(&self) -> u32 {
        ((self.registers[REG_DMA_LEN_HI] as u32) << 8) | (self.registers[REG_DMA_LEN_LO] as u32)
    }

    pub fn dma_source_transfer(&self) -> u32 {
        ((self.registers[REG_DMA_SRC_HI] as u32 & 0x3F) << 17)
            | ((self.registers[REG_DMA_SRC_MID] as u32) << 9)
            | ((self.registers[REG_DMA_SRC_LO] as u32) << 1)
    }

    /// Check if DMA mode is 0 or 1 (68k Transfer)
    pub fn is_dma_transfer(&self) -> bool {
        // Mode bit is bit 7 of reg 23?
        // Actually:
        // Reg 23:
        // Bit 7: Type (0=transfer, 1=fill/copy)
        // Bit 6: Type (0=fill, 1=copy) - only if Bit 7 is 1
        // So for transfer, Bit 7 must be 0.
        (self.registers[REG_DMA_SRC_HI] & DMA_TYPE_BIT) == 0
    }

    pub fn is_dma_fill(&self) -> bool {
        (self.registers[REG_DMA_SRC_HI] & DMA_MODE_MASK) == DMA_MODE_FILL
    }

    pub fn execute_dma(&mut self) -> u32 {
        let length = self.dma_length();
        // If length is 0, it is treated as 0x10000 (64KB)
        let len = if length == 0 { 0x10000 } else { length as usize };

        let mode = self.registers[REG_DMA_SRC_HI] & DMA_MODE_MASK;

        match mode {
            DMA_MODE_FILL => {
                // VRAM Fill (Mode 2)
                let data = self.last_data_write;
                let mut addr = self.control_address;
                let inc = self.auto_increment() as u16;
                let fill_byte = (data >> 8) as u8;

                for _ in 0..len {
                    self.vram[addr as usize] = fill_byte;
                    addr = addr.wrapping_add(inc);
                }
                self.control_address = addr;

                // Clear DMA length registers
                self.registers[REG_DMA_LEN_LO] = 0;
                self.registers[REG_DMA_LEN_HI] = 0;
            }
            DMA_MODE_COPY => {
                // VRAM Copy (Mode 3)
                let mut source = (self.dma_source() & 0xFFFF) as u16;
                let mut dest = self.control_address;
                let inc = self.auto_increment() as u16;

                for _ in 0..len {
                    let val = self.vram[source as usize];
                    self.vram[dest as usize] = val;
                    source = source.wrapping_add(1);
                    dest = dest.wrapping_add(inc);
                }
                self.control_address = dest;

                // Clear DMA length registers
                self.registers[REG_DMA_LEN_LO] = 0;
                self.registers[REG_DMA_LEN_HI] = 0;
            }
            _ => {}
        }

        self.dma_pending = false;
        len as u32
    }

    pub fn sprite_table_address(&self) -> u16 {
        let mask = 0xFE00; // simplified
        ((self.registers[REG_SPRITE_TABLE] as u16) << 9) & mask
    }

    pub fn plane_a_address(&self) -> usize {
        ((self.registers[REG_PLANE_A] as usize) & 0x38) << 10
    }

    pub fn plane_b_address(&self) -> usize {
        ((self.registers[REG_PLANE_B] as usize) & 0x07) << 13
    }

    pub fn hscroll_address(&self) -> usize {
        ((self.registers[REG_HSCROLL] as usize) & 0x3F) << 10
    }

    pub fn plane_size(&self) -> (usize, usize) {
        let w = match self.registers[REG_PLANE_SIZE] & 0x03 {
            0 => 32,
            1 => 64,
            _ => 128, // 2 and 3 are invalid but behave like 64 or 128
        };
        let h = match (self.registers[REG_PLANE_SIZE] >> 4) & 0x03 {
            0 => 32,
            1 => 64,
            _ => 128,
        };
        (w, h)
    }

    fn bg_color(&self) -> (u8, u8) {
        let bg_idx = self.registers[REG_BG_COLOR];
        let pal = (bg_idx >> 4) & 0x03;
        let color = bg_idx & 0x0F;
        (pal, color)
    }

    fn get_cram_color(&self, palette: u8, index: u8) -> u16 {
        let addr = ((palette as usize) * 16) + (index as usize);
        self.cram_cache[addr]
    }

    // VDP State management

    /// Called every scanline
    pub fn step(&mut self, _cycles: u64) {
        // Very simplified timing
        // In reality, we'd update counters based on cycles
        // For now, this is driven by the main loop calling render_line
    }

    /// Check if VBlank interrupt is pending
    pub fn vblank_pending(&self) -> bool {
        (self.status & STATUS_VBLANK) != 0 && (self.registers[REG_MODE2] & MODE2_VINT_ENABLE) != 0
    }

    /// Set VBlank status
    pub fn set_vblank(&mut self, active: bool) {
        if active {
            self.status |= STATUS_VBLANK; // VBlank flag
            self.status |= STATUS_VINT_PENDING; // VInterrupt pending
        } else {
            self.status &= !STATUS_VBLANK;
            self.status &= !STATUS_VINT_PENDING;
        }
    }

    pub fn trigger_vint(&mut self) {
        self.status |= STATUS_VINT_PENDING;
    }

    /// Check if HBlank interrupt is pending
    pub fn hblank_pending(&self) -> bool {
        // Simplified
        (self.registers[REG_MODE1] & MODE1_HINT_ENABLE) != 0
    }

    /// Update V30 rolling offset for NTSC mode
    pub fn update_v30_offset(&mut self) {
        if !self.is_pal && self.v30_mode() {
            // Calculated optimal increment for NTSC (60Hz) running V30 (312 lines timing):
            // 262 mod 240 = 22.
            // This simulates the drift of a 50Hz signal on a 60Hz display.
            self.v30_offset = (self.v30_offset + 22) % 240;
        }
    }

    // Debugging helpers
    pub fn dump_vram(&self) -> Vec<u8> {
        self.vram.to_vec()
    }

    pub fn dump_cram(&self) -> Vec<u8> {
        self.cram.to_vec()
    }

    /// Check if control is pending (for testing)
    #[cfg(test)]
    pub fn is_control_pending(&self) -> bool {
        self.control_pending
    }

    /// Read H/V counter
    pub fn read_hv_counter(&self) -> u16 {
        let h = (self.h_counter >> 1) as u8;
        let v = if self.v_counter > 0xFF {
            (self.v_counter - 0x100) as u8
        } else {
            self.v_counter as u8
        };
        ((v as u16) << 8) | (h as u16)
    }

    /// Set V-counter (scanline)
    pub fn set_v_counter(&mut self, v: u16) {
        self.v_counter = v;
    }

    /// Set H-counter
    pub fn set_h_counter(&mut self, h: u16) {
        self.h_counter = h;
    }

    // === Rendering ===

    /// Render a single scanline
    pub fn render_line(&mut self, line: u16) {
        if line >= self.screen_height() {
            return;
        }

        let width = self.screen_width();
        let draw_line = line;
        let fetch_line = if !self.is_pal && self.v30_mode() {
            (line + self.v30_offset) % 240
        } else {
            line
        };

        let line_offset = (draw_line as usize) * 320;

        // Get background color
        let (pal_line, color_idx) = self.bg_color();
        let bg_color = self.get_cram_color(pal_line, color_idx);

        // Fill with background color
        self.framebuffer[line_offset..(line_offset + width as usize)].fill(bg_color);

        if !self.display_enabled() {
            return;
        }

        // Plane rendering (Low priority)
        self.render_plane(false, fetch_line, draw_line, false); // Plane B low
        self.render_plane(true, fetch_line, draw_line, false); // Plane A low

        // Sprites low priority
        self.render_sprites(fetch_line, draw_line, false);

        // Plane rendering (High priority)
        self.render_plane(false, fetch_line, draw_line, true); // Plane B high
        self.render_plane(true, fetch_line, draw_line, true); // Plane A high
                                                              // Sprites high priority
        self.render_sprites(fetch_line, draw_line, true);
    }

    fn render_sprite_scanline(
        vram: &[u8; 0x10000],
        framebuffer: &mut [u16],
        cram_cache: &[u16; 64],
        line: u16,
        attr: &SpriteAttributes,
        line_offset: usize,
        screen_width: u16,
    ) {
        let sprite_v_px = (attr.v_size as u16) * 8;

        let py = line - attr.v_pos;
        let fetch_py = if attr.v_flip {
            (sprite_v_px - 1) - py
        } else {
            py
        };

        let tile_v_offset = fetch_py / 8;
        let pixel_v = fetch_py % 8;

        // Iterate by tiles instead of pixels for efficiency
        for t_h in 0..attr.h_size {
            let tile_h_offset = t_h as u16;
            let fetch_tile_h_offset = if attr.h_flip {
                (attr.h_size as u16 - 1) - tile_h_offset
            } else {
                tile_h_offset
            };

            // In a multi-tile sprite, tiles are arranged vertically first
            let tile_idx = attr
                .base_tile
                .wrapping_add(fetch_tile_h_offset * attr.v_size as u16)
                .wrapping_add(tile_v_offset);

            // Calculate pattern address for the row (pixel_v is 0..7)
            // Each tile is 32 bytes (4 bytes per row)
            let row_addr = (tile_idx as usize * 32) + (pixel_v as usize * 4);

            // Check if row is within VRAM bounds
            if row_addr + 4 > 0x10000 {
                continue;
            }

            // Prefetch the 4 bytes (8 pixels) for this row
            // We use wrapping arithmetic for safety although checks above should prevent OOB
            let p0 = vram[row_addr];
            let p1 = vram[(row_addr + 1) & 0xFFFF];
            let p2 = vram[(row_addr + 2) & 0xFFFF];
            let p3 = vram[(row_addr + 3) & 0xFFFF];
            let patterns = [p0, p1, p2, p3];

            let base_screen_x = attr.h_pos.wrapping_add(tile_h_offset * 8);

            for i in 0..8 {
                let screen_x = base_screen_x.wrapping_add(i);
                if screen_x >= screen_width {
                    continue;
                }

                let eff_col = if attr.h_flip { 7 - i } else { i };

                let byte = patterns[(eff_col as usize) / 2];
                let color_idx = if eff_col % 2 == 0 {
                    byte >> 4
                } else {
                    byte & 0x0F
                };

                if color_idx != 0 {
                    let addr = ((attr.palette as usize) * 16) + (color_idx as usize);
                    let color = cram_cache[addr];
                    framebuffer[line_offset + screen_x as usize] = color;
                }
            }
        }
    }

    fn render_sprites(&mut self, fetch_line: u16, draw_line: u16, priority_filter: bool) {
        let screen_width = self.screen_width();
        let line_offset = (draw_line as usize) * 320;

        let sat_base = self.sprite_table_address() as usize;
        let max_sprites = if self.h40_mode() { 80 } else { 64 };

        let iter = SpriteIterator {
            vram: &self.vram,
            next_idx: 0,
            count: 0,
            max_sprites,
            sat_base,
        };

        for attr in iter {
            // Check if sprite is visible on this line
            let sprite_v_px = (attr.v_size as u16) * 8;
            if attr.priority == priority_filter
                && fetch_line >= attr.v_pos
                && fetch_line < attr.v_pos + sprite_v_px
            {
                Self::render_sprite_scanline(
                    &self.vram,
                    &mut self.framebuffer,
                    &self.cram_cache,
                    fetch_line,
                    &attr,
                    line_offset,
                    screen_width,
                );
            }
        }
    }

    fn get_scroll_values(&self, is_plane_a: bool) -> (u16, u16) {
        let vs_addr = if is_plane_a { 0 } else { 2 };
        let v_scroll =
            (((self.vsram[vs_addr] as u16) << 8) | (self.vsram[vs_addr + 1] as u16)) & 0x03FF;

        let hs_base = self.hscroll_address();
        let hs_addr = if is_plane_a { hs_base } else { hs_base + 2 };
        let hi = self.vram[hs_addr];
        let lo = self.vram[hs_addr + 1];
        let h_scroll = (((hi as u16) << 8) | (lo as u16)) & 0x03FF;
        (v_scroll, h_scroll)
    }

    fn fetch_nametable_entry(
        &self,
        base: usize,
        tile_v: usize,
        tile_h: usize,
        plane_w: usize,
    ) -> u16 {
        let nt_entry_addr = base + (tile_v * plane_w + tile_h) * 2;
        let hi = self.vram[nt_entry_addr & 0xFFFF];
        let lo = self.vram[(nt_entry_addr + 1) & 0xFFFF];
        ((hi as u16) << 8) | (lo as u16)
    }

    fn fetch_tile_pattern(&self, tile_index: u16, pixel_v: u16, v_flip: bool) -> [u8; 4] {
        let row = if v_flip { 7 - pixel_v } else { pixel_v };
        let row_addr = (tile_index as usize * 32) + (row as usize * 4);

        let p0 = self.vram[row_addr & 0xFFFF];
        let p1 = self.vram[(row_addr + 1) & 0xFFFF];
        let p2 = self.vram[(row_addr + 2) & 0xFFFF];
        let p3 = self.vram[(row_addr + 3) & 0xFFFF];
        [p0, p1, p2, p3]
    }

    fn draw_tile_segment(
        &mut self,
        patterns: [u8; 4],
        palette: u8,
        h_flip: bool,
        pixel_h: u16,
        count: u16,
        start_idx: usize,
    ) {
        for i in 0..count {
            let current_pixel_h = pixel_h + i;
            let eff_col = if h_flip {
                7 - current_pixel_h
            } else {
                current_pixel_h
            };

            let byte = patterns[(eff_col as usize) / 2];

            let col = if eff_col % 2 == 0 {
                byte >> 4
            } else {
                byte & 0x0F
            };

            if col != 0 {
                let color = self.get_cram_color(palette, col);
                self.framebuffer[start_idx + i as usize] = color;
            }
        }
    }

    fn render_plane(
        &mut self,
        is_plane_a: bool,
        fetch_line: u16,
        draw_line: u16,
        priority_filter: bool,
    ) {
        let (plane_w, plane_h) = self.plane_size();
        let name_table_base = if is_plane_a {
            self.plane_a_address()
        } else {
            self.plane_b_address()
        };

        let (v_scroll, h_scroll) = self.get_scroll_values(is_plane_a);

        let scrolled_v = fetch_line.wrapping_add(v_scroll);
        let tile_v = ((scrolled_v as usize) >> 3) & (plane_h - 1);
        let pixel_v = scrolled_v & 7;

        let screen_width = self.screen_width();
        let line_offset = (draw_line as usize) * 320;

        let mut screen_x: u16 = 0;
        let mut scrolled_h = (0u16).wrapping_sub(h_scroll);

        let plane_mask = plane_w - 1;

        // Prologue: Align to 8-pixel boundary
        let pixel_h = scrolled_h & 7;
        if pixel_h != 0 {
            let pixels_left = 8 - pixel_h;
            let count = std::cmp::min(pixels_left, screen_width - screen_x);

            let tile_h = ((scrolled_h as usize) >> 3) & plane_mask;
            let entry = self.fetch_nametable_entry(name_table_base, tile_v, tile_h, plane_w);

            let priority = (entry & 0x8000) != 0;
            if priority == priority_filter {
                let palette = ((entry >> 13) & 0x03) as u8;
                let v_flip = (entry & 0x1000) != 0;
                let h_flip = (entry & 0x0800) != 0;
                let tile_index = entry & 0x07FF;

                let patterns = self.fetch_tile_pattern(tile_index, pixel_v as u16, v_flip);

                self.draw_tile_segment(
                    patterns,
                    palette,
                    h_flip,
                    pixel_h,
                    count,
                    line_offset + screen_x as usize,
                );
            }
            screen_x += count;
            scrolled_h = scrolled_h.wrapping_add(count);
        }

        // Main Loop: Process 8 pixels at a time
        while screen_x + 8 <= screen_width {
            let tile_h = ((scrolled_h as usize) >> 3) & plane_mask;
            let entry = self.fetch_nametable_entry(name_table_base, tile_v, tile_h, plane_w);

            let priority = (entry & 0x8000) != 0;
            if priority != priority_filter {
                screen_x += 8;
                scrolled_h = scrolled_h.wrapping_add(8);
                continue;
            }

            let palette = ((entry >> 13) & 0x03) as u8;
            let palette_base = (palette as usize) * 16;
            let v_flip = (entry & 0x1000) != 0;
            let h_flip = (entry & 0x0800) != 0;
            let tile_index = entry & 0x07FF;

            let patterns = self.fetch_tile_pattern(tile_index, pixel_v as u16, v_flip);
            let p0 = patterns[0];
            let p1 = patterns[1];
            let p2 = patterns[2];
            let p3 = patterns[3];

            let dest_idx = line_offset + (screen_x as usize);

            if h_flip {
                let mut col = p3 & 0x0F;
                if col != 0 { self.framebuffer[dest_idx] = self.cram_cache[palette_base + col as usize]; }

                col = p3 >> 4;
                if col != 0 { self.framebuffer[dest_idx + 1] = self.cram_cache[palette_base + col as usize]; }

                col = p2 & 0x0F;
                if col != 0 { self.framebuffer[dest_idx + 2] = self.cram_cache[palette_base + col as usize]; }

                col = p2 >> 4;
                if col != 0 { self.framebuffer[dest_idx + 3] = self.cram_cache[palette_base + col as usize]; }

                col = p1 & 0x0F;
                if col != 0 { self.framebuffer[dest_idx + 4] = self.cram_cache[palette_base + col as usize]; }

                col = p1 >> 4;
                if col != 0 { self.framebuffer[dest_idx + 5] = self.cram_cache[palette_base + col as usize]; }

                col = p0 & 0x0F;
                if col != 0 { self.framebuffer[dest_idx + 6] = self.cram_cache[palette_base + col as usize]; }

                col = p0 >> 4;
                if col != 0 { self.framebuffer[dest_idx + 7] = self.cram_cache[palette_base + col as usize]; }
            } else {
                let mut col = p0 >> 4;
                if col != 0 { self.framebuffer[dest_idx] = self.cram_cache[palette_base + col as usize]; }

                col = p0 & 0x0F;
                if col != 0 { self.framebuffer[dest_idx + 1] = self.cram_cache[palette_base + col as usize]; }

                col = p1 >> 4;
                if col != 0 { self.framebuffer[dest_idx + 2] = self.cram_cache[palette_base + col as usize]; }

                col = p1 & 0x0F;
                if col != 0 { self.framebuffer[dest_idx + 3] = self.cram_cache[palette_base + col as usize]; }

                col = p2 >> 4;
                if col != 0 { self.framebuffer[dest_idx + 4] = self.cram_cache[palette_base + col as usize]; }

                col = p2 & 0x0F;
                if col != 0 { self.framebuffer[dest_idx + 5] = self.cram_cache[palette_base + col as usize]; }

                col = p3 >> 4;
                if col != 0 { self.framebuffer[dest_idx + 6] = self.cram_cache[palette_base + col as usize]; }

                col = p3 & 0x0F;
                if col != 0 { self.framebuffer[dest_idx + 7] = self.cram_cache[palette_base + col as usize]; }
            }
            screen_x += 8;
            scrolled_h = scrolled_h.wrapping_add(8);
        }

        // Epilogue: Remaining pixels
        if screen_x < screen_width {
            let count = screen_width - screen_x;
            let tile_h = ((scrolled_h as usize) >> 3) & plane_mask;
            let entry = self.fetch_nametable_entry(name_table_base, tile_v, tile_h, plane_w);

            let priority = (entry & 0x8000) != 0;
            if priority == priority_filter {
                let palette = ((entry >> 13) & 0x03) as u8;
                let v_flip = (entry & 0x1000) != 0;
                let h_flip = (entry & 0x0800) != 0;
                let tile_index = entry & 0x07FF;

                let patterns = self.fetch_tile_pattern(tile_index, pixel_v as u16, v_flip);

                self.draw_tile_segment(
                    patterns,
                    palette,
                    h_flip,
                    0,
                    count as u16,
                    line_offset + screen_x as usize,
                );
            }
        }
    }

}

impl Debuggable for Vdp {
    fn read_state(&self) -> Value {
        json!({
            "status": self.status,
            "h_counter": self.h_counter,
            "v_counter": self.v_counter,
            "dma_pending": self.dma_pending,
            "registers": self.registers,
            "control": {
                "pending": self.control_pending,
                "code": self.control_code,
                "address": self.control_address,
            }
        })
    }

    fn write_state(&mut self, state: &Value) {
        if let Some(status) = state["status"].as_u64() {
            self.status = status as u16;
        }
        if let Some(h_counter) = state["h_counter"].as_u64() {
            self.h_counter = h_counter as u16;
        }
        if let Some(v_counter) = state["v_counter"].as_u64() {
            self.v_counter = v_counter as u16;
        }
        if let Some(dma_pending) = state["dma_pending"].as_bool() {
            self.dma_pending = dma_pending;
        }

        if let Some(registers) = state["registers"].as_array() {
            for (i, val) in registers.iter().enumerate() {
                if i < 24 {
                    if let Some(v) = val.as_u64() {
                        self.registers[i] = v as u8;
                    }
                }
            }
        }

        let control = &state["control"];
        if let Some(pending) = control["pending"].as_bool() {
            self.control_pending = pending;
        }
        if let Some(code) = control["code"].as_u64() {
            self.control_code = code as u8;
        }
        if let Some(address) = control["address"].as_u64() {
            self.control_address = address as u16;
        }
    }
}

#[cfg(test)]
mod tests_render;

#[cfg(test)]
mod tests_dma;

#[cfg(test)]
mod test_command;

#[cfg(test)]
mod tests_control;

#[cfg(test)]
mod tests_properties;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_vdp_debuggable() {
        let mut vdp = Vdp::new();
        let state = json!({
            "status": 0x1234,
            "h_counter": 0x56,
            "v_counter": 0x78,
            "dma_pending": true,
            "registers": [
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24
            ],
            "control": {
                "pending": true,
                "code": 0x0F,
                "address": 0x3FFF
            }
        });

        vdp.write_state(&state);

        assert_eq!(vdp.status, 0x1234);
        assert_eq!(vdp.h_counter, 0x56);
        assert_eq!(vdp.v_counter, 0x78);
        assert_eq!(vdp.dma_pending, true);
        assert_eq!(vdp.registers[0], 1);
        assert_eq!(vdp.registers[23], 24);
        assert_eq!(vdp.control_pending, true);
        assert_eq!(vdp.control_code, 0x0F);
        assert_eq!(vdp.control_address, 0x3FFF);

        // Verify read_state mirrors the written state
        let new_state = vdp.read_state();
        assert_eq!(new_state["status"], 0x1234);
        assert_eq!(new_state["registers"][23], 24);
        assert_eq!(new_state["control"]["address"], 0x3FFF);
    }
}

#[cfg(test)]
mod tests_security;

#[cfg(test)]
mod tests_bulk_write;
