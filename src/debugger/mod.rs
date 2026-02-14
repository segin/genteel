use serde_json::Value;

pub mod gdb;

pub use gdb::{GdbMemory, GdbRegisters, GdbServer, StopReason, DEFAULT_PORT};

/// A trait for components that can be debugged.
pub trait Debuggable {
    /// Reads the component's state and returns it as a JSON value.
    fn read_state(&self) -> Value;

    /// Writes the component's state from a JSON value.
    fn write_state(&mut self, state: &Value);
}
