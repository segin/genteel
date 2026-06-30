import re

def fix():
    with open("src/vdp/render.rs", "r") as f:
        src = f.read()

    # Fix line_buf in render_plane
    src = src.replace(
"""            self.render_tile(
                is_plane_a,
                use_v_scroll,
                tile_base,
                tile_w,
                plane_h,
                tile_w - 1, // tile_w_mask
                tile_h_scroll,
                fetch_line,
                &mut screen_x,
                line_buf,
            );""",
"""            self.render_tile(
                is_plane_a,
                use_v_scroll,
                tile_base,
                tile_w,
                plane_h,
                tile_w - 1, // tile_w_mask
                tile_h_scroll,
                fetch_line,
                &mut screen_x,
                line_buf,
            );"""
    )

    # Let's fix RenderOps implementations:

    src = src.replace(
"""    fn render_plane(
        &mut self,
        is_plane_a: bool,
        fetch_line: u16,
        draw_line: u16,
        priority_filter: bool,
    ) {""",
"""    fn render_plane(
        &self,
        is_plane_a: bool,
        fetch_line: u16,
        line_buf: &mut [u8; 320],
    ) {"""
    )

    src = src.replace(
"""    #[allow(clippy::too_many_arguments)]
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
    ) {""",
"""    #[allow(clippy::too_many_arguments)]
    fn render_tile(
        &self,
        is_plane_a: bool,
        enable_v_scroll: bool,
        name_table_base: usize,
        plane_w: usize,
        plane_h: usize,
        plane_w_mask: usize,
        h_scroll: u16,
        fetch_line: u16,
        screen_x: &mut u16,
        line_buf: &mut [u8; 320],
    ) {"""
    )

    src = src.replace(
"""    fn render_sprites(
        &mut self,
        sprites: &[SpriteAttributes],
        fetch_line: u16,
        draw_line: u16,
        priority_filter: bool,
    ) {""",
"""    fn render_sprites(
        &self,
        sprites: &[SpriteAttributes],
        fetch_line: u16,
        line_buf: &mut [u8; 320],
    ) {"""
    )

    src = src.replace(
"""    fn draw_partial_tile_row(
        &mut self,
        entry: u16,
        pixel_v: u16,
        pixel_h: u16,
        count: u16,
        dest_idx: usize,
    ) {""",
"""    fn draw_partial_tile_row(
        &self,
        entry: u16,
        pixel_v: u16,
        pixel_h: u16,
        count: u16,
        dest_idx: usize,
        line_buf: &mut [u8; 320],
    ) {"""
    )

    src = src.replace(
"""    fn draw_full_tile_row(&mut self, entry: u16, pixel_v: u16, dest_idx: usize) {""",
"""    #[inline(always)]
    fn draw_full_tile_row(&self, entry: u16, pixel_v: u16, dest_idx: usize, line_buf: &mut [u8; 320]) {"""
    )

    # Fix usages in test_draw_row_refactor
    with open("src/vdp/tests_draw_row_refactor.rs", "r") as f2:
        test_src = f2.read()

    # Fix dest_idx comments to reflect per-scanline line_buf semantics
    test_src = test_src.replace(
        "let dest_idx = 0; // Start of framebuffer",
        "let dest_idx = 0; // Start of scanline"
    )
    test_src = test_src.replace(
        "// Try to draw at end of framebuffer\n    let dest_idx = vdp.framebuffer.len() - 4; // Not enough space for 8 pixels",
        "// Try to draw at end of scanline\n    let dest_idx = 316; // Not enough space for 8 pixels in 320-byte line_buf"
    )
    # Use regex for robust replacement of all vdp.draw_full_tile_row calls:
    # add a line_buf declaration and pass it as the new 4th argument.
    test_src = re.sub(
        r"([ \t]+)vdp\.draw_full_tile_row\(([^,]+,\s*[^,]+,\s*[^,)]+)\);",
        r"\1let mut line_buf = [0u8; 320];\n\1vdp.draw_full_tile_row(\2, &mut line_buf);",
        test_src,
    )

    with open("src/vdp/tests_draw_row_refactor.rs", "w") as f2:
        f2.write(test_src)

    with open("src/vdp/render.rs", "w") as f:
        f.write(src)

if __name__ == "__main__":
    fix()
