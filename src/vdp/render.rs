use super::constants::*;
use super::Vdp;
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
    pub index: u8,
    pub link: u8,
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
            index: self.next_idx,
            link,
        };

        self.count += 1;
        self.next_idx = link;

        if link == 0 {
            self.count = self.max_sprites; // Stop after this one
        }

        Some(attr)
    }
}

pub struct TileRenderParams {
    pub is_plane_a: bool,
    pub enable_v_scroll: bool,
    pub name_table_base: usize,
    pub plane_w: usize,
    pub plane_h: usize,
    pub plane_w_mask: usize,
    pub h_scroll: u16,
    pub fetch_line: u16,
    pub scanline_width: u16,
}

pub trait RenderOps {
    fn render_line(&mut self, line: u16);
    /// Re-render only pixels [start_x .. screen_width] of `line` using the
    /// current VDP state. Pixels at [0 .. start_x] are left untouched so
    /// they retain whatever state they were drawn with previously
    /// (segmented mid-line render for road-gradient effects).
    fn render_line_from(&mut self, line: u16, start_x: u16);
    fn render_plane(&self, is_plane_a: bool, fetch_line: u16, line_buf: &mut [u8; 320]);
    fn render_tile(&self, params: &TileRenderParams, screen_x: &mut u16, line_buf: &mut [u8; 320]);
    fn get_active_sprites(&mut self, line: u16, sprites: &mut [SpriteAttributes]) -> usize;
    fn render_sprites(
        &self,
        sprites: &[SpriteAttributes],
        fetch_line: u16,
        line_buf: &mut [u8; 320],
    ) -> bool;
    fn get_v_scroll(&self, is_plane_a: bool, tile_h: usize, fetch_line: u16) -> u16;
    fn get_h_scroll(&self, is_plane_a: bool, fetch_line: u16) -> u16;
    fn fetch_nametable_entry(
        &self,
        base: usize,
        tile_v: usize,
        tile_h: usize,
        plane_w: usize,
    ) -> u16;
    fn fetch_tile_pattern(&self, tile_index: u16, pixel_v: u16, v_flip: bool) -> [u8; 4];
    fn draw_partial_tile_row(
        &self,
        entry: u16,
        pixel_v: u16,
        pixel_h: u16,
        count: u16,
        dest_idx: usize,
        line_buf: &mut [u8; 320],
    );
    fn draw_full_tile_row(
        &self,
        entry: u16,
        pixel_v: u16,
        dest_idx: usize,
        line_buf: &mut [u8; 320],
    );
    fn bg_color(&self) -> (u8, u8);
    fn get_cram_color(&self, palette: u8, index: u8) -> u16;
    fn get_cram_rgb565(&self) -> [u16; 64];
    fn get_cram_raw(&self) -> [u16; 64];
}

pub struct PixelLayerData {
    pub bg_color_idx: u8,
    pub s_pri: bool,
    pub s_trans: bool,
    pub s_col: u8,
    pub a_pri: bool,
    pub a_trans: bool,
    pub a_col: u8,
    pub b_pri: bool,
    pub b_trans: bool,
    pub b_col: u8,
}

pub struct CompositeLineParams<'a> {
    pub line_offset: usize,
    pub bg_color_idx: u8,
    pub bg_color_val: u16,
    pub buf_b: &'a [u8; 320],
    pub buf_a: &'a [u8; 320],
    pub buf_s: &'a [u8; 320],
    /// Inclusive start pixel for composite (used for mid-line segmented
    /// re-render). Defaults to 0 for a full-line composite.
    pub start_x: usize,
}

impl Vdp {
    fn composite_line(&mut self, params: &CompositeLineParams) {
        let sh_enabled = (self.registers[REG_MODE4] & 0x08) != 0;
        let mask_col0 = (self.registers[REG_MODE1] & 0x20) != 0;

        for x in params.start_x..320 {
            if mask_col0 && x < 8 {
                self.framebuffer[params.line_offset + x] = params.bg_color_val;
                continue;
            }

            let b = params.buf_b[x];
            let a = params.buf_a[x];
            let s = params.buf_s[x];

            let b_pri = (b & 0x80) != 0;
            let a_pri = (a & 0x80) != 0;
            let s_pri = (s & 0x80) != 0;

            let b_col = b & 0x3F;
            let a_col = a & 0x3F;
            let s_col = s & 0x3F;

            let b_trans = (b_col & 0x0F) == 0;
            let a_trans = (a_col & 0x0F) == 0;
            let s_trans = (s_col & 0x0F) == 0;

            let px = PixelLayerData {
                bg_color_idx: params.bg_color_idx,
                s_pri,
                s_trans,
                s_col,
                a_pri,
                a_trans,
                a_col,
                b_pri,
                b_trans,
                b_col,
            };

            if !sh_enabled {
                let (top_col, _) = self.determine_top_layer(&px);
                self.framebuffer[params.line_offset + x] = self.cram_cache[top_col as usize];
            } else {
                let (top_col, state) = self.resolve_shadow_highlight_pixel(&px);
                let color = self.cram_cache[top_col as usize];
                let final_color = self.apply_color_transform(color, state);
                self.framebuffer[params.line_offset + x] = final_color;
            }
        }
    }

    /// Full S/H pipeline.
    ///
    /// Hardware behavior:
    ///   * Default shadow-state = 0 (shadow). The BG color is in shadow unless
    ///     something with priority is on top.
    ///   * A non-transparent high-priority plane pixel lifts the state to 1 (normal).
    ///   * A non-transparent, non-operator priority sprite lifts the state to 1.
    ///   * Operator sprites ($3E = highlight, $3F = shadow, palette 3 only)
    ///     are themselves invisible — the plane/BG pixel underneath shows
    ///     through — and modify the state by +1/-1 saturating to [0, 2].
    pub(crate) fn resolve_shadow_highlight_pixel(&self, px: &PixelLayerData) -> (u8, u8) {
        let sprite_visible = !px.s_trans;
        let sprite_is_operator = sprite_visible && (px.s_col == 0x3E || px.s_col == 0x3F);

        let mut state: u8 = 0; // shadow by default

        // High-priority plane pixels lift the state to normal.
        if (px.a_pri && !px.a_trans) || (px.b_pri && !px.b_trans) {
            state = 1;
        }
        // A priority sprite that isn't an operator also lifts the state.
        if px.s_pri && sprite_visible && !sprite_is_operator {
            state = 1;
        }

        // Top color: operator sprites hide themselves; pick top from non-sprite layers.
        let top_col = if sprite_is_operator {
            let masked = PixelLayerData {
                bg_color_idx: px.bg_color_idx,
                s_pri: false,
                s_trans: true,
                s_col: 0,
                a_pri: px.a_pri,
                a_trans: px.a_trans,
                a_col: px.a_col,
                b_pri: px.b_pri,
                b_trans: px.b_trans,
                b_col: px.b_col,
            };
            self.determine_top_layer(&masked).0
        } else {
            self.determine_top_layer(px).0
        };

        // Apply the operator's state modification.
        if sprite_is_operator {
            if px.s_col == 0x3E && state < 2 {
                state += 1;
            } else if px.s_col == 0x3F && state > 0 {
                state -= 1;
            }
        }

        (top_col, state)
    }

    pub(crate) fn determine_top_layer(&self, px: &PixelLayerData) -> (u8, u8) {
        let mut top_col = px.bg_color_idx;
        let mut top_layer = 0; // 0=BG, 1=B, 2=A, 3=S

        if px.s_pri && !px.s_trans {
            top_col = px.s_col;
            top_layer = 3;
        } else if px.a_pri && !px.a_trans {
            top_col = px.a_col;
            top_layer = 2;
        } else if px.b_pri && !px.b_trans {
            top_col = px.b_col;
            top_layer = 1;
        } else if !px.s_trans {
            top_col = px.s_col;
            top_layer = 3;
        } else if !px.a_trans {
            top_col = px.a_col;
            top_layer = 2;
        } else if !px.b_trans {
            top_col = px.b_col;
            top_layer = 1;
        }

        (top_col, top_layer)
    }

    pub(crate) fn apply_color_transform(&self, color: u16, state: u8) -> u16 {
        let r = (color >> 11) & 0x1F;
        let g = (color >> 5) & 0x3F;
        let b = color & 0x1F;

        match state {
            0 => {
                // Shadow mode halves each channel.
                ((r >> 1) << 11) | ((g >> 1) << 5) | (b >> 1)
            }
            2 => {
                // Highlight mode moves halfway toward full intensity.
                let r_hi = r + ((0x1F - r) >> 1);
                let g_hi = g + ((0x3F - g) >> 1);
                let b_hi = b + ((0x1F - b) >> 1);
                (r_hi << 11) | (g_hi << 5) | b_hi
            }
            _ => color,
        }
    }
}

fn render_sprite_scanline(
    vram: &[u8],
    line_buf: &mut [u8; 320],
    line: u16,
    attr: &SpriteAttributes,
    screen_width: u16,
    collision: &mut bool,
) {
    let sprite_v_px = (attr.v_size as u16) * 8;

    let py = (line as i32) - (attr.v_pos as i16 as i32);
    if py < 0 || py >= sprite_v_px as i32 {
        return;
    }

    let fetch_py = if attr.v_flip {
        (sprite_v_px - 1) - (py as u16)
    } else {
        py as u16
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
        let tile_idx = (attr
            .base_tile
            .wrapping_add(fetch_tile_h_offset * attr.v_size as u16)
            .wrapping_add(tile_v_offset))
            & 0x07FF;

        // Calculate pattern address for the row (pixel_v is 0..7)
        // Each tile is 32 bytes (4 bytes per row)
        let row_addr = (tile_idx as usize * 32) + (pixel_v as usize * 4);

        // Check if row is within VRAM bounds
        if row_addr + 4 > 0x10000 {
            continue;
        }

        // Prefetch the 4 bytes (8 pixels) for this row.
        // row_addr is guaranteed to be 4-byte aligned (32*k + 4*j).
        // We already checked row_addr + 4 <= 0x10000. Using unwrap() increases safety and eliminates the unsafe block.
        let patterns: [u8; 4] = vram[row_addr..row_addr + 4].try_into().unwrap();

        let base_screen_x = (attr.h_pos as i16 as i32) + (tile_h_offset as i32 * 8);

        // Optimization: If the entire 8-pixel block is visible, skip per-pixel checks.
        if base_screen_x >= 0 && base_screen_x + 8 <= screen_width as i32 {
            for i in 0..8 {
                let screen_x = base_screen_x + i;
                let eff_col = if attr.h_flip { 7 - i } else { i };

                let byte = patterns[(eff_col as usize) / 2];

                let color_idx = if eff_col % 2 == 0 {
                    byte >> 4
                } else {
                    byte & 0x0F
                };

                if color_idx != 0 {
                    let addr = (attr.palette << 4) | color_idx;
                    let pri_mask = if attr.priority { 0x80 } else { 0x00 };
                    if let Some(pixel) = line_buf.get_mut(screen_x as usize) {
                        if (*pixel & 0x0F) != 0 {
                            *collision = true;
                        }
                        *pixel = addr | pri_mask;
                    }
                }
            }
        } else {
            for i in 0..8 {
                let screen_x = base_screen_x + i;
                if screen_x < 0 || screen_x >= screen_width as i32 {
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
                    let addr = (attr.palette << 4) | color_idx;
                    let pri_mask = if attr.priority { 0x80 } else { 0x00 };
                    if let Some(pixel) = line_buf.get_mut(screen_x as usize) {
                        if (*pixel & 0x0F) != 0 {
                            *collision = true;
                        }
                        *pixel = addr | pri_mask;
                    }
                }
            }
        }
    }
}

impl RenderOps for Vdp {
    fn render_line(&mut self, line: u16) {
        if line >= self.screen_height() {
            return;
        }

        // SAT cache (LSU) is latched at the line boundary by `tick`; do not
        // refresh here so mid-line VRAM writes to the SAT region don't take
        // effect until next line, matching hardware. Cold-start case (cache
        // never latched yet) falls back to an initial sync.
        self.ensure_sat_cache();

        let draw_line = line;
        let fetch_line = line;
        let line_offset = (draw_line as usize) * 320;

        let (pal_line, color_idx) = self.bg_color();
        let bg_color_val = self.get_cram_color(pal_line, color_idx);
        let bg_color_idx = (pal_line << 4) | color_idx;

        if !self.display_enabled() || line >= self.screen_height() {
            self.framebuffer[line_offset..line_offset + 320].fill(bg_color_val);
            self.rendered_scanlines[line as usize] = true;
            return;
        }

        let mut sprite_buffer = [SpriteAttributes::default(); 80];
        let sprite_count = self.get_active_sprites(fetch_line, &mut sprite_buffer);
        let active_sprites = &sprite_buffer[..sprite_count];

        let mut buf_b = [0u8; 320];
        let mut buf_a = [0u8; 320];
        let mut buf_s = [0u8; 320];

        if std::env::var("GENTEEL_DEBUG_PLANE").as_deref() != Ok("a") {
            self.render_plane(false, fetch_line, &mut buf_b);
        }
        if std::env::var("GENTEEL_DEBUG_PLANE").as_deref() != Ok("b") {
            self.render_plane(true, fetch_line, &mut buf_a);
        }
        if self.render_sprites(active_sprites, fetch_line, &mut buf_s) {
            self.status |= STATUS_COLLISION;
        }

        let composite_params = CompositeLineParams {
            line_offset,
            bg_color_idx,
            bg_color_val,
            buf_b: &buf_b,
            buf_a: &buf_a,
            buf_s: &buf_s,
            start_x: 0,
        };
        self.composite_line(&composite_params);
        self.rendered_scanlines[line as usize] = true;
        // Reset the per-line split marker so subsequent mid-line writes can
        // segment from pixel 0.
        self.line_split_x = 0;
    }

    fn render_line_from(&mut self, line: u16, start_x: u16) {
        let width = self.screen_width();
        if line >= self.screen_height() || start_x >= width {
            return;
        }

        self.ensure_sat_cache();

        let fetch_line = line;
        let line_offset = (line as usize) * 320;

        let (pal_line, color_idx) = self.bg_color();
        let bg_color_val = self.get_cram_color(pal_line, color_idx);
        let bg_color_idx = (pal_line << 4) | color_idx;

        if !self.display_enabled() {
            // Mid-line display-disable: pixels [start_x..width] become BG.
            let start = line_offset + start_x as usize;
            let end = line_offset + width as usize;
            self.framebuffer[start..end].fill(bg_color_val);
            return;
        }

        // We re-render the full plane/sprite buffers but composite only the
        // [start_x..320] range back into the framebuffer.
        let mut sprite_buffer = [SpriteAttributes::default(); 80];
        let sprite_count = self.get_active_sprites(fetch_line, &mut sprite_buffer);
        let active_sprites = &sprite_buffer[..sprite_count];

        let mut buf_b = [0u8; 320];
        let mut buf_a = [0u8; 320];
        let mut buf_s = [0u8; 320];

        if std::env::var("GENTEEL_DEBUG_PLANE").as_deref() != Ok("a") {
            self.render_plane(false, fetch_line, &mut buf_b);
        }
        if std::env::var("GENTEEL_DEBUG_PLANE").as_deref() != Ok("b") {
            self.render_plane(true, fetch_line, &mut buf_a);
        }
        if self.render_sprites(active_sprites, fetch_line, &mut buf_s) {
            self.status |= STATUS_COLLISION;
        }

        let composite_params = CompositeLineParams {
            line_offset,
            bg_color_idx,
            bg_color_val,
            buf_b: &buf_b,
            buf_a: &buf_a,
            buf_s: &buf_s,
            start_x: start_x as usize,
        };
        self.composite_line(&composite_params);
    }

    fn render_plane(&self, is_plane_a: bool, fetch_line: u16, line_buf: &mut [u8; 320]) {
        let (plane_w, plane_h) = self.plane_size();
        let name_table_base = if is_plane_a {
            self.plane_a_address()
        } else {
            self.plane_b_address()
        };

        let screen_width = self.screen_width();
        let h_scroll = self.get_h_scroll(is_plane_a, fetch_line);
        let h40_mode = self.h40_mode();

        let mut screen_x: u16 = 0;

        if is_plane_a {
            let h_pos = self.registers[REG_WINDOW_H_POS];
            let v_pos = self.registers[REG_WINDOW_V_POS];
            let win_h_point = (h_pos as u16 & 0x1F) * 16;
            let v_point = (v_pos as u16 & 0x1F) * 8;
            let win_h_dir = (h_pos & 0x80) != 0;
            let v_dir = (v_pos & 0x80) != 0;
            let win_full_line = if v_dir {
                fetch_line >= v_point
            } else {
                fetch_line < v_point
            };
            let win_addr = self.window_address();
            let win_w = if h40_mode { 64 } else { 32 };

            let win_params = TileRenderParams {
                is_plane_a: true,
                enable_v_scroll: false,
                name_table_base: win_addr,
                plane_w: win_w,
                plane_h,
                plane_w_mask: win_w - 1,
                h_scroll: 0,
                fetch_line,
                scanline_width: screen_width,
            };
            let plane_params = TileRenderParams {
                is_plane_a: true,
                enable_v_scroll: true,
                name_table_base,
                plane_w,
                plane_h,
                plane_w_mask: plane_w - 1,
                h_scroll,
                fetch_line,
                scanline_width: screen_width,
            };

            // H40 window/Plane-A boundary glitch (G10/R6):
            // The hardware artifact only occurs once per line (the first
            // window->plane-A transition) and only when the window doesn't
            // span the entire line. We additionally require the transition
            // boundary to fall on a tile-aligned screen X — the pre-fetch
            // race that causes the glitch only happens when the window
            // boundary coincides with a fetch slot boundary.
            let glitch_enabled = h40_mode && h_scroll != 0 && !win_h_dir && !win_full_line;
            let mut glitch_fired = false;
            let mut prev_in_window = win_full_line
                || if win_h_dir {
                    screen_x >= win_h_point
                } else {
                    screen_x < win_h_point
                };
            while screen_x < screen_width {
                let now_in_window = if win_full_line {
                    true
                } else if win_h_dir {
                    screen_x >= win_h_point
                } else {
                    screen_x < win_h_point
                };

                if glitch_enabled
                    && !glitch_fired
                    && prev_in_window
                    && !now_in_window
                    && (screen_x & 0x07) == 0
                {
                    // Boundary glitch: one extra window tile before plane A.
                    self.render_tile(&win_params, &mut screen_x, line_buf);
                    glitch_fired = true;
                    if screen_x >= screen_width {
                        break;
                    }
                }

                let params = if now_in_window {
                    &win_params
                } else {
                    &plane_params
                };
                self.render_tile(params, &mut screen_x, line_buf);
                prev_in_window = now_in_window;
            }
        } else {
            // Plane B never has a window
            let tile_params = TileRenderParams {
                is_plane_a: false,
                enable_v_scroll: true,
                name_table_base,
                plane_w,
                plane_h,
                plane_w_mask: plane_w - 1,
                h_scroll,
                fetch_line,
                scanline_width: screen_width,
            };
            while screen_x < screen_width {
                self.render_tile(&tile_params, &mut screen_x, line_buf);
            }
        }
    }

    fn render_tile(&self, params: &TileRenderParams, screen_x: &mut u16, line_buf: &mut [u8; 320]) {
        let current_x = *screen_x;
        let scrolled_h = current_x.wrapping_sub(params.h_scroll);
        let pixel_h = scrolled_h & 0x07;
        let tile_h = ((scrolled_h >> 3) as usize) & params.plane_w_mask;

        // Fetch V-scroll for this specific column (per-column VS support).
        // If not using scroll (e.g. Window plane), V-scroll is 0.
        //
        // H40 column-0 VSRAM-AND quirk:
        //   In H40 mode with 2-cell V-scroll enabled and non-zero H-scroll,
        //   the VDP pre-fetches the V-scroll entry for column "-1" of the
        //   previous frame. The bus settles to the bitwise AND of the last
        //   two strip entries (strips 18 and 19, i.e. cells 36-37 and 38-39).
        //   Cells 0 and 1 (current_x < 16) both share this value because
        //   they map to strip 0 which is what gets overridden.
        let v_scroll = if params.enable_v_scroll {
            let mode3 = self.registers[REG_MODE3];
            let two_cell_mode = (mode3 & 0x04) != 0;
            let h40 = self.h40_mode();
            if h40 && two_cell_mode && current_x < 16 && params.h_scroll != 0 {
                let vs_strip19 = self.get_v_scroll(params.is_plane_a, 38, params.fetch_line);
                let vs_strip18 = self.get_v_scroll(params.is_plane_a, 36, params.fetch_line);
                // R5: tighten the AND quirk. If either source strip is zero,
                // the AND would clobber the other strip with zero, breaking
                // games (e.g. Road Rash II) that intentionally keep the last
                // strip unwritten while strip 0 carries the real value.
                // Fall back to strip-19 only in that case (the looser
                // pre-G5 behavior). The AND only fires when both strips
                // carry meaningful (non-zero) bits.
                if vs_strip19 != 0 && vs_strip18 != 0 {
                    vs_strip19 & vs_strip18
                } else {
                    vs_strip19
                }
            } else {
                let tile_column = (current_x >> 3) as usize;
                self.get_v_scroll(params.is_plane_a, tile_column, params.fetch_line)
            }
        } else {
            0
        };

        // Vertical position in plane
        let scrolled_v = params.fetch_line.wrapping_add(v_scroll);
        let tile_v = ((scrolled_v / 8) as usize) % params.plane_h;
        let pixel_v = scrolled_v % 8;

        let pixels_left_in_tile = 8 - pixel_h;
        let pixels_to_process =
            std::cmp::min(pixels_left_in_tile, params.scanline_width - current_x);

        let entry =
            self.fetch_nametable_entry(params.name_table_base, tile_v, tile_h, params.plane_w);

        if pixels_to_process == 8 && pixel_h == 0 {
            // Fast path for full aligned tile
            self.draw_full_tile_row(entry, pixel_v, current_x as usize, line_buf);
        } else {
            self.draw_partial_tile_row(
                entry,
                pixel_v,
                pixel_h,
                pixels_to_process,
                current_x as usize,
                line_buf,
            );
        }
        *screen_x = current_x + pixels_to_process;
    }

    fn get_active_sprites(&mut self, line: u16, sprites: &mut [SpriteAttributes]) -> usize {
        // Cold-start sync only. Production latches at line boundary in `tick`.
        self.ensure_sat_cache();

        let max_sprites = if self.h40_mode() { 80 } else { 64 };
        let line_limit = if self.h40_mode() { 20 } else { 16 };
        let pixel_limit = if self.h40_mode() { 320 } else { 256 };

        // X=0 sprite mask state. Triggered when an X=0 sprite is encountered AND
        // either a non-X=0 sprite has already been added to this line, OR the
        // previous line had a sprite overflow. Once triggered, subsequent
        // sprites on this line are not rendered (but still count toward
        // per-line limits for overflow purposes).
        let mut mask_triggered = false;
        let mut visible_added: usize = 0;
        let mut slots_consumed: usize = 0;
        let mut pixels: usize = 0;
        let prev_overflow = self.prev_line_sprite_overflow;
        let mut overflow_this_line = false;

        let iter = SpriteIterator {
            vram: &self.sat,
            next_idx: 0,
            count: 0,
            max_sprites,
            sat_base: 0,
        };

        for attr in iter {
            let sprite_v_px = (attr.v_size as u16) * 8;
            let sprite_top = attr.v_pos as i16 as i32;
            let line_i = line as i32;

            if line_i < sprite_top || line_i >= sprite_top + sprite_v_px as i32 {
                continue;
            }

            // An X=0 sprite that meets the trigger condition activates the mask.
            // "X=0" means the raw 10-bit SAT X field is 0 (i.e. h_pos after the
            // 128 subtraction equals 0xFF80 / -128). Such a sprite is fully
            // off-screen and never renders, but it still consumes a per-line
            // slot and dot budget on hardware, and arms the mask.
            let is_x0_mask = attr.h_pos == 0u16.wrapping_sub(128);
            if is_x0_mask {
                if visible_added > 0 || prev_overflow {
                    mask_triggered = true;
                }
            } else if !mask_triggered && visible_added < sprites.len() {
                sprites[visible_added] = attr;
                visible_added += 1;
            }

            slots_consumed += 1;
            pixels += (attr.h_size as usize) * 8;

            if slots_consumed >= line_limit {
                overflow_this_line = true;
                break;
            }

            if pixels >= pixel_limit {
                overflow_this_line = true;
                break;
            }
        }

        self.prev_line_sprite_overflow = overflow_this_line;
        if overflow_this_line {
            self.status |= STATUS_SOVR;
        }
        visible_added
    }

    fn render_sprites(
        &self,
        sprites: &[SpriteAttributes],
        fetch_line: u16,
        line_buf: &mut [u8; 320],
    ) -> bool {
        let screen_width = self.screen_width();
        let mut collision = false;

        // Render in reverse order so that sprites with lower indices (higher priority)
        // are drawn last and appear on top.
        for attr in sprites.iter().rev() {
            render_sprite_scanline(
                &self.vram,
                line_buf,
                fetch_line,
                attr,
                screen_width,
                &mut collision,
            );
        }
        collision
    }
    /// Fetch Vertical scroll value for the given column.
    /// Supports both Full-screen and 2-cell (16-pixel) strip modes.
    /// V-scroll values in VSRAM are stored as signed words. We preserve the
    /// raw register contents so wraparound matches hardware behavior.
    fn get_v_scroll(&self, is_plane_a: bool, tile_h: usize, fetch_line: u16) -> u16 {
        let (mode3, vsram) = if self.latched_scroll_valid && self.latched_scroll_line == fetch_line
        {
            (self.latched_mode3, &self.latched_vsram)
        } else {
            (self.registers[REG_MODE3], &self.vsram)
        };

        // Vertical Scroll (Bits 2 of Mode 3: 0=Full Screen, 1=2-Cell Strips)
        if (mode3 & 0x04) != 0 {
            // 2-Cell (16-pixel) strips. Each entry in VSRAM is 4 bytes and handles 2 cells.
            // Entry 0: Plane A Cell 0-1, Entry 1: Plane B Cell 0-1, etc.
            let strip_idx = tile_h >> 1;
            let vs_addr = (strip_idx * 4) + (if is_plane_a { 0 } else { 2 });
            if vs_addr + 1 < vsram.len() {
                ((vsram[vs_addr] as u16) << 8) | (vsram[vs_addr + 1] as u16)
            } else {
                0
            }
        } else {
            // Full Screen
            let vs_addr = if is_plane_a { 0 } else { 2 };
            ((vsram[vs_addr] as u16) << 8) | (vsram[vs_addr + 1] as u16)
        }
    }

    /// Fetch Horizontal scroll value for the given line.
    /// Supports Full-screen, 8-pixel strip (Cell), and Per-line modes.
    /// H-scroll values are stored as raw words. We keep the full register
    /// contents so negative scroll values wrap the same way as hardware.
    fn get_h_scroll(&self, _is_plane_a: bool, fetch_line: u16) -> u16 {
        if self.latched_scroll_valid && self.latched_scroll_line == fetch_line {
            return if _is_plane_a {
                self.latched_hscroll_a
            } else {
                self.latched_hscroll_b
            };
        }

        let mode3 = self.registers[REG_MODE3];

        // Horizontal Scroll (Bits 1-0 of Mode 3: 00=Full, 01=Invalid(Full), 10=Cell(8px), 11=Line)
        let hs_mode = mode3 & 0x03;
        let hs_base = self.hscroll_address();

        let hs_addr = match hs_mode {
            0x00 | 0x01 => hs_base,                               // Full screen
            0x02 => hs_base + (((fetch_line as usize) >> 3) * 4), // 8-pixel high strips (Cell)
            0x03 => hs_base + ((fetch_line as usize) * 4),        // Per-line
            _ => hs_base,
        };

        let final_hs_addr = if _is_plane_a {
            hs_addr
        } else {
            hs_addr.wrapping_add(2)
        };

        let hi = self.vram[final_hs_addr & 0xFFFF];
        let lo = self.vram[final_hs_addr.wrapping_add(1) & 0xFFFF];

        ((hi as u16) << 8) | (lo as u16)
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
        let hi = self.vram[nt_entry_addr & 0xFFFF];
        let lo = self.vram[(nt_entry_addr + 1) & 0xFFFF];
        ((hi as u16) << 8) | (lo as u16)
    }

    #[inline(always)]
    fn fetch_tile_pattern(&self, tile_index: u16, pixel_v: u16, v_flip: bool) -> [u8; 4] {
        let row = if v_flip { 7 - pixel_v } else { pixel_v };
        let row_addr = (tile_index as usize * 32) + (row as usize * 4);
        // Mask to 64KB boundary and align to 4 bytes.
        let addr = (row_addr & 0xFFFF) & 0xFFFC;

        self.vram[addr..addr + 4].try_into().unwrap()
    }

    fn draw_partial_tile_row(
        &self,
        entry: u16,
        pixel_v: u16,
        pixel_h: u16,
        count: u16,
        dest_idx: usize,
        line_buf: &mut [u8; 320],
    ) {
        let priority = (entry & 0x8000) != 0;
        let palette = ((entry >> 13) & 0x03) as u8;
        let v_flip = (entry & 0x1000) != 0;
        let h_flip = (entry & 0x0800) != 0;
        let tile_index = entry & 0x07FF;

        let patterns = self.fetch_tile_pattern(tile_index, pixel_v, v_flip);
        let pri_mask = if priority { 0x80 } else { 0x00 };

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

            let final_val = (palette << 4) | col | pri_mask;
            line_buf[dest_idx + i as usize] = final_val;
        }
    }

    #[inline(always)]
    fn draw_full_tile_row(
        &self,
        entry: u16,
        pixel_v: u16,
        dest_idx: usize,
        line_buf: &mut [u8; 320],
    ) {
        let priority = (entry & 0x8000) != 0;
        let palette = ((entry >> 13) & 0x03) as u8;
        let v_flip = (entry & 0x1000) != 0;
        let h_flip = (entry & 0x0800) != 0;
        let tile_index = entry & 0x07FF;

        let patterns = self.fetch_tile_pattern(tile_index, pixel_v, v_flip);

        let pri_mask = if priority { 0x80 } else { 0x00 };
        let pal_base = palette << 4;

        // Optimization: Skip empty rows if priority is also low
        if !priority && u32::from_ne_bytes(patterns) == 0 {
            return;
        }

        if dest_idx + 8 > line_buf.len() {
            return;
        }

        let p0 = patterns[0];
        let p1 = patterns[1];
        let p2 = patterns[2];
        let p3 = patterns[3];

        let dest = &mut line_buf[dest_idx..dest_idx + 8];

        if !h_flip {
            dest[0] = pal_base | (p0 >> 4) | pri_mask;
            dest[1] = pal_base | (p0 & 0x0F) | pri_mask;
            dest[2] = pal_base | (p1 >> 4) | pri_mask;
            dest[3] = pal_base | (p1 & 0x0F) | pri_mask;
            dest[4] = pal_base | (p2 >> 4) | pri_mask;
            dest[5] = pal_base | (p2 & 0x0F) | pri_mask;
            dest[6] = pal_base | (p3 >> 4) | pri_mask;
            dest[7] = pal_base | (p3 & 0x0F) | pri_mask;
        } else {
            dest[0] = pal_base | (p3 & 0x0F) | pri_mask;
            dest[1] = pal_base | (p3 >> 4) | pri_mask;
            dest[2] = pal_base | (p2 & 0x0F) | pri_mask;
            dest[3] = pal_base | (p2 >> 4) | pri_mask;
            dest[4] = pal_base | (p1 & 0x0F) | pri_mask;
            dest[5] = pal_base | (p1 >> 4) | pri_mask;
            dest[6] = pal_base | (p0 & 0x0F) | pri_mask;
            dest[7] = pal_base | (p0 >> 4) | pri_mask;
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
        self.cram_cache[addr & 0x3F]
    }

    fn get_cram_rgb565(&self) -> [u16; 64] {
        self.cram_cache
    }

    fn get_cram_raw(&self) -> [u16; 64] {
        let mut raw = [0u16; 64];
        for (i, value) in raw.iter_mut().enumerate() {
            *value = ((self.cram[i * 2] as u16) << 8) | (self.cram[i * 2 + 1] as u16);
        }
        raw
    }
}
