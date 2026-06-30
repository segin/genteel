use super::render::{SpriteAttributes, SpriteIterator};

#[test]
fn test_sprite_iterator_basic() {
    // SAT entry:
    // Byte 0-1: V-pos 128 + 10 = 138 (0x008A)
    // Byte 2: H-size 2, V-size 3 (size = ((2-1) << 2) | (3-1) = 0x04 | 0x02 = 0x06)
    // Byte 3: Link 1
    // Byte 4-5: Priority 1, Palette 2, V-flip 1, H-flip 0, Tile 0x123
    //           0x8000 | (2 << 13) | 0x1000 | 0x123 = 0x8000 | 0x4000 | 0x1000 | 0x123 = 0xD123
    // Byte 6-7: H-pos 128 + 20 = 148 (0x0094)

    let mut vram = [0u8; 16];
    vram[0] = 0x00;
    vram[1] = 0x8A;
    vram[2] = 0x06;
    vram[3] = 0x01;
    vram[4] = 0xD1;
    vram[5] = 0x23;
    vram[6] = 0x00;
    vram[7] = 0x94;

    let mut iter = SpriteIterator {
        vram: &vram,
        next_idx: 0,
        count: 0,
        max_sprites: 80,
        sat_base: 0,
    };

    let attr = iter.next().expect("Should have one sprite");
    assert_eq!(attr.v_pos, 10);
    assert_eq!(attr.h_pos, 20);
    assert_eq!(attr.h_size, 2);
    assert_eq!(attr.v_size, 3);
    assert_eq!(attr.priority, true);
    assert_eq!(attr.palette, 2);
    assert_eq!(attr.v_flip, true);
    assert_eq!(attr.h_flip, false);
    assert_eq!(attr.base_tile, 0x123);
    assert_eq!(attr.index, 0);
    assert_eq!(attr.link, 1);
}

#[test]
fn test_sprite_iterator_multiple() {
    let mut vram = [0u8; 16];
    // Sprite 0 -> Link 1
    vram[0..8].copy_from_slice(&[0x00, 0x80, 0x00, 0x01, 0x00, 0x00, 0x00, 0x80]);
    // Sprite 1 -> Link 0 (Stop)
    vram[8..16].copy_from_slice(&[0x00, 0x81, 0x00, 0x00, 0x00, 0x00, 0x00, 0x81]);

    let iter = SpriteIterator {
        vram: &vram,
        next_idx: 0,
        count: 0,
        max_sprites: 80,
        sat_base: 0,
    };

    let attrs: Vec<SpriteAttributes> = iter.collect();
    assert_eq!(attrs.len(), 2);
    assert_eq!(attrs[0].v_pos, 0);
    assert_eq!(attrs[1].v_pos, 1);
}

#[test]
fn test_sprite_iterator_link_zero() {
    let mut vram = [0u8; 24];
    // Sprite 0 -> Link 0
    vram[0..8].copy_from_slice(&[0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80]);
    // Sprite 1 (should not be reached)
    vram[8..16].copy_from_slice(&[0x00, 0x81, 0x00, 0x02, 0x00, 0x00, 0x00, 0x81]);

    let iter = SpriteIterator {
        vram: &vram,
        next_idx: 0,
        count: 0,
        max_sprites: 80,
        sat_base: 0,
    };

    let attrs: Vec<SpriteAttributes> = iter.collect();
    assert_eq!(attrs.len(), 1, "Should stop when link is 0");
}

#[test]
fn test_sprite_iterator_max_sprites() {
    let mut vram = [0u8; 16];
    // Sprite 0 -> Link 1
    vram[0..8].copy_from_slice(&[0x00, 0x80, 0x00, 0x01, 0x00, 0x00, 0x00, 0x80]);
    // Sprite 1 -> Link 2
    vram[8..16].copy_from_slice(&[0x00, 0x81, 0x00, 0x02, 0x00, 0x00, 0x00, 0x81]);

    let iter = SpriteIterator {
        vram: &vram,
        next_idx: 0,
        count: 0,
        max_sprites: 1, // Limit to 1
        sat_base: 0,
    };

    let attrs: Vec<SpriteAttributes> = iter.collect();
    assert_eq!(attrs.len(), 1, "Should stop at max_sprites");
}

#[test]
fn test_sprite_iterator_oob() {
    let vram = [0u8; 8];

    let mut iter = SpriteIterator {
        vram: &vram,
        next_idx: 1, // Points to index 1, but VRAM only has 8 bytes (index 0)
        count: 0,
        max_sprites: 80,
        sat_base: 0,
    };

    assert!(iter.next().is_none(), "Should return None for OOB link");
}

#[test]
fn test_sprite_iterator_sat_base() {
    let mut vram = [0u8; 16];
    // Sprite at offset 8
    vram[8..16].copy_from_slice(&[0x00, 0x8A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80]);

    let mut iter = SpriteIterator {
        vram: &vram,
        next_idx: 0,
        count: 0,
        max_sprites: 80,
        sat_base: 8,
    };

    let attr = iter.next().expect("Should find sprite at sat_base");
    assert_eq!(attr.v_pos, 10);
}
