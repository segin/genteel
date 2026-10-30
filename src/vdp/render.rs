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
    /// Number of tile columns actually drawn (in screen order). Less than
    /// `h_size` when the line's sprite dot budget runs out inside this sprite.
    #[serde(default)]
    pub draw_tiles: u8,
}

/// Walks a sprite attribute table in link order (used by the sprite viewer).
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

        // Sprite Y is a 9-bit field outside interlace mode 2.
        let cur_v = ((data >> 48) as u16) & 0x01FF;
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

        // Sprite X is a 9-bit field (bits 0-8); bit 9+ are unused.
        let cur_h = (data as u16) & 0x01FF;
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
            draw_tiles: h_size,
        };

        self.count += 1;
        self.next_idx = link;

        // The list ends at a link of 0 or one past the mode's last sprite.
        if link == 0 || link as usize >= self.max_sprites {
            self.count = self.max_sprites;
        }

        Some(attr)
    }
}

#[derive(Clone, Copy)]
pub struct TileRenderParams {
    pub is_plane_a: bool,
    pub enable_v_scroll: bool,
    pub name_table_base: usize,
    pub plane_w: usize,
    pub plane_h: usize,
    pub plane_w_mask: usize,
    pub h_scroll: u16,
    /// Vertical position used for name table / pattern row lookup (the
    /// field line, doubled plus the field parity in interlace mode 2).
    pub fetch_line: u16,
    /// Exclusive end of the pixels this segment may draw.
    pub scanline_width: u16,
    /// Screen X at which this plane segment starts (non-zero for plane A to
    /// the right of a left-side window).
    pub col_origin: u16,
    /// Interlace mode 2: 8x16 tiles.
    pub im2: bool,
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
    /// Raw VSRAM vertical scroll entry for cell column `tile_h` (2-cell mode)
    /// or the full-screen entry.
    fn get_v_scroll(&self, is_plane_a: bool, tile_h: usize) -> u16;
    fn get_h_scroll(&self, is_plane_a: bool, fetch_line: u16) -> u16;
    fn fetch_nametable_entry(
        &self,
        base: usize,
        tile_v: usize,
        tile_h: usize,
        plane_w: usize,
    ) -> u16;
    fn fetch_tile_pattern(&self, tile_index: u16, pixel_v: u16, v_flip: bool) -> [u8; 4];
    fn bg_color(&self) -> (u8, u8);
    fn get_cram_color(&self, palette: u8, index: u8) -> u16;
    fn get_cram_raw(&self) -> [u16; 64];
}

#[derive(Clone, Copy)]
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

/// The plane/sprite layer buffers of one line.
pub(crate) struct LineLayers<'a> {
    pub b: &'a [u8; 320],
    pub a: &'a [u8; 320],
    pub s: &'a [u8; 320],
}

impl PixelLayerData {
    #[inline(always)]
    fn new(bg_color_idx: u8, b: u8, a: u8, s: u8) -> Self {
        let (b_col, a_col, s_col) = (b & 0x3F, a & 0x3F, s & 0x3F);
        PixelLayerData {
            bg_color_idx,
            s_pri: (s & 0x80) != 0,
            s_trans: (s_col & 0x0F) == 0,
            s_col,
            a_pri: (a & 0x80) != 0,
            a_trans: (a_col & 0x0F) == 0,
            a_col,
            b_pri: (b & 0x80) != 0,
            b_trans: (b_col & 0x0F) == 0,
            b_col,
        }
    }
}

/// Colour index of the top opaque pixel, in priority order: high-priority
/// sprite, plane A, plane B, then low-priority sprite, plane A, plane B,
/// then the backdrop. The same rule as `top_layer`, on raw layer bytes.
#[inline(always)]
pub(super) fn top_color(bg_color_idx: u8, b: u8, a: u8, s: u8) -> u8 {
    let opaque = |p: u8| (p & 0x0F) != 0;
    let high = |p: u8| (p & 0x80) != 0 && opaque(p);
    let p = if high(s) {
        s
    } else if high(a) {
        a
    } else if high(b) {
        b
    } else if opaque(s) {
        s
    } else if opaque(a) {
        a
    } else if opaque(b) {
        b
    } else {
        return bg_color_idx;
    };
    p & 0x3F
}

/// Composite pixels `[start_x, width)` of one framebuffer row from the
/// layer buffers, and fill `[width, 320)` (the unused part of the row in
/// H32) with the backdrop.
#[allow(clippy::too_many_arguments)]
fn composite_row(
    row: &mut [u16],
    layers: &LineLayers,
    cram: &[u16; 64],
    regs: &[u8; NUM_REGISTERS],
    bg_color_idx: u8,
    bg_color_val: u16,
    start_x: usize,
    width: usize,
) {
    let sh_enabled = (regs[REG_MODE4] & 0x08) != 0;
    let width = width.min(320);
    let start_x = start_x.min(width);
    // Column 0 masking (register 0 bit 5) shows the backdrop.
    let mask_end = if (regs[REG_MODE1] & 0x20) != 0 { 8 } else { 0 };
    let first = mask_end.clamp(start_x, width);
    row[start_x..first].fill(bg_color_val);

    let pixels = (first..width).map(|x| (x, layers.b[x], layers.a[x], layers.s[x]));
    if sh_enabled {
        for (x, b, a, s) in pixels {
            let px = PixelLayerData::new(bg_color_idx, b, a, s);
            let (top_col, state) = shadow_highlight(&px);
            row[x] = color_transform(cram[top_col as usize], state);
        }
    } else {
        for (x, b, a, s) in pixels {
            row[x] = cram[top_color(bg_color_idx, b, a, s) as usize];
        }
    }
    // Empty pixels: the backdrop, shadowed in shadow/highlight mode.
    let empty = if sh_enabled {
        color_transform(bg_color_val, 0)
    } else {
        bg_color_val
    };
    row[width..320].fill(empty);
}

/// Shadow/highlight resolution, following Genesis Plus GX
/// `make_lut_bg_ste` / `make_lut_bgobj_ste`. Returns the colour index and
/// intensity (0 = shadow, 1 = normal, 2 = highlight).
///
///   * The planes are normal intensity if either plane pixel has its
///     priority bit set, even when that pixel is transparent; otherwise
///     they are shadowed.
///   * A sprite pixel is considered unless it is low priority and the
///     plane pixel is opaque and high priority.
///   * Palette 3 colour 14 (highlight) and 15 (shadow) sprite pixels are
///     operators: the plane/backdrop shows through, one step brighter
///     (shadow -> normal, normal -> highlight) or forced to shadow.
///   * Sprite colour 14 of palettes 0-2 is always normal intensity.
///   * Other sprite pixels are normal if the sprite is high priority or
///     the planes are normal, otherwise shadowed.
pub(super) fn shadow_highlight(px: &PixelLayerData) -> (u8, u8) {
    let planes_normal = px.a_pri || px.b_pri;
    let plane_only = PixelLayerData {
        s_pri: false,
        s_trans: true,
        s_col: 0,
        ..*px
    };
    let (bg_col, bg_layer) = top_layer(&plane_only);
    let bg_high_opaque = match bg_layer {
        2 => px.a_pri,
        1 => px.b_pri,
        _ => false,
    };
    let bg_state = u8::from(planes_normal);

    if px.s_trans || (!px.s_pri && bg_high_opaque) {
        return (bg_col, bg_state);
    }
    match px.s_col {
        0x3E => (bg_col, bg_state + 1),
        0x3F => (bg_col, 0),
        0x0E | 0x1E | 0x2E => (px.s_col, 1),
        _ => (px.s_col, u8::from(px.s_pri || planes_normal)),
    }
}

/// Top pixel and its layer (0 = backdrop, 1 = B, 2 = A, 3 = sprite).
pub(super) fn top_layer(px: &PixelLayerData) -> (u8, u8) {
    if px.s_pri && !px.s_trans {
        (px.s_col, 3)
    } else if px.a_pri && !px.a_trans {
        (px.a_col, 2)
    } else if px.b_pri && !px.b_trans {
        (px.b_col, 1)
    } else if !px.s_trans {
        (px.s_col, 3)
    } else if !px.a_trans {
        (px.a_col, 2)
    } else if !px.b_trans {
        (px.b_col, 1)
    } else {
        (px.bg_color_idx, 0)
    }
}

/// Apply a shadow (0) or highlight (2) intensity to an RGB565 colour.
pub(super) fn color_transform(color: u16, state: u8) -> u16 {
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

impl Vdp {
    /// Vertical position of `line` for plane/sprite lookup: the line itself,
    /// or in interlace mode 2 the line doubled plus the field parity.
    #[inline]
    pub(crate) fn vertical_fetch_line(&self, line: u16) -> u16 {
        if self.interlace_mode_2() {
            let odd = u16::from((self.status & STATUS_ODD_FRAME) != 0);
            (line << 1) | odd
        } else {
            line
        }
    }

    /// Pattern bytes for one row of the tile named by `entry`.
    #[inline]
    fn tile_row_pattern(&self, entry: u16, pixel_v: u16, im2: bool) -> [u8; 4] {
        let v_flip = (entry & 0x1000) != 0;
        if im2 {
            // Interlace mode 2: 8x16 tiles of 64 bytes, 10-bit tile index.
            let row = if v_flip { 15 - pixel_v } else { pixel_v };
            let addr = ((((entry & 0x03FF) as usize) << 6) + (row as usize * 4)) & 0xFFFC;
            self.vram[addr..addr + 4].try_into().unwrap()
        } else {
            self.fetch_tile_pattern(entry & 0x07FF, pixel_v, v_flip)
        }
    }

    /// Vertical scroll for the plane column containing screen pixel `x`.
    ///
    /// In 2-cell mode each 16-pixel column of the scrolled plane uses its own
    /// VSRAM entry, counted from the first fully scrolled-in column (GPGX
    /// `render_bg_m5_vs`). The partially shown column at the left edge uses,
    /// in H40, the AND of the plane A and plane B values of VSRAM entry 19;
    /// in H32 it is not scrolled.
    fn column_v_scroll(&self, params: &TileRenderParams, x: u16) -> u16 {
        if (self.registers[REG_MODE3] & 0x04) == 0 {
            return self.get_v_scroll(params.is_plane_a, 0);
        }
        let shift = (params.h_scroll & 0x0F) as i32;
        let column = (x as i32 - shift).div_euclid(16);
        if column < (params.col_origin / 16) as i32 {
            if self.h40_mode() {
                self.get_v_scroll(true, 38) & self.get_v_scroll(false, 38)
            } else {
                0
            }
        } else {
            self.get_v_scroll(params.is_plane_a, column as usize * 2)
        }
    }

    /// Render the plane and sprite layers of `line` into the layer cache.
    fn render_layers(&mut self, line: u16) {
        self.ensure_sat_cache();

        /* Re-rendering the same line must evaluate sprites against the same
        previous-line overflow state as its first render, and must not set
        the sprite overflow and collision flags again: the 68k may have read
        (and so cleared) them since the line was first drawn. */
        let rerender = self.layer_line == Some(line);
        let flags = self.status & (STATUS_SOVR | STATUS_COLLISION);
        if rerender {
            self.prev_line_sprite_overflow = self.layer_prev_overflow;
        } else {
            self.layer_prev_overflow = self.prev_line_sprite_overflow;
        }

        let mut buf_b = [0u8; 320];
        let mut buf_a = [0u8; 320];
        let mut buf_s = [0u8; 320];

        if self.display_enabled() {
            let mut sprite_buffer = [SpriteAttributes::default(); 80];
            let sprite_count = self.get_active_sprites(line, &mut sprite_buffer);
            let active_sprites = &sprite_buffer[..sprite_count];

            if self.debug_plane != Some('a') {
                self.render_plane(false, line, &mut buf_b);
            }
            if self.debug_plane != Some('b') {
                self.render_plane(true, line, &mut buf_a);
            }
            if self.render_sprites(active_sprites, line, &mut buf_s) {
                self.status |= STATUS_COLLISION;
            }
        }
        if rerender {
            self.status = (self.status & !(STATUS_SOVR | STATUS_COLLISION)) | flags;
        }

        self.layer_b = buf_b;
        self.layer_a = buf_a;
        self.layer_s = buf_s;
        self.layer_line = Some(line);
    }

    /// Composite the cached layers of `line` into the framebuffer from
    /// `start_x` to the end of the line.
    fn composite_from(&mut self, line: u16, start_x: u16) {
        let line_offset = (line as usize) * 320;
        let (pal_line, color_idx) = self.bg_color();
        let bg_color_val = self.get_cram_color(pal_line, color_idx);
        let bg_color_idx = (pal_line << 4) | color_idx;

        if !self.display_enabled() {
            self.framebuffer[line_offset + start_x as usize..line_offset + 320].fill(bg_color_val);
            return;
        }

        let width = usize::from(self.screen_width());
        let layers = LineLayers {
            b: &self.layer_b,
            a: &self.layer_a,
            s: &self.layer_s,
        };
        composite_row(
            &mut self.framebuffer[line_offset..line_offset + 320],
            &layers,
            &self.cram_cache,
            &self.registers,
            bg_color_idx,
            bg_color_val,
            start_x as usize,
            width,
        );
    }

    /// Recomposite `line` from `start_x` with the current colours, reusing
    /// the line's cached layers.
    pub(crate) fn recomposite_line_from(&mut self, line: u16, start_x: u16) {
        if line >= self.screen_height() || start_x >= self.screen_width() {
            return;
        }
        if self.layer_line != Some(line) {
            self.render_layers(line);
        }
        self.composite_from(line, start_x);
    }
}

#[allow(clippy::too_many_arguments)]
fn render_sprite_scanline(
    vram: &[u8],
    line_buf: &mut [u8; 320],
    line: u16,
    attr: &SpriteAttributes,
    screen_width: u16,
    im2: bool,
    collision: &mut bool,
) {
    let row_h: u16 = if im2 { 16 } else { 8 };
    let sprite_v_px = (attr.v_size as u16) * row_h;

    let py = (line as i32) - (attr.v_pos as i16 as i32);
    if py < 0 || py >= sprite_v_px as i32 {
        return;
    }

    let fetch_py = if attr.v_flip {
        (sprite_v_px - 1) - (py as u16)
    } else {
        py as u16
    };

    let tile_v_offset = fetch_py / row_h;
    let pixel_v = fetch_py % row_h;
    let pri_mask = if attr.priority { 0x80 } else { 0x00 };

    // Iterate by tiles instead of pixels for efficiency
    for t_h in 0..attr.draw_tiles.min(attr.h_size) {
        let tile_h_offset = t_h as u16;
        let fetch_tile_h_offset = if attr.h_flip {
            (attr.h_size as u16 - 1) - tile_h_offset
        } else {
            tile_h_offset
        };

        // In a multi-tile sprite, tiles are arranged vertically first.
        let tile = attr
            .base_tile
            .wrapping_add(fetch_tile_h_offset * attr.v_size as u16)
            .wrapping_add(tile_v_offset);
        let row_addr = if im2 {
            ((tile & 0x03FF) as usize * 64) + (pixel_v as usize * 4)
        } else {
            ((tile & 0x07FF) as usize * 32) + (pixel_v as usize * 4)
        };

        // Check if row is within VRAM bounds
        if row_addr + 4 > 0x10000 {
            continue;
        }

        let patterns: [u8; 4] = vram[row_addr..row_addr + 4].try_into().unwrap();
        let base_screen_x = (attr.h_pos as i16 as i32) + (tile_h_offset as i32 * 8);

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
                if let Some(pixel) = line_buf.get_mut(screen_x as usize) {
                    if (*pixel & 0x0F) != 0 {
                        *collision = true;
                    } else {
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
        self.render_layers(line);
        self.composite_from(line, 0);
        self.rendered_scanlines[line as usize] = true;
    }

    fn render_line_from(&mut self, line: u16, start_x: u16) {
        if line >= self.screen_height() || start_x >= self.screen_width() {
            return;
        }
        self.render_layers(line);
        self.composite_from(line, start_x);
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
        let im2 = self.interlace_mode_2();
        let v_line = self.vertical_fetch_line(fetch_line);

        let mut screen_x: u16 = 0;

        let plane_params = TileRenderParams {
            is_plane_a,
            enable_v_scroll: true,
            name_table_base,
            plane_w,
            plane_h,
            plane_w_mask: plane_w - 1,
            h_scroll,
            fetch_line: v_line,
            scanline_width: screen_width,
            col_origin: 0,
            im2,
        };

        if !is_plane_a {
            // Plane B never has a window
            while screen_x < screen_width {
                self.render_tile(&plane_params, &mut screen_x, line_buf);
            }
            return;
        }

        let h_pos = self.registers[REG_WINDOW_H_POS];
        let v_pos = self.registers[REG_WINDOW_V_POS];
        let win_h_point = ((h_pos as u16 & 0x1F) * 16).min(screen_width);
        let v_point = (v_pos as u16 & 0x1F) * 8;
        let win_h_dir = (h_pos & 0x80) != 0;
        let v_dir = (v_pos & 0x80) != 0;
        let win_full_line = if v_dir {
            fetch_line >= v_point
        } else {
            fetch_line < v_point
        };
        let win_w = if h40_mode { 64 } else { 32 };

        let win_params = TileRenderParams {
            is_plane_a: true,
            enable_v_scroll: false,
            name_table_base: self.window_address(),
            plane_w: win_w,
            plane_h,
            plane_w_mask: win_w - 1,
            h_scroll: 0,
            fetch_line: v_line,
            scanline_width: screen_width,
            col_origin: 0,
            im2,
        };
        /* Plane A segment: left of a right-side window it must stop at the
        window boundary; right of a left-side window its columns are counted
        from the boundary. */
        let plane_params = TileRenderParams {
            scanline_width: if !win_full_line && win_h_dir {
                win_h_point
            } else {
                screen_width
            },
            col_origin: if !win_full_line && !win_h_dir {
                win_h_point
            } else {
                0
            },
            ..plane_params
        };

        /* H40 window/plane A boundary glitch: once per line, at the first
        window to plane A transition, when the window does not span the
        whole line and plane A has a fine horizontal scroll (GPGX
        `render_bg_m5` applies its "window bug" only `if (shift)`, i.e.
        `xscroll & 0x0F`). The transition must also fall on a tile-aligned
        screen X. */
        let glitch_enabled = h40_mode && (h_scroll & 0x0F) != 0 && !win_h_dir && !win_full_line;
        let mut glitch_fired = false;
        let in_window = |x: u16| {
            win_full_line
                || if win_h_dir {
                    x >= win_h_point
                } else {
                    x < win_h_point
                }
        };
        let mut prev_in_window = in_window(screen_x);
        while screen_x < screen_width {
            let now_in_window = in_window(screen_x);

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
    }

    fn render_tile(&self, params: &TileRenderParams, screen_x: &mut u16, line_buf: &mut [u8; 320]) {
        let current_x = *screen_x;
        let scrolled_h = current_x.wrapping_sub(params.h_scroll);
        let pixel_h = scrolled_h & 0x07;
        let tile_h = ((scrolled_h >> 3) as usize) & params.plane_w_mask;

        let v_scroll = if params.enable_v_scroll {
            self.column_v_scroll(params, current_x)
        } else {
            0
        };

        // Vertical position in plane
        let scrolled_v = params.fetch_line.wrapping_add(v_scroll);
        let (tile_v, pixel_v) = if params.im2 {
            (
                ((scrolled_v >> 4) as usize) % params.plane_h,
                scrolled_v & 15,
            )
        } else {
            (
                ((scrolled_v >> 3) as usize) % params.plane_h,
                scrolled_v & 7,
            )
        };

        let pixels_left_in_tile = 8 - pixel_h;
        let limit = params.scanline_width.max(current_x + 1);
        let pixels_to_process = std::cmp::min(pixels_left_in_tile, limit - current_x);

        let entry =
            self.fetch_nametable_entry(params.name_table_base, tile_v, tile_h, params.plane_w);
        let patterns = self.tile_row_pattern(entry, pixel_v, params.im2);

        if pixels_to_process == 8 && pixel_h == 0 {
            blit_full_tile_row(entry, patterns, current_x as usize, line_buf);
        } else {
            blit_partial_tile_row(
                entry,
                patterns,
                pixel_h,
                pixels_to_process,
                current_x as usize,
                line_buf,
            );
        }
        *screen_x = current_x + pixels_to_process;
    }

    fn get_active_sprites(&mut self, line: u16, sprites: &mut [SpriteAttributes]) -> usize {
        self.ensure_sat_cache();

        let h40 = self.h40_mode();
        let max_sprites = if h40 { 80 } else { 64 };
        let line_limit = if h40 { 20 } else { 16 };
        let pixel_limit = if h40 { 320 } else { 256 };

        /* Y position, size and link come from the internal SAT cache; the
        pattern/attribute word and X position are read from VRAM (GPGX
        `parse_satb_m5`). Y is 9 bits (10 bits, offset 256 and in half-lines
        in interlace mode 2). */
        let im2 = self.interlace_mode_2();
        let (y_mask, y_offset, row_h) = if im2 {
            (0x03FFu16, 256u16, 16u16)
        } else {
            (0x01FF, 128, 8)
        };
        let v_line = self.vertical_fetch_line(line);
        let sat_base = self.sprite_table_address();

        /* X=0 sprite mask state. Triggered when an X=0 sprite is encountered
        and either a non-X=0 sprite has already been added to this line, or
        the previous line reached its sprite dot limit. Once triggered,
        later sprites on this line are not rendered (but still count toward
        the per-line limits). */
        let mut mask_triggered = false;
        let mut visible_added: usize = 0;
        let mut slots_consumed: usize = 0;
        let mut pixels: usize = 0;
        let prev_overflow = self.prev_line_sprite_overflow;
        let mut count_overflow = false;

        let mut idx: usize = 0;
        for _ in 0..max_sprites {
            let c = idx * 8;
            let y = u16::from_be_bytes([self.sat[c], self.sat[c + 1]]) & y_mask;
            let size = self.sat[c + 2];
            let link = (self.sat[c + 3] & 0x7F) as usize;
            let h_size = ((size >> 2) & 0x03) + 1;
            let v_size = (size & 0x03) + 1;

            let top = y as i32 - y_offset as i32;
            let rel = v_line as i32 - top;
            if rel >= 0 && rel < (v_size as u16 * row_h) as i32 {
                /* One sprite more than the per-line count limit sets SOVR
                (GPGX `parse_satb_m5`); once the dot budget is used up, later
                sprites are simply not drawn (`render_obj_m5`). */
                if slots_consumed >= line_limit {
                    count_overflow = true;
                    break;
                }
                if pixels >= pixel_limit {
                    break;
                }

                let a = (sat_base + c + 4) & 0xFFFF;
                let attr_word = u16::from_be_bytes([self.vram[a], self.vram[(a + 1) & 0xFFFF]]);
                let x_word =
                    u16::from_be_bytes([self.vram[(a + 2) & 0xFFFF], self.vram[(a + 3) & 0xFFFF]]);
                let h_pos = (x_word & 0x01FF).wrapping_sub(128);
                // The dot budget can run out inside this sprite: draw only the
                // tiles that fit.
                let draw_tiles = (h_size as usize).min((pixel_limit - pixels) / 8) as u8;

                let attr = SpriteAttributes {
                    v_pos: top as i16 as u16,
                    h_pos,
                    h_size,
                    v_size,
                    priority: (attr_word & 0x8000) != 0,
                    palette: ((attr_word >> 13) & 0x03) as u8,
                    v_flip: (attr_word & 0x1000) != 0,
                    h_flip: (attr_word & 0x0800) != 0,
                    base_tile: attr_word & 0x07FF,
                    index: idx as u8,
                    link: link as u8,
                    draw_tiles,
                };

                /* An X=0 sprite (raw X field 0) is fully off-screen and never
                renders, but it still consumes a per-line slot and dot budget,
                and arms the mask. */
                let is_x0_mask = (x_word & 0x01FF) == 0;
                if is_x0_mask {
                    if visible_added > 0 || prev_overflow {
                        mask_triggered = true;
                    }
                } else if !mask_triggered && visible_added < sprites.len() {
                    sprites[visible_added] = attr;
                    visible_added += 1;
                }

                slots_consumed += 1;
                pixels += (h_size as usize) * 8;
            }

            // The list ends at link 0 or one past the mode's last sprite.
            if link == 0 || link >= max_sprites {
                break;
            }
            idx = link;
        }

        /* Reaching the dot limit (not the count limit) arms the X=0 mask for
        the next line (GPGX `render_obj_m5`: `spr_ovr = pixelcount >= w`). */
        self.prev_line_sprite_overflow = pixels >= pixel_limit;
        if count_overflow {
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
        let im2 = self.interlace_mode_2();
        let v_line = self.vertical_fetch_line(fetch_line);
        let mut collision = false;

        /* Sprites are drawn front to back: the first sprite in link order
        wins a pixel, and a later opaque pixel over it only flags a
        collision. */
        for attr in sprites.iter() {
            render_sprite_scanline(
                &self.vram,
                line_buf,
                v_line,
                attr,
                screen_width,
                im2,
                &mut collision,
            );
        }
        collision
    }

    /// Fetch the vertical scroll value for cell column `tile_h`.
    /// Supports both Full-screen and 2-cell (16-pixel) strip modes.
    fn get_v_scroll(&self, is_plane_a: bool, tile_h: usize) -> u16 {
        let plane_off = if is_plane_a { 0 } else { 2 };
        let vs_addr = if (self.registers[REG_MODE3] & 0x04) != 0 {
            // Entry n covers cells 2n and 2n+1: plane A word, then plane B.
            (tile_h >> 1) * 4 + plane_off
        } else {
            plane_off
        };
        if vs_addr + 1 < VSRAM_SIZE {
            u16::from_be_bytes([self.vsram[vs_addr], self.vsram[vs_addr + 1]])
        } else {
            0
        }
    }

    /// Fetch Horizontal scroll value for the given line.
    /// Supports Full-screen, 8-pixel strip (Cell), and Per-line modes.
    fn get_h_scroll(&self, is_plane_a: bool, fetch_line: u16) -> u16 {
        let (a, b) = self.compute_hscroll_words(fetch_line, self.registers[REG_MODE3]);
        if is_plane_a {
            a
        } else {
            b
        }
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

    fn get_cram_raw(&self) -> [u16; 64] {
        let mut raw = [0u16; 64];
        for (i, value) in raw.iter_mut().enumerate() {
            // CRAM is stored low byte first; return the word as the VDP sees it.
            *value = u16::from_le_bytes([self.cram[i * 2], self.cram[i * 2 + 1]]);
        }
        raw
    }
}

pub(super) fn blit_partial_tile_row(
    entry: u16,
    patterns: [u8; 4],
    pixel_h: u16,
    count: u16,
    dest_idx: usize,
    line_buf: &mut [u8; 320],
) {
    let pri_mask = if (entry & 0x8000) != 0 { 0x80 } else { 0x00 };
    let palette = ((entry >> 13) & 0x03) as u8;
    let h_flip = (entry & 0x0800) != 0;

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

        if let Some(px) = line_buf.get_mut(dest_idx + i as usize) {
            *px = (palette << 4) | col | pri_mask;
        }
    }
}

#[inline(always)]
pub(super) fn blit_full_tile_row(
    entry: u16,
    patterns: [u8; 4],
    dest_idx: usize,
    line_buf: &mut [u8; 320],
) {
    let priority = (entry & 0x8000) != 0;
    let palette = ((entry >> 13) & 0x03) as u8;
    let h_flip = (entry & 0x0800) != 0;

    let pri_mask = if priority { 0x80 } else { 0x00 };
    let pal_base = palette << 4;

    // Optimization: Skip empty rows if priority is also low
    if !priority && u32::from_ne_bytes(patterns) == 0 {
        return;
    }

    if dest_idx + 8 > line_buf.len() {
        return;
    }

    let [p0, p1, p2, p3] = patterns;
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
