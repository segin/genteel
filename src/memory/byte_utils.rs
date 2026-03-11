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

/// Split a 32-bit long into two 16-bit words (Big Endian)
#[inline(always)]
pub fn split_u32_to_words(value: u32) -> (u16, u16) {
    ((value >> 16) as u16, value as u16)
}

/// Serde helper for arrays larger than 32 elements
pub mod big_array {
    use serde::ser::SerializeTuple;
    use serde::{Deserializer, Serializer};

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
                for (i, item) in arr.iter_mut().enumerate() {
                    *item = seq
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

    #[test]
    fn test_u16_ops() {
        assert_eq!(join_u16(0x12, 0x34), 0x1234);
        assert_eq!(split_u16(0x1234), (0x12, 0x34));
    }

    #[test]
    fn test_u32_ops() {
        assert_eq!(join_u32(0x12, 0x34, 0x56, 0x78), 0x12345678);
        assert_eq!(split_u32(0x12345678), (0x12, 0x34, 0x56, 0x78));
    }

    #[test]
    fn test_u32_word_ops() {
        assert_eq!(join_u32_words(0x1234, 0x5678), 0x12345678);
        assert_eq!(split_u32_to_words(0x12345678), (0x1234, 0x5678));
    }

    #[test]
    fn test_u32_word_ops_extended() {
        // Edge cases for join_u32_words
        assert_eq!(join_u32_words(0x0000, 0x0000), 0x00000000);
        assert_eq!(join_u32_words(0xFFFF, 0xFFFF), 0xFFFFFFFF);
        assert_eq!(join_u32_words(0x0000, 0xFFFF), 0x0000FFFF);
        assert_eq!(join_u32_words(0xFFFF, 0x0000), 0xFFFF0000);

        // Edge cases for split_u32_to_words
        assert_eq!(split_u32_to_words(0x00000000), (0x0000, 0x0000));
        assert_eq!(split_u32_to_words(0xFFFFFFFF), (0xFFFF, 0xFFFF));
        assert_eq!(split_u32_to_words(0x0000FFFF), (0x0000, 0xFFFF));
        assert_eq!(split_u32_to_words(0xFFFF0000), (0xFFFF, 0x0000));
    }

    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
    struct BigArrayTestStruct {
        #[serde(with = "crate::memory::byte_utils::big_array")]
        data: [u8; 64],
    }

    #[test]
    fn test_big_array_serialization() {
        let original = BigArrayTestStruct {
            data: [42; 64],
        };

        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: BigArrayTestStruct = serde_json::from_str(&serialized).unwrap();

        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_big_array_deserialization_error() {
        let mut json = String::from(r#"{"data":["#);
        for _ in 0..63 {
            json.push_str("42,");
        }
        json.push_str("42]}"); // length 64, this should pass

        let deserialized: Result<BigArrayTestStruct, _> = serde_json::from_str(&json);
        assert!(deserialized.is_ok());

        // Now test invalid length
        let mut json_invalid = String::from(r#"{"data":["#);
        for _ in 0..62 {
            json_invalid.push_str("42,");
        }
        json_invalid.push_str("42]}"); // length 63, this should fail

        let deserialized_invalid: Result<BigArrayTestStruct, _> = serde_json::from_str(&json_invalid);
        assert!(deserialized_invalid.is_err());
    }
}
