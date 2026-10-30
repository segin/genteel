use super::*;

#[test]
fn test_dma_fill_vram() {
    let mut vdp = Vdp::new();
    vdp.bypass_fifo = true;

    // 1. Enable DMA, display off (register 1 = 0x14).
    vdp.write_control(0x8114);

    /* 2. DMA length 0x10: a VRAM fill writes one byte per length unit,
    stepping the address by the auto-increment. */
    vdp.write_control(0x9310); // Reg 19 = 0x10
    vdp.write_control(0x9400); // Reg 20 = 0x00

    // 3. Set DMA Mode to Fill (Reg 23 bits 7,6 = 1,0) -> 0x80
    // And destination is determined by Control Code? Yes.
    // Reg 23 also holds high bits of source, but for Fill it's ignored or used as mode.
    vdp.write_control(0x9780); // Reg 23 = 0x80

    // 4. Set Auto-increment to 1
    vdp.write_control(0x8F01);

    // 5. Setup DMA Fill destination (VRAM 0x0000)
    // Command: VRAM Write (0x1).
    // Addr 0x0000.
    // Word 1: 0x4000.
    // Word 2: DMA bit (CD5) IS set for Fill. (0x0080).

    vdp.write_control(0x4000);
    vdp.write_control(0x0080);

    // Check if dma_pending is set
    // It should be set because CD5 is 1
    assert!(
        vdp.command.dma_pending,
        "DMA pending should be true for Fill setup"
    );

    /* 6. Write the fill data. The fill then runs one byte per access slot
    (display off: every non-refresh slot), so one line completes it. */
    vdp.write_data(0xAA00);
    assert!(
        vdp.command.dma_pending,
        "the fill runs after the data write"
    );
    vdp.tick(3420, |_| 0);

    assert!(
        !vdp.command.dma_pending,
        "DMA pending should be false once the fill is done"
    );

    // 7. Verify VRAM (GPGX `vdp_dma_fill`): the triggering word 0xAA00 is
    // written normally at 0x0000 (MSB at 0, LSB at 1), then 16 fill steps
    // write the MSB to (address ^ 1) for addresses 0x0001..=0x0010.
    assert_eq!(vdp.vram[0], 0xAA, "fill step at 0x0001 lands on 0x0000");
    assert_eq!(vdp.vram[1], 0x00, "LSB of the triggering word survives");
    for i in 2..0x10 {
        assert_eq!(vdp.vram[i], 0xAA, "Mismatch at index 0x{:04X}", i);
    }
    // The last step (address 0x0010) lands on 0x0011, leaving 0x0010.
    assert_eq!(vdp.vram[0x10], 0x00);
    assert_eq!(vdp.vram[0x11], 0xAA);
    assert_eq!(vdp.vram[0x12], 0x00, "Should stop after 16 fill steps");
    assert_eq!(vdp.registers[REG_DMA_LEN_LO], 0, "length counts down to 0");
    assert_eq!(vdp.registers[REG_DMA_LEN_HI], 0);
}

#[test]
fn test_dma_copy_vram() {
    let mut vdp = Vdp::new();
    vdp.bypass_fifo = true;

    // 1. Enable DMA
    vdp.write_control(0x8114);

    // 2. Setup Source Data at 0x1000
    // We can write directly to vram for setup to be easier
    for i in 0..0x10 {
        vdp.vram[0x1000 + i] = (i as u8) + 1; // 1, 2, 3...
    }

    // 3. Set DMA Length to 0x10
    vdp.write_control(0x9310);
    vdp.write_control(0x9400);

    // 4. Set DMA Mode to Copy (Mode 3) and Source Address
    // Mode 3 is bits 7,6 = 11. Reg 23 = 0xC0.
    // Copy source is a plain byte VRAM address (regs 21/22, no <<1), so
    // source 0x1000 = reg21 (low) 0x00, reg22 (high) 0x10.
    vdp.write_control(0x9500); // Reg 21 = 0x00
    vdp.write_control(0x9610); // Reg 22 = 0x10 -> byte source 0x1000
    vdp.write_control(0x97C0); // Reg 23 = 0xC0

    // 5. Set Auto-increment to 1
    vdp.write_control(0x8F01);

    // 6. Setup Destination (0x0000)
    // Command 0x30 (VRAM copy: CD5|CD4) at 0x0000.
    vdp.write_control(0x0000);
    vdp.write_control(0x00C0);

    assert!(vdp.command.dma_pending);

    // 7. Execute DMA
    let cycles = vdp.execute_dma().unwrap();

    assert!(!vdp.command.dma_pending);
    assert_eq!(cycles, 0x10);
    assert_eq!(vdp.registers[REG_DMA_LEN_LO], 0);
    assert_eq!(vdp.dma_copy_source(), 0x1010, "source advances by length");

    // Verify VRAM
    // 0x0000 should contain what was at 0x1000
    for i in 0..0x10 {
        let expected = (i as u8) + 1;
        assert_eq!(vdp.vram[i], expected, "Mismatch at index 0x{:04X}", i);
    }
}

#[test]
fn test_dma_fill_wrap_around() {
    let mut vdp = Vdp::new();
    vdp.bypass_fifo = true;

    // 1. Enable DMA (Reg 1 bit 4)
    vdp.write_control(0x8114);

    // 2. Set DMA Length to 0 (0x0000)
    vdp.write_control(0x9300); // Reg 19 = 0x00
    vdp.write_control(0x9400); // Reg 20 = 0x00

    // 3. Set DMA Mode to Fill (Reg 23 bits 7,6 = 1,0) -> 0x80
    vdp.write_control(0x9780); // Reg 23 = 0x80

    // 4. Set Auto-increment to 1
    vdp.write_control(0x8F01);

    // 5. Setup DMA Fill destination (VRAM 0x0000)
    vdp.write_control(0x4000);
    vdp.write_control(0x0080);

    assert!(
        vdp.command.dma_pending,
        "DMA pending should be true for Fill setup"
    );

    // 6. Write Fill Data: a fill of 64KB (length 0 is 0x10000), run to
    // completion without slot timing.
    vdp.write_data(0xAA00);
    assert_eq!(vdp.execute_dma(), Ok(0x10000));

    assert!(
        !vdp.command.dma_pending,
        "DMA pending should be false after the fill"
    );

    // 7. Verify VRAM
    // 0x10000 fill steps from address 1 cover every (address ^ 1), so the
    // whole of VRAM ends up holding the MSB.
    for i in 0..0x10000 {
        assert_eq!(vdp.vram[i], 0xAA, "Mismatch at index 0x{:04X}", i);
    }
}

#[test]
fn test_dma_copy_wrap_around() {
    let mut vdp = Vdp::new();
    vdp.bypass_fifo = true;

    // 1. Enable DMA
    vdp.write_control(0x8114);

    // 2. Setup initial VRAM state
    // The copy reads (source ^ 1), so the first byte read is vram[1].
    vdp.vram[1] = 0xFF;

    // 3. Set DMA Length to 0 (0x0000) -> 64KB
    vdp.write_control(0x9300);
    vdp.write_control(0x9400);

    // 4. Set DMA Mode to Copy (Reg 23 = 0xC0) and Source Address 0x0000
    // Source: 0x000000. Reg 21=0, Reg 22=0, Reg 23=0xC0
    vdp.write_control(0x9500); // Reg 21 = 0x00
    vdp.write_control(0x9600); // Reg 22 = 0x00
    vdp.write_control(0x97C0); // Reg 23 = 0xC0

    // 5. Set Auto-increment to 1
    vdp.write_control(0x8F01);

    // 6. Setup Destination (0x0001) - overlapping copy
    // Command 0x4000 (VRAM Write) + addr 0x0001
    // Word 1: 0x0001
    // Word 2: 0x00C0 (CD5|CD4: VRAM copy).
    vdp.write_control(0x0001);
    vdp.write_control(0x00C0);

    assert!(vdp.command.dma_pending);

    // 7. Execute DMA
    // Length 0 -> 0x10000 bytes.
    // Copy from 0 to 1.
    // This overlapping copy should propagate 0xFF through the entire VRAM.
    let count = vdp.execute_dma().unwrap();

    assert!(!vdp.command.dma_pending);
    assert_eq!(count, 0x10000);

    // Verify VRAM
    for i in 0..0x10000 {
        assert_eq!(vdp.vram[i], 0xFF, "Mismatch at index 0x{:04X}", i);
    }
}

/// A VDP in active display (line 10, MCLK 0) with DMA enabled.
fn active_vdp(h40: bool) -> Vdp {
    let mut vdp = Vdp::new();
    vdp.registers[REG_MODE2] = MODE2_DISPLAY_ENABLE | MODE2_DMA_ENABLE;
    vdp.registers[REG_MODE4] = if h40 { MODE4_RS1 } else { 0 };
    vdp.registers[REG_AUTO_INC] = 1;
    vdp.status &= !STATUS_VBLANK;
    vdp.v_counter = 10;
    vdp.mclk_line_clocks = 0;
    vdp
}

fn set_length(vdp: &mut Vdp, len: u32) {
    vdp.registers[REG_DMA_LEN_LO] = (len & 0xFF) as u8;
    vdp.registers[REG_DMA_LEN_HI] = ((len >> 8) & 0xFF) as u8;
}

/// Start a fill of `len` bytes at VRAM 0 through the ports.
fn start_fill(vdp: &mut Vdp, len: u32) {
    set_length(vdp, len);
    vdp.registers[REG_DMA_SRC_HI] = DMA_MODE_FILL;
    vdp.write_control(0x4000);
    vdp.write_control(0x0080);
    vdp.write_data(0x5500);
}

/// Units the running DMA has processed since it started with `len`.
fn units_done(vdp: &Vdp, len: u32) -> u32 {
    if !vdp.command.dma_pending {
        return len;
    }
    let left = match vdp.dma_length() {
        0 => 0x10000,
        n => n,
    };
    len - left
}

/// A fill or copy leaves the 68k running: nothing is charged to it, it is
/// not frozen, and the DMA busy flag reads set until the operation ends.
#[test]
fn fill_and_copy_set_busy_flag_without_freezing_the_68k() {
    let mut vdp = active_vdp(true);
    start_fill(&mut vdp, 0); // 64 KiB
    assert!(!vdp.command.bus_dma_active(), "the 68k is not frozen");
    assert_ne!(vdp.read_status() & STATUS_DMA, 0, "busy while filling");
    vdp.tick(3420, |_| 0);
    assert_ne!(
        vdp.read_status() & STATUS_DMA,
        0,
        "an active line is not enough"
    );
    // Display off: ~204 bytes per line, so 64 KiB takes ~322 lines.
    vdp.registers[REG_MODE2] &= !MODE2_DISPLAY_ENABLE;
    vdp.tick(3420 * 330, |_| 0);
    assert_eq!(vdp.read_status() & STATUS_DMA, 0, "busy clears at the end");
    assert!(vdp.vram.iter().skip(2).all(|&b| b == 0x55));

    let mut vdp = active_vdp(true);
    set_length(&mut vdp, 0x100);
    vdp.registers[REG_DMA_SRC_HI] = DMA_MODE_COPY;
    vdp.write_control(0x0000);
    vdp.write_control(0x00C0);
    assert!(!vdp.command.bus_dma_active());
    assert_ne!(vdp.peek_status() & STATUS_DMA, 0, "busy while copying");
}

/// The busy flag of an armed fill is set from the CD5 control write (GPGX
/// `vdp_68k_ctrl_w`), before its data-port write.
#[test]
fn armed_fill_reads_busy() {
    let mut vdp = active_vdp(false);
    set_length(&mut vdp, 4);
    vdp.registers[REG_DMA_SRC_HI] = DMA_MODE_FILL;
    vdp.write_control(0x4000);
    vdp.write_control(0x0080);
    assert_ne!(vdp.peek_status() & STATUS_DMA, 0);
    vdp.tick(3420 * 4, |_| 0);
    assert!(vdp.command.dma_pending, "waits for the data write");
    assert_eq!(vdp.dma_length(), 4);
}

/// In active display a fill writes one byte per external slot: 18 per H40
/// line, 16 per H32 line, less the two slots of the triggering write on the
/// first line (GPGX `dma_timing`, `vdp_dma_update`). It spans lines.
#[test]
fn active_display_fill_runs_at_slot_rate_across_lines() {
    for (h40, per_line) in [(true, 18), (false, 16)] {
        let mut vdp = active_vdp(h40);
        start_fill(&mut vdp, 100);
        vdp.tick(3420, |_| 0);
        assert_eq!(units_done(&vdp, 100), per_line - 2, "first line, h40={h40}");
        vdp.tick(3420, |_| 0);
        assert_eq!(units_done(&vdp, 100), 2 * per_line - 2, "h40={h40}");
        assert!(vdp.command.dma_pending);
    }
}

#[derive(Clone, Copy, Debug)]
enum Op {
    BusVram,
    BusCram,
    Copy,
    Fill,
}

/// Start `op` of `len` units on `vdp`.
fn start_op(vdp: &mut Vdp, op: Op, len: u32) {
    set_length(vdp, len);
    match op {
        Op::BusVram | Op::BusCram => {
            vdp.registers[REG_DMA_SRC_HI] = 0x7F; // work RAM
            vdp.registers[REG_AUTO_INC] = 2;
            let first = if matches!(op, Op::BusVram) {
                0x4000
            } else {
                0xC000
            };
            vdp.write_control(first);
            vdp.write_control(0x0080);
        }
        Op::Copy => {
            vdp.registers[REG_DMA_SRC_HI] = DMA_MODE_COPY;
            vdp.write_control(0x0000);
            vdp.write_control(0x00C0);
        }
        Op::Fill => {
            vdp.registers[REG_DMA_SRC_HI] = DMA_MODE_FILL;
            vdp.write_control(0x4000);
            vdp.write_control(0x0080);
            vdp.write_data(0x1100);
            // Skip the first line, which loses the two start slots.
            vdp.tick(3420, |_| 0);
        }
    }
}

/// GPGX `vdp_dma_update` units per line: the `dma_timing` slot counts
/// (16/18 active, 166/204 blank), halved for a 68k DMA to VRAM and for a
/// copy, and 161/198 for a 68k DMA to CRAM/VSRAM in blanking.
fn gpgx_rate(op: Op, h40: bool, blank: bool) -> u32 {
    let slots = match (blank, h40) {
        (false, false) => 16,
        (false, true) => 18,
        (true, false) => 166,
        (true, true) => 204,
    };
    match op {
        Op::BusVram | Op::Copy => slots / 2,
        Op::BusCram if blank => slots - if h40 { 6 } else { 5 },
        Op::BusCram | Op::Fill => slots,
    }
}

/// Every DMA kind moves the GPGX number of units per line, in active
/// display and in blanking (display off or vertical blanking), H32 and H40.
#[test]
fn dma_units_per_line_match_gpgx_table() {
    for op in [Op::BusVram, Op::BusCram, Op::Copy, Op::Fill] {
        for h40 in [false, true] {
            for blank in ["active", "display off", "vblank"] {
                let mut vdp = active_vdp(h40);
                match blank {
                    "display off" => vdp.registers[REG_MODE2] &= !MODE2_DISPLAY_ENABLE,
                    "vblank" => {
                        vdp.v_counter = 230;
                        vdp.status |= STATUS_VBLANK;
                    }
                    _ => {}
                }
                start_op(&mut vdp, op, 1000);
                let before = units_done(&vdp, 1000);
                vdp.tick(3420, |_| 0);
                let per_line = units_done(&vdp, 1000) - before;
                assert_eq!(
                    per_line,
                    gpgx_rate(op, h40, blank != "active"),
                    "{op:?} h40={h40} {blank}"
                );
            }
        }
    }
}

/// A register 23 write after the CD5 command does not change the latched
/// DMA kind: an armed fill stays a fill and does not freeze the 68k.
#[test]
fn register_23_write_does_not_change_an_armed_fill() {
    let mut vdp = active_vdp(true);
    set_length(&mut vdp, 8);
    vdp.registers[REG_DMA_SRC_HI] = DMA_MODE_FILL;
    vdp.write_control(0x4000);
    vdp.write_control(0x0080);
    vdp.write_control(0x977F); // register 23: 68k DMA from work RAM
                               /* A register write also loads the command's low address and code bits
                               (GPGX `vdp_68k_ctrl_w`), so set the VRAM address again with a command
                               without CD5; the fill stays armed. */
    vdp.write_control(0x4000);
    vdp.write_control(0x0000);
    vdp.write_data(0x9900);
    assert_eq!(vdp.command.dma_kind, DmaKind::Fill);
    assert!(!vdp.command.bus_dma_active(), "the 68k is not frozen");
    let mut bus_reads = 0;
    vdp.registers[REG_MODE2] &= !MODE2_DISPLAY_ENABLE;
    vdp.tick(3420, |_| {
        bus_reads += 1;
        0
    });
    assert_eq!(bus_reads, 0, "no 68k bus access");
    assert!(!vdp.command.dma_pending);
    assert_eq!(
        &vdp.vram[0..10],
        &[0x99, 0x00, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x00, 0x99]
    );
}

/// Clearing DMA enable between the fill command and its data write does
/// not cancel the fill: it was armed at the control write.
#[test]
fn fill_armed_before_dma_disable_still_runs() {
    let mut vdp = active_vdp(true);
    set_length(&mut vdp, 4);
    vdp.registers[REG_DMA_SRC_HI] = DMA_MODE_FILL;
    vdp.write_control(0x4000);
    vdp.write_control(0x0080);
    vdp.write_control(0x8100); // register 1: display and DMA off
    vdp.write_control(0x4000);
    vdp.write_control(0x0000);
    vdp.write_data(0x7700);
    vdp.tick(3420, |_| 0);
    assert!(!vdp.command.dma_pending);
    assert_eq!(&vdp.vram[0..6], &[0x77, 0x00, 0x77, 0x77, 0x00, 0x77]);
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(64))]
    /// Any DMA length of any kind completes with exactly `len` units, no
    /// earlier than the GPGX per-line rate allows and within one line of it,
    /// leaving the length registers at zero and the busy flag clear.
    #[test]
    fn prop_dma_completes_at_slot_rate(
        op_idx in 0usize..4,
        h40: bool,
        display_off: bool,
        len in 1u32..3000,
    ) {
        let op = [Op::BusVram, Op::BusCram, Op::Copy, Op::Fill][op_idx];
        let mut vdp = active_vdp(h40);
        if display_off {
            vdp.registers[REG_MODE2] &= !MODE2_DISPLAY_ENABLE;
        }
        start_op(&mut vdp, op, len);
        let mut done = units_done(&vdp, len);
        let rate = gpgx_rate(op, h40, display_off);
        let mut lines = 0u32;
        while vdp.command.dma_pending {
            vdp.v_counter = 10; // stay on an active line
            vdp.tick(3420, |_| 0);
            let now = units_done(&vdp, len);
            proptest::prop_assert!(now - done <= rate);
            done = now;
            lines += 1;
            proptest::prop_assert!(lines <= len / rate + 2);
        }
        proptest::prop_assert_eq!(done, len);
        proptest::prop_assert_eq!(vdp.dma_length(), 0);
        proptest::prop_assert_eq!(vdp.peek_status() & STATUS_DMA, 0);
    }
}
