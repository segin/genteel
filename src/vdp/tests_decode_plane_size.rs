use crate::vdp::Vdp;

#[test]
fn test_decode_plane_size_horizontal() {
    // HSZ bits (0-1)
    // 00 -> 32
    assert_eq!(Vdp::decode_plane_size(0x00).0, 32);
    // 01 -> 64
    assert_eq!(Vdp::decode_plane_size(0x01).0, 64);
    // 10 -> 32 (fallback)
    assert_eq!(Vdp::decode_plane_size(0x02).0, 32);
    // 11 -> 128
    assert_eq!(Vdp::decode_plane_size(0x03).0, 128);
}

#[test]
fn test_decode_plane_size_vertical() {
    // VSZ bits (4-5)
    // 00 -> 32
    assert_eq!(Vdp::decode_plane_size(0x00).1, 32);
    // 01 -> 64
    assert_eq!(Vdp::decode_plane_size(0x10).1, 64);
    // 10 -> 32 (fallback)
    assert_eq!(Vdp::decode_plane_size(0x20).1, 32);
    // 11 -> 128
    assert_eq!(Vdp::decode_plane_size(0x30).1, 128);
}

#[test]
fn test_decode_plane_size_combinations() {
    // HSZ=128 (11), VSZ=64 (01) -> 0x13
    assert_eq!(Vdp::decode_plane_size(0x13), (128, 64));

    // HSZ=32 (fallback 10), VSZ=128 (11) -> 0x32
    assert_eq!(Vdp::decode_plane_size(0x32), (32, 128));

    // HSZ=64 (01), VSZ=32 (fallback 10) -> 0x21
    assert_eq!(Vdp::decode_plane_size(0x21), (64, 32));

    // HSZ=fallback (10), VSZ=fallback (10) -> 0x22
    assert_eq!(Vdp::decode_plane_size(0x22), (32, 32));
}

#[test]
fn test_decode_plane_size_ignore_other_bits() {
    // Ensure bits other than 0-1 and 4-5 are ignored
    // 0xEE = 1110 1110
    // Bits 0-1: 10 (fallback -> 32)
    // Bits 4-5: 10 (fallback -> 32)
    assert_eq!(Vdp::decode_plane_size(0xEE), (32, 32));

    // 0xCD = 1100 1101
    // Bits 0-1: 01 (64)
    // Bits 4-5: 00 (32)
    assert_eq!(Vdp::decode_plane_size(0xCD), (64, 32));
}
