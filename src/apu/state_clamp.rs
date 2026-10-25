//! Serde helpers that clamp restored APU fields into their valid range.
//!
//! APU state is restored from save states and from the debugger
//! (`Debuggable::write_state`), both of which may carry hand-edited or
//! corrupted values. Fields that index tables or feed shifts are clamped on
//! deserialization so a malformed state can never make the APU panic.

use serde::{Deserialize, Deserializer};

/// Deserialize a `u8` and clamp it to `MAX`.
pub(crate) fn u8_max<'de, D, const MAX: u8>(d: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(u8::deserialize(d)?.min(MAX))
}

/// Deserialize a `u16` and clamp it to `MAX`.
pub(crate) fn u16_max<'de, D, const MAX: u16>(d: D) -> Result<u16, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(u16::deserialize(d)?.min(MAX))
}

/// Deserialize a `u32` and clamp it to `MAX`.
pub(crate) fn u32_max<'de, D, const MAX: u32>(d: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(u32::deserialize(d)?.min(MAX))
}

/// Deserialize an `i32` and clamp it to `MIN..=MAX`.
pub(crate) fn i32_range<'de, D, const MIN: i32, const MAX: i32>(d: D) -> Result<i32, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(i32::deserialize(d)?.clamp(MIN, MAX))
}

/// Deserialize a `[u8; N]` and clamp each element to `MAX`.
pub(crate) fn u8_array_max<'de, D, const N: usize, const MAX: u8>(d: D) -> Result<[u8; N], D::Error>
where
    D: Deserializer<'de>,
    [u8; N]: Deserialize<'de>,
{
    Ok(<[u8; N]>::deserialize(d)?.map(|v| v.min(MAX)))
}

/// Deserialize a `[u16; N]` and clamp each element to `MAX`.
pub(crate) fn u16_array_max<'de, D, const N: usize, const MAX: u16>(
    d: D,
) -> Result<[u16; N], D::Error>
where
    D: Deserializer<'de>,
    [u16; N]: Deserialize<'de>,
{
    Ok(<[u16; N]>::deserialize(d)?.map(|v| v.min(MAX)))
}

/// Deserialize an `[i32; N]` and clamp each element to `MIN..=MAX`.
pub(crate) fn i32_array_range<'de, D, const N: usize, const MIN: i32, const MAX: i32>(
    d: D,
) -> Result<[i32; N], D::Error>
where
    D: Deserializer<'de>,
    [i32; N]: Deserialize<'de>,
{
    Ok(<[i32; N]>::deserialize(d)?.map(|v| v.clamp(MIN, MAX)))
}
