use super::Vdp;
use super::constants::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct SpriteAttributes {
    pub v_pos: u16,
    pub h_pos: u16,
    pub h_size: u8, // tiles
    pub v_size: u8, // tiles
    pub priority: bool,
    pub palette: u8,
    pub v_flip: bool,
    pub h_flip: bool,
    pub base_tile: u16,
}

pub struct SpriteIterator<'a> {
    pub vram: &'a [u8],
    pub next_idx: u8,
    pub count: usize,
    pub max_sprites: usize,
    pub sat_base: usize,
}

impl<'a> Iterator for SpriteIterator<'a> {
    type Item = SpriteAttributes;

    fn next(&mut self) -> Option<Self::Item> {
        if self.count >= self.max_sprites {
            return None;
        }

        // Check SAT boundary
        if self.sat_base + (self.next_idx as usize * 8) + 8 > self.vram.len() {
            return None;
        }

        let addr = self.sat_base + (self.next_idx as usize * 8);

        // Optimization: Read all 8 bytes at once
        let chunk: [u8; 8] = self.vram[addr..addr + 8].try_into().unwrap();
        let data = u64::from_be_bytes(chunk);

        let cur_v = ((data >> 48) as u16) & 0x03FF;
        let v_pos = cur_v.wrapping_sub(128);

        let size = (data >> 40) as u8;
        let h_size = ((size >> 2) & 0x03) + 1;
        let v_size = (size & 0x03) + 1;

        let link = (data >> 32) as u8 & 0x7F;

        let attr_word = (data >> 16) as u16;
        let priority = (attr_word & 0x8000) != 0;
        let palette = ((attr_word >> 13) & 0x03) as u8;
        let v_flip = (attr_word & 0x1000) != 0;
        let h_flip = (attr_word & 0x0800) != 0;
        let base_tile = attr_word & 0x07FF;

        let cur_h = (data as u16) & 0x03FF;
        let h_pos = cur_h.wrapping_sub(128);

        let attr = SpriteAttributes {
            v_pos,
            h_pos,
            h_size,
            v_size,
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

pub trait RenderOps {
    fn render_line(&mut self, line: u16);
    fn render_plane(
        &mut self,
        is_plane_a: bool,
        fetch_line: u16,
        draw_line: u16,
        priority_filter: bool,
    );
    fn render_tile(
        &mut self,
        is_plane_a: bool,
        enable_v_scroll: bool,
        name_table_base: usize,
        plane_w: usize,
        plane_h: usize,
        plane_w_mask: usize,
        h_scroll: u16,
        fetch_line: u16,
        line_offset: usize,
        screen_x: &mut u16,
        priority_filter: bool,
    );
    fn get_active_sprites<'a>(&self, line: u16, sprites: &'a mut [SpriteAttributes]) -> usize;
    fn render_sprites(
        &mut self,
        sprites: &[SpriteAttributes],
        fetch_line: u16,
        draw_line: u16,
        priority_filter: bool,
    );
    fn get_scroll_values(&self, is_plane_a: bool, fetch_line: u16, tile_h: usize) -> (u16, u16);
    fn fetch_nametable_entry(
        &self,
        base: usize,
        tile_v: usize,
        tile_h: usize,
        plane_w: usize,
    ) -> u16;
    fn fetch_tile_pattern(&self, tile_index: u16, pixel_v: u16, v_flip: bool) -> [u8; 4];
    fn draw_partial_tile_row(
        &mut self,
        entry: u16,
        pixel_v: u16,
        pixel_h: u16,
        count: u16,
        dest_idx: usize,
    );
    unsafe fn draw_full_tile_row(&mut self, entry: u16, pixel_v: u16, dest_idx: usize);
    fn bg_color(&self) -> (u8, u8);
    fn get_cram_color(&self, palette: u8, index: u8) -> u16;
}

fn render_sprite_scanline(
    vram: &[u8],
    framebuffer: &mut [u16],
    cram_cache: &[u16; 64],
    line: u16,
    attr: &SpriteAttributes,
    line_offset: usize,
    screen_width: u16,
) {
    let sprite_v_px = (attr.v_size as u16) * 8;

    let py = line.wrapping_sub(attr.v_pos);
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

        // Prefetch the 4 bytes (8 pixels) for this row.
        // row_addr is guaranteed to be 4-byte aligned (32*k + 4*j).
        // We already checked row_addr + 4 <= 0x10000.
        let patterns: [u8; 4] = unsafe {
             vram.get_unchecked(row_addr..row_addr + 4).try_into().unwrap_unchecked()
        };

        let base_screen_x = attr.h_pos.wrapping_add(tile_h_offset * 8);

        // Optimization: If the entire 8-pixel block is visible, skip per-pixel checks.
        if (base_screen_x as u32) + 8 <= screen_width as u32 {
            for i in 0..8 {
                let screen_x = base_screen_x.wrapping_add(i);
                let eff_col = if attr.h_flip { 7 - i } else { i };

                // SAFETY: eff_col is 0..8, so index 0..3 is valid. patterns is [u8; 4].
                let byte = unsafe { *patterns.get_unchecked((eff_col as usize) / 2) };

                let color_idx = if eff_col % 2 == 0 {
                    byte >> 4
                } else {
                    byte & 0x0F
                };

                if color_idx != 0 {
                    let addr = ((attr.palette as usize) << 4) | (color_idx as usize);
                    // SAFETY: cram_cache size 64. addr < 64.
                    unsafe {
                        let color = *cram_cache.get_unchecked(addr);
                        *framebuffer.get_unchecked_mut(line_offset + screen_x as usize) = color;
                    }
                }
            }
        } else {
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
                    let addr = ((attr.palette as usize) << 4) | (color_idx as usize);
                    let color = cram_cache[addr];
                    framebuffer[line_offset + screen_x as usize] = color;
                }
            }
        }
    }
}

impl RenderOps for Vdp {
    fn render_line(&mut self, line: u16) {
        if line >= 240 {
            return;
        }

        let draw_line = line;
        let fetch_line = line;

        let line_offset = (draw_line as usize) * 320;

        let (pal_line, color_idx) = self.bg_color();
        let bg_color = self.get_cram_color(pal_line, color_idx);

        self.framebuffer[line_offset..line_offset + 320].fill(bg_color);

        if !self.display_enabled() || line >= self.screen_height() {
            return;
        }

        // Pre-calculate visible sprites for this line to avoid traversing the SAT twice
        let mut sprite_buffer = [SpriteAttributes::default(); 80];
        let sprite_count = self.get_active_sprites(fetch_line, &mut sprite_buffer);
        let active_sprites = &sprite_buffer[..sprite_count];

        // Layer order: B Low -> A Low -> Sprites Low -> B High -> A High -> Sprites High
        self.render_plane(false, fetch_line, draw_line, false); // Plane B Low
        self.render_plane(true, fetch_line, draw_line, false); // Plane A Low
        self.render_sprites(active_sprites, fetch_line, draw_line, false);
        self.render_plane(false, fetch_line, draw_line, true); // Plane B High
        self.render_plane(true, fetch_line, draw_line, true); // Plane A High
        self.render_sprites(active_sprites, fetch_line, draw_line, true);

        // Apply Register 0 Bit 5 (Mask 1st Column)
        if (self.registers[REG_MODE1] & 0x20) != 0 {
            let (pal_line, color_idx) = self.bg_color();
            let bg_color = self.get_cram_color(pal_line, color_idx);
            self.framebuffer[line_offset..line_offset + 8].fill(bg_color);
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

        let screen_width = self.screen_width();
        let line_offset = (draw_line as usize) * 320;

        // Fetch H-scroll once for the line (but may be overridden by Window)
        let (_, h_scroll) = self.get_scroll_values(is_plane_a, fetch_line, 0);

        let mut screen_x: u16 = 0;

        while screen_x < screen_width {
            let (tile_base, tile_h_scroll, use_v_scroll, tile_w) = if is_plane_a && self.is_window_area(screen_x, fetch_line) {
                let win_w = if self.h40_mode() { 64 } else { 32 };
                (self.window_address(), 0, false, win_w)
            } else {
                (name_table_base, h_scroll, true, plane_w)
            };

            self.render_tile(
                is_plane_a,
                use_v_scroll,
                tile_base,
                tile_w,
                plane_h,
                tile_w - 1, // tile_w_mask
                tile_h_scroll,
                fetch_line,
                line_offset,
                &mut screen_x,
                priority_filter,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_tile(
        &mut self,
        is_plane_a: bool,
        enable_v_scroll: bool,
        name_table_base: usize,
        plane_w: usize,
        plane_h: usize,
        plane_w_mask: usize,
        h_scroll: u16,
        fetch_line: u16,
        line_offset: usize,
        screen_x: &mut u16,
        priority_filter: bool,
    ) {
        // Horizontal position in plane
        let scrolled_h = (*screen_x).wrapping_sub(h_scroll);
        let pixel_h = scrolled_h & 0x07;
        let tile_h = ((scrolled_h >> 3) as usize) & plane_w_mask;

        // Fetch V-scroll for this specific column (per-column VS support)
        // If not using scroll (e.g. Window plane), V-scroll is 0.
        let (v_scroll, _) = if enable_v_scroll {
            self.get_scroll_values(is_plane_a, fetch_line, (*screen_x >> 3) as usize)
        } else {
            (0, 0)
        };

        // Vertical position in plane
        let scrolled_v = fetch_line.wrapping_add(v_scroll);
        let tile_v = (scrolled_v as usize / 8) % plane_h;
        let pixel_v = scrolled_v % 8;

        let pixels_left_in_tile = 8 - pixel_h;
        let pixels_to_process = std::cmp::min(pixels_left_in_tile, self.screen_width() - *screen_x);

        let entry = self.fetch_nametable_entry(name_table_base, tile_v, tile_h, plane_w);
        let priority = (entry & 0x8000) != 0;

        if priority == priority_filter {
            if pixels_to_process == 8 && pixel_h == 0 {
                // Fast path for full aligned tile
                unsafe {
                    self.draw_full_tile_row(entry, pixel_v, line_offset + *screen_x as usize);
                }
            } else {
                self.draw_partial_tile_row(
                    entry,
                    pixel_v,
                    pixel_h,
                    pixels_to_process,
                    line_offset + *screen_x as usize,
                );
            }
        }
        *screen_x += pixels_to_process;
    }

    fn get_active_sprites<'a>(&self, line: u16, sprites: &'a mut [SpriteAttributes]) -> usize {
        let mut count = 0;

        let sat_base = self.sprite_table_address() as usize;
        let max_sprites = if self.h40_mode() { 80 } else { 64 };

        let iter = SpriteIterator {
            vram: &self.vram,
            next_idx: 0,
            count: 0,
            max_sprites,
            sat_base,
        };

        let line_limit = if self.h40_mode() { 20 } else { 16 };

        for attr in iter {
            let sprite_v_px = (attr.v_size as u16) * 8;
            // Handle wrapping v_pos (top clipping) correctly using wrapping subtraction
            if line.wrapping_sub(attr.v_pos) < sprite_v_px {
                if count < sprites.len() {
                    sprites[count] = attr;
                    count += 1;
                }
                if count >= line_limit {
                    break;
                }
            }
        }
        count
    }

    fn render_sprites(
        &mut self,
        sprites: &[SpriteAttributes],
        fetch_line: u16,
        draw_line: u16,
        priority_filter: bool,
    ) {
        let screen_width = self.screen_width();
        let line_offset = (draw_line as usize) * 320;

        // Render in reverse order so that sprites with lower indices (higher priority)
        // are drawn last and appear on top.
        for attr in sprites.iter().rev() {
            if attr.priority == priority_filter {
                render_sprite_scanline(
                    &self.vram,
                    &mut self.framebuffer,
                    &self.cram_cache,
                    fetch_line,
                    attr,
                    line_offset,
                    screen_width,
                );
            }
        }
    }
    fn get_scroll_values(&self, is_plane_a: bool, fetch_line: u16, tile_h: usize) -> (u16, u16) {
        let mode3 = self.registers[REG_MODE3];

        // Vertical Scroll (Bits 2 of Mode 3: 0=Full Screen, 1=2-Cell Strips)
        let v_scroll = if (mode3 & 0x04) != 0 {
            // 2-Cell (16-pixel) strips. Each entry in VSRAM is 4 bytes and handles 2 cells.
            // Entry 0: Plane A Cell 0-1, Entry 1: Plane B Cell 0-1, etc.
            let strip_idx = tile_h >> 1;
            let vs_addr = (strip_idx * 4) + (if is_plane_a { 0 } else { 2 });
            if vs_addr + 1 < self.vsram.len() {
                (((self.vsram[vs_addr] as u16) << 8) | (self.vsram[vs_addr + 1] as u16)) & 0x03FF
            } else {
                0
            }
        } else {
            // Full Screen
            let vs_addr = if is_plane_a { 0 } else { 2 };
            (((self.vsram[vs_addr] as u16) << 8) | (self.vsram[vs_addr + 1] as u16)) & 0x03FF
        };

        // Horizontal Scroll (Bits 1-0 of Mode 3: 00=Full, 01=Invalid/Cell, 10=Cell, 11=Line)
        let hs_mode = mode3 & 0x03;
        let hs_base = self.hscroll_address();

        let hs_addr = match hs_mode {
            0x00 => hs_base, // Full screen
            0x01 | 0x02 => hs_base + (((fetch_line as usize) >> 3) * 4), // 8-pixel high strips (Cell)
            0x03 => hs_base + ((fetch_line as usize) * 4),               // Per-line
            _ => hs_base,
        } + (if is_plane_a { 0 } else { 2 });

        let hi = self.vram[hs_addr & 0xFFFF];
        let lo = self.vram[(hs_addr + 1) & 0xFFFF];
        // H-scroll is 10-bit signed value.
        let mut h_scroll = (((hi as u16) << 8) | (lo as u16)) & 0x03FF;
        if (h_scroll & 0x0200) != 0 {
            h_scroll |= 0xFC00; // Sign extend to 16 bits
        }

        (v_scroll, h_scroll)
    }

    #[inline(always)]
    fn fetch_nametable_entry(
        &self,
        base: usize,
        tile_v: usize,
        tile_h: usize,
        plane_w: usize,
    ) -> u16 {
        let nt_entry_addr = base + (tile_v * plane_w + tile_h) * 2;
        // SAFETY: nt_entry_addr & 0xFFFF guarantees range 0..65535, which is within vram bounds (65536)
        unsafe {
            let hi = *self.vram.get_unchecked(nt_entry_addr & 0xFFFF);
            let lo = *self.vram.get_unchecked((nt_entry_addr + 1) & 0xFFFF);
            ((hi as u16) << 8) | (lo as u16)
        }
    }

    #[inline(always)]
    fn fetch_tile_pattern(&self, tile_index: u16, pixel_v: u16, v_flip: bool) -> [u8; 4] {
        let row = if v_flip { 7 - pixel_v } else { pixel_v };
        let row_addr = (tile_index as usize * 32) + (row as usize * 4);
        // Mask to 64KB boundary and align to 4 bytes.
        // We use (row_addr & 0xFFFF) to ensure we wrap within 64KB, and then mask with 0xFFFC
        // to clear the bottom 2 bits for alignment. 0xFFFC effectively does both, but we make
        // the wrapping explicit for clarity and safety against potential type width assumptions.
        let addr = (row_addr & 0xFFFF) & 0xFFFC;

        // SAFETY:
        // 1. addr is explicitly masked to be <= 0xFFFC and 4-byte aligned.
        // 2. self.vram is [u8; 0x10000].
        // 3. Reading 4 bytes from addr <= 0xFFFC accesses bytes up to 0xFFFF, which is within bounds.
        unsafe {
            let ptr = self.vram.as_ptr().add(addr) as *const u32;
            ptr.read_unaligned().to_ne_bytes()
        }
    }

    fn draw_partial_tile_row(
        &mut self,
        entry: u16,
        pixel_v: u16,
        pixel_h: u16,
        count: u16,
        dest_idx: usize,
    ) {
        let palette = ((entry >> 13) & 0x03) as u8;
        let v_flip = (entry & 0x1000) != 0;
        let h_flip = (entry & 0x0800) != 0;
        let tile_index = entry & 0x07FF;

        let patterns = self.fetch_tile_pattern(tile_index, pixel_v, v_flip);

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
                self.framebuffer[dest_idx + i as usize] = color;
            }
        }
    }

    #[inline(always)]
    unsafe fn draw_full_tile_row(&mut self, entry: u16, pixel_v: u16, dest_idx: usize) {
        let palette = ((entry >> 13) & 0x03) as u8;
        let v_flip = (entry & 0x1000) != 0;
        let h_flip = (entry & 0x0800) != 0;
        let tile_index = entry & 0x07FF;

        let patterns = self.fetch_tile_pattern(tile_index, pixel_v, v_flip);

        // Optimization: Skip empty rows
        if u32::from_ne_bytes(patterns) == 0 {
            return;
        }

        let p0 = patterns[0];
        let p1 = patterns[1];
        let p2 = patterns[2];
        let p3 = patterns[3];

        let palette_base = (palette as usize) * 16;

        // SAFETY: Caller ensures dest_idx + 7 is within framebuffer bounds.
        // palette is 2 bits, so palette_base is max 48. col is 4 bits (0-15).
        // Max index is 63, which is within cram_cache bounds (64).
        // Get base pointers to avoid repeated offset calculations
        let cram_ptr = self.cram_cache.as_ptr().add(palette_base);
        let dest_ptr = self.framebuffer.as_mut_ptr().add(dest_idx);

        if h_flip {
            let mut col = p3 & 0x0F;
            if col != 0 {
                *dest_ptr = *cram_ptr.add(col as usize);
            }
            col = p3 >> 4;
            if col != 0 {
                *dest_ptr.add(1) = *cram_ptr.add(col as usize);
            }

            col = p2 & 0x0F;
            if col != 0 {
                *dest_ptr.add(2) = *cram_ptr.add(col as usize);
            }
            col = p2 >> 4;
            if col != 0 {
                *dest_ptr.add(3) = *cram_ptr.add(col as usize);
            }

            col = p1 & 0x0F;
            if col != 0 {
                *dest_ptr.add(4) = *cram_ptr.add(col as usize);
            }
            col = p1 >> 4;
            if col != 0 {
                *dest_ptr.add(5) = *cram_ptr.add(col as usize);
            }

            col = p0 & 0x0F;
            if col != 0 {
                *dest_ptr.add(6) = *cram_ptr.add(col as usize);
            }
            col = p0 >> 4;
            if col != 0 {
                *dest_ptr.add(7) = *cram_ptr.add(col as usize);
            }
        } else {
            let mut col = p0 >> 4;
            if col != 0 {
                *dest_ptr = *cram_ptr.add(col as usize);
            }
            col = p0 & 0x0F;
            if col != 0 {
                *dest_ptr.add(1) = *cram_ptr.add(col as usize);
            }

            col = p1 >> 4;
            if col != 0 {
                *dest_ptr.add(2) = *cram_ptr.add(col as usize);
            }
            col = p1 & 0x0F;
            if col != 0 {
                *dest_ptr.add(3) = *cram_ptr.add(col as usize);
            }

            col = p2 >> 4;
            if col != 0 {
                *dest_ptr.add(4) = *cram_ptr.add(col as usize);
            }
            col = p2 & 0x0F;
            if col != 0 {
                *dest_ptr.add(5) = *cram_ptr.add(col as usize);
            }

            col = p3 >> 4;
            if col != 0 {
                *dest_ptr.add(6) = *cram_ptr.add(col as usize);
            }
            col = p3 & 0x0F;
            if col != 0 {
                *dest_ptr.add(7) = *cram_ptr.add(col as usize);
            }
        }
    }

    fn bg_color(&self) -> (u8, u8) {
        let bg_idx = self.registers[REG_BG_COLOR];
        let pal = (bg_idx >> 4) & 0x03;
        let color = bg_idx & 0x0F;
        (pal, color)
    }

    #[inline(always)]
    fn get_cram_color(&self, palette: u8, index: u8) -> u16 {
        let addr = ((palette as usize) * 16) + (index as usize);
        unsafe { *self.cram_cache.get_unchecked(addr & 0x3F) }
    }
}
