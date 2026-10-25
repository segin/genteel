use crate::vdp::{Vdp, VRAM_READ};

#[test]
fn test_vram_read_prefetch_wraps_at_end_of_vram() {
    let mut vdp = Vdp::new();
    vdp.command.code = VRAM_READ;
    vdp.command.address = 0xFFFF;
    vdp.vram[0xFFFF] = 0x12;
    vdp.vram[0xFFFE] = 0x34;

    vdp.try_prefetch();

    // Address bit 0 is ignored, so the aligned word at 0xFFFE is returned.
    assert_eq!(vdp.command.read_buffer, 0x3412);
    assert_eq!(vdp.command.address, 0xFFFF);
}

#[test]
fn test_vram_read_prefetch_ignores_address_bit_0() {
    let mut vdp = Vdp::new();
    vdp.command.code = VRAM_READ;
    vdp.command.address = 0x0001;
    vdp.registers[15] = 2;
    vdp.vram[0x0001] = 0xAB;
    vdp.vram[0x0000] = 0xCD;

    vdp.try_prefetch();

    assert_eq!(vdp.command.read_buffer, 0xCDAB);
    assert_eq!(vdp.command.address, 0x0003);
}
