//! Consistency of the VDP line timeline: HV counter, HBLANK/VBLANK flags,
//! interrupts and the line the renderer is drawing must agree.

use super::*;
use crate::debugger::Debuggable;

const RED: u16 = 0xF800;

/// Display on, backdrop = CRAM entry 1, everything else transparent, so every
/// visible pixel shows CRAM entry 1.
fn backdrop_vdp(h40: bool) -> Vdp {
    let mut vdp = Vdp::new();
    vdp.registers[REG_MODE2] = MODE2_DISPLAY_ENABLE;
    vdp.registers[REG_MODE4] = if h40 { MODE4_RS1 } else { 0 };
    vdp.registers[REG_BG_COLOR] = 0x01;
    vdp.registers[REG_AUTO_INC] = 2;
    vdp.cram_cache[1] = RED;
    vdp
}

fn line_pixels(vdp: &Vdp, line: usize) -> &[u16] {
    let w = vdp.screen_width() as usize;
    &vdp.framebuffer[line * 320..line * 320 + w]
}

/// Poll the HV counter the way a raster routine does (in 8-cycle 68k steps)
/// until the V counter reads `v`.
fn poll_until_v(vdp: &mut Vdp, v: u8) {
    for _ in 0..100_000 {
        if (vdp.read_hv_counter() >> 8) as u8 == v {
            return;
        }
        vdp.tick(56, |_| 0);
    }
    panic!("V counter never reached 0x{:02X}", v);
}

fn poll_then_cram_write_lands_on_polled_line(h40: bool) {
    let mut vdp = backdrop_vdp(h40);
    let target = 100u8;
    poll_until_v(&mut vdp, target);

    // Write CRAM entry 1 = green through the FIFO.
    vdp.write_control(0xC002);
    vdp.write_control(0x0000);
    vdp.write_data(0x00E0);
    // Run until the polled line and the next one have been fully emitted.
    poll_until_v(&mut vdp, target + 2);
    let green = vdp.cram_cache[1];
    assert_ne!(green, RED);

    let n = target as usize;
    assert!(
        line_pixels(&vdp, n).iter().all(|&p| p == green),
        "line N (the polled line) must use the new colour everywhere"
    );
    assert!(
        line_pixels(&vdp, n - 1).iter().all(|&p| p == RED),
        "line N-1 was already emitted and must keep the old colour"
    );
    assert!(line_pixels(&vdp, n + 1).iter().all(|&p| p == green));
}

#[test]
fn poll_v_then_cram_write_lands_on_polled_line_h40() {
    poll_then_cram_write_lands_on_polled_line(true);
}

#[test]
fn poll_v_then_cram_write_lands_on_polled_line_h32() {
    poll_then_cram_write_lands_on_polled_line(false);
}

/// A write made immediately at the V-counter change still catches the last
/// pixels of the previous line, which the beam is emitting at that moment.
#[test]
fn write_at_line_change_affects_only_previous_line_tail() {
    let mut vdp = backdrop_vdp(true);
    vdp.bypass_fifo = true;
    poll_until_v(&mut vdp, 50);
    let x0 = vdp.beam_target();
    vdp.write_control(0xC002);
    vdp.write_control(0x0000);
    vdp.write_data(0x00E0);
    let green = vdp.cram_cache[1];

    let (line, x) = x0.expect("beam is in the tail of line 49");
    assert_eq!(line, 49);
    assert!(x >= 304);
    let prev = line_pixels(&vdp, 49);
    assert!(prev[..x as usize].iter().all(|&p| p == RED));
    assert!(prev[x as usize..].iter().all(|&p| p == green));
}

#[test]
fn h_counter_at_first_active_pixel() {
    let mut vdp = backdrop_vdp(true);
    vdp.mclk_line_clocks = vdp.line_timing().active_start;
    assert_eq!(vdp.read_hv_counter() & 0xFF, 0x0D, "H40 pixel 0 at H 0x0D");
    assert_eq!(vdp.mid_line_pixel_x(), 0);

    let mut vdp = backdrop_vdp(false);
    vdp.mclk_line_clocks = vdp.line_timing().active_start;
    assert_eq!(vdp.read_hv_counter() & 0xFF, 0x0C, "H32 pixel 0 at H 0x0C");
}

fn hblank_flag_matches_h_counter(h40: bool) {
    let mut vdp = backdrop_vdp(h40);
    // HBLANK is set from H 0xB3 (H40) / 0x93 (H32) until H 0x05.
    let (set_from, clear_from) = if h40 { (0xB3u8, 0x05u8) } else { (0x93, 0x05) };
    let mut seen_set = false;
    for m in 0..MCLK_PER_LINE {
        vdp.mclk_line_clocks = m;
        let h = vdp.read_hv_counter() as u8;
        let hblank = vdp.peek_status() & STATUS_HBLANK != 0;
        seen_set |= hblank;
        let in_active_h = h >= clear_from && h < set_from;
        let strictly_blank = h > set_from || h < clear_from;
        if hblank {
            assert!(
                !in_active_h || h == set_from || h == clear_from,
                "HBLANK set at H 0x{:02X} (MCLK {})",
                h,
                m
            );
        } else {
            assert!(
                !strictly_blank,
                "HBLANK clear at H 0x{:02X} (MCLK {})",
                h, m
            );
        }
    }
    assert!(seen_set);
}

#[test]
fn hblank_flag_matches_h_counter_h40() {
    hblank_flag_matches_h_counter(true);
}

#[test]
fn hblank_flag_matches_h_counter_h32() {
    hblank_flag_matches_h_counter(false);
}

#[test]
fn vblank_set_when_v_counter_first_reads_e0() {
    let mut vdp = backdrop_vdp(true);
    poll_until_v(&mut vdp, 0xDF);
    assert_eq!(vdp.peek_status() & STATUS_VBLANK, 0);
    for _ in 0..1000 {
        vdp.tick(8, |_| 0);
        if (vdp.read_hv_counter() >> 8) as u8 == 0xE0 {
            assert_ne!(
                vdp.peek_status() & STATUS_VBLANK,
                0,
                "VBLANK must be set as soon as V reads 0xE0"
            );
            return;
        }
        assert_eq!(vdp.peek_status() & STATUS_VBLANK, 0);
    }
    panic!("V counter never reached 0xE0");
}

#[test]
fn vblank_clears_on_last_line_of_frame() {
    let mut vdp = backdrop_vdp(true);
    vdp.v_counter = 260;
    vdp.status |= STATUS_VBLANK;
    vdp.mclk_line_clocks = MCLK_PER_LINE - 1;
    vdp.tick(1, |_| 0);
    assert_eq!(vdp.v_counter, 261);
    assert_eq!((vdp.read_hv_counter() >> 8) as u8, 0xFF);
    assert_eq!(vdp.peek_status() & STATUS_VBLANK, 0);
}

#[test]
fn status_poll_keeps_vint_pending_until_acknowledged() {
    let mut vdp = backdrop_vdp(true);
    vdp.registers[REG_MODE2] |= MODE2_VINT_ENABLE;
    poll_until_v(&mut vdp, 0xE1);
    assert!(vdp.vblank_pending());

    // The 68k polls status with interrupts masked.
    let s1 = vdp.read_status();
    let s2 = vdp.read_status();
    assert_ne!(s1 & STATUS_VINT_PENDING, 0);
    assert_ne!(s2 & STATUS_VINT_PENDING, 0);
    assert!(
        vdp.vblank_pending(),
        "VINT must still be pending when interrupts are unmasked"
    );

    vdp.acknowledge_vint();
    assert!(!vdp.vblank_pending());
    assert_eq!(vdp.peek_status() & STATUS_VINT_PENDING, 0);
}

#[test]
fn peek_status_has_no_side_effects() {
    let mut vdp = backdrop_vdp(true);
    vdp.status |= STATUS_SOVR | STATUS_COLLISION | STATUS_VINT_PENDING;
    vdp.write_control(0x4000); // first half of a command: pending
    let before = vdp.peek_status();
    for _ in 0..10 {
        assert_eq!(vdp.peek_status(), before);
    }
    assert!(vdp.is_control_pending());
    assert_ne!(vdp.status & (STATUS_SOVR | STATUS_COLLISION), 0);
}

#[test]
fn pending_hint_survives_line_boundaries_until_acknowledged() {
    let mut vdp = backdrop_vdp(true);
    vdp.registers[REG_MODE1] = MODE1_HINT_ENABLE;
    vdp.registers[REG_H_INT_COUNTER] = 20;
    vdp.v_counter = 10;
    vdp.line_counter = 0;
    vdp.mclk_line_clocks = MCLK_PER_LINE - 1;
    vdp.tick(1, |_| 0);
    assert!(vdp.hint_pending());

    // The VINT handler runs with HINT masked for several lines.
    vdp.tick(MCLK_PER_LINE * 5, |_| 0);
    assert!(vdp.hint_pending(), "unacknowledged HINT must stay asserted");

    vdp.acknowledge_hint();
    assert!(!vdp.hint_pending());
}

#[test]
fn long_tick_processes_every_line() {
    let mut vdp = backdrop_vdp(true);
    vdp.registers[REG_MODE2] |= MODE2_VINT_ENABLE;
    vdp.registers[REG_MODE1] = MODE1_HINT_ENABLE;
    vdp.registers[REG_H_INT_COUNTER] = 3;
    vdp.tick(MCLK_PER_LINE * 10 + 5, |_| 0);
    assert_eq!(vdp.v_counter, 10);
    assert_eq!(vdp.mclk_line_clocks, 5);
    for line in 0..10 {
        assert!(vdp.rendered_scanlines[line], "line {} not rendered", line);
    }
    // Line counter: fires and reloads on lines 1, 5 and 9, counting down in
    // between, so line 10 leaves it at 2.
    assert_eq!(vdp.line_counter, 2);

    let mut vdp = backdrop_vdp(true);
    vdp.registers[REG_MODE2] |= MODE2_VINT_ENABLE;
    vdp.tick(MCLK_PER_LINE * 230, |_| 0);
    assert!(vdp.vblank_pending(), "VINT inside a long tick must fire");
}

#[test]
fn out_of_range_counters_are_clamped() {
    let mut vdp = backdrop_vdp(true);
    vdp.v_counter = u16::MAX;
    vdp.mclk_line_clocks = u32::MAX;
    let _ = vdp.read_hv_counter();
    let _ = vdp.peek_status();
    vdp.tick(1, |_| 0);
    assert!(vdp.v_counter < 262);
    assert!(vdp.mclk_line_clocks < MCLK_PER_LINE);

    let mut state = vdp.read_state();
    state["v_counter"] = serde_json::json!(65535);
    state["mclk_line_clocks"] = serde_json::json!(4_000_000_000u32);
    state["fifo_history_idx"] = serde_json::json!(999);
    let mut restored = Vdp::new();
    restored.write_state(&state);
    assert!(restored.v_counter < 262);
    assert!(restored.mclk_line_clocks < MCLK_PER_LINE);
    restored.write_data(0x1234);
    let _ = restored.fifo_next_entry();
    restored.tick(MCLK_PER_LINE * 2, |_| 0);
}
