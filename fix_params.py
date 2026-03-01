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
    
    # Wait, the error `cannot find value line_buf in this scope` was in `render_plane` line 431.
    # Ah, the argument to `render_tile` is `line_buf`, but `render_plane` doesn't have it?
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
    
    test_src = test_src.replace(
        "vdp.draw_full_tile_row(entry, pixel_v, dest_idx);",
        "let mut buf = [0u8; 320];
    vdp.draw_full_tile_row(entry, pixel_v, dest_idx, &mut buf);"
    )
    test_src = test_src.replace(
        "vdp.draw_full_tile_row(0, 0, dest_idx);",
        "let mut buf = [0u8; 320];
    vdp.draw_full_tile_row(0, 0, dest_idx, &mut buf);"
    )
    test_src = test_src.replace(
        "vdp.draw_full_tile_row(entry, 0, 0);",
        "let mut buf = [0u8; 320];
    vdp.draw_full_tile_row(entry, 0, 0, &mut buf);"
    )
    
    with open("src/vdp/tests_draw_row_refactor.rs", "w") as f2:
        f2.write(test_src)

    with open("src/vdp/render.rs", "w") as f:
        f.write(src)

fix()