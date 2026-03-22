def main():
    # Context part given in the prompt
    refactor_info = (
        """    fn draw_full_tile_row(&mut self, entry: u16, pixel_v: u16, dest_idx: usize) {""",
        """    #[inline(always)]
    fn draw_full_tile_row(&self, entry: u16, pixel_v: u16, dest_idx: usize, line_buf: &mut [u8; 320]) {"""
    )

    # Fix usages in test_draw_row_refactor
    with open("src/vdp/tests_draw_row_refactor.rs", "r") as f2:
        test_src = f2.read()

    test_src = test_src.replace(
        "let mut buf = [0u8; 320];",
        "let mut line_buf = [0u8; 320];"
    ).replace(
        "&mut buf",
        "&mut line_buf"
    ).replace(
        "buf[",
        "line_buf["
    )

    with open("src/vdp/tests_draw_row_refactor.rs", "w") as f2:
        f2.write(test_src)

if __name__ == "__main__":
    main()
