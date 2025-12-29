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

/// Video Display Processor
#[derive(Debug)]
pub struct Vdp {
    /// Video RAM (64KB) - stores tile patterns and nametables
    pub vram: [u8; 0x10000],

    /// Color RAM (128 bytes) - 64 colors, 2 bytes each (9-bit color)
    /// Format: ----BBB-GGG-RRR- (each component 0-7)
    pub cram: [u8; 128],

    /// Vertical Scroll RAM (80 bytes) - 40 columns × 2 bytes
    pub vsram: [u8; 80],

    /// VDP Registers (24 registers, but only first 24 are meaningful)
    pub registers: [u8; 24],

    /// Control port state
    control_pending: bool,
    control_code: u8,
    control_address: u16,

    /// DMA state
    dma_pending: bool,

    /// Status register
    status: u16,

    /// Horizontal and vertical counters
    h_counter: u16,
    v_counter: u16,

    /// Internal line counter for HINT
    line_counter: u8,

    /// Framebuffer (320×224 pixels, 2 bytes per pixel for RGB565)
    pub framebuffer: Vec<u16>,
}

impl Vdp {
    /// Create a new VDP
    pub fn new() -> Self {
        Self {
            vram: [0; 0x10000],
            cram: [0; 128],
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
            framebuffer: vec![0; 320 * 224],
        }
    }

    /// Reset the VDP
    pub fn reset(&mut self) {
        self.vram.fill(0);
        self.cram.fill(0);
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

    /// V30 mode (30 rows instead of 28)?
    pub fn v30_mode(&self) -> bool {
        (self.registers[1] & 0x08) != 0
    }

    /// H40 mode (40 columns instead of 32)?
    pub fn h40_mode(&self) -> bool {
        (self.registers[12] & 0x81) == 0x81
    }

    /// Screen height in pixels
    pub fn screen_height(&self) -> u16 {
        if self.v30_mode() { 240 } else { 224 }
    }

    /// Screen width in pixels
    pub fn screen_width(&self) -> u16 {
        if self.h40_mode() { 320 } else { 256 }
    }

    // === Port I/O ===

    /// Read from data port
    pub fn read_data(&mut self) -> u16 {
        self.control_pending = false;

        let addr = self.control_address;
        let data = match self.control_code & 0x0F {
            0x00 => {
                // VRAM read
                let hi = self.vram[addr as usize] as u16;
                let lo = self.vram[(addr.wrapping_add(1)) as usize] as u16;
                (hi << 8) | lo
            }
            0x08 => {
                // CRAM read
                let cram_addr = (addr & 0x7F) as usize;
                let hi = self.cram[cram_addr] as u16;
                let lo = self.cram[cram_addr | 1] as u16;
                (hi << 8) | lo
            }
            0x04 => {
                // VSRAM read
                let vsram_addr = (addr & 0x7F) as usize;
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

        let addr = self.control_address;
        match self.control_code & 0x0F {
            0x01 => {
                // VRAM write
                let vram_addr = addr as usize & 0xFFFF;
                self.vram[vram_addr] = (value >> 8) as u8;
                self.vram[(vram_addr + 1) & 0xFFFF] = value as u8;
            }
            0x03 => {
                // CRAM write
                let cram_addr = (addr & 0x7E) as usize;
                self.cram[cram_addr] = (value >> 8) as u8;
                self.cram[cram_addr | 1] = value as u8;
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

    /// Read from control port (status register)
    pub fn read_status(&mut self) -> u16 {
        self.control_pending = false;

        // Build status register
        let mut status = self.status;

        // Bit 1: DMA busy
        if self.dma_pending {
            status |= 0x0002;
        }

        // Bit 2: HBlank
        // Bit 3: VBlank
        if self.v_counter >= self.screen_height() {
            status |= 0x0008;
        }

        // Bit 9: FIFO empty (always empty for now)
        status |= 0x0200;

        status
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
                    self.registers[reg] = data;
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
            self.control_code = (self.control_code & 0x03) | ((value >> 2) as u8 & 0x3C);
            self.control_pending = false;

            // Check for DMA
            if self.dma_enabled() && (self.control_code & 0x20) != 0 {
                self.dma_pending = true;
            }
        }
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

    // === Rendering ===

    /// Render a single scanline
    pub fn render_line(&mut self, line: u16) {
        if line >= self.screen_height() {
            return;
        }

        let width = self.screen_width();
        let line_offset = (line as usize) * 320;

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

        // Render planes (simplified - just background color for now)
        // Full implementation would render:
        // 1. Plane B (low priority)
        // 2. Plane A (low priority)
        // 3. Sprites (low priority)
        // 4. Plane B (high priority)
        // 5. Plane A (high priority)
        // 6. Sprites (high priority)
    }

    /// Get color from CRAM as RGB565
    fn get_cram_color(&self, palette: u8, index: u8) -> u16 {
        let addr = ((palette as usize) << 5) | ((index as usize) << 1);
        if addr >= 128 {
            return 0;
        }

        let hi = self.cram[addr] as u16;
        let lo = self.cram[addr | 1] as u16;
        let color = (hi << 8) | lo;

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

    /// Render a full frame
    pub fn render_frame(&mut self) {
        let height = self.screen_height();
        for line in 0..height {
            self.render_line(line);
        }
    }
}

impl Default for Vdp {
    fn default() -> Self {
        Self::new()
    }
}

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
}
