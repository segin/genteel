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
<<<<<<< HEAD
pub fn join_u32_from_u16(high: u16, low: u16) -> u32 {
=======
pub fn join_u32_words(high: u16, low: u16) -> u32 {
>>>>>>> main
    ((high as u32) << 16) | (low as u32)
}

/// Split a 32-bit long into two 16-bit words (Big Endian)
#[inline(always)]
<<<<<<< HEAD
pub fn split_u32_to_u16(long: u32) -> (u16, u16) {
    ((long >> 16) as u16, long as u16)
=======
pub fn split_u32_to_words(value: u32) -> (u16, u16) {
    ((value >> 16) as u16, value as u16)
>>>>>>> main
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
<<<<<<< HEAD
    fn test_u16_ops() {
        assert_eq!(join_u16(0x12, 0x34), 0x1234);
=======
    fn test_join_u16() {
        assert_eq!(join_u16(0x12, 0x34), 0x1234);
    }

    #[test]
    fn test_split_u16() {
>>>>>>> main
        assert_eq!(split_u16(0x1234), (0x12, 0x34));
    }

    #[test]
<<<<<<< HEAD
    fn test_u32_ops() {
        assert_eq!(join_u32(0x12, 0x34, 0x56, 0x78), 0x12345678);
=======
    fn test_join_u32() {
        assert_eq!(join_u32(0x12, 0x34, 0x56, 0x78), 0x12345678);
    }

    #[test]
    fn test_split_u32() {
>>>>>>> main
        assert_eq!(split_u32(0x12345678), (0x12, 0x34, 0x56, 0x78));
    }

    #[test]
<<<<<<< HEAD
    fn test_u32_u16_ops() {
        assert_eq!(join_u32_from_u16(0x1234, 0x5678), 0x12345678);
        assert_eq!(split_u32_to_u16(0x12345678), (0x1234, 0x5678));
=======
    fn test_join_u32_words() {
        assert_eq!(join_u32_words(0x1234, 0x5678), 0x12345678);
    }

    #[test]
    fn test_split_u32_to_words() {
        assert_eq!(split_u32_to_words(0x12345678), (0x1234, 0x5678));
>>>>>>> main
    }
}
