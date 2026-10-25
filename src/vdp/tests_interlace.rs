//! Interlace modes: field flag, V counter and double-resolution rendering.

use super::*;

const RED: u16 = 0xF800;
const GREEN: u16 = 0x07E0;

fn run_to_vblank_start(vdp: &mut Vdp) {
    // Advance to the line change into the first blanking line.
    let lines = vdp.frame_lines() as u32;
    let active = vdp.screen_height() as u32;
    let current = vdp.v_counter as u32;
    let to_go = (active + lines - current - 1) % lines;
    vdp.tick(
        to_go * MCLK_PER_LINE + (MCLK_PER_LINE - vdp.mclk_line_clocks),
        |_| 0,
    );
    assert_eq!(vdp.v_counter as u32, active);
}

#[test]
fn odd_field_flag_toggles_each_frame_while_interlaced() {
    let mut vdp = Vdp::new();
    vdp.registers[REG_MODE4] = MODE4_RS1 | MODE4_LSM0;
    let mut flags = Vec::new();
    for _ in 0..4 {
        run_to_vblank_start(&mut vdp);
        flags.push(vdp.peek_status() & STATUS_ODD_FRAME != 0);
        vdp.tick(MCLK_PER_LINE, |_| 0);
    }
    assert_eq!(flags, vec![true, false, true, false]);

    vdp.registers[REG_MODE4] = MODE4_RS1;
    run_to_vblank_start(&mut vdp);
    assert_eq!(
        vdp.peek_status() & STATUS_ODD_FRAME,
        0,
        "clear when progressive"
    );
}

#[test]
fn interlace_mode_selection() {
    let mut vdp = Vdp::new();
    vdp.registers[REG_MODE4] = MODE4_LSM0;
    assert!(vdp.interlaced() && !vdp.interlace_mode_2());
    vdp.registers[REG_MODE4] = MODE4_LSM0 | MODE4_LSM1;
    assert!(vdp.interlaced() && vdp.interlace_mode_2());
    vdp.registers[REG_MODE4] = MODE4_LSM1;
    assert!(!vdp.interlaced(), "LSM1 alone is not an interlace mode");
}

#[test]
fn v_counter_in_interlace_modes() {
    let mut vdp = Vdp::new();
    vdp.v_counter = 0x90;
    vdp.registers[REG_MODE4] = MODE4_LSM0 | MODE4_LSM1;
    // Doubled to 0x120, bit 0 replaced by bit 8.
    assert_eq!(vdp.read_hv_counter() >> 8, 0x21);
    vdp.v_counter = 0x10;
    assert_eq!(vdp.read_hv_counter() >> 8, 0x20);

    vdp.registers[REG_MODE4] = MODE4_LSM0;
    vdp.v_counter = 0x11;
    // Bit 0 replaced by bit 8 (0) in interlace mode 1.
    assert_eq!(vdp.read_hv_counter() >> 8, 0x10);
    vdp.v_counter = 0xEB; // raw 0x1E5 after the NTSC jump
    assert_eq!(vdp.read_hv_counter() >> 8, 0xE5);
}

fn im2_vdp(odd: bool) -> Vdp {
    let mut vdp = Vdp::new();
    vdp.registers[REG_MODE2] = MODE2_DISPLAY_ENABLE;
    vdp.registers[REG_MODE4] = MODE4_RS1 | MODE4_LSM0 | MODE4_LSM1;
    vdp.registers[REG_PLANE_A] = 0x30;
    vdp.registers[REG_PLANE_B] = 0x05;
    vdp.registers[REG_SPRITE_TABLE] = 0x6A;
    vdp.registers[REG_HSCROLL] = 0x3C;
    if odd {
        vdp.status |= STATUS_ODD_FRAME;
    }
    vdp.cram_cache[1] = RED;
    vdp.cram_cache[2] = GREEN;
    // 8x16 tile 1 occupies bytes 64..128: rows 0-7 colour 1, rows 8-15
    // colour 2.
    for row in 0..16 {
        let v = if row < 8 { 0x11 } else { 0x22 };
        for b in 0..4 {
            vdp.vram[64 + row * 4 + b] = v;
        }
    }
    vdp
}

#[test]
fn im2_planes_use_8x16_tiles_and_field_parity() {
    for (odd, line, expected) in [
        (false, 3, RED),
        (true, 3, RED),
        (false, 4, GREEN),
        (true, 4, GREEN),
    ] {
        let mut vdp = im2_vdp(odd);
        vdp.vram[0xC000] = 0x00;
        vdp.vram[0xC001] = 0x01;
        vdp.render_line(line);
        assert_eq!(
            vdp.framebuffer[line as usize * 320],
            expected,
            "field line {} (odd={}) -> half-line {}",
            line,
            odd,
            line * 2 + odd as u16
        );
    }

    // Row 7 vs row 8 is decided by the field: half-line 7 is odd field line 3.
    let mut vdp = im2_vdp(true);
    for b in 0..4 {
        vdp.vram[64 + 7 * 4 + b] = 0x22;
    }
    vdp.vram[0xC001] = 0x01;
    vdp.render_line(3);
    assert_eq!(vdp.framebuffer[3 * 320], GREEN);
    let mut vdp = im2_vdp(false);
    for b in 0..4 {
        vdp.vram[64 + 7 * 4 + b] = 0x22;
    }
    vdp.vram[0xC001] = 0x01;
    vdp.render_line(3);
    assert_eq!(
        vdp.framebuffer[3 * 320],
        RED,
        "even field line 3 is half-line 6"
    );
}

#[test]
fn im2_vflip_flips_all_16_rows() {
    let mut vdp = im2_vdp(false);
    vdp.vram[0xC000] = 0x10; // v-flip
    vdp.vram[0xC001] = 0x01;
    vdp.render_line(0); // half-line 0 -> flipped row 15 (colour 2)
    assert_eq!(vdp.framebuffer[0], GREEN);
}

#[test]
fn im2_sprites_use_ten_bit_y_and_16_line_tiles() {
    let mut vdp = im2_vdp(false);
    // Sprite 0: raw Y = 256 + 10 (half-line 10), 1x1 (8x16), tile 1, X = 40.
    let sat = 0xD400;
    vdp.vram[sat..sat + 8].copy_from_slice(&[0x01, 0x0A, 0x00, 0x00, 0x00, 0x01, 0x00, 128 + 40]);
    vdp.render_line(4); // half-line 8: above the sprite
    assert_ne!(vdp.framebuffer[4 * 320 + 40], RED);
    vdp.render_line(5); // half-line 10: sprite row 0
    assert_eq!(vdp.framebuffer[5 * 320 + 40], RED);
    vdp.render_line(9); // half-line 18: sprite row 8
    assert_eq!(vdp.framebuffer[9 * 320 + 40], GREEN);
    vdp.render_line(13); // half-line 26: past the 16-line sprite
    assert_ne!(vdp.framebuffer[13 * 320 + 40], GREEN);
}
