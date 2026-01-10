//! Frontend Module - SDL2 + egui GUI
//!
//! Provides cross-platform windowing, input handling, and GUI menus
//! for the Genesis emulator.

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use sdl2::EventPump;

use crate::io::ControllerState;
use crate::input::FrameInput;

/// Genesis display dimensions
pub const GENESIS_WIDTH: u32 = 320;
pub const GENESIS_HEIGHT: u32 = 224;

/// Key mapping for player 1
pub fn keycode_to_button(keycode: Keycode) -> Option<(&'static str, bool)> {
    match keycode {
        // Player 1 - Arrow keys + ZXC/Enter
        Keycode::Up => Some(("up", true)),
        Keycode::Down => Some(("down", true)),
        Keycode::Left => Some(("left", true)),
        Keycode::Right => Some(("right", true)),
        Keycode::Z => Some(("a", true)),
        Keycode::X => Some(("b", true)),
        Keycode::C => Some(("c", true)),
        Keycode::Return => Some(("start", true)),
        // 6-button extension
        Keycode::A => Some(("x", true)),
        Keycode::S => Some(("y", true)),
        Keycode::D => Some(("z", true)),
        Keycode::Q => Some(("mode", true)),
        _ => None,
    }
}

/// Poll SDL2 events and update controller state
pub fn poll_input(event_pump: &mut EventPump, state: &mut ControllerState) -> bool {
    for event in event_pump.poll_iter() {
        match event {
            Event::Quit { .. } => return false,
            
            Event::KeyDown { keycode: Some(keycode), .. } => {
                if keycode == Keycode::Escape {
                    return false;
                }
                if let Some((button, _)) = keycode_to_button(keycode) {
                    state.set_button(button, true);
                }
            }
            
            Event::KeyUp { keycode: Some(keycode), .. } => {
                if let Some((button, _)) = keycode_to_button(keycode) {
                    state.set_button(button, false);
                }
            }
            
            _ => {}
        }
    }
    true
}

/// Poll events and return frame input for both players
pub fn poll_frame_input(event_pump: &mut EventPump, current: &mut FrameInput) -> bool {
    for event in event_pump.poll_iter() {
        match event {
            Event::Quit { .. } => return false,
            
            Event::KeyDown { keycode: Some(keycode), .. } => {
                if keycode == Keycode::Escape {
                    return false;
                }
                if let Some((button, _)) = keycode_to_button(keycode) {
                    current.p1.set_button(button, true);
                }
            }
            
            Event::KeyUp { keycode: Some(keycode), .. } => {
                if let Some((button, _)) = keycode_to_button(keycode) {
                    current.p1.set_button(button, false);
                }
            }
            
            _ => {}
        }
    }
    true
}

/// Convert RGB565 framebuffer to RGB24 for SDL2
/// VDP outputs RGB565: RRRRR GGGGGG BBBBB
/// SDL2 expects RGB24: RRRRRRRR GGGGGGGG BBBBBBBB
pub fn rgb565_to_rgb24(framebuffer_565: &[u16]) -> Vec<u8> {
    let mut rgb24 = Vec::with_capacity(framebuffer_565.len() * 3);
    
    for &pixel in framebuffer_565 {
        // Extract RGB565 components
        let r5 = ((pixel >> 11) & 0x1F) as u8;
        let g6 = ((pixel >> 5) & 0x3F) as u8;
        let b5 = (pixel & 0x1F) as u8;
        
        // Scale to 8-bit (replicate upper bits to fill lower bits)
        let r8 = (r5 << 3) | (r5 >> 2);
        let g8 = (g6 << 2) | (g6 >> 4);
        let b8 = (b5 << 3) | (b5 >> 2);
        
        rgb24.push(r8);
        rgb24.push(g8);
        rgb24.push(b8);
    }
    
    rgb24
}

/// Frontend window manager
pub struct Frontend {
    pub sdl_context: sdl2::Sdl,
    pub video_subsystem: sdl2::VideoSubsystem,
    pub canvas: sdl2::render::Canvas<sdl2::video::Window>,
    pub event_pump: EventPump,
    pub texture_creator: sdl2::render::TextureCreator<sdl2::video::WindowContext>,
    pub running: bool,
    pub paused: bool,
    /// Current live input state
    pub live_input: FrameInput,
    /// Audio device (kept alive to maintain audio callback)
    _audio_device: Option<sdl2::audio::AudioDevice<AudioCallback>>,
    /// Shared audio buffer for sample transfer
    pub audio_buffer: crate::audio::SharedAudioBuffer,
}

/// SDL2 audio callback wrapper
pub struct AudioCallback {
    buffer: crate::audio::SharedAudioBuffer,
}

impl sdl2::audio::AudioCallback for AudioCallback {
    type Channel = i16;
    
    fn callback(&mut self, out: &mut [i16]) {
        crate::audio::audio_callback(&self.buffer, out);
    }
}

impl Frontend {
    /// Create a new frontend window
    pub fn new(title: &str, scale: u32) -> Result<Self, String> {
        let sdl_context = sdl2::init()?;
        let video_subsystem = sdl_context.video()?;
        
        let window = video_subsystem
            .window(title, GENESIS_WIDTH * scale, GENESIS_HEIGHT * scale)
            .position_centered()
            .resizable()
            .build()
            .map_err(|e| e.to_string())?;
        
        let canvas = window
            .into_canvas()
            .accelerated()
            .present_vsync()
            .build()
            .map_err(|e| e.to_string())?;
        
        let event_pump = sdl_context.event_pump()?;
        let texture_creator = canvas.texture_creator();
        
        // Initialize audio
        let audio_buffer = crate::audio::create_audio_buffer();
        let audio_device = Self::init_audio(&sdl_context, audio_buffer.clone())?;
        
        Ok(Self {
            sdl_context,
            video_subsystem,
            canvas,
            event_pump,
            texture_creator,
            running: true,
            paused: false,
            live_input: FrameInput::default(),
            _audio_device: Some(audio_device),
            audio_buffer,
        })
    }
    
    /// Initialize SDL2 audio device
    fn init_audio(sdl: &sdl2::Sdl, buffer: crate::audio::SharedAudioBuffer) -> Result<sdl2::audio::AudioDevice<AudioCallback>, String> {
        let audio_subsystem = sdl.audio()?;
        
        let desired_spec = sdl2::audio::AudioSpecDesired {
            freq: Some(crate::audio::SAMPLE_RATE as i32),
            channels: Some(2), // Stereo
            samples: Some(1024), // Buffer size
        };
        
        let device = audio_subsystem.open_playback(None, &desired_spec, |_spec| {
            AudioCallback { buffer }
        })?;
        
        // Start playback
        device.resume();
        
        Ok(device)
    }
    
    /// Poll events and update state
    pub fn poll_events(&mut self) -> bool {
        self.running = poll_frame_input(&mut self.event_pump, &mut self.live_input);
        self.running
    }
    
    /// Get current frame input (for live play mode)
    pub fn get_input(&self) -> FrameInput {
        self.live_input.clone()
    }
    
    /// Render a frame buffer to the window
    pub fn render_frame(&mut self, framebuffer: &[u8]) -> Result<(), String> {
        let mut texture = self.texture_creator
            .create_texture_streaming(PixelFormatEnum::RGB24, GENESIS_WIDTH, GENESIS_HEIGHT)
            .map_err(|e| e.to_string())?;
        
        texture.update(None, framebuffer, (GENESIS_WIDTH * 3) as usize)
            .map_err(|e| e.to_string())?;
        
        self.canvas.clear();
        self.canvas.copy(&texture, None, None)?;
        self.canvas.present();
        
        Ok(())
    }
    
    /// Present a blank/placeholder frame
    pub fn present_blank(&mut self) {
        self.canvas.set_draw_color(sdl2::pixels::Color::RGB(0, 0, 0));
        self.canvas.clear();
        self.canvas.present();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_keycode_mapping() {
        assert_eq!(keycode_to_button(Keycode::Z), Some(("a", true)));
        assert_eq!(keycode_to_button(Keycode::X), Some(("b", true)));
        assert_eq!(keycode_to_button(Keycode::Up), Some(("up", true)));
    }
}
