//! Genteel - An instrumentable Sega Mega Drive/Genesis emulator
//!
//! This library provides the core emulation components for the Genesis.

pub mod cpu;
pub mod apu;
pub mod vdp;
pub mod memory;
pub mod io;
pub mod z80;
pub mod debugger;
pub mod input;
pub mod frontend;
pub mod audio;

pub use cpu::Cpu;
pub use memory::Memory;
pub use z80::Z80;
pub use input::{InputManager, InputScript};
pub use audio::{AudioBuffer, SharedAudioBuffer, create_audio_buffer};

