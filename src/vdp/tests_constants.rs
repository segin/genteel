use super::constants::*;

#[test]
fn test_external_slots_length() {
    assert_eq!(H40_EXTERNAL_SLOTS.len(), 210);
    assert_eq!(H32_EXTERNAL_SLOTS.len(), 171);
}

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
    // Count the true values in H40_EXTERNAL_SLOTS
    let true_count = H40_EXTERNAL_SLOTS.iter().filter(|&&x| x).count();
    assert_eq!(true_count, 18, "H40_EXTERNAL_SLOTS should have exactly 18 external slots");
}

#[test]
fn test_h32_external_slots_content() {
    // Count the true values in H32_EXTERNAL_SLOTS
    let true_count = H32_EXTERNAL_SLOTS.iter().filter(|&&x| x).count();
    assert_eq!(true_count, 16, "H32_EXTERNAL_SLOTS should have exactly 16 external slots");
}
