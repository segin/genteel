//! Regression tests locking in the VDP audit fixes.

use super::*;

/// Build `n` 1x1 sprites all on screen line 10, linked 0->1->..->(n-1)->0, each
/// with a distinct non-zero X (so none are X=0 mask sprites). SAT at 0xD400.
fn sprites_on_line_10(n: usize) -> Vdp {
    let mut vdp = Vdp::new();
    vdp.registers[12] = 0x81; // H40
    vdp.registers[5] = 0x6A; // SAT at 0xD400
    let sat_base = 0xD400;
    for i in 0..n {
        let addr = sat_base + i * 8;
        vdp.vram[addr] = 0x00;
        vdp.vram[addr + 1] = 128 + 10; // Y = 10
        vdp.vram[addr + 2] = 0x00; // 1x1 size
        vdp.vram[addr + 3] = if i + 1 == n { 0 } else { (i + 1) as u8 };
        let raw_x = 128u16 + (i as u16) * 8;
        vdp.vram[addr + 6] = (raw_x >> 8) as u8;
        vdp.vram[addr + 7] = (raw_x & 0xFF) as u8;
    }
    vdp
}

/// H2: sprite overflow (bit 6) and collision (bit 5) are read-and-clear.
#[test]
fn read_status_clears_sprite_overflow_and_collision() {
    let mut vdp = Vdp::new();
    vdp.status |= STATUS_SOVR | STATUS_COLLISION;

    let s = vdp.read_status();
    assert_ne!(s & STATUS_SOVR, 0, "SOVR present in the returned value");
    assert_ne!(
        s & STATUS_COLLISION,
        0,
        "collision present in the returned value"
    );

    let s2 = vdp.read_status();
    assert_eq!(s2 & STATUS_SOVR, 0, "SOVR must clear on status read");
    assert_eq!(
        s2 & STATUS_COLLISION,
        0,
        "collision must clear on status read"
    );
}

/// M4: the status region bit (bit 0) reflects PAL/NTSC.
#[test]
fn status_reports_pal_region_bit() {
    let mut pal = Vdp::new();
    pal.set_pal(true);
    assert_ne!(pal.read_status() & STATUS_PAL, 0, "PAL bit set on PAL");

    let mut ntsc = Vdp::new();
    ntsc.set_pal(false);
    assert_eq!(ntsc.read_status() & STATUS_PAL, 0, "PAL bit clear on NTSC");
}

/// M8: VBLANK (bit 3) also reads set when the display is force-blanked.
#[test]
fn status_vblank_set_when_display_disabled() {
    let mut vdp = Vdp::new();
    // Display disabled at reset -> VBLANK reads set.
    assert_ne!(
        vdp.read_status() & STATUS_VBLANK,
        0,
        "VBLANK set when display off"
    );

    // Enable display and clear the vblank-region flag (simulate active display).
    vdp.registers[REG_MODE2] = MODE2_DISPLAY_ENABLE;
    vdp.status &= !STATUS_VBLANK;
    assert_eq!(
        vdp.read_status() & STATUS_VBLANK,
        0,
        "VBLANK clear when display on and in active display"
    );
}

/// H3: per-cell H-scroll (mode %10) uses a 32-byte-per-cell-row stride.
#[test]
fn hscroll_per_cell_uses_cell_stride() {
    let mut vdp = Vdp::new();
    vdp.registers[REG_HSCROLL] = 0x00; // H-scroll table at VRAM 0
                                       // Plane-A H-scroll words for cell rows 0/1/2 live at byte offsets 0/32/64.
    vdp.vram[1] = 0x11; // cell row 0 -> 0x0011
    vdp.vram[33] = 0x22; // cell row 1 -> 0x0022
    vdp.vram[65] = 0x33; // cell row 2 -> 0x0033

    let mode3 = 0x02; // per-cell H-scroll
    assert_eq!(
        vdp.compute_hscroll_words(0, mode3).0,
        0x0011,
        "line 0 -> cell row 0"
    );
    assert_eq!(
        vdp.compute_hscroll_words(8, mode3).0,
        0x0022,
        "line 8 -> cell row 1 (offset 32)"
    );
    assert_eq!(
        vdp.compute_hscroll_words(16, mode3).0,
        0x0033,
        "line 16 -> cell row 2 (offset 64)"
    );
}

/// M1: the HINT counter also fires on the first blanking line.
#[test]
fn hint_fires_on_first_blanking_line() {
    let mut vdp = Vdp::new();
    vdp.registers[0] = MODE1_HINT_ENABLE;
    vdp.registers[REG_H_INT_COUNTER] = 0; // HINT every line
                                          // NTSC V28 -> active_lines = 224. Sit on the last active line.
    vdp.v_counter = 223;
    vdp.line_counter = 0;
    vdp.mclk_line_clocks = 3419;

    // Cross into line 224 (first blanking line) and past the HINT MCLK offset.
    vdp.tick(1 + 200, |_| 0);

    assert_eq!(vdp.v_counter, 224);
    assert!(
        vdp.hint_pending(),
        "HINT must fire on the first blanking line"
    );
}

/// M2: the sprite-overflow flag fires only when the line exceeds the limit.
#[test]
fn sprite_overflow_only_when_exceeding_limit() {
    let mut buf = [SpriteAttributes::default(); 80];

    // Exactly 20 sprites (H40 limit) -> all render, NO overflow.
    let mut vdp = sprites_on_line_10(20);
    let count = vdp.get_active_sprites(10, &mut buf);
    assert_eq!(count, 20);
    assert_eq!(
        vdp.status & STATUS_SOVR,
        0,
        "exactly 20 sprites must not set overflow"
    );

    // 21 sprites -> 20 render, overflow set.
    let mut vdp = sprites_on_line_10(21);
    let count = vdp.get_active_sprites(10, &mut buf);
    assert_eq!(count, 20);
    assert_ne!(vdp.status & STATUS_SOVR, 0, "21 sprites must set overflow");
}

/// M5: a write past a full FIFO drains the oldest entry first (preserves order).
#[test]
fn fifo_overflow_preserves_write_order() {
    let mut vdp = Vdp::new();
    vdp.bypass_fifo = false;
    vdp.registers[REG_AUTO_INC] = 2;
    vdp.command.code = VRAM_WRITE;
    vdp.command.address = 0x000;

    // 5 writes: fill the 4-deep FIFO, then one overflow. The overflow must
    // commit the OLDEST queued word (in order), not itself out of order.
    for i in 0..5u16 {
        vdp.write_data(0x1000 + i);
    }

    assert_eq!(vdp.vram[0], 0x10, "oldest word committed first (high byte)");
    assert_eq!(vdp.vram[1], 0x00, "oldest word committed first (low byte)");
    assert_eq!(
        vdp.vram[8], 0x00,
        "5th (newest) write must not commit out of order"
    );
    assert_eq!(vdp.vram[9], 0x00);
}

/// H1: VRAM copy uses a plain byte source (no <<1 doubling / compounding).
#[test]
fn step_dma_copy_uses_byte_source() {
    let mut vdp = Vdp::new();
    vdp.vram[0x20] = 0xAB;
    vdp.registers[REG_DMA_SRC_HI] = DMA_MODE_COPY;
    vdp.registers[REG_DMA_SRC_MID] = 0x00;
    vdp.registers[REG_DMA_SRC_LO] = 0x20; // byte source 0x0020
    vdp.registers[REG_DMA_LEN_LO] = 0x02;
    vdp.registers[REG_AUTO_INC] = 1;
    vdp.command.dma_pending = true;
    vdp.command.address = 0x100;

    let mut read = |_: u32| 0u16;
    vdp.step_dma(&mut read);

    assert_eq!(
        vdp.vram[0x100], 0xAB,
        "copied from byte source 0x20 (not doubled)"
    );
    let src = ((vdp.registers[REG_DMA_SRC_MID] as u16) << 8) | vdp.registers[REG_DMA_SRC_LO] as u16;
    assert_eq!(src, 0x21, "copy source advances by exactly 1 byte");
}

/// M3: a VRAM fill does nothing (and consumes no length) before the data write.
#[test]
fn step_dma_fill_waits_for_data_write() {
    let mut vdp = Vdp::new();
    vdp.registers[REG_DMA_SRC_HI] = DMA_MODE_FILL;
    vdp.registers[REG_DMA_LEN_LO] = 0x04;
    vdp.command.dma_pending = true;
    vdp.command.address = 0x50;
    vdp.command.code = VRAM_WRITE;
    vdp.last_data_write = 0x0000;

    let mut read = |_: u32| 0u16;
    vdp.step_dma(&mut read);

    assert_eq!(
        vdp.vram[0x50], 0x00,
        "fill must not run before the data-port write"
    );
    assert_eq!(
        vdp.registers[REG_DMA_LEN_LO], 0x04,
        "fill must not consume length early"
    );
    assert!(
        vdp.command.dma_pending,
        "fill stays pending until the data write"
    );
}

/// Deferred/F1 (intentionally NOT changed): `acknowledge_vint` clears the F flag
/// on the 68k interrupt-acknowledge. The hardware-accurate behaviour (clear only
/// on a status read) was tried but hangs games whose VINT handler relies on the
/// ack auto-clear (Sonic loops forever in VINT), so the pragmatic auto-clear is
/// retained.
#[test]
fn acknowledge_vint_clears_f_flag_for_compat() {
    let mut vdp = Vdp::new();
    vdp.status |= STATUS_VINT_PENDING;
    vdp.acknowledge_vint();
    assert_eq!(
        vdp.status & STATUS_VINT_PENDING,
        0,
        "interrupt ack clears F (retained for game compatibility)"
    );
}

/// Deferred/HV-latch: with reg 0 bit 1 set, a TH transition freezes the HV
/// counter; reads return the frozen value until latching is disabled.
#[test]
fn hv_counter_latch_freezes_when_enabled() {
    let mut vdp = Vdp::new();
    vdp.v_counter = 100;
    vdp.mclk_line_clocks = 1000;
    let value_at_latch = vdp.read_hv_counter();

    // Enable the latch (reg 0 bit 1) and capture on a TH transition.
    vdp.registers[0] = 0x02;
    vdp.latch_hv_counter();

    // Advance the beam; the latched read must not move.
    vdp.v_counter = 150;
    vdp.mclk_line_clocks = 2000;
    assert_eq!(
        vdp.read_hv_counter(),
        value_at_latch,
        "latched HV counter stays frozen while enabled"
    );

    // Disabling the latch returns the live counter.
    vdp.registers[0] = 0x00;
    assert_ne!(
        vdp.read_hv_counter(),
        value_at_latch,
        "HV counter is live again once latching is disabled"
    );
}
