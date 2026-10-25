use super::constants::*;
use super::timing::{is_external_slot, H32_TIMING, H40_TIMING};

#[test]
fn test_dma_mode_constants() {
    // Verify DMA mode constants are correctly masked
    assert_eq!(DMA_MODE_FILL & DMA_MODE_MASK, DMA_MODE_FILL);
    assert_eq!(DMA_MODE_COPY & DMA_MODE_MASK, DMA_MODE_COPY);

    // Ensure they represent distinct bits inside the mask
    assert_ne!(DMA_MODE_FILL, DMA_MODE_COPY);
}

#[test]
fn test_h40_external_slots_content() {
    let count = (0..H40_TIMING.slots)
        .filter(|&s| is_external_slot(s, true))
        .count();
    assert_eq!(count, 18, "H40 has exactly 18 external slots per line");
}

#[test]
fn test_h32_external_slots_content() {
    let count = (0..H32_TIMING.slots)
        .filter(|&s| is_external_slot(s, false))
        .count();
    assert_eq!(count, 16, "H32 has exactly 16 external slots per line");
}
