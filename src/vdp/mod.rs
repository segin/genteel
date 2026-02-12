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
    pub registers: [u8; 24],

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

    /// PAL region flag
    pub is_pal: bool,

    /// Framebuffer (320×240 pixels max, 2 bytes per pixel for RGB565)
    pub framebuffer: Vec<u16>,
}

impl Vdp {
    /// Create a new VDP
    pub fn new() -> Self {
        Self {
            vram: [0; 0x10000],
            cram: [0; 128],
            cram_cache: [0; 64],
            vsram: [0; 80],
            registers: [0; 24],
            control_pending: false,
            control_code: 0,
            control_address: 0,
            dma_pending: false,
            status: 0x3400, // Initial status: FIFO empty, etc.
            h_counter: 0,
            v_counter: 0,
            line_counter: 0,
            last_data_write: 0,
            v30_offset: 0,
            is_pal: false,
            framebuffer: vec![0; 320 * 240],
        }
    }

    /// Reset the VDP
    pub fn reset(&mut self) {
        self.vram.fill(0);
        self.cram.fill(0);
        self.cram_cache.fill(0);
        self.vsram.fill(0);
        self.registers.fill(0);
        self.control_pending = false;
        self.control_code = 0;
        self.control_address = 0;
        self.dma_pending = false;
        self.status = 0x3400;
        self.h_counter = 0;
        self.v_counter = 0;
        self.line_counter = 0;
        self.v30_offset = 0;
        self.framebuffer.fill(0);
    }

    // === Register accessors ===

    /// Mode Register 1 (register 0)
    pub fn mode1(&self) -> u8 {
        self.registers[0]
    }

    /// Mode Register 2 (register 1)
    pub fn mode2(&self) -> u8 {
        self.registers[1]
    }

    /// Plane A Name Table Address (register 2)
    /// Returns the VRAM address of plane A nametable
    pub fn plane_a_address(&self) -> u16 {
        ((self.registers[2] & 0x38) as u16) << 10
    }

    /// Window Name Table Address (register 3)
    pub fn window_address(&self) -> u16 {
        ((self.registers[3] & 0x3E) as u16) << 10
    }

    /// Plane B Name Table Address (register 4)
    pub fn plane_b_address(&self) -> u16 {
        ((self.registers[4] & 0x07) as u16) << 13
    }

    /// Sprite Attribute Table Address (register 5)
    pub fn sprite_table_address(&self) -> u16 {
        ((self.registers[5] & 0x7F) as u16) << 9
    }

    /// Background Color (register 7)
    /// Returns palette line (0-3) and color index (0-15)
    pub fn bg_color(&self) -> (u8, u8) {
        let reg = self.registers[7];
        ((reg >> 4) & 0x03, reg & 0x0F)
    }

    /// H Interrupt Counter (register 10)
    pub fn hint_counter(&self) -> u8 {
        self.registers[10]
    }

    /// Mode Register 3 (register 11)
    pub fn mode3(&self) -> u8 {
        self.registers[11]
    }

    /// Mode Register 4 (register 12)
    pub fn mode4(&self) -> u8 {
        self.registers[12]
    }

    /// H Scroll Data Address (register 13)
    pub fn hscroll_address(&self) -> u16 {
        ((self.registers[13] & 0x3F) as u16) << 10
    }

    /// Auto-Increment Value (register 15)
    pub fn auto_increment(&self) -> u8 {
        self.registers[15]
    }

    /// Plane Size (register 16)
    /// Returns (width, height) in tiles (32, 64, or 128)
    pub fn plane_size(&self) -> (u16, u16) {
        let reg = self.registers[16];
        let w = match reg & 0x03 {
            0 => 32,
            1 => 64,
            3 => 128,
            _ => 32,
        };
        let h = match (reg >> 4) & 0x03 {
            0 => 32,
            1 => 64,
            3 => 128,
            _ => 32,
        };
        (w, h)
    }

    /// Window H Position (register 17)
    pub fn window_h_pos(&self) -> (bool, u8) {
        let reg = self.registers[17];
        let right = (reg & 0x80) != 0;
        let pos = reg & 0x1F;
        (right, pos)
    }

    /// Window V Position (register 18)
    pub fn window_v_pos(&self) -> (bool, u8) {
        let reg = self.registers[18];
        let down = (reg & 0x80) != 0;
        let pos = reg & 0x1F;
        (down, pos)
    }

    /// Display enabled?
    pub fn display_enabled(&self) -> bool {
        (self.registers[1] & 0x40) != 0
    }

    /// DMA enabled?
    pub fn dma_enabled(&self) -> bool {
        (self.registers[1] & 0x10) != 0
    }

    /// Return the current DMA mode (00, 01, 10, 11)
    pub fn dma_mode(&self) -> u8 {
        self.registers[23] >> 6
    }

    /// Return the DMA source address from registers 21-23
    /// Note: This is the raw value from registers (effectively source >> 1 for 68k transfers)
    pub fn dma_source(&self) -> u32 {
        ((self.registers[23] as u32 & 0x3F) << 16)
            | ((self.registers[22] as u32) << 8)
            | (self.registers[21] as u32)
    }

    /// Return the current control code (0-63)
    pub fn control_code(&self) -> u8 {
        self.control_code
    }

    /// V30 mode (30 rows instead of 28)?
    pub fn v30_mode(&self) -> bool {
        (self.registers[1] & 0x08) != 0
    }

    /// H40 mode (40 columns instead of 32)?
    pub fn h40_mode(&self) -> bool {
        (self.registers[12] & 0x80) != 0
    }

    /// Screen height in pixels
    pub fn screen_height(&self) -> u16 {
        if self.v30_mode() { 240 } else { 224 }
    }

    /// Screen width in pixels
    pub fn screen_width(&self) -> u16 {
        if self.h40_mode() { 320 } else { 256 }
    }

    // === DMA Helpers ===

    /// Calculate DMA length from registers 19 and 20
    pub fn dma_length(&self) -> u32 {
        let len = ((self.registers[20] as u32) << 8) | (self.registers[19] as u32);
        if len == 0 { 0x10000 } else { len }
    }

    /// Calculate DMA source address for 68k transfer (Mode 0/1)
    /// This returns the byte address (shifted left by 1 from the register value)
    pub fn dma_source_transfer(&self) -> u32 {
        ((self.registers[23] as u32 & 0x3F) << 17)
            | ((self.registers[22] as u32) << 9)
            | ((self.registers[21] as u32) << 1)
    }

    /// Check if DMA mode is 0 or 1 (68k Transfer)
    pub fn is_dma_transfer(&self) -> bool {
        self.dma_mode() <= 1
    }

    /// Set the region (PAL=true, NTSC=false)
    pub fn set_region(&mut self, is_pal: bool) {
        self.is_pal = is_pal;
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

    // === Port I/O ===

    /// Read from data port
    pub fn read_data(&mut self) -> u16 {
        self.control_pending = false;

        let addr = self.control_address;
        let data = match self.control_code & 0x0F {
            0x00 => {
                // VRAM read - always word aligned
                let aligned_addr = addr & 0xFFFE;
                let hi = self.vram[aligned_addr as usize] as u16;
                let lo = self.vram[(aligned_addr.wrapping_add(1)) as usize] as u16;
                (hi << 8) | lo
            }
            0x08 => {
                // CRAM read - always word aligned
                let cram_addr = (addr & 0x7E) as usize;
                let hi = self.cram[cram_addr] as u16;
                let lo = self.cram[cram_addr | 1] as u16;
                (hi << 8) | lo
            }
            0x04 => {
                // VSRAM read - always word aligned
                let vsram_addr = (addr & 0x7E) as usize;
                if vsram_addr < 80 {
                    let hi = self.vsram[vsram_addr] as u16;
                    let lo = self.vsram[(vsram_addr + 1).min(79)] as u16;
                    (hi << 8) | lo
                } else {
                    0
                }
            }
            _ => 0,
        };

        // Auto-increment address
        self.control_address = self.control_address.wrapping_add(self.auto_increment() as u16);

        data
    }

    /// Write to data port
    pub fn write_data(&mut self, value: u16) {
        self.control_pending = false;
        self.last_data_write = value;

        let addr = self.control_address;
        match self.control_code & 0x0F {
            0x01 => {
                // VRAM write
                let vram_addr = addr as usize & 0xFFFF;
                self.vram[vram_addr] = (value >> 8) as u8;
                self.vram[(vram_addr + 1) & 0xFFFF] = value as u8;
            }
            0x03 => { // CRAM write
                let cram_addr = (addr & 0x7E) as usize; // Ensure word alignment

                // If writing to odd address, swap bytes ("interesting side effect")
                let write_val = if addr & 1 != 0 {
                    (value << 8) | (value >> 8)
                } else {
                    value
                };

                self.cram[cram_addr] = (write_val >> 8) as u8;
                self.cram[cram_addr + 1] = (write_val & 0xFF) as u8;
                self.update_cram_cache(cram_addr >> 1);

                if write_val != 0 {
                    eprintln!("DEBUG: NON-ZERO CRAM WRITE: addr=0x{:02X} val=0x{:04X}", cram_addr, write_val);
                }
            }
            0x05 => {
                // VSRAM write
                let vsram_addr = (addr & 0x7E) as usize;
                if vsram_addr < 80 {
                    self.vsram[vsram_addr] = (value >> 8) as u8;
                    self.vsram[(vsram_addr + 1).min(79)] = value as u8;
                }
            }
            _ => {}
        }

        // Auto-increment address
        self.control_address = self.control_address.wrapping_add(self.auto_increment() as u16);
    }

    /// Read Status Register
    pub fn read_status(&mut self) -> u16 {
        self.control_pending = false;
        let mut status = self.status;
        // set bit 3 if in VBlank
        if self.v_counter >= 224 {
            status |= 0x0008;
        }

        // Clear VInt pending bit on read
        self.status &= !0x0080;

        // Ensure other status bits (like DMA Busy, etc) are correct
        if self.dma_pending {
            status |= 0x0002;
        }

        // Bit 9: FIFO empty (always empty for now)
        status |= 0x0200;

        status
    }

    /// Set VInt pending bit (bit 7)
    pub fn trigger_vint(&mut self) {
        self.status |= 0x0080;
    }

    /// Write to control port
    pub fn write_control(&mut self, value: u16) {
        if !self.control_pending {
            // First word
            if (value & 0xC000) == 0x8000 {
                // Register write: 100RRRRR DDDDDDDD
                let reg = ((value >> 8) & 0x1F) as usize;
                let data = value as u8;
                if reg < 24 {
                    if self.registers[reg] != data {
                        self.registers[reg] = data;
                        eprintln!("DEBUG: VDP REG WRITE: Reg {} = 0x{:02X}", reg, data);
                    }
                }
                self.control_pending = false;
            } else {
                // First half of command word
                self.control_address = (self.control_address & 0xC000) | (value & 0x3FFF);
                self.control_code = (self.control_code & 0x3C) | ((value >> 14) as u8 & 0x03);
                self.control_pending = true;
            }
        } else {
            // Second word of command
            self.control_address = (self.control_address & 0x3FFF) | ((value & 0x0003) << 14);

            // Command bits CD5..CD2 are in bits 13..10 of the second word
            // Extract them and place them in bits 5..2 of control_code
            let cd_upper = ((value >> 8) & 0x3C) as u8;
            self.control_code = (self.control_code & 0x03) | cd_upper;

            self.control_pending = false;

            eprintln!("DEBUG: VDP COMMAND: addr=0x{:04X} code=0x{:02X}", self.control_address, self.control_code);

            // Check for DMA
            if self.dma_enabled() && (self.control_code & 0x20) != 0 {
                eprintln!("DEBUG: VDP DMA PENDING SET");
                self.dma_pending = true;
            }
        }
    }

    /// Get current control address (for testing)
    #[cfg(test)]
    pub fn get_control_address(&self) -> u16 {
        self.control_address
    }

    /// Get current control code (for testing)
    #[cfg(test)]
    pub fn get_control_code(&self) -> u8 {
        self.control_code
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
        for x in 0..width as usize {
            self.framebuffer[line_offset + x] = bg_color;
        }

        if !self.display_enabled() {
            return;
        }

        // Plane rendering (Low priority)
        // Plane rendering (Low priority)
        self.render_plane(false, fetch_line, draw_line, false); // Plane B low
        self.render_plane(true, fetch_line, draw_line, false);  // Plane A low

        // Sprites low priority
        self.render_sprites(fetch_line, draw_line, false);

        // Plane rendering (High priority)
        self.render_plane(false, fetch_line, draw_line, true); // Plane B high
        self.render_plane(true, fetch_line, draw_line, true);  // Plane A high
        // Sprites high priority
        self.render_sprites(fetch_line, draw_line, true);
    }

    fn render_sprites(&mut self, fetch_line: u16, draw_line: u16, priority_filter: bool) {
        let sat_base = self.sprite_table_address() as usize;
        let mut sprite_idx = 0;
        let mut sprite_count = 0;
        let max_sprites = if self.h40_mode() { 80 } else { 64 };

        let screen_width = self.screen_width();
        let line_offset = (draw_line as usize) * 320;

        // SAT structure is 8 bytes per sprite
        // We follow the 'link' pointer starting from sprite 0
        loop {
            let addr = sat_base + (sprite_idx as usize * 8);
            if addr + 8 > 0x10000 { break; }

            let cur_v = (((self.vram[addr] as u16) << 8) | (self.vram[addr + 1] as u16)) & 0x03FF;
            let v_pos = cur_v.wrapping_sub(128);

            let size = self.vram[addr + 2];
            let sprite_h_tiles = ((size >> 2) & 0x03) + 1;
            let sprite_v_tiles = (size & 0x03) + 1;
            let sprite_h_px = sprite_h_tiles * 8;
            let sprite_v_px = sprite_v_tiles * 8;

            let link = self.vram[addr + 3] & 0x7F;

            let attr = ((self.vram[addr + 4] as u16) << 8) | (self.vram[addr + 5] as u16);
            let priority = (attr & 0x8000) != 0;
            let palette = ((attr >> 13) & 0x03) as u8;
            let v_flip = (attr & 0x1000) != 0;
            let h_flip = (attr & 0x0800) != 0;
            let base_tile = attr & 0x07FF;

            let cur_h = (((self.vram[addr + 6] as u16) << 8) | (self.vram[addr + 7] as u16)) & 0x03FF;
            let h_pos = cur_h.wrapping_sub(128);

            // Check if sprite is visible on this line
            if priority == priority_filter && fetch_line >= v_pos && fetch_line < v_pos + sprite_v_px as u16 {
                let py = fetch_line - v_pos;
                let fetch_py = if v_flip { (sprite_v_px as u16 - 1) - py } else { py };

                let tile_v_offset = fetch_py / 8;
                let pixel_v = fetch_py % 8;

                for px in 0..sprite_h_px {
                    let screen_x = h_pos + px as u16;
                    if screen_x >= screen_width { continue; }

                    let fetch_px = if h_flip { (sprite_h_px as u16 - 1) - px as u16 } else { px as u16 };
                    let tile_h_offset = fetch_px / 8;
                    let pixel_h = fetch_px % 8;

                    // In a multi-tile sprite, tiles are arranged vertically first
                    let tile_idx = base_tile + (tile_h_offset * sprite_v_tiles as u16) + tile_v_offset;

                    let pattern_addr = (tile_idx * 32) + (pixel_v * 4) + (pixel_h / 2);
                    if pattern_addr as usize + 4 > 0x10000 { continue; }

                    let byte = self.vram[pattern_addr as usize];
                    let color_idx = if pixel_h % 2 == 0 { byte >> 4 } else { byte & 0x0F };

                    if color_idx != 0 {
                        let color = self.get_cram_color(palette, color_idx);
                        self.framebuffer[line_offset + screen_x as usize] = color;
                    }
                }
            }

            sprite_count += 1;
            sprite_idx = link;
            if sprite_idx == 0 || sprite_count >= max_sprites {
                break;
            }
        }
    }

    fn render_plane(&mut self, is_plane_a: bool, fetch_line: u16, draw_line: u16, priority_filter: bool) {
        let (plane_w, plane_h) = self.plane_size();
        let name_table_base = if is_plane_a { self.plane_a_address() } else { self.plane_b_address() };

        // Get vertical scroll
        let vs_addr = if is_plane_a { 0 } else { 2 };
        let v_scroll = (((self.vsram[vs_addr] as u16) << 8) | (self.vsram[vs_addr + 1] as u16)) & 0x03FF;

        // Get horizontal scroll (per-screen for now)
        let hs_base = self.hscroll_address();
        let hs_addr = if is_plane_a { hs_base } else { hs_base + 2 };
        let hi = self.vram[hs_addr as usize];
        let lo = self.vram[hs_addr as usize + 1];
        let h_scroll = (((hi as u16) << 8) | (lo as u16)) & 0x03FF;
        let scrolled_v = fetch_line.wrapping_add(v_scroll);
        let tile_v = (scrolled_v / 8) % plane_h;
        let pixel_v = scrolled_v % 8;

        let screen_width = self.screen_width();
        let line_offset = (draw_line as usize) * 320;

        for screen_x in 0..screen_width {
            let scrolled_h = (screen_x as u16).wrapping_sub(h_scroll);
            let tile_h = (scrolled_h / 8) % plane_w;
            let pixel_h = scrolled_h % 8;

            // Fetch nametable entry (2 bytes)
            let nt_entry_addr = name_table_base + (tile_v * plane_w + tile_h) * 2;
            let hi = self.vram[nt_entry_addr as usize];
            let lo = self.vram[nt_entry_addr as usize + 1];
            let entry = ((hi as u16) << 8) | (lo as u16);

            let priority = (entry & 0x8000) != 0;
            if priority != priority_filter {
                continue;
            }

            let palette = ((entry >> 13) & 0x03) as u8;
            let v_flip = (entry & 0x1000) != 0;
            let h_flip = (entry & 0x0800) != 0;
            let tile_idx = entry & 0x07FF;

            // Fetch tile pixel (4 bits per pixel)
            let fetch_v = if v_flip { 7 - pixel_v } else { pixel_v };
            let fetch_h = if h_flip { 7 - pixel_h } else { pixel_h };

            let pattern_addr = (tile_idx * 32) + (fetch_v * 4) + (fetch_h / 2);
            let byte = self.vram[pattern_addr as usize];
            let color_idx = if fetch_h % 2 == 0 { byte >> 4 } else { byte & 0x0F };

            // Color 0 is transparent
            if color_idx != 0 {
                let color = self.get_cram_color(palette, color_idx);
                self.framebuffer[line_offset + screen_x as usize] = color;
            }
        }
    }

    fn update_cram_cache(&mut self, index: usize) {
        if index >= 64 {
            return;
        }

        let addr = index * 2;
        let hi = self.cram[addr] as u16;
        let lo = self.cram[addr + 1] as u16;
        let val = (hi << 8) | lo;
        self.cram_cache[index] = self.genesis_color_to_rgb565(val);
    }

    /// Helper to convert Genesis color to RGB565
    fn genesis_color_to_rgb565(&self, color: u16) -> u16 {
        // Genesis color format: ----BBB-GGG-RRR-
        // Convert to RGB565: RRRRR GGGGGG BBBBB
        let r = ((color >> 1) & 0x07) as u16;
        let g = ((color >> 5) & 0x07) as u16;
        let b = ((color >> 9) & 0x07) as u16;

        // Scale 3-bit to 5/6-bit
        let r5 = (r << 2) | (r >> 1);
        let g6 = (g << 3) | g;
        let b5 = (b << 2) | (b >> 1);

        (r5 << 11) | (g6 << 5) | b5
    }

    /// Get color from CRAM as RGB565
    fn get_cram_color(&self, palette: u8, index: u8) -> u16 {
        let entry = ((palette as usize) << 4) | (index as usize);
        if entry >= 64 {
            return 0;
        }
        self.cram_cache[entry]
    }

    /// Public wrapper for get_cram_color (for testing)
    #[cfg(test)]
    pub fn get_cram_color_pub(&self, palette: u8, index: u8) -> u16 {
        self.get_cram_color(palette, index)
    }

    /// Render a full frame
    pub fn render_frame(&mut self) {
        let height = self.screen_height();
        for line in 0..height {
            self.render_line(line);
        }
    }

    /// Execute pending DMA operation
    /// Returns the number of bytes transferred (for cycle timing)
    pub fn execute_dma(&mut self) -> u32 {
        if !self.dma_pending {
            return 0;
        }
        // DMA type is in register 23 bits 6-7
        let dma_type = self.dma_mode() & 0x03;

        // DMA length from registers 19-20 (in words for most modes)
        let dma_length = self.dma_length();
        // DMA source from registers 21-23 (bits 0-5 of reg 23)
        // Note: Registers store A23-A1, so we shift left by 1 to get byte address
        let dma_source = self.dma_source() << 1;
        let bytes_transferred = match dma_type {
            0 | 1 => {
                // 68k to VRAM/CRAM/VSRAM - requires bus access (handled externally)
                // This mode needs M68k bus access which we don't have here
                // For now, just clear the pending flag - actual transfer should be
                // done by the main loop with bus access
                0
            }
            2 => {
                // VRAM Fill
                let fill_data = (self.last_data_write >> 8) as u8;
                let dest_code = self.control_code & 0x0F;

                for _ in 0..dma_length {
                    match dest_code {
                        0x01 => {
                            let addr = self.control_address as usize;
                            self.vram[addr & 0xFFFF] = fill_data;
                        }
                        0x03 => {
                            let addr = (self.control_address & 0x7F) as usize;
                            self.cram[addr] = fill_data;
                            self.update_cram_cache(addr >> 1);
                        }
                        0x05 => {
                            let addr = (self.control_address & 0x7F) as usize;
                            if addr < 80 {
                                self.vsram[addr] = fill_data;
                            }
                        }
                        _ => {}
                    }
                    self.control_address = self.control_address.wrapping_add(self.auto_increment() as u16);
                }

                self.dma_pending = false;
                eprintln!("DEBUG: VDP FILL/COPY EXECUTE: Mode={} Length={} Dest={:02X}", dma_type, dma_length, dest_code);
                dma_length
            }
            3 => {
                // VRAM Copy
                let mut src = (dma_source as usize) & 0xFFFF;
                let mut dst = self.control_address as usize;

                for _ in 0..dma_length {
                    self.vram[dst & 0xFFFF] = self.vram[src & 0xFFFF];
                    src = src.wrapping_add(1);
                    dst = dst.wrapping_add(self.auto_increment() as usize);
                }

                self.dma_pending = false;
                eprintln!("DEBUG: VDP FILL/COPY EXECUTE: Mode={} Length={}", dma_type, dma_length);
                dma_length
            }
            _ => {
                self.dma_pending = false;
                0
            }
        };

        bytes_transferred
    }
}

impl Default for Vdp {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests_properties;
#[cfg(test)]
mod test_command;
#[cfg(test)]
mod tests_dma;

#[cfg(test)]
mod tests_control;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vdp_new() {
        let vdp = Vdp::new();
        assert_eq!(vdp.vram.len(), 0x10000);
        assert_eq!(vdp.cram.len(), 128);
        assert_eq!(vdp.vsram.len(), 80);
        assert_eq!(vdp.registers.len(), 24);
    }

    #[test]
    fn test_register_write() {
        let mut vdp = Vdp::new();

        // Write to register 0: 0x8004 (reg 0 = 0x04)
        vdp.write_control(0x8004);
        assert_eq!(vdp.registers[0], 0x04);

        // Write to register 1: 0x8174 (reg 1 = 0x74)
        vdp.write_control(0x8174);
        assert_eq!(vdp.registers[1], 0x74);
    }

    #[test]
    fn test_display_mode() {
        let mut vdp = Vdp::new();

        // Enable display
        vdp.write_control(0x8144);
        assert!(vdp.display_enabled());

        // H40 mode
        vdp.write_control(0x8C81);
        assert!(vdp.h40_mode());
        assert_eq!(vdp.screen_width(), 320);
    }

    #[test]
    fn test_vram_write() {
        let mut vdp = Vdp::new();

        // Set VRAM write mode to address 0x0000
        // First word: CD1-0 = 01 (VRAM write), A13-0 = 0
        vdp.write_control(0x4000);
        // Second word: completes the command
        vdp.write_control(0x0000);

        // Write data
        vdp.write_data(0x1234);

        assert_eq!(vdp.vram[0], 0x12);
        assert_eq!(vdp.vram[1], 0x34);
    }

    #[test]
    fn test_cram_write() {
        let mut vdp = Vdp::new();

        // Set CRAM write mode: CD bits = 0011
        vdp.write_control(0xC000);
        vdp.write_control(0x0000);

        // Write color
        vdp.write_data(0x0EEE); // White-ish

        assert_eq!(vdp.cram[0], 0x0E);
        assert_eq!(vdp.cram[1], 0xEE);
    }

    #[test]
    fn test_cram_write_odd_address() {
        let mut vdp = Vdp::new();

        // Set CRAM write mode to address 0x0001 (odd)
        vdp.write_control(0xC001);
        vdp.write_control(0x0000);

        // Write data 0x1234
        vdp.write_data(0x1234);

        // Expected behavior (LSB masked, byte swapped):
        // address = 1 -> mask -> 0
        // value = 0x1234 -> swap -> 0x3412
        // cram[0] = 0x34 (High byte)
        // cram[1] = 0x12 (Low byte)
        assert_eq!(vdp.cram[0], 0x34);
        assert_eq!(vdp.cram[1], 0x12);
    }

    #[test]
    fn test_plane_addresses() {
        let mut vdp = Vdp::new();

        // Set plane A to 0xC000
        vdp.write_control(0x8230); // Register 2 = 0x30
        assert_eq!(vdp.plane_a_address(), 0xC000);

        // Set plane B to 0x2000
        vdp.write_control(0x8401); // Register 4 = 0x01
        assert_eq!(vdp.plane_b_address(), 0x2000);
    }

    #[test]
    fn test_auto_increment() {
        let mut vdp = Vdp::new();

        // Set auto-increment to 2
        vdp.write_control(0x8F02);
        assert_eq!(vdp.auto_increment(), 2);

        // Set VRAM write to address 0x0000
        vdp.write_control(0x4000);
        vdp.write_control(0x0000);

        // Write two words
        vdp.write_data(0x1111);
        vdp.write_data(0x2222);

        assert_eq!(vdp.vram[0], 0x11);
        assert_eq!(vdp.vram[1], 0x11);
        assert_eq!(vdp.vram[2], 0x22);
        assert_eq!(vdp.vram[3], 0x22);
    }

    #[test]
    fn test_plane_rendering() {
        let mut vdp = Vdp::new();

        // 1. Set background color to palette 0, color 0
        vdp.write_control(0x8700);

        // 2. Set Plane A nametable to 0xE000
        vdp.write_control(0x8238);

        // 3. Set auto-increment to 2
        vdp.write_control(0x8F02);

        // 4. Define Tile 1 at VRAM 0x0020 (8x8 pixels, 4 bits each = 32 bytes)
        // Fill it with color index 2
        vdp.write_control(0x4020); vdp.write_control(0x0000);
        for _ in 0..16 {
            vdp.write_data(0x2222);
        }

        // 5. Define CRAM index 2 (Palette 0, Color 2) as pure Red (0x000E)
        vdp.write_control(0xC004); vdp.write_control(0x0000);
        vdp.write_data(0x000E);

        // 6. Set Nametable entry at (0,0) in Plane A (address 0xE000)
        // Entry: bit 15=priority, 14-13=pal, 12=vflip, 11=hflip, 10-0=index
        // We want: Tile 1, Palette 0, No flip, No priority = 0x0001
        vdp.write_control(0x4000); vdp.write_control(0x0002); // 0xE000 -> CD=01, A15-14=11 -> 0x4000 0003?
        // Wait, command format: CD1-0 A13-0 | CD5-2 0000 00 A15-14
        // For 0xE000 (1110 0000 0000 0000):
        // CD=01 (VRAM Write)
        // Word 1: 01 100000 000000 -> 0x6000
        // Word 2: 0000 0000 0000 0011 -> 0x0003
        vdp.write_control(0x6000); vdp.write_control(0x0003);
        vdp.write_data(0x0001);

        // 7. Enable display
        vdp.write_control(0x8144);

        // 8. Render line 0
        vdp.render_line(0);

        // Pixel at (0,0) should be Red (0xF800 in RGB565)
        assert_eq!(vdp.framebuffer[0], 0xF800);
    }

    #[test]
    fn test_sprite_rendering() {
        let mut vdp = Vdp::new();
        vdp.vram.fill(0);

        // 1. Set auto-increment to 2
        vdp.write_control(0x8F02);

        // 2. Set SAT at 0xD000 (Reg 5 = 0x68)
        vdp.write_control(0x8568);

        // 3. Set Plane A background color (Pal 0, Col 0)
        vdp.write_control(0x8700);

        // 4. Define Sprite 0 at (10, 10) -> internal pos (138, 138)
        vdp.write_control(0x5000); vdp.write_control(0x0003);
        vdp.write_data(0x008A); // V-pos 138
        vdp.write_data(0x0000); // Size 1x1, Link 0
        vdp.write_data(0x0001); // Attr: Tile 1, Pal 0
        vdp.write_data(0x008A); // H-pos 138

        // 5. Define Tile 1 at 0x0020 (all color 3)
        vdp.write_control(0x4020); vdp.write_control(0x0000);
        for _ in 0..16 { vdp.write_data(0x3333); }

        // 6. Define CRAM index 3 (Pal 0, Col 3) as Blue (0x0E00)
        vdp.write_control(0xC006); vdp.write_control(0x0000);
        vdp.write_data(0x0E00);

        // 7. Enable display
        vdp.write_control(0x8144);

        // 8. Render line 10
        vdp.render_line(10);

        // Pixel at (10, 10) should be Blue (0x001F in RGB565)
        assert_eq!(vdp.framebuffer[320 * 10 + 10], 0x001F);
    }

    #[test]
    fn test_register_update_idempotent() {
        let mut vdp = Vdp::new();

        // 1. Initial write to Register 5 (Sprite Attribute Table Base)
        // Reg 5: 0x85xx. Let's write 0x68 (default in some docs)
        vdp.write_control(0x8568);
        assert_eq!(vdp.registers[5], 0x68);

        // 2. Write same value again
        // This should trigger the optimization (no write, no log)
        vdp.write_control(0x8568);
        assert_eq!(vdp.registers[5], 0x68);

        // 3. Write different value
        // Reg 5: 0x70
        vdp.write_control(0x8570);
        assert_eq!(vdp.registers[5], 0x70);
}

    #[test]
    fn test_vsram_limits() {
        let mut vdp = Vdp::new();

        // Test 1: Valid Write to Address 0 (VSRAM Write CD=5)
        // Word 1: 0x4000 (CD1-0=01 -> 1)
        // Word 2: 0x0400 (CD5-2=0100 -> 4) -> Total CD=5
        vdp.write_control(0x4000);
        vdp.write_control(0x0400);
        vdp.write_data(0x1234);
        assert_eq!(vdp.vsram[0], 0x12);
        assert_eq!(vdp.vsram[1], 0x34);

        // Test 2: Invalid Write to Address 80
        vdp.vsram[0] = 0; vdp.vsram[1] = 0;
        // Address 80 = 0x50.
        // Word 1: 0x4050.
        // Word 2: 0x0400.
        vdp.write_control(0x4050);
        vdp.write_control(0x0400);
        vdp.write_data(0xDEAD);

        // Verify that VSRAM size is strictly 80 bytes
        assert_eq!(vdp.vsram.len(), 80);

        // Verify no write happened to any part of VSRAM
        for i in 0..80 {
            assert_eq!(vdp.vsram[i], 0, "VSRAM byte {} was modified", i);
        }

        // Test 3: Read from Address 80
        // VSRAM Read CD=4 (000100)
        // Word 1: 0x0000 | 0x0050 = 0x0050.
        // Word 2: 0x0400 (CD5-2=0100 -> 4).
        vdp.write_control(0x0050);
        vdp.write_control(0x0400);
        let val = vdp.read_data();
        assert_eq!(val, 0);
    }

    #[test]
    fn test_read_data_alignment() {
        let mut vdp = Vdp::new();
        vdp.write_control(0x8F02); // Auto-inc = 2

        // --- VRAM Test ---
        // Setup VRAM read at 0x0000 (Even)
        vdp.write_control(0x0000); vdp.write_control(0x0000);
        // Write data
        vdp.write_control(0x4000); vdp.write_control(0x0000);
        vdp.write_data(0xAAAA); // at 0x0000
        vdp.write_data(0xBBBB); // at 0x0002

        // Read back from 0x0000 (Even)
        vdp.write_control(0x0000); vdp.write_control(0x0000);
        assert_eq!(vdp.read_data(), 0xAAAA, "VRAM Even Read Failed");

        // Read back from 0x0001 (Odd) - Should be aligned to 0x0000
        vdp.write_control(0x0001); vdp.write_control(0x0000);
        assert_eq!(vdp.read_data(), 0xAAAA, "VRAM Odd Read Failed");

        // --- CRAM Test ---
        // CRAM Write at 0x0000
        vdp.write_control(0xC000); vdp.write_control(0x0000);
        vdp.write_data(0x0111); // at 0x0000

        // CRAM Read at 0x0000 (Even)
        // CRAM Read cmd: CD=001000 (0x08). Word 2 (CD5..2=0010): 0x0800
        vdp.write_control(0x0000); vdp.write_control(0x0800);
        assert_eq!(vdp.read_data(), 0x0111, "CRAM Even Read Failed");

        // CRAM Read at 0x0001 (Odd)
        vdp.write_control(0x0001); vdp.write_control(0x0800);
        assert_eq!(vdp.read_data(), 0x0111, "CRAM Odd Read Failed");

        // --- VSRAM Test ---
        // VSRAM Write at 0x0000
        vdp.write_control(0x4000); vdp.write_control(0x0400);
        vdp.write_data(0x0222);

        // VSRAM Read at 0x0000 (Even)
        vdp.write_control(0x0000); vdp.write_control(0x0400);
        assert_eq!(vdp.read_data(), 0x0222, "VSRAM Even Read Failed");

        // VSRAM Read at 0x0001 (Odd)
        vdp.write_control(0x0001); vdp.write_control(0x0400);
        assert_eq!(vdp.read_data(), 0x0222, "VSRAM Odd Read Failed");
    }
}

impl Debuggable for Vdp {
    fn read_state(&self) -> Value {
        json!({
            "v_counter": self.v_counter,
            "h_counter": self.h_counter,
            "status": self.status,
            "mode1": self.registers[0],
            "mode2": self.registers[1],
            "mode3": self.registers[11],
            "mode4": self.registers[12],
            "dma_pending": self.dma_pending,
            "control_pending": self.control_pending,
            "control_code": self.control_code,
            "control_address": self.control_address,
            "v30_offset": self.v30_offset,
            "is_pal": self.is_pal,
        })
    }

    fn write_state(&mut self, _state: &Value) {
        // VDP state write not fully supported yet
    }
}
