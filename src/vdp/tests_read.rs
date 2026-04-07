use crate::vdp::{Vdp, VRAM_READ};

#[test]
fn test_vram_read_prefetch_wraps_at_end_of_vram() {
    let mut vdp = Vdp::new();
    vdp.command.code = VRAM_READ;
    vdp.command.address = 0xFFFF;
    vdp.vram[0xFFFF] = 0x12;
    vdp.vram[0xFFFE] = 0x34;

    vdp.try_prefetch();

    assert_eq!(vdp.command.read_buffer, 0x1234);
    assert_eq!(vdp.command.address, 0xFFFF);
}

#[test]
fn test_vram_read_prefetch_uses_vram_word_layout_for_odd_addresses() {
    let mut vdp = Vdp::new();
    vdp.command.code = VRAM_READ;
    vdp.command.address = 0x0001;
    vdp.registers[15] = 2;
    vdp.vram[0x0001] = 0xAB;
    vdp.vram[0x0000] = 0xCD;

    vdp.try_prefetch();

    assert_eq!(vdp.command.read_buffer, 0xABCD);
    assert_eq!(vdp.command.address, 0x0003);
}
