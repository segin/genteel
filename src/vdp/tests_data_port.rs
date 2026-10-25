//! Data/control port behaviour: prefetch, FIFO ordering, address alignment
//! and the implemented bit widths of CRAM and VSRAM.

use super::*;
use proptest::prelude::*;

fn set_command(vdp: &mut Vdp, first: u16, second: u16) {
    vdp.write_control(first);
    vdp.write_control(second);
}

/// Encode a VDP command for `code` (CD5-CD0) at `addr`.
fn command_words(code: u8, addr: u16) -> (u16, u16) {
    let code = code as u16;
    let first = ((code & 0x03) << 14) | (addr & 0x3FFF);
    let second = ((code & 0x3C) << 2) | ((addr >> 14) & 0x03);
    (first, second)
}

#[test]
fn vram_copy_command_does_not_prefetch_or_advance_address() {
    let mut vdp = Vdp::new();
    vdp.registers[REG_MODE2] = MODE2_DMA_ENABLE;
    vdp.registers[REG_AUTO_INC] = 1;
    vdp.registers[REG_DMA_SRC_HI] = DMA_MODE_COPY;
    vdp.registers[REG_DMA_SRC_MID] = 0x10;
    vdp.registers[REG_DMA_SRC_LO] = 0x00;
    vdp.registers[REG_DMA_LEN_LO] = 1;
    vdp.vram[0x1000] = 0x5A;
    vdp.vram[0x1001] = 0xA5;

    // $0000 / $00C0: code 0x30 (VRAM copy, CD5|CD4), destination $0000.
    set_command(&mut vdp, 0x0000, 0x00C0);
    assert_eq!(vdp.command.code, 0x30);
    assert!(vdp.command.dma_pending);
    assert_eq!(
        vdp.command.address, 0x0000,
        "a DMA command must not prefetch and advance the address"
    );

    vdp.step_dma(&mut |_| 0);
    assert!(!vdp.command.dma_pending);
    // The single copied byte lands at the destination address, not dest+inc.
    let copied = [vdp.vram[0x0000], vdp.vram[0x0001]];
    assert!(
        copied.contains(&0x5A) || copied.contains(&0xA5),
        "copy wrote to {:02X?}",
        copied
    );
    assert_eq!(
        vdp.vram[0x0002], 0,
        "copy must not land at dest + increment"
    );
}

#[test]
fn read_after_queued_write_returns_written_data() {
    let mut vdp = Vdp::new();
    vdp.registers[REG_AUTO_INC] = 2;

    let (w1, w2) = command_words(VRAM_WRITE, 0x0400);
    set_command(&mut vdp, w1, w2);
    vdp.write_data(0xBEEF);
    assert!(
        !vdp.fifo.is_empty(),
        "write must still be queued in the FIFO"
    );

    let (r1, r2) = command_words(VRAM_READ, 0x0400);
    set_command(&mut vdp, r1, r2);
    assert_eq!(vdp.read_data(), 0xBEEF);
    assert!(vdp.fifo.is_empty());
}

#[test]
fn vram_read_ignores_address_bit_0() {
    let mut vdp = Vdp::new();
    vdp.vram[0x0100] = 0x12;
    vdp.vram[0x0101] = 0x34;
    let (r1, r2) = command_words(VRAM_READ, 0x0101);
    set_command(&mut vdp, r1, r2);
    assert_eq!(vdp.read_data(), 0x1234);
}

#[test]
fn cram_read_ignores_address_bit_0() {
    let mut vdp = Vdp::new();
    vdp.bypass_fifo = true;
    vdp.registers[REG_AUTO_INC] = 2;
    let (w1, w2) = command_words(CRAM_WRITE, 0x0002);
    set_command(&mut vdp, w1, w2);
    vdp.write_data(0x0E00);
    vdp.write_data(0x00E0);

    // Reading from the odd address 0x03 must return entry 1, not a straddle
    // of entries 1 and 2.
    let (r1, r2) = command_words(CRAM_READ, 0x0003);
    set_command(&mut vdp, r1, r2);
    assert_eq!(vdp.read_data() & CRAM_COLOR_MASK, 0x0E00);
}

#[test]
fn cram_stores_only_implemented_colour_bits() {
    let mut vdp = Vdp::new();
    vdp.bypass_fifo = true;
    let (w1, w2) = command_words(CRAM_WRITE, 0x0000);
    set_command(&mut vdp, w1, w2);
    vdp.write_data(0xFFFF);
    assert_eq!(vdp.get_cram_raw()[0], 0x0EEE);

    let (r1, r2) = command_words(CRAM_READ, 0x0000);
    set_command(&mut vdp, r1, r2);
    // Unused bits come from the next FIFO slot, which still holds 0.
    assert_eq!(vdp.read_data(), 0x0EEE);
}

#[test]
fn get_cram_raw_returns_native_word_order() {
    let mut vdp = Vdp::new();
    vdp.bypass_fifo = true;
    let (w1, w2) = command_words(CRAM_WRITE, 0x0010);
    set_command(&mut vdp, w1, w2);
    vdp.write_data(0x0A24);
    assert_eq!(vdp.get_cram_raw()[8], 0x0A24);
}

#[test]
fn vsram_write_past_end_is_discarded() {
    let mut vdp = Vdp::new();
    vdp.bypass_fifo = true;
    let (w1, w2) = command_words(VSRAM_WRITE, 0x0050);
    set_command(&mut vdp, w1, w2);
    vdp.write_data(0x0123);
    assert!(
        vdp.vsram.iter().all(|&b| b == 0),
        "write at 0x50 must not wrap onto entry 0"
    );
}

#[test]
fn vsram_ignores_address_bit_0_and_masks_to_11_bits() {
    let mut vdp = Vdp::new();
    vdp.bypass_fifo = true;
    let (w1, w2) = command_words(VSRAM_WRITE, 0x0003);
    set_command(&mut vdp, w1, w2);
    vdp.write_data(0xFFFF);
    assert_eq!(vdp.vsram[0x02], 0x07);
    assert_eq!(vdp.vsram[0x03], 0xFF);
    assert_eq!(vdp.vsram[0x04], 0x00, "misaligned write must not straddle");
}

#[test]
fn vsram_read_past_end_returns_entry_0() {
    let mut vdp = Vdp::new();
    vdp.vsram[0] = 0x01;
    vdp.vsram[1] = 0x23;
    let (r1, r2) = command_words(VSRAM_READ, 0x0060);
    set_command(&mut vdp, r1, r2);
    assert_eq!(vdp.read_data() & VSRAM_ENTRY_MASK, 0x0123);
}

#[test]
fn register_write_loads_address_and_code_bits() {
    let mut vdp = Vdp::new();
    let (w1, w2) = command_words(VSRAM_WRITE, 0xC000);
    set_command(&mut vdp, w1, w2);

    vdp.write_control(0x8F02); // register 15 = 2
    assert_eq!(vdp.registers[REG_AUTO_INC], 2);
    assert_eq!(vdp.command.address & 0x3FFF, 0x0F02);
    assert_eq!(vdp.command.address & 0xC000, 0xC000, "A15-A14 retained");
    assert_eq!(
        vdp.command.code & 0x03,
        0x02,
        "CD1-CD0 loaded from bits 15-14"
    );
    assert_eq!(
        vdp.command.code & 0x3C,
        VSRAM_WRITE & 0x3C,
        "CD5-CD2 retained"
    );
    assert!(!vdp.command.pending);
}

proptest! {
    #[test]
    fn prop_vsram_write_respects_size_alignment_and_width(addr in 0u16..0x80, value: u16) {
        let mut vdp = Vdp::new();
        vdp.bypass_fifo = true;
        let (w1, w2) = command_words(VSRAM_WRITE, addr);
        set_command(&mut vdp, w1, w2);
        vdp.write_data(value);

        let idx = (addr & 0x7E) as usize;
        for (i, &b) in vdp.vsram.iter().enumerate() {
            if idx < VSRAM_SIZE && i == idx {
                prop_assert_eq!(b as u16, (value & VSRAM_ENTRY_MASK) >> 8);
            } else if idx < VSRAM_SIZE && i == idx + 1 {
                prop_assert_eq!(b as u16, value & 0xFF);
            } else {
                prop_assert_eq!(b, 0);
            }
        }
    }

    #[test]
    fn prop_cram_write_read_roundtrip_masks_and_aligns(addr in 0u16..0x80, value: u16) {
        let mut vdp = Vdp::new();
        vdp.bypass_fifo = true;
        let (w1, w2) = command_words(CRAM_WRITE, addr);
        set_command(&mut vdp, w1, w2);
        vdp.write_data(value);

        prop_assert_eq!(vdp.get_cram_raw()[((addr & 0x7E) >> 1) as usize], value & CRAM_COLOR_MASK);

        let (r1, r2) = command_words(CRAM_READ, addr);
        set_command(&mut vdp, r1, r2);
        prop_assert_eq!(vdp.read_data() & CRAM_COLOR_MASK, value & CRAM_COLOR_MASK);
    }

    #[test]
    fn prop_vram_read_is_word_aligned(addr: u16, hi: u8, lo: u8) {
        let mut vdp = Vdp::new();
        let base = (addr & 0xFFFE) as usize;
        vdp.vram[base] = hi;
        vdp.vram[base + 1] = lo;
        let (r1, r2) = command_words(VRAM_READ, addr);
        set_command(&mut vdp, r1, r2);
        prop_assert_eq!(vdp.read_data(), u16::from_be_bytes([hi, lo]));
    }
}
