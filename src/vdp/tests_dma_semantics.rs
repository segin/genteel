//! DMA source wrapping, fill targets, FIFO ordering and register
//! bookkeeping.

use super::*;
use proptest::prelude::*;

fn dma_vdp() -> Vdp {
    let mut vdp = Vdp::new();
    vdp.registers[REG_MODE2] = MODE2_DMA_ENABLE;
    vdp.registers[REG_AUTO_INC] = 2;
    vdp
}

#[test]
fn ram_dma_source_wraps_within_128k_block() {
    let mut vdp = dma_vdp();
    vdp.registers[REG_DMA_SRC_HI] = 0x7F;
    vdp.registers[REG_DMA_SRC_MID] = 0xFF;
    vdp.registers[REG_DMA_SRC_LO] = 0xFF;
    vdp.registers[REG_DMA_LEN_LO] = 2;
    vdp.write_control(0x4000);
    vdp.write_control(0x0080);
    assert!(vdp.command.dma_pending);

    let mut reads = Vec::new();
    for _ in 0..2 {
        vdp.step_dma(&mut |a| {
            reads.push(a);
            0
        });
    }
    assert_eq!(reads[0], 0xFFFFFE);
    assert_eq!(reads[1] & 0xFFFF, 0x0000, "wraps to the start of RAM");
    assert_eq!(vdp.registers[REG_DMA_SRC_HI], 0x7F, "register 23 unchanged");
    assert!(!vdp.command.dma_pending);
}

#[test]
fn rom_dma_source_wraps_within_128k_block() {
    let mut vdp = dma_vdp();
    vdp.registers[REG_DMA_SRC_HI] = 0x01;
    vdp.registers[REG_DMA_SRC_MID] = 0xFF;
    vdp.registers[REG_DMA_SRC_LO] = 0xFF;
    vdp.registers[REG_DMA_LEN_LO] = 2;
    vdp.write_control(0x4000);
    vdp.write_control(0x0080);

    let mut reads = Vec::new();
    for _ in 0..2 {
        vdp.step_dma(&mut |a| {
            reads.push(a);
            0
        });
    }
    assert_eq!(reads, vec![0x03FFFE, 0x020000]);
    assert_eq!(vdp.registers[REG_DMA_SRC_HI], 0x01);
}

#[test]
fn execute_dma_rejects_68k_transfer_without_bus_reader() {
    let mut vdp = dma_vdp();
    vdp.registers[REG_DMA_SRC_HI] = 0x00;
    vdp.registers[REG_DMA_LEN_LO] = 4;
    vdp.write_control(0x4000);
    vdp.write_control(0x0080);
    assert_eq!(vdp.execute_dma(), Err(DmaError::NoBusReader));
    assert!(vdp.command.dma_pending, "transfer stays pending for tick");
    assert!(
        vdp.vram.iter().all(|&b| b == 0),
        "no 0xFFFF garbage written"
    );
    assert_eq!(vdp.registers[REG_DMA_LEN_LO], 4);
}

#[test]
fn dma_fill_commits_queued_fifo_writes_first() {
    let mut vdp = dma_vdp();
    vdp.registers[REG_AUTO_INC] = 2;
    // Two ordinary VRAM writes stay queued in the FIFO.
    vdp.write_control(0x4000);
    vdp.write_control(0x0000);
    vdp.write_data(0x1111);
    vdp.write_data(0x2222);
    assert_eq!(vdp.fifo.len(), 2);

    // Fill 0x0000..0x0010 with 0x77.
    vdp.registers[REG_AUTO_INC] = 1;
    vdp.registers[REG_DMA_SRC_HI] = DMA_MODE_FILL;
    vdp.registers[REG_DMA_LEN_LO] = 0x10;
    vdp.write_control(0x4000);
    vdp.write_control(0x0080);
    vdp.write_data(0x7700);
    assert!(vdp.fifo.is_empty());

    // Let any (incorrectly) remaining FIFO entries drain.
    vdp.tick(3420 * 2, |_| 0);
    assert_eq!(
        vdp.vram[0x02], 0x77,
        "queued write must not land after the fill"
    );
    assert_eq!(vdp.vram[0x03], 0x77);
}

#[test]
fn dma_fill_and_copy_leave_length_registers_zero() {
    let mut vdp = dma_vdp();
    vdp.registers[REG_AUTO_INC] = 1;
    vdp.registers[REG_DMA_SRC_HI] = DMA_MODE_FILL;
    vdp.registers[REG_DMA_LEN_LO] = 0x20;
    vdp.registers[REG_DMA_LEN_HI] = 0x01;
    vdp.write_control(0x4000);
    vdp.write_control(0x0080);
    vdp.write_data(0x5500);
    assert_eq!(vdp.dma_length(), 0);
    assert_eq!(
        vdp.dma_copy_source(),
        0x0120,
        "source advanced by the length"
    );

    let mut vdp = dma_vdp();
    vdp.registers[REG_AUTO_INC] = 1;
    vdp.registers[REG_DMA_SRC_HI] = DMA_MODE_COPY;
    vdp.registers[REG_DMA_LEN_LO] = 0x08;
    vdp.write_control(0x0100);
    vdp.write_control(0x00C0);
    vdp.status |= STATUS_VBLANK;
    vdp.tick(3420, |_| 0);
    assert!(!vdp.command.dma_pending);
    assert_eq!(vdp.dma_length(), 0);
}

/// Write `values` through the data port to VRAM 0x8000.. (to fill the FIFO
/// history) and then trigger a fill at `target` with `fill`.
fn fill_after_history(code_first: u16, code_second: u16, values: &[u16], fill: u16) -> Vdp {
    let mut vdp = dma_vdp();
    vdp.bypass_fifo = true;
    vdp.write_control(0x4000);
    vdp.write_control(0x0002);
    for &v in values {
        vdp.write_data(v);
    }
    vdp.registers[REG_AUTO_INC] = 2;
    vdp.registers[REG_DMA_SRC_HI] = DMA_MODE_FILL;
    vdp.registers[REG_DMA_LEN_LO] = 3;
    vdp.write_control(code_first);
    vdp.write_control(code_second);
    vdp.write_data(fill);
    vdp
}

#[test]
fn cram_fill_uses_next_fifo_entry() {
    // CRAM write at 0 with CD5: 0xC000 / 0x0080.
    let vdp = fill_after_history(0xC000, 0x0080, &[0x0222, 0x0444, 0x0666], 0x0EEE);
    let cram = vdp.get_cram_raw();
    assert_eq!(cram[0], 0x0EEE, "the triggering word is written normally");
    // The next FIFO slot holds the write made three writes before the trigger.
    assert_eq!(&cram[1..4], &[0x0222, 0x0222, 0x0222]);
    assert_eq!(cram[4], 0);
}

#[test]
fn vsram_fill_uses_next_fifo_entry() {
    // VSRAM write at 0 with CD5: 0x4000 / 0x0090.
    let vdp = fill_after_history(0x4000, 0x0090, &[0x0123, 0x0456, 0x0789], 0x0055);
    let word = |i: usize| u16::from_be_bytes([vdp.vsram[i * 2], vdp.vsram[i * 2 + 1]]);
    assert_eq!(word(0), 0x0055);
    assert_eq!((word(1), word(2), word(3)), (0x0123, 0x0123, 0x0123));
    assert_eq!(word(4), 0);
}

#[test]
fn vram_fill_writes_msb_to_adjacent_addresses() {
    let mut vdp = dma_vdp();
    vdp.registers[REG_AUTO_INC] = 2;
    vdp.registers[REG_DMA_SRC_HI] = DMA_MODE_FILL;
    vdp.registers[REG_DMA_LEN_LO] = 3;
    vdp.write_control(0x4100); // VRAM 0x0100
    vdp.write_control(0x0080);
    vdp.write_data(0xABCD);
    // Trigger word at 0x100/0x101, then MSB at 0x103, 0x105, 0x107.
    assert_eq!(
        &vdp.vram[0x100..0x108],
        &[0xAB, 0xCD, 0, 0xAB, 0, 0xAB, 0, 0xAB]
    );
}

proptest! {
    #[test]
    fn prop_68k_dma_source_stays_in_its_block(
        hi in 0u8..0x40,
        mid: u8,
        lo: u8,
        len in 1u16..600,
    ) {
        let mut vdp = dma_vdp();
        vdp.registers[REG_DMA_SRC_HI] = hi;
        vdp.registers[REG_DMA_SRC_MID] = mid;
        vdp.registers[REG_DMA_SRC_LO] = lo;
        vdp.registers[REG_DMA_LEN_LO] = (len & 0xFF) as u8;
        vdp.registers[REG_DMA_LEN_HI] = (len >> 8) as u8;
        vdp.write_control(0x4000);
        vdp.write_control(0x0080);
        let start = vdp.dma_source_transfer();
        let mut n = 0u32;
        while vdp.command.dma_pending {
            vdp.step_dma(&mut |a| {
                assert_eq!(a >> 17, start >> 17, "left the 128 KiB block");
                assert_eq!(a, (start & !0x1FFFF) | ((start + 2 * n) & 0x1FFFF));
                0
            });
            n += 1;
        }
        prop_assert_eq!(n, len as u32);
        prop_assert_eq!(vdp.registers[REG_DMA_SRC_HI], hi);
        prop_assert_eq!(vdp.dma_length(), 0);
    }
}
