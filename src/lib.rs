//! Genteel - An instrumentable Sega Mega Drive/Genesis emulator
//!
//! This library provides the core emulation components for the Genesis.

pub mod apu;
pub mod audio;
pub mod cpu;
pub mod debugger;
pub mod frontend;
pub mod input;
pub mod io;
pub mod memory;
pub mod vdp;
pub mod z80;

pub use audio::{AudioBuffer, SharedAudioBuffer, create_audio_buffer};
pub use cpu::Cpu;
pub use input::{InputManager, InputScript};
pub use memory::Memory;
pub use z80::Z80;
