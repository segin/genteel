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
