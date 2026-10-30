//! Rendering rules: window clipping, 2-cell vertical scroll columns, sprite
//! table parsing limits, the SAT cache and shadow/highlight priorities.

use super::render::{shadow_highlight, PixelLayerData};
use super::*;

const RED: u16 = 0xF800;
const GREEN: u16 = 0x07E0;
const BLUE: u16 = 0x001F;

/// Display on, H40, plane A at 0xC000, tile 1 = colour 1 (red), tile 2 =
/// colour 2 (green), tile 3 = colour 3 (blue).
fn base_vdp() -> Vdp {
    let mut vdp = Vdp::new();
    vdp.registers[REG_MODE2] = MODE2_DISPLAY_ENABLE;
    vdp.registers[REG_PLANE_A] = 0x30;
    vdp.registers[REG_PLANE_B] = 0x05; // plane B at 0xA000 (all tile 0)
    vdp.registers[REG_MODE4] = MODE4_RS1;
    vdp.registers[REG_HSCROLL] = 0x3C; // H-scroll table at 0xF000
    vdp.cram_cache[1] = RED;
    vdp.cram_cache[2] = GREEN;
    vdp.cram_cache[3] = BLUE;
    for i in 0..32 {
        vdp.vram[32 + i] = 0x11;
        vdp.vram[64 + i] = 0x22;
        vdp.vram[96 + i] = 0x33;
    }
    vdp
}

fn set_nt(vdp: &mut Vdp, base: usize, width: usize, row: usize, col: usize, tile: u16) {
    let a = base + (row * width + col) * 2;
    vdp.vram[a] = (tile >> 8) as u8;
    vdp.vram[a + 1] = tile as u8;
}

fn fill_row(vdp: &mut Vdp, base: usize, width: usize, row: usize, tile: u16) {
    for col in 0..width {
        set_nt(vdp, base, width, row, col, tile);
    }
}

fn set_hscroll_a(vdp: &mut Vdp, value: u16) {
    vdp.vram[0xF000] = (value >> 8) as u8;
    vdp.vram[0xF001] = value as u8;
}

fn set_vsram(vdp: &mut Vdp, entry: usize, plane_b: bool, value: u16) {
    let i = entry * 4 + if plane_b { 2 } else { 0 };
    vdp.vsram[i] = (value >> 8) as u8;
    vdp.vsram[i + 1] = value as u8;
}

#[test]
fn plane_a_stops_at_right_side_window_boundary() {
    let mut vdp = base_vdp();
    fill_row(&mut vdp, 0xC000, 32, 0, 1); // plane A: red
    vdp.registers[REG_WINDOW] = 0x2C; // window at 0xB000 (H40)
    fill_row(&mut vdp, 0xB000, 64, 0, 2); // window: green
    vdp.registers[REG_WINDOW_H_POS] = 0x80 | 10; // window from x = 160
    vdp.registers[REG_WINDOW_V_POS] = 0x00;
    set_hscroll_a(&mut vdp, 3);

    vdp.render_line(0);
    assert!(vdp.framebuffer[150..160].iter().all(|&p| p == RED));
    assert!(
        vdp.framebuffer[160..176].iter().all(|&p| p == GREEN),
        "plane A pixels must not spill past the window boundary: {:04X?}",
        &vdp.framebuffer[158..166]
    );
}

#[test]
fn two_cell_vscroll_columns_follow_the_scrolled_plane() {
    let mut vdp = base_vdp();
    vdp.registers[REG_MODE3] = 0x04; // 2-cell vertical scroll
    fill_row(&mut vdp, 0xC000, 32, 0, 1); // row 0: red
    fill_row(&mut vdp, 0xC000, 32, 1, 2); // row 1: green
    set_hscroll_a(&mut vdp, 4);
    set_vsram(&mut vdp, 0, false, 0); // column 0 -> row 0
    set_vsram(&mut vdp, 1, false, 8); // column 1 -> row 1

    vdp.render_line(0);
    // Column 0 of the scrolled plane spans screen x 4..20, column 1 20..36.
    assert_eq!(vdp.framebuffer[4], RED);
    assert_eq!(vdp.framebuffer[19], RED, "x=19 is still plane column 0");
    assert_eq!(vdp.framebuffer[20], GREEN);
    assert_eq!(vdp.framebuffer[35], GREEN);
}

#[test]
fn h40_partial_left_column_uses_and_of_both_planes_entry_19() {
    let mut vdp = base_vdp();
    vdp.registers[REG_MODE3] = 0x04;
    fill_row(&mut vdp, 0xC000, 32, 0, 1);
    fill_row(&mut vdp, 0xC000, 32, 1, 2);
    fill_row(&mut vdp, 0xC000, 32, 3, 3);
    set_hscroll_a(&mut vdp, 4);
    set_vsram(&mut vdp, 0, false, 0);
    // A19 = 0x18 (row 3), B19 = 0x0C: AND = 0x08 (row 1, green).
    set_vsram(&mut vdp, 19, false, 0x18);
    set_vsram(&mut vdp, 19, true, 0x0C);

    vdp.render_line(0);
    assert!(vdp.framebuffer[0..4].iter().all(|&p| p == GREEN));
    assert_eq!(vdp.framebuffer[4], RED);
}

#[test]
fn h32_partial_left_column_is_not_vertically_scrolled() {
    let mut vdp = base_vdp();
    vdp.registers[REG_MODE4] = 0; // H32
    vdp.registers[REG_MODE3] = 0x04;
    fill_row(&mut vdp, 0xC000, 32, 0, 1);
    fill_row(&mut vdp, 0xC000, 32, 1, 2);
    set_hscroll_a(&mut vdp, 4);
    set_vsram(&mut vdp, 0, false, 8);
    set_vsram(&mut vdp, 19, false, 8);
    set_vsram(&mut vdp, 19, true, 8);

    vdp.render_line(0);
    assert!(vdp.framebuffer[0..4].iter().all(|&p| p == RED));
    assert_eq!(vdp.framebuffer[4], GREEN);
}

fn put_sprite(vdp: &mut Vdp, slot: usize, raw_y: u16, size: u8, link: u8, tile: u16, raw_x: u16) {
    let a = vdp.sprite_table_address() + slot * 8;
    let bytes = [
        (raw_y >> 8) as u8,
        raw_y as u8,
        size,
        link,
        (tile >> 8) as u8,
        tile as u8,
        (raw_x >> 8) as u8,
        raw_x as u8,
    ];
    vdp.vram[a..a + 8].copy_from_slice(&bytes);
}

fn sprite_vdp() -> Vdp {
    let mut vdp = base_vdp();
    vdp.registers[REG_SPRITE_TABLE] = 0x6A; // 0xD400
    vdp.cram_cache[0x11] = BLUE;
    // Tile 4: colour 1 (palette 1 via attribute -> 0x11).
    for i in 0..32 {
        vdp.vram[128 + i] = 0x11;
    }
    vdp
}

#[test]
fn sprite_y_is_nine_bits() {
    let mut vdp = sprite_vdp();
    // Raw Y 0x280 & 0x1FF = 0x080 -> screen line 0.
    put_sprite(&mut vdp, 0, 0x280, 0x00, 0, 0x2004, 128 + 40);
    vdp.render_line(0);
    assert_eq!(vdp.framebuffer[40], BLUE);
}

#[test]
fn sprite_link_past_last_sprite_ends_the_list() {
    let mut vdp = sprite_vdp();
    vdp.registers[REG_MODE4] = 0; // H32: 64 sprites
    vdp.registers[REG_SPRITE_TABLE] = 0x60; // 0xC000.. keep away from plane A
    vdp.registers[REG_PLANE_A] = 0x38; // plane A at 0xE000
    put_sprite(&mut vdp, 0, 128 + 10, 0x00, 70, 0x2004, 128 + 8);
    put_sprite(&mut vdp, 70, 128 + 10, 0x00, 0, 0x2004, 128 + 40);
    let mut buf = [SpriteAttributes::default(); 80];
    assert_eq!(vdp.get_active_sprites(10, &mut buf), 1);
}

#[test]
fn sprite_crossing_dot_budget_is_truncated() {
    let mut vdp = sprite_vdp();
    // Nine 4-tile sprites (288 dots) and one 3-tile sprite (312 dots), all
    // off-screen to the left but not at X=0.
    for slot in 0..10 {
        let size = if slot == 9 { 0x08 } else { 0x0C };
        put_sprite(&mut vdp, slot, 128 + 10, size, slot as u8 + 1, 0x2004, 1);
    }
    // A 4-tile sprite at x = 100: only 8 dots of budget remain.
    put_sprite(&mut vdp, 10, 128 + 10, 0x0C, 0, 0x2004, 128 + 100);
    vdp.render_line(10);
    let row = &vdp.framebuffer[10 * 320..11 * 320];
    assert!(row[100..108].iter().all(|&p| p == BLUE), "first tile drawn");
    assert!(
        row[108..132].iter().all(|&p| p != BLUE),
        "tiles past the dot budget must not be drawn"
    );
}

#[test]
fn sat_cache_follows_vram_writes_but_not_register_5() {
    let mut vdp = sprite_vdp();
    vdp.bypass_fifo = true;
    vdp.registers[REG_AUTO_INC] = 2;
    vdp.ensure_sat_cache();

    // Write sprite 0 through the data port: Y=line 10, 1x1, X=40.
    vdp.write_control(0x4000 | (0xD400 & 0x3FFF));
    vdp.write_control(0x0003); // address bits 15-14 = 11
    for w in [128 + 10, 0x0000, 0x2004, 128 + 40] {
        vdp.write_data(w);
    }
    vdp.render_line(10);
    assert_eq!(
        vdp.framebuffer[10 * 320 + 40],
        BLUE,
        "cache picked up the write"
    );

    /* Point register 5 at a table whose Y says line 50; the cache must keep
    the old Y/size/link, while attribute and X come from the new table. */
    vdp.registers[REG_SPRITE_TABLE] = 0x70; // 0xE000
    put_sprite(&mut vdp, 0, 128 + 50, 0x00, 0, 0x2004, 128 + 80);
    vdp.render_line(10);
    assert_eq!(
        vdp.framebuffer[10 * 320 + 80],
        BLUE,
        "Y from cache, X from VRAM"
    );
    vdp.render_line(50);
    assert_ne!(vdp.framebuffer[50 * 320 + 80], BLUE);
}

#[test]
fn cram_recomposite_matches_full_rerender() {
    let setup = || {
        let mut vdp = base_vdp();
        vdp.registers[REG_SPRITE_TABLE] = 0x6A;
        fill_row(&mut vdp, 0xC000, 32, 0, 1);
        set_nt(&mut vdp, 0xC000, 32, 0, 3, 0x8002); // a high-priority tile
        vdp.registers[REG_MODE4] = MODE4_RS1 | 0x08; // shadow/highlight on
        vdp.render_line(0);
        vdp
    };
    let original = setup();
    let mut a = setup();
    let mut b = setup();
    a.cram_cache[1] = BLUE;
    a.recomposite_line_from(0, 100);
    b.cram_cache[1] = BLUE;
    b.render_line_from(0, 100);
    assert_eq!(&a.framebuffer[..320], &b.framebuffer[..320]);
    assert_eq!(&a.framebuffer[..100], &original.framebuffer[..100]);
    assert_ne!(a.framebuffer[100], original.framebuffer[100]);
}

#[test]
fn shadow_highlight_priority_rules() {
    let px = |s_pri, s_col: u8, a_pri, a_col: u8, b_pri, b_col: u8| PixelLayerData {
        bg_color_idx: 0x05,
        s_pri,
        s_trans: s_col & 0x0F == 0,
        s_col,
        a_pri,
        a_trans: a_col & 0x0F == 0,
        a_col,
        b_pri,
        b_trans: b_col & 0x0F == 0,
        b_col,
    };
    // A transparent high-priority plane pixel still lifts the backdrop.
    assert_eq!(
        shadow_highlight(&px(false, 0, true, 0, false, 0)),
        (0x05, 1)
    );
    // Sprite colour 14 of palettes 0-2 is never shadowed.
    assert_eq!(
        shadow_highlight(&px(false, 0x1E, false, 0, false, 0)),
        (0x1E, 1)
    );
    // Palette 3 colour 14 is an operator, not a colour.
    assert_eq!(
        shadow_highlight(&px(false, 0x3E, false, 0x03, false, 0)),
        (0x03, 1)
    );
    // A low-priority operator under a high-priority opaque plane pixel has
    // no effect.
    assert_eq!(
        shadow_highlight(&px(false, 0x3F, false, 0, true, 0x22)),
        (0x22, 1)
    );
    // A low-priority sprite over shadowed planes is shadowed.
    assert_eq!(
        shadow_highlight(&px(false, 0x13, false, 0, false, 0)),
        (0x13, 0)
    );
}

mod robustness {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(48))]
        /// Arbitrary (ROM-controlled) VRAM, VSRAM, CRAM and register contents
        /// never make the renderer or the timeline index out of bounds.
        #[test]
        fn prop_render_never_panics_on_arbitrary_state(
            regs in proptest::collection::vec(any::<u8>(), NUM_REGISTERS),
            vram_seed: u64,
            vsram in proptest::collection::vec(any::<u8>(), 80),
            lines in proptest::collection::vec(0u16..240, 1..8),
            tick in 0u32..20_000,
        ) {
            let mut vdp = Vdp::new();
            vdp.registers.copy_from_slice(&regs);
            let mut x = vram_seed | 1;
            for b in vdp.vram.iter_mut() {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                *b = x as u8;
            }
            vdp.vsram.copy_from_slice(&vsram);
            for line in lines {
                vdp.render_line(line);
                vdp.recomposite_line_from(line, line % 320);
            }
            vdp.tick(tick, |_| 0xFFFF);
            let _ = vdp.read_hv_counter();
        }
    }
}
