//! Helper functions for byte manipulation.
//!
//! These functions provide centralized logic for combining bytes into words/longs
//! and splitting words/longs into bytes, using big-endian byte order as per the
//! Genesis/M68k architecture.

/// Join two bytes into a 16-bit word (Big Endian)
#[inline(always)]
pub fn join_u16(high: u8, low: u8) -> u16 {
    u16::from_be_bytes([high, low])
}

/// Split a 16-bit word into two bytes (Big Endian)
#[inline(always)]
pub fn split_u16(word: u16) -> (u8, u8) {
    let bytes = word.to_be_bytes();
    (bytes[0], bytes[1])
}

/// Join four bytes into a 32-bit long (Big Endian)
#[inline(always)]
pub fn join_u32(b0: u8, b1: u8, b2: u8, b3: u8) -> u32 {
    u32::from_be_bytes([b0, b1, b2, b3])
}

/// Split a 32-bit long into four bytes (Big Endian)
#[inline(always)]
pub fn split_u32(value: u32) -> (u8, u8, u8, u8) {
    let bytes = value.to_be_bytes();
    (bytes[0], bytes[1], bytes[2], bytes[3])
}

/// Join two 16-bit words into a 32-bit long (Big Endian)
#[inline(always)]
pub fn join_u32_words(high: u16, low: u16) -> u32 {
    ((high as u32) << 16) | (low as u32)
}

/// Alias for join_u32_words
#[inline(always)]
pub fn join_u32_from_u16(high: u16, low: u16) -> u32 {
    join_u32_words(high, low)
}

/// Split a 32-bit long into two 16-bit words (Big Endian)
#[inline(always)]
pub fn split_u32_to_words(value: u32) -> (u16, u16) {
    ((value >> 16) as u16, value as u16)
}

/// Alias for split_u32_to_words
#[inline(always)]
pub fn split_u32_to_u16(long: u32) -> (u16, u16) {
    split_u32_to_words(long)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_u16_roundtrip(h in any::<u8>(), l in any::<u8>()) {
            let w = join_u16(h, l);
            let (h2, l2) = split_u16(w);
            prop_assert_eq!(h, h2);
            prop_assert_eq!(l, l2);
        }

        #[test]
        fn prop_u16_split_join_roundtrip(w in any::<u16>()) {
            let (h, l) = split_u16(w);
            let w2 = join_u16(h, l);
            prop_assert_eq!(w, w2);
        }

        #[test]
        fn prop_u32_roundtrip(b0 in any::<u8>(), b1 in any::<u8>(), b2 in any::<u8>(), b3 in any::<u8>()) {
            let l = join_u32(b0, b1, b2, b3);
            let (r0, r1, r2, r3) = split_u32(l);
            prop_assert_eq!(b0, r0);
            prop_assert_eq!(b1, r1);
            prop_assert_eq!(b2, r2);
            prop_assert_eq!(b3, r3);
        }

        #[test]
        fn prop_u32_split_join_roundtrip(val in any::<u32>()) {
            let (b0, b1, b2, b3) = split_u32(val);
            let val2 = join_u32(b0, b1, b2, b3);
            prop_assert_eq!(val, val2);
        }

        #[test]
        fn prop_u32_words_roundtrip(h in any::<u16>(), l in any::<u16>()) {
            let val = join_u32_words(h, l);
            let (h2, l2) = split_u32_to_words(val);
            prop_assert_eq!(h, h2);
            prop_assert_eq!(l, l2);
        }

        #[test]
        fn prop_u32_words_split_join_roundtrip(val in any::<u32>()) {
            let (h, l) = split_u32_to_words(val);
            let val2 = join_u32_words(h, l);
            prop_assert_eq!(val, val2);
        }

        #[test]
        fn prop_u16_value(h in any::<u8>(), l in any::<u8>()) {
            let val = join_u16(h, l);
            let expected = (h as u16) * 256 + (l as u16);
            prop_assert_eq!(val, expected);
        }

        #[test]
        fn prop_u32_value(b0 in any::<u8>(), b1 in any::<u8>(), b2 in any::<u8>(), b3 in any::<u8>()) {
            let val = join_u32(b0, b1, b2, b3);
            let expected = ((b0 as u32) << 24) | ((b1 as u32) << 16) | ((b2 as u32) << 8) | (b3 as u32);
            prop_assert_eq!(val, expected);
        }
    }

    #[test]
    fn test_u16_ops() {
        assert_eq!(join_u16(0x12, 0x34), 0x1234);
        assert_eq!(split_u16(0x1234), (0x12, 0x34));
    }

    #[test]
    fn test_u16_ops_edge_cases() {
        // Zero
        assert_eq!(join_u16(0, 0), 0);
        assert_eq!(split_u16(0), (0, 0));

        // Max values
        assert_eq!(join_u16(0xFF, 0xFF), 0xFFFF);
        assert_eq!(split_u16(0xFFFF), (0xFF, 0xFF));

        // Mixed
        assert_eq!(join_u16(0xFF, 0), 0xFF00);
        assert_eq!(split_u16(0xFF00), (0xFF, 0));

        assert_eq!(join_u16(0, 0xFF), 0x00FF);
        assert_eq!(split_u16(0x00FF), (0, 0xFF));
    }

    #[test]
    fn test_u32_ops() {
        assert_eq!(join_u32(0x12, 0x34, 0x56, 0x78), 0x12345678);
        assert_eq!(split_u32(0x12345678), (0x12, 0x34, 0x56, 0x78));
    }

    #[test]
    fn test_u32_ops_edge_cases() {
        // Zero
        assert_eq!(join_u32(0, 0, 0, 0), 0);
        assert_eq!(split_u32(0), (0, 0, 0, 0));

        // Max values
        assert_eq!(join_u32(0xFF, 0xFF, 0xFF, 0xFF), 0xFFFFFFFF);
        assert_eq!(split_u32(0xFFFFFFFF), (0xFF, 0xFF, 0xFF, 0xFF));

        // Mixed patterns
        assert_eq!(join_u32(0xFF, 0, 0xFF, 0), 0xFF00FF00);
        assert_eq!(split_u32(0xFF00FF00), (0xFF, 0, 0xFF, 0));
    }

    #[test]
    fn test_u32_word_ops() {
        assert_eq!(join_u32_words(0x1234, 0x5678), 0x12345678);
        assert_eq!(split_u32_to_words(0x12345678), (0x1234, 0x5678));

        assert_eq!(join_u32_from_u16(0x1234, 0x5678), 0x12345678);
        assert_eq!(split_u32_to_u16(0x12345678), (0x1234, 0x5678));
    }

    #[test]
    fn test_u32_word_ops_edge_cases() {
        // Zero
        assert_eq!(join_u32_words(0, 0), 0);
        assert_eq!(split_u32_to_words(0), (0, 0));

        // Max
        assert_eq!(join_u32_words(0xFFFF, 0xFFFF), 0xFFFFFFFF);
        assert_eq!(split_u32_to_words(0xFFFFFFFF), (0xFFFF, 0xFFFF));

        // Mixed
        assert_eq!(join_u32_words(0xFFFF, 0), 0xFFFF0000);
        assert_eq!(split_u32_to_words(0xFFFF0000), (0xFFFF, 0));
    }

    #[test]
    fn test_aliases_match() {
        let h = 0xABCD;
        let l = 0xEF01;
        assert_eq!(join_u32_words(h, l), join_u32_from_u16(h, l));

        let w = 0xABCDEF01;
        assert_eq!(split_u32_to_words(w), split_u32_to_u16(w));
    }
}
