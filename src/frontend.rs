//! Frontend Module - winit + pixels
//!
//! Provides cross-platform windowing, input handling, and rendering
//! for the Genesis emulator using pure Rust libraries.

#[cfg(feature = "gui")]
use winit::keyboard::KeyCode;

/// Genesis display dimensions
pub const GENESIS_WIDTH: u32 = 320;
pub const GENESIS_HEIGHT: u32 = 240;

/// Key mapping for player 1
#[cfg(feature = "gui")]
pub fn keycode_to_button(keycode: KeyCode) -> Option<(&'static str, bool)> {
    match keycode {
        // Player 1 - Arrow keys + ZXC/Enter
        KeyCode::ArrowUp => Some(("up", true)),
        KeyCode::ArrowDown => Some(("down", true)),
        KeyCode::ArrowLeft => Some(("left", true)),
        KeyCode::ArrowRight => Some(("right", true)),
        KeyCode::KeyZ => Some(("a", true)),
        KeyCode::KeyX => Some(("b", true)),
        KeyCode::KeyC => Some(("c", true)),
        KeyCode::Enter => Some(("start", true)),
        // 6-button extension
        KeyCode::KeyA => Some(("x", true)),
        KeyCode::KeyS => Some(("y", true)),
        KeyCode::KeyD => Some(("z", true)),
        KeyCode::KeyQ => Some(("mode", true)),
        _ => None,
    }
}

/// Convert RGB565 framebuffer to RGBA8 for pixels crate
pub fn rgb565_to_rgba8(framebuffer_565: &[u16], output: &mut [u8]) {
    for (&pixel, chunk) in framebuffer_565.iter().zip(output.chunks_exact_mut(4)) {
        // Extract RGB565 components
        let r5 = ((pixel >> 11) & 0x1F) as u8;
        let g6 = ((pixel >> 5) & 0x3F) as u8;
        let b5 = (pixel & 0x1F) as u8;

        // Scale to 8-bit
        chunk[0] = (r5 << 3) | (r5 >> 2);     // R
        chunk[1] = (g6 << 2) | (g6 >> 4); // G
        chunk[2] = (b5 << 3) | (b5 >> 2); // B
        chunk[3] = 255;                    // A
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    #[cfg(feature = "gui")]
    fn test_keycode_mapping() {
        assert_eq!(keycode_to_button(KeyCode::KeyZ), Some(("a", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyX), Some(("b", true)));
        assert_eq!(keycode_to_button(KeyCode::ArrowUp), Some(("up", true)));
    }
    
    #[test]
    fn test_rgb565_to_rgba8_black() {
        let input = [0x0000u16];
        let mut output = [0u8; 4];
        rgb565_to_rgba8(&input, &mut output);
        assert_eq!(output, [0, 0, 0, 255]);
    }
    
    #[test]
    fn test_rgb565_to_rgba8_white() {
        let input = [0xFFFFu16];
        let mut output = [0u8; 4];
        rgb565_to_rgba8(&input, &mut output);
        assert_eq!(output, [255, 255, 255, 255]);
    }
}
