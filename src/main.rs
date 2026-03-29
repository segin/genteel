#![deny(warnings)]
/// Graceful println that ignores broken pipe errors (for `| head` usage)
#[allow(unused_macros)]
macro_rules! println_safe {
    ($($arg:tt)*) => {{
        use std::io::Write;
        let _ = writeln!(std::io::stdout(), $($arg)*);
    }};
}
pub mod apu;
pub mod audio;
pub mod cpu;
pub mod debugger;
pub mod frontend;
#[cfg(feature = "gui")]
pub mod gui;
pub mod input;
pub mod io;
pub mod memory;
#[cfg(all(feature = "gui", test))]
pub mod tests_gui;
pub mod vdp;
pub mod wav_writer;
pub mod z80;

pub const SLOT_EXTS: [&str; 10] = ["s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9"];

/// Maximum save state size in bytes (25MB) to prevent OOM
const MAX_STATE_SIZE: u64 = 25 * 1024 * 1024;

/// Maximum SRAM size in bytes (2MB) to prevent OOM/DoS
const MAX_SRAM_SIZE: u64 = 2 * 1024 * 1024;

use crate::vdp::RenderOps;
use apu::Apu;
use cpu::Cpu;
use debugger::{GdbMemory, GdbRegisters, GdbServer, StopReason};
use frontend::InputMapping;
use input::{InputManager, InputScript};
use memory::bus::Bus;
use memory::{SharedBus, Z80Bus};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::rc::Rc;
use z80::Z80;

#[derive(Debug, Clone, Copy)]
struct Z80Change {
    new_req: bool,
    new_rst: bool,
}
struct SystemContext<'a> {
    cpu: &'a mut Cpu,
    bus: &'a mut Bus,
    z80: &'a mut Z80<Z80Bus, Z80Bus>,
    z80_cycle_debt: &'a mut f32,
    apu_cycle_debt: &'a mut f32,
    z80_last_bus_req: &'a mut bool,
    z80_last_reset: &'a mut bool,
    z80_trace_count: &'a mut u32,
    internal_frame_count: u64,
    debug: bool,
}

#[derive(Debug, Clone, Copy)]
struct CpuBatchResult {
    cycles: u32,
    z80_change: Option<Z80Change>,
}

mod shared_bus_serde {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(bus: &Rc<RefCell<Bus>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use crate::memory::SharedBus;
        SharedBus::new(bus.clone()).serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Rc<RefCell<Bus>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        use crate::memory::SharedBus;
        let shared = SharedBus::deserialize(deserializer)?;
        Ok(shared.bus)
    }
}

#[derive(Serialize, Deserialize)]
pub struct Emulator {
    pub cpu: Cpu,
    pub z80: Z80<Z80Bus, Z80Bus>,
    pub apu: Apu,
    #[serde(with = "shared_bus_serde")]
    pub bus: Rc<RefCell<Bus>>,
    pub input: InputManager,
    pub audio_buffer: Vec<i16>,
    #[serde(skip)]
    pub wav_writer: Option<wav_writer::FileWavWriter>,
    pub internal_frame_count: u64,
    pub z80_last_bus_req: bool,
    pub z80_last_reset: bool,
    pub z80_trace_count: u32,
    pub input_mapping: InputMapping,
    pub debug: bool,
    pub paused: bool,
    pub single_step: bool,
    #[serde(skip)]
    pub gdb: Option<GdbServer>,
    pub allowed_paths: Vec<std::path::PathBuf>,
    pub current_rom_path: Option<std::path::PathBuf>,
    pub z80_cycle_debt: f32,
    pub apu_cycle_debt: f32,
}
impl Default for Emulator {
    fn default() -> Self {
        Self::new()
    }
}
impl Emulator {
    pub fn new() -> Self {
        let bus = Rc::new(RefCell::new(Bus::new()));
        // M68k uses SharedBus wrapper for main Genesis bus access
        let mut bus_ref = bus.borrow_mut();
        let cpu = Cpu::new(&mut *bus_ref);
        drop(bus_ref);
        // Z80 uses Z80Bus which routes to sound chips and banked 68k memory
        // It also handles Z80 I/O (which is unconnected on Genesis)
        let z80_bus = Z80Bus::new(SharedBus::new(bus.clone()));
        let z80 = Z80::new(z80_bus.clone(), z80_bus);
        let mut emulator = Self {
            cpu,
            z80,
            apu: Apu::new(),
            bus,
            input: InputManager::new(),
            audio_buffer: Vec::with_capacity(crate::audio::samples_per_frame() * 2),
            wav_writer: None,
            internal_frame_count: 0,
            z80_last_bus_req: false,
            z80_last_reset: true,
            z80_trace_count: 0,
            input_mapping: InputMapping::default(),
            debug: false,
            paused: false,
            single_step: false,
            gdb: None,
            current_rom_path: None,
            allowed_paths: Vec::new(),
            z80_cycle_debt: 0.0,
            apu_cycle_debt: 0.0,
        };
        {
            let mut bus = emulator.bus.borrow_mut();
            emulator.cpu.reset(&mut *bus);
        }
        emulator.z80.reset();
        emulator
    }

    /// Hard reset of the emulator (clears RAM, VRAM, resets CPUs, keeps ROM)
    pub fn hard_reset(&mut self) {
        {
            let mut bus = self.bus.borrow_mut();
            bus.reset();
            self.cpu.reset(&mut *bus);
        }
        self.z80.reset();
        self.internal_frame_count = 0;
    }

    /// Close current ROM and return to default state
    pub fn close_rom(&mut self) {
        self.save_sram();

        let allowed_paths = self.allowed_paths.clone();
        let mapping = self.input_mapping;
        let sample_rate = self.bus.borrow().sample_rate;

        *self = Self::new();

        self.allowed_paths = allowed_paths;
        self.input_mapping = mapping;
        self.bus.borrow_mut().sample_rate = sample_rate;
    }

    pub fn load_sram(&mut self) {
        let Some(path) = &self.current_rom_path else {
            return;
        };
        let sram_path = path.with_extension("srm");
        let Ok(mut file) = std::fs::File::open(&sram_path) else {
            return;
        };

        // 1. Check metadata size
        if let Ok(metadata) = file.metadata() {
            if metadata.len() > MAX_SRAM_SIZE {
                eprintln!("SRAM file too large: {} bytes", metadata.len());
                return;
            }
        }

        // 2. Read with limit to prevent OOM
        use std::io::Read;
        let mut data = Vec::new();
        if let Err(e) = file.take(MAX_SRAM_SIZE + 1).read_to_end(&mut data) {
            eprintln!("Failed to read SRAM: {}", e);
            return;
        }

        if data.len() as u64 > MAX_SRAM_SIZE {
            eprintln!("SRAM exceeds size limit");
            return;
        }

        println!("Loading SRAM from {:?}", sram_path);
        let mut bus = self.bus.borrow_mut();
        if data.len() == bus.sram.len() {
            bus.sram.copy_from_slice(&data);
        } else {
            let len = data.len().min(bus.sram.len());
            bus.sram[..len].copy_from_slice(&data[..len]);
        }
    }

    pub fn save_sram(&self) {
        let Some(path) = &self.current_rom_path else {
            return;
        };
        let bus = self.bus.borrow();
        if !bus.sram.is_empty() {
            let sram_path = path.with_extension("srm");
            if let Err(e) = std::fs::write(&sram_path, &*bus.sram) {
                eprintln!("Failed to save SRAM to {:?}: {}", sram_path, e);
            } else {
                println!("Saved SRAM to {:?}", sram_path);
            }
        }
    }

    pub fn save_state(&self, slot: u8) {
        let Some(path) = &self.current_rom_path else {
            return;
        };
        let state_path = path.with_extension(SLOT_EXTS[slot as usize]);

        self.save_state_to_path(state_path);
    }

    pub fn save_state_to_path(&self, state_path: std::path::PathBuf) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            if let Err(e) = std::fs::write(&state_path, json) {
                eprintln!("Failed to save state to {:?}: {}", state_path, e);
            } else {
                println!("Saved state to {:?}", state_path);
            }
        }
    }

    pub fn load_state(&mut self, slot: u8) {
        let Some(path) = &self.current_rom_path else {
            return;
        };
        let state_path = path.with_extension(SLOT_EXTS[slot as usize]);

        self.load_state_from_path(state_path);
    }

    pub fn delete_state(&self, slot: u8) {
        let Some(path) = &self.current_rom_path else {
            return;
        };
        let state_path = path.with_extension(SLOT_EXTS[slot as usize]);

        if state_path.exists() {
            if let Err(e) = std::fs::remove_file(&state_path) {
                eprintln!("Failed to delete state {:?}: {}", state_path, e);
            } else {
                println!("Deleted state {:?}", state_path);
            }
        }
    }

    pub fn load_state_from_path(&mut self, state_path: std::path::PathBuf) {
        let Ok(file) = std::fs::File::open(&state_path) else {
            return;
        };

        // 1. Check metadata size
        if let Ok(metadata) = file.metadata() {
            if metadata.len() > MAX_STATE_SIZE {
                eprintln!("Save state too large: {} bytes", metadata.len());
                return;
            }
        }

        // 2. Read with limit to prevent OOM
        use std::io::Read;
        let mut json = String::new();
        if let Err(e) = file.take(MAX_STATE_SIZE + 1).read_to_string(&mut json) {
            eprintln!("Failed to read save state: {}", e);
            return;
        }

        if json.len() as u64 > MAX_STATE_SIZE {
            eprintln!("Save state exceeds size limit");
            return;
        }

        match serde_json::from_str::<Self>(&json) {
            Ok(new_emulator) => {
                // 1. Preserve critical session state
                let gdb = self.gdb.take();
                let allowed_paths = self.allowed_paths.clone();
                let current_rom_path = self.current_rom_path.clone();
                let sample_rate = self.bus.borrow().sample_rate;

                // 2. Load ROM data into the new emulator's bus
                if let Some(ref rom_path) = current_rom_path {
                    if let Ok(data) = std::fs::read(rom_path) {
                        let mut bus = new_emulator.bus.borrow_mut();
                        bus.load_rom(&data);
                    }
                }

                // 3. Apply the new state
                *self = new_emulator;

                // 4. Restore critical session state
                self.gdb = gdb;
                self.allowed_paths = allowed_paths;
                self.current_rom_path = current_rom_path;
                self.bus.borrow_mut().sample_rate = sample_rate;

                println!("Loaded state from {:?}", state_path);
            }
            Err(e) => eprintln!("Failed to parse save state: {}", e),
        }
    }

    /// Add a path to the whitelist of allowed ROM directories.
    /// If no paths are added, `load_rom` will fail (secure by default).
    /// The path is canonicalized before addition.
    pub fn add_allowed_path<P: AsRef<std::path::Path>>(&mut self, path: P) -> std::io::Result<()> {
        let canonical = path.as_ref().canonicalize()?;
        self.allowed_paths.push(canonical);
        Ok(())
    }

    pub fn load_rom(&mut self, path: &str) -> std::io::Result<()> {
        // Security: Validate path against whitelist
        let path_obj = std::path::Path::new(path);
        let canonical_path = path_obj.canonicalize()?;

        if self.allowed_paths.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "ROM loading is restricted. No allowed paths configured.",
            ));
        }

        let allowed = self
            .allowed_paths
            .iter()
            .any(|allowed_base| canonical_path.starts_with(allowed_base));

        if !allowed {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("Access denied to ROM path: {:?}", canonical_path),
            ));
        }

        let data = if path.to_lowercase().ends_with(".zip") {
            // Extract ROM from zip file
            Self::load_rom_from_zip(path)?
        } else {
            let mut file = std::fs::File::open(path)?;
            let metadata = file.metadata()?;
            let size = metadata.len();
            Self::read_rom_with_limit(&mut file, size)?
        };
        let mut bus = self.bus.borrow_mut();
        bus.load_rom(&data);
        drop(bus);

        self.current_rom_path = Some(canonical_path);
        self.load_sram();

        // Reset again to load initial PC/SP from ROM vectors
        let mut bus = self.bus.borrow_mut();
        self.cpu.reset(&mut *bus);
        self.z80.reset();
        Ok(())
    }
    fn read_rom_with_limit<R: std::io::Read>(
        reader: &mut R,
        size: u64,
    ) -> std::io::Result<Vec<u8>> {
        use std::io::Read;
        const MAX_ROM_SIZE: u64 = 32 * 1024 * 1024; // 32 MB
        if size > MAX_ROM_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("ROM size {} exceeds limit of {} bytes", size, MAX_ROM_SIZE),
            ));
        }
        // Check if size fits in usize (for 32-bit/16-bit systems)
        if size > usize::MAX as u64 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "ROM size too large for memory address space",
            ));
        }
        let mut data = Vec::with_capacity(size as usize);
        reader.take(MAX_ROM_SIZE + 1).read_to_end(&mut data)?;
        if data.len() as u64 > MAX_ROM_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Decompressed ROM size exceeds limit",
            ));
        }
        Ok(data)
    }
    /// Load ROM from a zip file (finds first .bin, .md, .gen, or .smd file)
    fn load_rom_from_zip(path: &str) -> std::io::Result<Vec<u8>> {
        let file = std::fs::File::open(path)?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        // ROM file extensions to look for
        let rom_extensions = [".bin", ".md", ".gen", ".smd", ".32x"];
        // Find first ROM file in archive
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

            let name = entry.name();
            // Case-insensitive check without allocating strings
            let is_rom = rom_extensions.iter().any(|&ext| {
                name.len() >= ext.len() && name[name.len() - ext.len()..].eq_ignore_ascii_case(ext)
            });

            if is_rom {
                let size = entry.size();
                if size > 32 * 1024 * 1024 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "ROM size exceeds limit of 32MB",
                    ));
                }
                let data = Self::read_rom_with_limit(&mut entry, size)?;
                println!("Extracted ROM: {} ({} bytes)", entry.name(), data.len());
                return Ok(data);
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No ROM file found in zip archive",
        ))
    }
    /// Step one frame with current input state
    pub fn step_frame(&mut self, input: Option<&input::FrameInput>) {
        if self.paused && !self.single_step {
            return;
        }
        // Reset single_step if it was set
        self.single_step = false;

        // Apply inputs from script or live input
        let (p1, p2, command) = {
            let frame_input = match input {
                Some(i) => {
                    self.input.record((*i).clone());
                    std::borrow::Cow::Borrowed(i)
                }
                None => self.input.advance_frame(),
            };
            (frame_input.p1, frame_input.p2, frame_input.command.clone())
        };

        {
            let mut bus = self.bus.borrow_mut();
            if let Some(ctrl) = bus.io.controller(1) {
                *ctrl = p1;
            }
            if let Some(ctrl) = bus.io.controller(2) {
                *ctrl = p2;
            }
        }

        // Handle commands (e.g., SCREENSHOT <path>)
        if let Some(cmd) = &command {
            self.execute_script_command(cmd);
        }

        self.step_frame_internal();
    }

    fn execute_script_command(&self, cmd: &str) {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() {
            return;
        }

        let cmd_upper = parts[0].to_uppercase();
        match cmd_upper.as_str() {
            "SCREENSHOT" => self.handle_screenshot_cmd(&parts),
            "READ_BYTE" | "WRITE_BYTE" | "ASSERT_BYTE" => {
                self.handle_byte_cmd(cmd_upper.as_str(), &parts)
            }
            "READ_WORD" | "WRITE_WORD" | "ASSERT_WORD" => {
                self.handle_word_cmd(cmd_upper.as_str(), &parts)
            }
            "READ_LONG" | "WRITE_LONG" | "ASSERT_LONG" => {
                self.handle_long_cmd(cmd_upper.as_str(), &parts)
            }
            "LOG" => self.handle_log_cmd(&parts),
            _ => {
                eprintln!("Script Warning: Unknown command '{}'", parts[0]);
            }
        }
    }

    fn handle_screenshot_cmd(&self, parts: &[&str]) {
        if parts.len() > 1 {
            let raw_path = parts[1];
            // Security: Sanitize path to prevent arbitrary file writes
            // Only allow saving to current directory by using only the file name component
            let path = std::path::Path::new(raw_path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("screenshot.png");

            if path != raw_path {
                eprintln!(
                    "Script Warning: Sanitized screenshot path '{}' to '{}'",
                    raw_path, path
                );
            }

            if let Err(e) = self.save_screenshot(path) {
                eprintln!("Script Error: Failed to save screenshot to {}: {}", path, e);
            } else {
                println!("Script: Saved screenshot to {}", path);
            }
        }
    }

    fn handle_byte_cmd(&self, cmd: &str, parts: &[&str]) {
        match cmd {
            "READ_BYTE" => {
                if parts.len() > 1 {
                    if let Ok(addr) = u32::from_str_radix(parts[1].trim_start_matches("0x"), 16) {
                        let val = self.bus.borrow_mut().read_byte(addr);
                        println!("Script: READ_BYTE 0x{:06X} = 0x{:02X}", addr, val);
                    }
                }
            }
            "WRITE_BYTE" => {
                if parts.len() > 2 {
                    let addr_res = u32::from_str_radix(parts[1].trim_start_matches("0x"), 16);
                    let val_res = u8::from_str_radix(parts[2].trim_start_matches("0x"), 16);
                    if let (Ok(addr), Ok(val)) = (addr_res, val_res) {
                        self.bus.borrow_mut().write_byte(addr, val);
                        println!("Script: WRITE_BYTE 0x{:06X} = 0x{:02X}", addr, val);
                    }
                }
            }
            "ASSERT_BYTE" => {
                if parts.len() > 2 {
                    let addr_res = u32::from_str_radix(parts[1].trim_start_matches("0x"), 16);
                    let val_res = u8::from_str_radix(parts[2].trim_start_matches("0x"), 16);
                    if let (Ok(addr), Ok(expected)) = (addr_res, val_res) {
                        let actual = self.bus.borrow_mut().read_byte(addr);
                        if actual != expected {
                            panic!("Script Assertion Failed: [0x{:06X}] == 0x{:02X} (Expected 0x{:02X})", addr, actual, expected);
                        }
                        println!(
                            "Script: ASSERT_BYTE 0x{:06X} == 0x{:02X} OK",
                            addr, expected
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_word_cmd(&self, cmd: &str, parts: &[&str]) {
        match cmd {
            "READ_WORD" => {
                if parts.len() > 1 {
                    if let Ok(addr) = u32::from_str_radix(parts[1].trim_start_matches("0x"), 16) {
                        let val = self.bus.borrow_mut().read_word(addr);
                        println!("Script: READ_WORD 0x{:06X} = 0x{:04X}", addr, val);
                    }
                }
            }
            "WRITE_WORD" => {
                if parts.len() > 2 {
                    let addr_res = u32::from_str_radix(parts[1].trim_start_matches("0x"), 16);
                    let val_res = u16::from_str_radix(parts[2].trim_start_matches("0x"), 16);
                    if let (Ok(addr), Ok(val)) = (addr_res, val_res) {
                        self.bus.borrow_mut().write_word(addr, val);
                        println!("Script: WRITE_WORD 0x{:06X} = 0x{:04X}", addr, val);
                    }
                }
            }
            "ASSERT_WORD" => {
                if parts.len() > 2 {
                    let addr_res = u32::from_str_radix(parts[1].trim_start_matches("0x"), 16);
                    let val_res = u16::from_str_radix(parts[2].trim_start_matches("0x"), 16);
                    if let (Ok(addr), Ok(expected)) = (addr_res, val_res) {
                        let actual = self.bus.borrow_mut().read_word(addr);
                        if actual != expected {
                            panic!("Script Assertion Failed: [0x{:06X}] == 0x{:04X} (Expected 0x{:04X})", addr, actual, expected);
                        }
                        println!(
                            "Script: ASSERT_WORD 0x{:06X} == 0x{:04X} OK",
                            addr, expected
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_long_cmd(&self, cmd: &str, parts: &[&str]) {
        match cmd {
            "READ_LONG" => {
                if parts.len() > 1 {
                    if let Ok(addr) = u32::from_str_radix(parts[1].trim_start_matches("0x"), 16) {
                        let val = self.bus.borrow_mut().read_long(addr);
                        println!("Script: READ_LONG 0x{:06X} = 0x{:08X}", addr, val);
                    }
                }
            }
            "WRITE_LONG" => {
                if parts.len() > 2 {
                    let addr_res = u32::from_str_radix(parts[1].trim_start_matches("0x"), 16);
                    let val_res = u32::from_str_radix(parts[2].trim_start_matches("0x"), 16);
                    if let (Ok(addr), Ok(val)) = (addr_res, val_res) {
                        self.bus.borrow_mut().write_long(addr, val);
                        println!("Script: WRITE_LONG 0x{:06X} = 0x{:08X}", addr, val);
                    }
                }
            }
            "ASSERT_LONG" => {
                if parts.len() > 2 {
                    let addr_res = u32::from_str_radix(parts[1].trim_start_matches("0x"), 16);
                    let val_res = u32::from_str_radix(parts[2].trim_start_matches("0x"), 16);
                    if let (Ok(addr), Ok(expected)) = (addr_res, val_res) {
                        let actual = self.bus.borrow_mut().read_long(addr);
                        if actual != expected {
                            panic!("Script Assertion Failed: [0x{:06X}] == 0x{:08X} (Expected 0x{:08X})", addr, actual, expected);
                        }
                        println!(
                            "Script: ASSERT_LONG 0x{:06X} == 0x{:08X} OK",
                            addr, expected
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_log_cmd(&self, parts: &[&str]) {
        if parts.len() > 1 {
            println!("Script LOG: {}", parts[1..].join(" "));
        }
    }
    pub fn step_frame_internal(&mut self) {
        let (lines, active_lines) = {
            let bus = self.bus.borrow();
            if bus.vdp.is_pal {
                (313, 240)
            } else {
                (262, 224)
            }
        };
        let samples_per_line =
            audio::samples_per_frame() as f32 / lines as f32;

        for line in 0..lines {
            self.step_scanline(line, active_lines, samples_per_line);
        }
        self.internal_frame_count += 1;
        if self.debug && self.internal_frame_count % 60 == 0 {
            self.log_debug(self.internal_frame_count);
        }

        self.generate_audio_samples(samples_per_line);
        self.bus.borrow_mut().vdp.update_v30_offset();
    }
    fn step_scanline(
        &mut self,
        line: u16,
        active_lines: u16,
        _samples_per_line: f32,
    ) {
        self.vdp_scanline_setup(line, active_lines);
        self.run_cpu_loop(line, active_lines);
        self.handle_interrupts();
    }
    fn vdp_scanline_setup(&mut self, line: u16, _active_lines: u16) {
        let mut bus = self.bus.borrow_mut();

        // Process scanline if within framebuffer bounds (320x240)
        if line < 240 {
            bus.vdp.render_line(line);
        }
    }

    fn sync_components(
        ctx: &mut SystemContext,
        m68k_cycles: u32,
        trigger_vint: bool,
    ) {
        let mclk = m68k_cycles * 7;

        // 1. Tick the Bus (VDP, etc)
        ctx.bus.tick(mclk);

        // 2. Z80 State and Timing
        let (z80_can_run, z80_is_reset, cycles_per_sample) = {
            let prev = *ctx.z80_last_bus_req;
            if ctx.debug && ctx.bus.z80_bus_request != prev {
                log::debug!(
                    "Bus Req Changed: {} -> {} at 68k PC={:06X}",
                    prev,
                    ctx.bus.z80_bus_request,
                    ctx.cpu.pc
                );
            }
            *ctx.z80_last_bus_req = ctx.bus.z80_bus_request;

            let z80_can_run = !ctx.bus.z80_reset && !ctx.bus.z80_bus_request;
            let z80_is_reset = ctx.bus.z80_reset;
            let cycles_per_sample = 7670453.0 / (ctx.bus.sample_rate as f32);
            (z80_can_run, z80_is_reset, cycles_per_sample)
        };

        // Handle Z80 Reset
        if z80_is_reset && !*ctx.z80_last_reset {
            ctx.z80.reset();
        }
        *ctx.z80_last_reset = z80_is_reset;

        // Trace Z80 if debugging
        if z80_can_run && ctx.internal_frame_count > 0 {
            ctx.z80.debug = ctx.debug && *ctx.z80_trace_count < 5000;
            if ctx.z80.debug {
                *ctx.z80_trace_count += 1;
            }
        } else {
            ctx.z80.debug = false;
        }

        // Trigger Z80 VInt if requested
        if trigger_vint && !z80_is_reset {
            ctx.z80.trigger_interrupt(0xFF);
        }

        // 3. Catch up Z80
        if z80_can_run {
            const Z80_CYCLES_PER_M68K_CYCLE: f32 = 3579545.0 / 7670453.0;
            *ctx.z80_cycle_debt += m68k_cycles as f32 * Z80_CYCLES_PER_M68K_CYCLE;
            // Bind the bus to avoid RefCell double borrow
            unsafe {
                ctx.z80.memory.bind_bus(ctx.bus);
                ctx.z80.io.bind_bus(ctx.bus);


            }

            while *ctx.z80_cycle_debt >= 1.0 {
                let cycles = ctx.z80.step();
                *ctx.z80_cycle_debt -= cycles as f32;
            }

            // Unbind to return to SharedBus mode
            ctx.z80.memory.unbind_bus();
            ctx.z80.io.unbind_bus();

        }

        // 4. Update APU and generate audio samples
        {
            ctx.bus.apu.tick_cycles(m68k_cycles);
            ctx.bus.audio_accumulator += m68k_cycles as f32;

            while ctx.bus.audio_accumulator >= cycles_per_sample {
                let (l, r) = ctx.bus.apu.generate_sample();
                if ctx.bus.audio_buffer.len() < 32768 {
                    ctx.bus.audio_buffer.push(l);
                    ctx.bus.audio_buffer.push(r);
                }
                ctx.bus.audio_accumulator -= cycles_per_sample;
            }
        }
    }

    /// Optimized synchronization that only catches up the Z80 and APU without VDP ticking.
    fn sync_audio_z80(ctx: &mut SystemContext, m68k_cycles: u32) {
        let cycles_per_sample = 7670453.0 / (ctx.bus.sample_rate as f32);

        // 1. Z80 State
        let z80_can_run = !ctx.bus.z80_reset && !ctx.bus.z80_bus_request;

        // 2. Catch up Z80
        if z80_can_run {
            const Z80_CYCLES_PER_M68K_CYCLE: f32 = 3579545.0 / 7670453.0;
            *ctx.z80_cycle_debt += m68k_cycles as f32 * Z80_CYCLES_PER_M68K_CYCLE;

            unsafe {
                ctx.z80.memory.bind_bus(ctx.bus);
                ctx.z80.io.bind_bus(ctx.bus);
            }

            while *ctx.z80_cycle_debt >= 1.0 {
                let cycles = ctx.z80.step();
                *ctx.z80_cycle_debt -= cycles as f32;

                // Sync APU with Z80 progress using accumulator to avoid drift
                let equivalent_m68k = cycles as f32 / Z80_CYCLES_PER_M68K_CYCLE;
                *ctx.apu_cycle_debt += equivalent_m68k;
                let steps = *ctx.apu_cycle_debt as u32;
                if steps > 0 {
                    ctx.bus.apu.tick_cycles(steps);
                    *ctx.apu_cycle_debt -= steps as f32;
                }
            }

            ctx.z80.memory.unbind_bus();
            ctx.z80.io.unbind_bus();
        } else {
             // Z80 is stopped, just tick APU directly
             *ctx.apu_cycle_debt += m68k_cycles as f32;
             let steps = *ctx.apu_cycle_debt as u32;
             if steps > 0 {
                 ctx.bus.apu.tick_cycles(steps);
                 *ctx.apu_cycle_debt -= steps as f32;
             }
        }

        // 3. Audio buffering
        ctx.bus.audio_accumulator += m68k_cycles as f32;
        while ctx.bus.audio_accumulator >= cycles_per_sample {
            let (l, r) = ctx.bus.apu.generate_sample();
            if ctx.bus.audio_buffer.len() < 32768 {
                ctx.bus.audio_buffer.push(l);
                ctx.bus.audio_buffer.push(r);
            }
            ctx.bus.audio_accumulator -= cycles_per_sample;
        }
    }

    fn run_cpu_batch_static(
        ctx: &mut SystemContext,
        max_cycles: u32,
        line: u16,
        active_lines: u16,
    ) -> CpuBatchResult {
        let (initial_req, initial_rst) = (ctx.bus.z80_bus_request, ctx.bus.z80_reset);
        let mut pending_cycles = 0;
        loop {
            if pending_cycles >= max_cycles {
                // Final sync for the batch
                let trigger_vint = line == active_lines && pending_cycles < 10;
                Self::sync_components(ctx, 0, trigger_vint);
                return CpuBatchResult {
                    cycles: pending_cycles,
                    z80_change: None,
                };
            }

            let m68k_cycles = if ctx.bus.dma_active() {
                2 // Yield 2 cycles to let the bus step during DMA
            } else {
                ctx.cpu.step_instruction(ctx.bus)
            };

            // Instead of full sync, we tick the VDP and only catch up sensitive components if needed.
            // Performance: Tick VDP with mclk
            ctx.bus.tick(m68k_cycles * 7);

            // Audio and Z80 sync: we can batch this a bit more than 1 instruction
            // but for DAC accuracy we still need regular updates.
            // Let's at least avoid the full sync_components (VDP overhead)
            Self::sync_audio_z80(ctx, m68k_cycles);

            // Real Genesis uses level-triggered interrupts.
            if ctx.cpu.pending_interrupt == 6 && !ctx.bus.vdp.vblank_pending() {
                ctx.cpu.cancel_interrupt(6);
            }

            // Check for Z80 state change (rare but needs early exit)
            let (req, rst) = (ctx.bus.z80_bus_request, ctx.bus.z80_reset);
            if req != initial_req || rst != initial_rst {
                return CpuBatchResult {
                    cycles: pending_cycles + m68k_cycles,
                    z80_change: Some(Z80Change {
                        new_req: req,
                        new_rst: rst,
                    }),
                };
            }

            pending_cycles += m68k_cycles;
        }
    }
    fn run_cpu_loop(&mut self, line: u16, active_lines: u16) {
        const CYCLES_PER_LINE: u32 = 488;
        let mut cycles_scanline: u32 = 0;
        let mut bus = self.bus.borrow_mut();
 
        // One context for the entire scanline loop
        let mut ctx = SystemContext {
            cpu: &mut self.cpu,
            bus: &mut *bus,
            z80: &mut self.z80,
            z80_cycle_debt: &mut self.z80_cycle_debt,
            apu_cycle_debt: &mut self.apu_cycle_debt,
            z80_last_bus_req: &mut self.z80_last_bus_req,
            z80_last_reset: &mut self.z80_last_reset,
            z80_trace_count: &mut self.z80_trace_count,
            internal_frame_count: self.internal_frame_count,
            debug: self.debug,
        };

        while cycles_scanline < CYCLES_PER_LINE {
            let remaining = CYCLES_PER_LINE - cycles_scanline;
            // Batch size of remaining line
            let result = Self::run_cpu_batch_static(&mut ctx, remaining, line, active_lines);

            cycles_scanline += result.cycles;

            if let Some(change) = result.z80_change {
                ctx.bus.z80_bus_request = change.new_req;
                ctx.bus.z80_reset = change.new_rst;
            }
        }
    }
    fn generate_audio_samples(&mut self, _samples_per_line: f32) {
        let mut bus = self.bus.borrow_mut();
        if bus.audio_buffer.is_empty() {
            return;
        }
        if let Some(writer) = &mut self.wav_writer {
            let _ = writer.write_samples(&bus.audio_buffer);
        }
        // Move samples to emulator buffer for frontend consumption
        if self.audio_buffer.len() < audio::samples_per_frame() * 2 {
            self.audio_buffer.extend(bus.audio_buffer.iter());
        }
        bus.audio_buffer.clear();
    }
    fn handle_interrupts(&mut self) {
        // H-Blank only lasts for a short period at the end of a scanline.
        // If the CPU hasn't taken the H-Int by the end of the subsequent scanline
        // (e.g., because it was handling V-Int or had interrupts masked), the VDP
        // will have dropped the IRQ line. We must cancel it here to prevent spurious interrupts.
        self.cpu.cancel_interrupt(4);

        let bus = self.bus.borrow_mut();
        if bus.vdp.vblank_pending() {
            self.cpu.request_interrupt(6);
        }
        if bus.vdp.hint_pending() {
            self.cpu.request_interrupt(4);
        }
        drop(bus);

        // Update audio visualization once per frame instead of per-sample
        self.bus.borrow_mut().apu.update_visualization();
    }
    /// Run headless for N frames (or until script ends if N is None)
    pub fn run(
        &mut self,
        frames: Option<u32>,
        screenshot_path: Option<String>,
        record_path: Option<String>,
    ) {
        match frames {
            Some(n) => println!("Running {} frames headless...", n),
            None => println!("Running headless until script ends..."),
        }

        if let Some(path) = &record_path {
            println!("Recording inputs to: {}", path);
            self.input.start_recording();
        }

        let start_time = std::time::Instant::now();
        let mut current = 0;
        loop {
            if let Some(n) = frames {
                if current >= n {
                    break;
                }
            } else if self.input.is_complete() {
                // Only stop if a script was actually loaded and it's done
                break;
            }

            self.step_frame(None);
            // Clear audio buffer in headless mode to prevent memory leak
            self.audio_buffer.clear();
            current += 1;

            // Log every 600 frames (approx 10 seconds)
            if self.debug && current % 600 == 0 {
                self.log_debug(current as u64);
            }
        }

        let elapsed = start_time.elapsed();
        if let Some(path) = screenshot_path {
            if let Err(e) = self.save_screenshot(&path) {
                eprintln!("Failed to save final screenshot: {}", e);
            } else {
                println!("Final screenshot saved to: {}", path);
            }
        }

        if let Some(path) = record_path {
            let script: InputScript = self.input.stop_recording();
            if let Err(e) = script.save(&path) {
                eprintln!("Failed to save recorded script: {}", e);
            } else {
                println!("Recorded script saved to: {}", path);
            }
        }

        println!(
            "Done in {:?} ({:.2} fps).",
            elapsed,
            current as f64 / elapsed.as_secs_f64()
        );
    }

    pub fn save_screenshot(&self, path: &str) -> Result<(), String> {
        let bus = self.bus.borrow();
        let fb = &bus.vdp.framebuffer;
        let mut rgb_data = Vec::with_capacity(fb.len() * 3);
        for &pixel in fb {
            let r5 = ((pixel >> 11) & 0x1F) as u8;
            let g6 = ((pixel >> 5) & 0x3F) as u8;
            let b5 = (pixel & 0x1F) as u8;
            rgb_data.push((r5 << 3) | (r5 >> 2));
            rgb_data.push((g6 << 2) | (g6 >> 4));
            rgb_data.push((b5 << 3) | (b5 >> 2));
        }
        image::save_buffer(path, &rgb_data, 320, 240, image::ExtendedColorType::Rgb8)
            .map_err(|e| e.to_string())
    }
    /// Poll GDB for commands and update state
    pub fn poll_gdb(&mut self) {
        let Some(gdb) = &mut self.gdb else { return };

        if !gdb.is_connected() {
            if gdb.accept() {
                println!("GDB client connected");
            } else {
                return;
            }
        }

        let mut mem_access = BusGdbMemory { bus: &self.bus };
        while let Some(cmd) = gdb.receive_packet() {
            let mut regs = GdbRegisters {
                d: self.cpu.d,
                a: self.cpu.a,
                sr: self.cpu.sr,
                pc: self.cpu.pc,
            };
            let response = gdb.process_command(&cmd, &mut regs, &mut mem_access);
            self.cpu.d = regs.d;
            self.cpu.a = regs.a;
            self.cpu.sr = regs.sr;
            self.cpu.pc = regs.pc;

            match response.as_str() {
                "CONTINUE" => {
                    self.paused = false;
                }
                "STEP" => {
                    self.single_step = true;
                    self.paused = false;
                }
                _ if !response.is_empty() => {
                    gdb.send_packet(&response).ok();
                }
                _ => {}
            }
        }
    }

    /// Run with GDB debugger attached (blocking loop)
    pub fn run_with_gdb(&mut self, port: u16, password: Option<String>) -> std::io::Result<()> {
        let gdb = GdbServer::new(port, password.clone())?;
        self.gdb = Some(gdb);
        let gdb = self.gdb.as_mut().unwrap();
        println!("Waiting for GDB connection on port {}...", port);
        if let Some(pwd) = password {
            println!(
                "🔒 Password protected. After connecting, run: monitor auth {}",
                pwd
            );
        }
        println!(
            "Connect with: m68k-elf-gdb -ex \"target remote :{}\" <elf_file>",
            port
        );
        // Wait for connection
        while !gdb.accept() {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let mut stepping = false;
        let mut running = false;
        let mut mem_access = BusGdbMemory { bus: &self.bus };
        loop {
            // Check for GDB commands
            if let Some(cmd) = gdb.receive_packet() {
                // Build register state from CPU
                let mut regs = GdbRegisters {
                    d: self.cpu.d,
                    a: self.cpu.a,
                    sr: self.cpu.sr,
                    pc: self.cpu.pc,
                };
                let response = gdb.process_command(&cmd, &mut regs, &mut mem_access);
                // Apply register changes back to CPU
                self.cpu.d = regs.d;
                self.cpu.a = regs.a;
                self.cpu.sr = regs.sr;
                self.cpu.pc = regs.pc;
                match response.as_str() {
                    "CONTINUE" => {
                        running = true;
                        stepping = false;
                    }
                    "STEP" => {
                        stepping = true;
                        running = true;
                    }
                    _ if !response.is_empty() => {
                        gdb.send_packet(&response).ok();
                    }
                    _ => {}
                }
            }
            // Execute if running
            if running {
                // Step one instruction
                let mut bus = self.bus.borrow_mut();
                self.cpu.step_instruction(&mut *bus);
                drop(bus);
                // Check for breakpoint
                if gdb.is_breakpoint(self.cpu.pc) {
                    gdb.stop_reason = StopReason::Breakpoint;
                    gdb.send_packet(StopReason::Breakpoint.signal_string()).ok();
                    running = false;
                } else if stepping {
                    gdb.stop_reason = StopReason::Step;
                    gdb.send_packet(StopReason::Step.signal_string()).ok();
                    running = false;
                }
            } else {
                // Not running, sleep a bit to avoid spinning
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            // Check if client disconnected
            if !gdb.is_connected() && !gdb.accept() {
                println!("GDB client disconnected");
                break;
            }
        }
        Ok(())
    }
    pub(crate) fn log_debug(&self, frame_count: u64) {
        let bus = self.bus.borrow();
        let disp_en = if bus.vdp.display_enabled() {
            "ON "
        } else {
            "OFF"
        };
        let dma_en = if bus.vdp.dma_enabled() { "ON " } else { "OFF" };
        let cram_val = if bus.vdp.cram.len() >= 2 {
            ((bus.vdp.cram[0] as u16) << 8) | (bus.vdp.cram[1] as u16)
        } else {
            0
        };
        let z80_pc = self.z80.pc;
        let z80_reset = if bus.z80_reset { "RST" } else { "RUN" };
        let z80_req = if bus.z80_bus_request { "BUS" } else { "---" };
        let z80_op = if (z80_pc as usize) < bus.z80_ram.len() {
            bus.z80_ram[z80_pc as usize]
        } else {
            0
        };
        println_safe!(
            "FRAME {:05} | 68k: PC={:06X} SR={:04X} | VDP: Disp={} DMA={} CRAM={:04X} | Z80: PC={:04X} OP={:02X} St={} Req={}",
            frame_count,
            self.cpu.pc,
            self.cpu.sr,
            disp_en,
            dma_en,
            cram_val,
            z80_pc,
            z80_op,
            z80_reset,
            z80_req
        );
    }

    /// Run with winit window (interactive play mode)
    #[cfg(feature = "gui")]
    pub fn run_with_frontend(self, record_path: Option<String>) -> Result<(), String> {
        gui::run(self, record_path)
    }
}
fn print_usage() {
    println!("Genteel - Sega Genesis/Mega Drive Emulator");
    println!();
    println!("Usage: genteel [OPTIONS] <ROM>");
    println!();
    println!("Options:");
    println!("  --script <path>  Load TAS input script");
    println!("  --record <path>  Record inputs to a script file");
    println!("  --headless <n>   Run N frames without display");
    println!("  --screenshot <path> Save screenshot after headless run");
    println!("  --gdb [port]     Start GDB server (default port: 1234)");
    println!("                   Note: Set GENTEEL_GDB_PASSWORD env var for custom password.");
    println!("  --dump-audio <file> Dump audio output to WAV file");
    println!(
        "  --input-mapping <type> Set keyboard mapping (original|ergonomic, default: original)"
    );
    println!("  --debug          Enable verbose debug output");
    println!("  --help           Show this help");
    println!();
    println!("Controls (play mode - original layout):");
    println!("  Arrow keys       D-pad");
    println!("  Z/X/C            A/B/C buttons");
    println!("  Enter            Start");
    println!("  Escape           Quit");
}
/// GDB memory accessor for Bus
struct BusGdbMemory<'a> {
    bus: &'a std::cell::RefCell<Bus>,
}
impl<'a> GdbMemory for BusGdbMemory<'a> {
    fn read_byte(&mut self, addr: u32) -> u8 {
        self.bus.borrow_mut().read_byte(addr)
    }
    fn write_byte(&mut self, addr: u32, value: u8) {
        // Assuming Z80 RAM is mapped within the first 0x2000 bytes of the Z80's address space
        // This is a heuristic based on the provided snippet's `addr < 0x2000` condition.
        // The GDB `write_byte` operates on the 68k's address space, so we need to
        // translate or ensure this is the correct place for Z80 writes.
        // If the intent was to log Z80Bus's internal write, that function is not in this file.
        // For now, applying the eprintln to the GDB memory write if it targets Z80 RAM.
        // This assumes the GDB server can write directly to Z80 RAM via the main bus.
        if addr < 0x2000 {
            // This condition is a guess based on the user's snippet
            eprintln!(
                "DEBUG: GDB Z80 RAM WRITE: addr=0x{:04X} val=0x{:02X}",
                addr, value
            );
        }
        self.bus.borrow_mut().write_byte(addr, value);
    }
}
#[derive(Debug, Default)]
struct Config {
    rom_path: Option<String>,
    script_path: Option<String>,
    headless: bool,
    headless_frames: Option<u32>,
    screenshot_path: Option<String>,
    gdb_port: Option<u16>,
    gdb_password: Option<String>,
    dump_audio_path: Option<String>,
    record_path: Option<String>,
    input_mapping: InputMapping,
    debug: bool,
    show_help: bool,
}
impl Config {
    fn from_args<I>(args: I) -> Self
    where
        I: IntoIterator<Item = String>,
    {
        let mut config = Config::default();
        config.gdb_password = std::env::var("GENTEEL_GDB_PASSWORD").ok();
        let mut iter = args.into_iter().skip(1);
        let mut current_opt = iter.next();
        while let Some(arg) = current_opt {
            match arg.as_str() {
                "--help" | "-h" => {
                    config.show_help = true;
                    current_opt = iter.next();
                }
                "--script" => {
                    config.script_path = iter.next();
                    current_opt = iter.next();
                }
                "--record" => {
                    config.record_path = iter.next();
                    current_opt = iter.next();
                }
                "--headless" => {
                    config.headless = true;
                    current_opt = iter.next();
                    if let Some(ref next) = current_opt {
                        if !next.starts_with('-') {
                            if let Ok(n) = next.parse::<u32>() {
                                config.headless_frames = Some(n);
                                current_opt = iter.next(); // consume
                            }
                        }
                    }
                }
                "--screenshot" => {
                    config.screenshot_path = iter.next();
                    current_opt = iter.next();
                }
                "--gdb" => {
                    let mut port = debugger::DEFAULT_PORT;
                    current_opt = iter.next();
                    if let Some(ref next) = current_opt {
                        if !next.starts_with('-') {
                            if let Ok(p) = next.parse() {
                                port = p;
                                current_opt = iter.next(); // consume it
                            }
                        }
                    }
                    config.gdb_port = Some(port);
                }
                "--dump-audio" => {
                    config.dump_audio_path = iter.next();
                    current_opt = iter.next();
                }
                "--input-mapping" => {
                    if let Some(mapping_str) = iter.next() {
                        match mapping_str.to_lowercase().as_str() {
                            "ergonomic" | "modern" | "wasd" => {
                                config.input_mapping = InputMapping::Ergonomic;
                            }
                            _ => {
                                config.input_mapping = InputMapping::Original;
                            }
                        }
                    }
                    current_opt = iter.next();
                }
                "--debug" => {
                    config.debug = true;
                    current_opt = iter.next();
                }
                arg if !arg.starts_with('-') => {
                    if let Some(ref mut path) = config.rom_path {
                        path.push(' ');
                        path.push_str(arg);
                    } else {
                        config.rom_path = Some(arg.to_string());
                    }
                    current_opt = iter.next();
                }
                _ => {
                    eprintln!("Unknown option: {}", arg);
                    current_opt = iter.next();
                }
            }
        }
        config
    }
}
fn main() {
    env_logger::init();
    let config = Config::from_args(std::env::args());
    if config.show_help {
        print_usage();
        return;
    }
    let rom_path = config.rom_path;
    let script_path = config.script_path;
    let gdb_port = config.gdb_port;
    let dump_audio_path = config.dump_audio_path;
    let mut emulator = Emulator::new();
    emulator.input_mapping = config.input_mapping;
    emulator.debug = config.debug;
    if let Some(path) = dump_audio_path {
        println!("Dumping audio to: {}", path);
        match wav_writer::WavWriter::new(&path, audio::SAMPLE_RATE, 2) {
            Ok(writer) => emulator.wav_writer = Some(writer),
            Err(e) => eprintln!("Failed to create audio dump file: {}", e),
        }
    }
    // Load ROM if provided
    if let Some(ref path) = rom_path {
        println!("Loading ROM: {}", path);

        // Security: Whitelist the directory containing the ROM
        if let Ok(canonical) = std::path::Path::new(path).canonicalize() {
            if let Some(parent) = canonical.parent() {
                if let Err(e) = emulator.add_allowed_path(parent) {
                    eprintln!("Warning: Failed to whitelist ROM directory: {}", e);
                }
            }
        }

        if let Err(e) = emulator.load_rom(path) {
            eprintln!("Failed to load ROM: {}", e);
            return;
        }
    } else {
        print_usage();
        return;
    }
    // Load input script if provided
    if let Some(ref path) = script_path {
        println!("Loading input script: {}", path);
        if let Err(e) = emulator.input.load_script(path) {
            eprintln!("Failed to load script: {}", e);
            return;
        }
    }
    // Run in appropriate mode
    if let Some(port) = gdb_port {
        if config.headless {
            // Debug mode with GDB (CLI)
            if let Err(e) = emulator.run_with_gdb(port, config.gdb_password) {
                eprintln!("GDB server error: {}", e);
            }
            return;
        } else {
            // Debug mode with GDB (GUI)
            match GdbServer::new(port, config.gdb_password) {
                Ok(gdb) => emulator.gdb = Some(gdb),
                Err(e) => {
                    eprintln!("Failed to start GDB server: {}", e);
                    return;
                }
            }
        }
    }

    if config.headless {
        emulator.run(
            config.headless_frames,
            config.screenshot_path,
            config.record_path,
        );
    } else {
        // Interactive mode with SDL2 window
        #[cfg(feature = "gui")]
        if let Err(e) = emulator.run_with_frontend(config.record_path) {
            eprintln!("Frontend error: {}", e);
        }
        #[cfg(not(feature = "gui"))]
        {
            eprintln!("Interactive mode requires 'gui' feature.");
            eprintln!("Use --headless <N> or --gdb <PORT> instead.");
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn test_zip_bomb_prevention() {
        let path = "test_bomb.zip";
        let mut zip_data = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_data));
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            // Generate 33MB of dummy data
            zip.start_file("large.bin", options).unwrap();
            let chunk = vec![0u8; 1024 * 1024]; // 1MB
            for _ in 0..33 {
                zip.write_all(&chunk).unwrap();
            }
            zip.finish().unwrap();
        }
        std::fs::write(path, &zip_data).unwrap();
        // Attempt to load
        let result = Emulator::load_rom_from_zip(path);
        // Cleanup
        let _ = std::fs::remove_file(path);
        // Verify rejection
        assert!(result.is_err(), "Should reject large ROM file (>32MB)");
    }
    #[test]
    fn test_zip_bomb_mismatch() {
        // Create 33MB of dummy data (simulating decompressed stream)
        let size = 33 * 1024 * 1024;
        let data = vec![0u8; size];
        let mut reader = std::io::Cursor::new(data);
        // Report size as small (e.g., 1KB), simulating a zip bomb header lie
        let reported_size = 1024;
        // This should fail because it reads > 32MB despite reported size
        let result = Emulator::read_rom_with_limit(&mut reader, reported_size);
        assert!(
            result.is_err(),
            "Should reject read size exceeding limit even if reported size is small"
        );
    }
    #[test]
    fn test_z80_ready_flag_behavior() {
        let mut emulator = Emulator::new();
        // Z80 Program to write 0x80 to 0x1FFD
        // LD A, 0x80      (3E 80)
        // LD (0x1FFD), A  (32 FD 1F)
        // HALT            (76)
        let z80_code = vec![0x3E, 0x80, 0x32, 0xFD, 0x1F, 0x76];
        // Ensure Z80 RAM is clear initially (Emulator::new clears it)
        // First, let Z80 run to dirty the PC (simulate running garbage or previous code)
        // By default Z80 is in reset (from new()). We must release it to run.
        emulator.bus.borrow_mut().write_word(0xA11200, 0x0100); // Release Reset
        emulator.step_frame_internal();
        // Check PC > 0 (it should be running NOPs/garbage from 0)
        assert!(emulator.z80.pc > 0, "Z80 PC should have advanced");
        // Sonic 1-like sequence:
        // 1. Assert Z80 Reset (Write 0 to 0xA11200)
        emulator.bus.borrow_mut().write_word(0xA11200, 0x0000);
        // Crucial: Step frame to allow emulator to detect the reset assertion
        emulator.step_frame_internal();
        // At this point, Z80 PC should be 0 because reset was detected
        assert_eq!(emulator.z80.pc, 0, "Z80 PC should be 0 after reset");
        // 2. Request Bus (Write 0x100 to 0xA11100)
        emulator.bus.borrow_mut().write_word(0xA11100, 0x0100);
        // 3. Load Code to Z80 RAM (0xA00000)
        {
            let mut bus = emulator.bus.borrow_mut();
            #[allow(clippy::needless_range_loop)]
            for i in 0..z80_code.len() {
                bus.write_byte(0xA00000 + i as u32, z80_code[i]);
            }
        }
        // Verify Z80 RAM at target address is 0 before execution
        assert_eq!(
            emulator.bus.borrow_mut().read_byte(0xA01FFD),
            0x00,
            "Z80 RAM should be 0 before run"
        );
        // 4. Release Bus (Write 0 to 0xA11100)
        emulator.bus.borrow_mut().write_word(0xA11100, 0x0000);
        // 5. Release Reset (Write 0x100 to 0xA11200)
        emulator.bus.borrow_mut().write_word(0xA11200, 0x0100);
        // Run for a few frames to let Z80 execute
        emulator.step_frame_internal();
        emulator.step_frame_internal();
        // Request bus to read result (Z80 RAM is only accessible when Z80 is stopped)
        emulator.bus.borrow_mut().write_word(0xA11100, 0x0100);
        // Check if 0xA01FFD is 0x80
        // We use read_byte on bus.
        // We must request the bus first to read Z80 RAM
        emulator.bus.borrow_mut().write_word(0xA11100, 0x0100);
        let val = emulator.bus.borrow_mut().read_byte(0xA01FFD);
        assert_eq!(val, 0x80, "Z80 should have written 0x80 to 0xA01FFD");
    }
    #[test]
    fn test_config_parsing() {
        let args = vec!["genteel".to_string(), "rom.bin".to_string()];
        let config = Config::from_args(args);
        assert_eq!(config.rom_path, Some("rom.bin".to_string()));
        assert!(!config.show_help);
        let args = vec!["genteel".to_string(), "--help".to_string()];
        let config = Config::from_args(args);
        assert!(config.show_help);
        let args = vec![
            "genteel".to_string(),
            "--script".to_string(),
            "script.txt".to_string(),
            "rom.bin".to_string(),
        ];
        let config = Config::from_args(args);
        assert_eq!(config.script_path, Some("script.txt".to_string()));
        assert_eq!(config.rom_path, Some("rom.bin".to_string()));
        let args = vec![
            "genteel".to_string(),
            "--headless".to_string(),
            "60".to_string(),
            "rom.bin".to_string(),
        ];
        let config = Config::from_args(args);
        assert_eq!(config.headless_frames, Some(60));
        assert_eq!(config.rom_path, Some("rom.bin".to_string()));
        let args = vec![
            "genteel".to_string(),
            "--headless".to_string(),
            "1200".to_string(),
            "--screenshot".to_string(),
            "final.png".to_string(),
            "rom.bin".to_string(),
        ];
        let config = Config::from_args(args);
        assert_eq!(config.headless_frames, Some(1200));
        assert_eq!(config.screenshot_path, Some("final.png".to_string()));
        assert_eq!(config.rom_path, Some("rom.bin".to_string()));
        let args = vec![
            "genteel".to_string(),
            "--gdb".to_string(),
            "rom.bin".to_string(),
        ];
        let config = Config::from_args(args);
        assert_eq!(config.gdb_port, Some(crate::debugger::DEFAULT_PORT));
        assert_eq!(config.rom_path, Some("rom.bin".to_string()));
        let args = vec![
            "genteel".to_string(),
            "--gdb".to_string(),
            "1234".to_string(),
            "rom.bin".to_string(),
        ];
        let config = Config::from_args(args);
        assert_eq!(config.gdb_port, Some(1234));
        assert_eq!(config.rom_path, Some("rom.bin".to_string()));

        // Test environment variable password
        std::env::set_var("GENTEEL_GDB_PASSWORD", "env_secret");
        let args = vec!["genteel".to_string(), "rom.bin".to_string()];
        let config = Config::from_args(args);
        assert_eq!(config.gdb_password, Some("env_secret".to_string()));
        std::env::remove_var("GENTEEL_GDB_PASSWORD");

        let args = vec![
            "genteel".to_string(),
            "rom".to_string(),
            "part2.bin".to_string(),
        ];
        let config = Config::from_args(args);
        assert_eq!(config.rom_path, Some("rom part2.bin".to_string()));
    }
    #[test]
    fn test_step_frame_basic() {
        let mut emulator = Emulator::new();
        emulator.step_frame(None);
        emulator.step_frame(None);
        assert_eq!(emulator.internal_frame_count, 2);
    }
    #[test]
    fn test_large_raw_rom_prevention() {
        let path = "large_rom.bin";
        // Create 33MB of dummy data
        let size = 33 * 1024 * 1024;
        let data = vec![0u8; size];
        std::fs::write(path, &data).unwrap();
        let mut emulator = Emulator::new();
        // Must whitelist the path first
        emulator.add_allowed_path(".").unwrap();
        let result = emulator.load_rom(path);
        // Cleanup
        let _ = std::fs::remove_file(path);
        // Verify rejection
        assert!(result.is_err(), "Should reject large ROM file (>32MB)");
    }

    #[test]
    fn test_path_traversal_protection() {
        let dummy_rom = "dummy_traversal.bin";
        std::fs::write(dummy_rom, b"dummy rom content").unwrap();

        let mut emulator = Emulator::new();

        // 1. Should fail without whitelist (Secure by Default)
        let result = emulator.load_rom(dummy_rom);
        assert!(result.is_err(), "Should fail without whitelisted path");
        assert_eq!(
            result.as_ref().err().unwrap().kind(),
            std::io::ErrorKind::PermissionDenied
        );

        // 2. Should fail if whitelist doesn't cover the file
        // Whitelist a different directory (e.g., system temp)
        let temp_dir = std::env::temp_dir();
        // Only if temp_dir exists and is different from CWD
        if let Ok(_) = emulator.add_allowed_path(&temp_dir) {
            let result = emulator.load_rom(dummy_rom);
            // Assuming dummy_rom is in CWD and not in temp_dir
            // If CWD == temp_dir, this test is weak but passes.
            // Usually CWD is project root.
            // We can check canonical paths to be sure.
            let cwd = std::env::current_dir().unwrap().canonicalize().unwrap();
            let temp = temp_dir.canonicalize().unwrap();
            if !cwd.starts_with(&temp) {
                assert!(result.is_err(), "Should fail if path not in whitelist");
            }
        }

        // 3. Should succeed if whitelisted
        emulator.add_allowed_path(".").unwrap();
        let result = emulator.load_rom(dummy_rom);
        assert!(result.is_ok(), "Should succeed if path is whitelisted");

        // Cleanup
        let _ = std::fs::remove_file(dummy_rom);
    }

    #[test]
    fn test_screenshot_path_sanitization() {
        let mut emulator = Emulator::new();
        let path = "/tmp/genteel_exploit.png";
        let sanitized_path = "genteel_exploit.png";

        // Ensure files don't exist
        if std::path::Path::new(path).exists() {
            let _ = std::fs::remove_file(path);
        }
        if std::path::Path::new(sanitized_path).exists() {
            let _ = std::fs::remove_file(sanitized_path);
        }

        // Construct input with command
        let mut input = crate::input::FrameInput::default();
        input.command = Some(format!("SCREENSHOT {}", path));

        emulator.step_frame(Some(&input));

        // Check vulnerability is fixed
        if std::path::Path::new(path).exists() {
            let _ = std::fs::remove_file(path);
            panic!("Vulnerability still present: file created at {}", path);
        }

        // Check sanitized behavior
        if std::path::Path::new(sanitized_path).exists() {
            // Success: created at sanitized path
            let _ = std::fs::remove_file(sanitized_path);
        } else {
            panic!(
                "Sanitization failed: file not created at {}",
                sanitized_path
            );
        }
    }

    #[test]
    fn test_large_sram_prevention() {
        let path = std::path::PathBuf::from("test_large_sram.bin");
        let srm_path = path.with_extension("srm");
        // Create a file larger than MAX_SRAM_SIZE (2MB + 1 byte)
        let mut file = std::fs::File::create(&srm_path).unwrap();
        let chunk = vec![0u8; 1024 * 1024]; // 1MB chunk
        for _ in 0..2 {
            std::io::Write::write_all(&mut file, &chunk).unwrap();
        }
        std::io::Write::write_all(&mut file, &[0u8]).unwrap(); // last byte
        drop(file);

        let mut emulator = Emulator::new();
        emulator.current_rom_path = Some(path.clone());

        let initial_sram_len = emulator.bus.borrow().sram.len();

        // Attempt to load - should fail/print warning and not change sram contents or crash
        emulator.load_sram();

        // SRAM size should not have changed to the large size
        assert_eq!(emulator.bus.borrow().sram.len(), initial_sram_len);

        // Cleanup
        let _ = std::fs::remove_file(srm_path);
    }

    #[test]
    fn test_large_state_prevention() {
        let path = std::path::PathBuf::from("test_large.state");
        // Create a file larger than MAX_STATE_SIZE (25MB + 1 byte)
        // Using valid UTF-8 data (spaces) to exercise the size check specifically.
        let mut file = std::fs::File::create(&path).unwrap();
        let chunk = vec![b' '; 1024 * 1024]; // 1MB chunk
        for _ in 0..25 {
            std::io::Write::write_all(&mut file, &chunk).unwrap();
        }
        std::io::Write::write_all(&mut file, &[b' ']).unwrap(); // last byte
        drop(file);

        let mut emulator = Emulator::new();
        let old_frame_count = emulator.internal_frame_count;
        // Attempt to load - should fail and not change state
        emulator.load_state_from_path(path.clone());
        assert_eq!(emulator.internal_frame_count, old_frame_count);

        // Cleanup
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_emulator_pause() {
        let mut emulator = Emulator::new();
        let initial_frames = emulator.internal_frame_count;

        emulator.paused = true;
        emulator.step_frame(None);
        assert_eq!(
            emulator.internal_frame_count, initial_frames,
            "Should not advance when paused"
        );

        emulator.single_step = true;
        emulator.step_frame(None);
        assert_eq!(
            emulator.internal_frame_count,
            initial_frames + 1,
            "Should advance one frame when single_stepping"
        );
        assert!(
            !emulator.single_step,
            "single_step should be reset after use"
        );

        emulator.step_frame(None);
        assert_eq!(
            emulator.internal_frame_count,
            initial_frames + 1,
            "Should still be paused"
        );

        emulator.paused = false;
        emulator.step_frame(None);
        assert_eq!(
            emulator.internal_frame_count,
            initial_frames + 2,
            "Should advance when resumed"
        );
    }
}
