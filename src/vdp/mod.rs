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
    pub is_pal: bool,

    /// Framebuffer (320x240 RGB565)
    pub framebuffer: Vec<u16>,
}

impl Vdp {
    pub fn new() -> Self {
        Vdp {
            vram: [0; 0x10000],
            cram: [0; 128],
            cram_cache: [0; 64],
            vsram: [0; 80],
            registers: [0; 24],
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

    pub fn write_data(&mut self, value: u16) {
        self.control_pending = false;
        self.last_data_write = value;

        // DMA fill check (if configured)
        if (self.registers[1] & 0x10) != 0 && (self.registers[23] & 0x80) != 0 {
            // DMA fill implementation would go here
            // For now, just handle normal write
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
                    val = (val >> 8) | (val << 8);
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
                self.cram_cache[addr >> 1] = ((r5 as u16) << 11) | ((g6 as u16) << 5) | (b5 as u16);

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
            let high = ((value >> 2) & 0x3C) as u8;
            self.control_code = (self.control_code & 0x03) | high;
            self.control_address = (self.control_address & 0x3FFF) | ((value & 0x3) << 14);
            self.control_pending = false;

            // DMA initiation check
            if (self.control_code & 0x20) != 0 {
                // DMA requested
                self.dma_pending = true;
            }
        } else {
            if (value & 0xC000) == 0x8000 {
                // Register write
                let reg = ((value >> 8) & 0x1F) as usize;
                let val = (value & 0xFF) as u8;
                if reg < 24 {
                    self.registers[reg] = val;
                }
            } else {
                // First word of command
                self.control_code = ((value >> 14) & 0x03) as u8;
                self.control_address = (value & 0x3FFF) as u16;
                self.control_pending = true;
            }
        }
    }

    pub fn read_status(&self) -> u16 {
        self.status
    }

    // Helper methods
    fn auto_increment(&self) -> u8 {
        self.registers[15]
    }

    pub fn mode1(&self) -> u8 {
        self.registers[0]
    }

    pub fn mode2(&self) -> u8 {
        self.registers[1]
    }

    pub fn h40_mode(&self) -> bool {
        (self.registers[12] & 0x81) == 0x81
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
        (self.registers[1] & 0x08) != 0
    }

    pub fn display_enabled(&self) -> bool {
        (self.registers[1] & 0x40) != 0
    }

    pub fn dma_enabled(&self) -> bool {
        (self.registers[1] & 0x10) != 0
    }

    pub fn dma_mode(&self) -> u8 {
        // Bit 4 of Reg 23 determines mode (0=memory, 1=vram fill/copy)
        // Bit 5 is unused? Actually bit 6 and 7 of reg 23 are type.
        // Simplified: Reg 23
        self.registers[23]
    }

    pub fn dma_source(&self) -> u32 {
        ((self.registers[23] as u32) << 17)
            | ((self.registers[22] as u32) << 9)
            | ((self.registers[21] as u32) << 1)
    }

    pub fn dma_length(&self) -> u32 {
        ((self.registers[20] as u32) << 8) | (self.registers[19] as u32)
    }

    pub fn dma_source_transfer(&self) -> u32 {
        ((self.registers[23] as u32 & 0x3F) << 17)
            | ((self.registers[22] as u32) << 9)
            | ((self.registers[21] as u32) << 1)
    }

    /// Check if DMA mode is 0 or 1 (68k Transfer)
    pub fn is_dma_transfer(&self) -> bool {
        // Mode bit is bit 7 of reg 23?
        // Actually:
        // Reg 23:
        // Bit 7: Type (0=transfer, 1=fill/copy)
        // Bit 6: Type (0=fill, 1=copy) - only if Bit 7 is 1
        // So for transfer, Bit 7 must be 0.
        (self.registers[23] & 0x80) == 0
    }

    pub fn execute_dma(&mut self) -> u32 {
        // Simple VRAM fill implementation for now
        let length = self.dma_length();
        let mode = self.registers[23] & 0xC0;

        if mode == 0x80 {
            // VRAM Fill
            let data = self.last_data_write;
            // Byte write to VRAM
            let mut addr = self.control_address;
            let inc = self.auto_increment() as u16;

            for _ in 0..length {
                if (addr as usize) < 0x10000 {
                    // VRAM fill writes the lower byte of data to the VRAM address
                    // If address is even, it writes high byte of word?
                    // VRAM is bytes.
                    // Actually VRAM fill repeats the data to the VRAM port.
                    // For now, simple implementation
                    let val = if (addr & 1) == 0 {
                        (data >> 8) as u8
                    } else {
                        (data & 0xFF) as u8
                    };
                    self.vram[addr as usize] = val;
                }
                addr = addr.wrapping_add(inc);
            }
            self.control_address = addr;
        }

        length // Return cycles used (placeholder)
    }

    pub fn sprite_table_address(&self) -> u16 {
        let mask = if self.h40_mode() { 0xFE00 } else { 0xFE00 }; // simplified
        ((self.registers[5] as u16) << 9) & mask
    }

    pub fn plane_a_address(&self) -> usize {
        ((self.registers[2] as usize) & 0x38) << 10
    }

    pub fn plane_b_address(&self) -> usize {
        ((self.registers[4] as usize) & 0x07) << 13
    }

    pub fn hscroll_address(&self) -> usize {
        ((self.registers[13] as usize) & 0x3F) << 10
    }

    pub fn plane_size(&self) -> (usize, usize) {
        let w = match self.registers[16] & 0x03 {
            0 => 32,
            1 => 64,
            _ => 128, // 2 and 3 are invalid but behave like 64 or 128
        };
        let h = match (self.registers[16] >> 4) & 0x03 {
            0 => 32,
            1 => 64,
            _ => 128,
        };
        (w, h)
    }

    fn bg_color(&self) -> (u8, u8) {
        let bg_idx = self.registers[7];
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
        (self.status & 0x0008) != 0 && (self.registers[1] & 0x20) != 0
    }

    /// Set VBlank status
    pub fn set_vblank(&mut self, active: bool) {
        if active {
            self.status |= 0x0008; // VBlank flag
            self.status |= 0x0080; // VInterrupt pending
        } else {
            self.status &= !0x0008;
            self.status &= !0x0080;
        }
    }

    pub fn trigger_vint(&mut self) {
        self.status |= 0x0080;
    }

    /// Check if HBlank interrupt is pending
    pub fn hblank_pending(&self) -> bool {
        // Simplified
        (self.registers[0] & 0x10) != 0
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
        for x in 0..width as usize {
            self.framebuffer[line_offset + x] = bg_color;
        }

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

    fn fetch_sprite_attributes(&self, sat_base: usize, index: u8) -> SpriteAttributes {
        let addr = sat_base + (index as usize * 8);

        let cur_v = (((self.vram[addr] as u16) << 8) | (self.vram[addr + 1] as u16)) & 0x03FF;
        let v_pos = cur_v.wrapping_sub(128);

        let size = self.vram[addr + 2];
        let h_size = ((size >> 2) & 0x03) + 1;
        let v_size = (size & 0x03) + 1;

        let link = self.vram[addr + 3] & 0x7F;

        let attr = ((self.vram[addr + 4] as u16) << 8) | (self.vram[addr + 5] as u16);
        let priority = (attr & 0x8000) != 0;
        let palette = ((attr >> 13) & 0x03) as u8;
        let v_flip = (attr & 0x1000) != 0;
        let h_flip = (attr & 0x0800) != 0;
        let base_tile = attr & 0x07FF;

        let cur_h = (((self.vram[addr + 6] as u16) << 8) | (self.vram[addr + 7] as u16)) & 0x03FF;
        let h_pos = cur_h.wrapping_sub(128);

        SpriteAttributes {
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
        }
    }

    fn render_sprite_scanline(
        &mut self,
        line: u16,
        attr: &SpriteAttributes,
        line_offset: usize,
        screen_width: u16,
    ) {
        let sprite_h_px = (attr.h_size as u16) * 8;
        let sprite_v_px = (attr.v_size as u16) * 8;

        let py = line - attr.v_pos;
        let fetch_py = if attr.v_flip {
            (sprite_v_px - 1) - py
        } else {
            py
        };

        let tile_v_offset = fetch_py / 8;
        let pixel_v = fetch_py % 8;

        for px in 0..sprite_h_px {
            let screen_x = attr.h_pos.wrapping_add(px);
            if screen_x >= screen_width {
                continue;
            }

            let fetch_px = if attr.h_flip {
                (sprite_h_px - 1) - px
            } else {
                px
            };
            let tile_h_offset = fetch_px / 8;
            let pixel_h = fetch_px % 8;

            // In a multi-tile sprite, tiles are arranged vertically first
            let tile_idx = attr.base_tile + (tile_h_offset * attr.v_size as u16) + tile_v_offset;

            let pattern_addr = (tile_idx * 32) + (pixel_v * 4) + (pixel_h / 2);
            if pattern_addr as usize + 4 > 0x10000 {
                continue;
            }

            let byte = self.vram[pattern_addr as usize];
            let color_idx = if pixel_h % 2 == 0 {
                byte >> 4
            } else {
                byte & 0x0F
            };

            if color_idx != 0 {
                let color = self.get_cram_color(attr.palette, color_idx);
                self.framebuffer[line_offset + screen_x as usize] = color;
            }
        }
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
            // Check SAT boundary
            if sat_base + (sprite_idx as usize * 8) + 8 > 0x10000 {
                break;
            }

            let attr = self.fetch_sprite_attributes(sat_base, sprite_idx as u8);

            // Check if sprite is visible on this line
            let sprite_v_px = (attr.v_size as u16) * 8;
            if attr.priority == priority_filter
                && fetch_line >= attr.v_pos
                && fetch_line < attr.v_pos + sprite_v_px
            {
                self.render_sprite_scanline(fetch_line, &attr, line_offset, screen_width);
            }

            sprite_count += 1;
            sprite_idx = attr.link;
            if sprite_idx == 0 || sprite_count >= max_sprites {
                break;
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

        // Get vertical scroll
        let vs_addr = if is_plane_a { 0 } else { 2 };
        let v_scroll =
            (((self.vsram[vs_addr] as u16) << 8) | (self.vsram[vs_addr + 1] as u16)) & 0x03FF;

        // Get horizontal scroll (per-screen for now)
        let hs_base = self.hscroll_address();
        let hs_addr = if is_plane_a { hs_base } else { hs_base + 2 };
        let hi = self.vram[hs_addr as usize];
        let lo = self.vram[hs_addr as usize + 1];
        let h_scroll = (((hi as u16) << 8) | (lo as u16)) & 0x03FF;
        let scrolled_v = fetch_line.wrapping_add(v_scroll);
        let tile_v = (scrolled_v as usize / 8) % plane_h;
        let pixel_v = scrolled_v % 8;

        let screen_width = self.screen_width();
        let line_offset = (draw_line as usize) * 320;

        let mut screen_x: u16 = 0;
        let mut scrolled_h = (0u16).wrapping_sub(h_scroll);

        while screen_x < screen_width {
            let pixel_h = scrolled_h % 8;
            let pixels_left_in_tile = 8 - pixel_h;
            let pixels_to_process = std::cmp::min(pixels_left_in_tile, screen_width - screen_x);

            let tile_h = (scrolled_h as usize / 8) % plane_w;

            // Fetch nametable entry (2 bytes)
            let nt_entry_addr = name_table_base + (tile_v * plane_w + tile_h) * 2;
            let hi = self.vram[nt_entry_addr & 0xFFFF];
            let lo = self.vram[(nt_entry_addr + 1) & 0xFFFF];
            let entry = ((hi as u16) << 8) | (lo as u16);

            let priority = (entry & 0x8000) != 0;
            if priority != priority_filter {
                screen_x += pixels_to_process;
                scrolled_h = scrolled_h.wrapping_add(pixels_to_process);
                continue;
            }

            let palette = ((entry >> 13) & 0x03) as u8;
            let v_flip = (entry & 0x1000) != 0;
            let h_flip = (entry & 0x0800) != 0;
            let tile_index = entry & 0x07FF;

            let row = if v_flip { 7 - pixel_v } else { pixel_v };
            let row_addr = (tile_index as usize * 32) + (row as usize * 4);

            // Prefetch the 4 bytes of pattern data for this row
            let p0 = self.vram[row_addr & 0xFFFF];
            let p1 = self.vram[(row_addr + 1) & 0xFFFF];
            let p2 = self.vram[(row_addr + 2) & 0xFFFF];
            let p3 = self.vram[(row_addr + 3) & 0xFFFF];
            let patterns = [p0, p1, p2, p3];

            for i in 0..pixels_to_process {
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
                    self.framebuffer[line_offset + (screen_x + i) as usize] = color;
                }
            }
            screen_x += pixels_to_process;
            scrolled_h = scrolled_h.wrapping_add(pixels_to_process);
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
            "control": {
                "pending": self.control_pending,
                "code": self.control_code,
                "address": self.control_address,
            }
        })
    }

    fn write_state(&mut self, _state: &Value) {
        // Not implemented
    }
}

#[cfg(test)]
mod test_command;
