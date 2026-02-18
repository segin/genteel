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

/// Serde helper for arrays larger than 32 elements
pub mod big_array {
    use serde::{Deserializer, Serializer};
    use serde::ser::SerializeTuple;

    pub fn serialize<S, const N: usize>(data: &[u8; N], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_tuple(N)?;
        for item in data {
            s.serialize_element(item)?;
        }
        s.end()
    }

    pub fn deserialize<'de, D, const N: usize>(deserializer: D) -> Result<[u8; N], D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ArrayVisitor<const N: usize>;

        impl<'de, const N: usize> serde::de::Visitor<'de> for ArrayVisitor<N> {
            type Value = [u8; N];

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_fmt(format_args!("an array of length {}", N))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<[u8; N], A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                // Note: Allocating a large array on stack might be risky if N is very large.
                // But [u8; N] is usually used for fixed size buffers.
                // For 64KB, it's fine on most threads (default stack 2MB).
                // Box::new could be used but we return [u8; N] by value.
                let mut arr = [0u8; N];
                for i in 0..N {
                    arr[i] = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(i, &self))?;
                }
                Ok(arr)
            }
        }

        deserializer.deserialize_tuple(N, ArrayVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // --- Unit Tests (Explicit Cases) ---

    #[test]
    fn test_join_u16_explicit() {
        assert_eq!(join_u16(0x00, 0x00), 0x0000);
        assert_eq!(join_u16(0x12, 0x34), 0x1234);
        assert_eq!(join_u16(0xFF, 0xFF), 0xFFFF);
        assert_eq!(join_u16(0x80, 0x01), 0x8001);
    }

    #[test]
    fn test_split_u16_explicit() {
        assert_eq!(split_u16(0x0000), (0x00, 0x00));
        assert_eq!(split_u16(0x1234), (0x12, 0x34));
        assert_eq!(split_u16(0xFFFF), (0xFF, 0xFF));
        assert_eq!(split_u16(0x8001), (0x80, 0x01));
    }

    #[test]
    fn test_join_u32_explicit() {
        assert_eq!(join_u32(0x00, 0x00, 0x00, 0x00), 0x00000000);
        assert_eq!(join_u32(0x12, 0x34, 0x56, 0x78), 0x12345678);
        assert_eq!(join_u32(0xFF, 0xFF, 0xFF, 0xFF), 0xFFFFFFFF);
    }

    #[test]
    fn test_split_u32_explicit() {
        assert_eq!(split_u32(0x00000000), (0x00, 0x00, 0x00, 0x00));
        assert_eq!(split_u32(0x12345678), (0x12, 0x34, 0x56, 0x78));
        assert_eq!(split_u32(0xFFFFFFFF), (0xFF, 0xFF, 0xFF, 0xFF));
    }

    #[test]
    fn test_u32_word_ops_explicit() {
        assert_eq!(join_u32_words(0x0000, 0x0000), 0x00000000);
        assert_eq!(join_u32_words(0x1234, 0x5678), 0x12345678);
        assert_eq!(join_u32_words(0xFFFF, 0xFFFF), 0xFFFFFFFF);

        assert_eq!(split_u32_to_words(0x00000000), (0x0000, 0x0000));
        assert_eq!(split_u32_to_words(0x12345678), (0x1234, 0x5678));
        assert_eq!(split_u32_to_words(0xFFFFFFFF), (0xFFFF, 0xFFFF));
    }

    // --- Property-Based Tests ---

    proptest! {
        // --- Round-Trip Consistency ---

        #[test]
        fn prop_u16_round_trip(val in any::<u16>()) {
            let (h, l) = split_u16(val);
            let joined = join_u16(h, l);
            prop_assert_eq!(val, joined, "split_u16 -> join_u16 mismatch");
        }

        #[test]
        fn prop_u16_components_round_trip(h in any::<u8>(), l in any::<u8>()) {
            let val = join_u16(h, l);
            let (h_out, l_out) = split_u16(val);
            prop_assert_eq!(h, h_out, "High byte mismatch");
            prop_assert_eq!(l, l_out, "Low byte mismatch");
        }

        #[test]
        fn prop_u32_round_trip(val in any::<u32>()) {
            let (b0, b1, b2, b3) = split_u32(val);
            let joined = join_u32(b0, b1, b2, b3);
            prop_assert_eq!(val, joined, "split_u32 -> join_u32 mismatch");
        }

        #[test]
        fn prop_u32_components_round_trip(b0 in any::<u8>(), b1 in any::<u8>(), b2 in any::<u8>(), b3 in any::<u8>()) {
            let val = join_u32(b0, b1, b2, b3);
            let (out_b0, out_b1, out_b2, out_b3) = split_u32(val);
            prop_assert_eq!(b0, out_b0, "Byte 0 mismatch");
            prop_assert_eq!(b1, out_b1, "Byte 1 mismatch");
            prop_assert_eq!(b2, out_b2, "Byte 2 mismatch");
            prop_assert_eq!(b3, out_b3, "Byte 3 mismatch");
        }

        #[test]
        fn prop_u32_words_round_trip(val in any::<u32>()) {
            let (h, l) = split_u32_to_words(val);
            let joined = join_u32_words(h, l);
            prop_assert_eq!(val, joined, "split_u32_to_words -> join_u32_words mismatch");
        }

        #[test]
        fn prop_u32_from_u16_round_trip(h in any::<u16>(), l in any::<u16>()) {
            let val = join_u32_from_u16(h, l);
            let (h_out, l_out) = split_u32_to_u16(val);
            prop_assert_eq!(h, h_out, "High word mismatch");
            prop_assert_eq!(l, l_out, "Low word mismatch");
        }

        // --- Logic Validation against Reference Implementation (Bit Shifting) ---

        #[test]
        fn prop_verify_join_u16_logic(h in any::<u8>(), l in any::<u8>()) {
            // Reference implementation using explicit shifts
            let expected = ((h as u16) << 8) | (l as u16);
            let actual = join_u16(h, l);
            prop_assert_eq!(expected, actual);
        }

        #[test]
        fn prop_verify_split_u16_logic(val in any::<u16>()) {
            // Reference implementation
            let expected_h = ((val >> 8) & 0xFF) as u8;
            let expected_l = (val & 0xFF) as u8;
            let (actual_h, actual_l) = split_u16(val);
            prop_assert_eq!(expected_h, actual_h);
            prop_assert_eq!(expected_l, actual_l);
        }

        #[test]
        fn prop_verify_join_u32_logic(b0 in any::<u8>(), b1 in any::<u8>(), b2 in any::<u8>(), b3 in any::<u8>()) {
            // Reference implementation
            let expected = ((b0 as u32) << 24) |
                           ((b1 as u32) << 16) |
                           ((b2 as u32) << 8)  |
                           (b3 as u32);
            let actual = join_u32(b0, b1, b2, b3);
            prop_assert_eq!(expected, actual);
        }

        #[test]
        fn prop_verify_split_u32_logic(val in any::<u32>()) {
            // Reference implementation
            let expected_b0 = ((val >> 24) & 0xFF) as u8;
            let expected_b1 = ((val >> 16) & 0xFF) as u8;
            let expected_b2 = ((val >> 8)  & 0xFF) as u8;
            let expected_b3 = (val & 0xFF) as u8;

            let (actual_b0, actual_b1, actual_b2, actual_b3) = split_u32(val);
            prop_assert_eq!(expected_b0, actual_b0);
            prop_assert_eq!(expected_b1, actual_b1);
            prop_assert_eq!(expected_b2, actual_b2);
            prop_assert_eq!(expected_b3, actual_b3);
        }
    }
}
