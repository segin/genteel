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

pub const SLOT_EXTS: [&str; 10] = [
    "s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9",
];

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

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy)]
struct Z80Change {
    instruction_cycles: u32,
    new_req: bool,
    new_rst: bool,
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
    pub current_rom_path: Option<std::path::PathBuf>,
    pub allowed_paths: Vec<std::path::PathBuf>,
}
impl Default for Emulator {
    fn default() -> Self {
        Self::new()
    }
}
impl Emulator {
    pub fn new() -> Self {
        let bus = Rc::new(RefCell::new(Bus::new()));
        let mut bus_ref = bus.borrow_mut();
        let cpu = Cpu::new(&mut *bus_ref);
        drop(bus_ref);
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
        if let Ok(data) = std::fs::read(&sram_path) {
            println!("Loading SRAM from {:?}", sram_path);
            let mut bus = self.bus.borrow_mut();
            if data.len() == bus.sram.len() {
                bus.sram.copy_from_slice(&data);
            } else {
                let len = data.len().min(bus.sram.len());
                bus.sram[..len].copy_from_slice(&data[..len]);
            }
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
        if let Ok(json) = std::fs::read_to_string(&state_path) {
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
    }

    /// Add a path to the whitelist of allowed ROM directories.
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
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            if !parts.is_empty() {
                match parts[0].to_uppercase().as_str() {
                    "SCREENSHOT" => {
                        if parts.len() > 1 {
                            let raw_path = parts[1];
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
                                eprintln!(
                                    "Script Error: Failed to save screenshot to {}: {}",
                                    path, e
                                );
                            } else {
                                println!("Script: Saved screenshot to {}", path);
                            }
                        }
                    }
                    "READ_BYTE" => {
                        if parts.len() > 1 {
                            if let Ok(addr) =
                                u32::from_str_radix(parts[1].trim_start_matches("0x"), 16)
                            {
                                let val = self.bus.borrow_mut().read_byte(addr);
                                println!("Script: READ_BYTE 0x{:06X} = 0x{:02X}", addr, val);
                            }
                        }
                    }
                    "WRITE_BYTE" => {
                        if parts.len() > 2 {
                            let addr_res =
                                u32::from_str_radix(parts[1].trim_start_matches("0x"), 16);
                            let val_res = u8::from_str_radix(parts[2].trim_start_matches("0x"), 16);
                            if let (Ok(addr), Ok(val)) = (addr_res, val_res) {
                                self.bus.borrow_mut().write_byte(addr, val);
                                println!("Script: WRITE_BYTE 0x{:06X} = 0x{:02X}", addr, val);
                            }
                        }
                    }
                    "ASSERT_BYTE" => {
                        if parts.len() > 2 {
                            let addr_res =
                                u32::from_str_radix(parts[1].trim_start_matches("0x"), 16);
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
                    "LOG" => {
                        if parts.len() > 1 {
                            println!("Script LOG: {}", parts[1..].join(" "));
                        }
                    }
                    _ => {}
                }
            }
        }

        self.step_frame_internal();
    }
    pub fn step_frame_internal(&mut self) {
        self.internal_frame_count += 1;
        if self.debug {
            #[cfg(feature = "gui")]
            self.log_debug(self.internal_frame_count);
        }
        const LINES_PER_FRAME: u16 = 262;
        let active_lines = self.bus.borrow().vdp.screen_height();
        let mut z80_cycle_debt: f32 = 0.0;

        for line in 0..LINES_PER_FRAME {
            // Update VDP VBlank status
            {
                let mut bus = self.bus.borrow_mut();
                if line == active_lines {
                    bus.vdp.set_vblank(true);
                } else if line == 0 {
                    bus.vdp.set_vblank(false);
                }
            }
            self.step_scanline(line, active_lines, &mut z80_cycle_debt);
        }
        self.generate_audio_samples();
        self.bus.borrow_mut().vdp.update_v30_offset();
    }
    fn step_scanline(
        &mut self,
        line: u16,
        active_lines: u16,
        z80_cycle_debt: &mut f32,
    ) {
        self.handle_interrupts(line, active_lines);
        self.run_cpu_loop(line, active_lines, z80_cycle_debt);
        // Render after CPU loop to capture mid-scanline register/palette changes (fixes Road Rash II flickering)
        self.vdp_scanline_render(line, active_lines);
    }
    fn vdp_scanline_render(&mut self, line: u16, _active_lines: u16) {
        let mut bus = self.bus.borrow_mut();
        if line < 240 {
            bus.vdp.render_line(line);
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn sync_components(
        bus_rc: &SharedBus,
        m68k_cycles: u32,
        z80: &mut Z80<Z80Bus, Z80Bus>,
        z80_cycle_debt: &mut f32,
        trigger_vint: bool,
        internal_frame_count: u64,
        z80_last_bus_req: &mut bool,
        z80_last_reset: &mut bool,
        z80_trace_count: &mut u32,
        debug: bool,
    ) {
        let mclk = m68k_cycles * 7;

        // 1. Tick the Bus (VDP, etc)
        {
            let mut bus = bus_rc.bus.borrow_mut();
            bus.tick(mclk);
        }

        // 2. Z80 State and Timing
        let (z80_can_run, z80_is_reset) = {
            let bus = bus_rc.bus.borrow();
            *z80_last_bus_req = bus.z80_bus_request;
            let z80_can_run = !bus.z80_reset && !bus.z80_bus_request;
            let z80_is_reset = bus.z80_reset;
            (z80_can_run, z80_is_reset)
        };

        // Handle Z80 Reset
        if z80_is_reset && !*z80_last_reset {
            z80.reset();
        }
        *z80_last_reset = z80_is_reset;

        // Trace Z80 if debugging
        if z80_can_run && internal_frame_count > 0 {
            z80.debug = debug && *z80_trace_count < 5000;
            if z80.debug {
                *z80_trace_count += 1;
            }
        } else {
            z80.debug = false;
        }

        // Trigger Z80 VInt if requested
        if trigger_vint && !z80_is_reset {
            z80.trigger_interrupt(0xFF);
        }

        // 3. Catch up Z80
        if z80_can_run {
            const Z80_CYCLES_PER_M68K_CYCLE: f32 = 3579545.0 / 7670453.0;
            *z80_cycle_debt += m68k_cycles as f32 * Z80_CYCLES_PER_M68K_CYCLE;
            while *z80_cycle_debt >= 1.0 {
                let cycles = z80.step();
                *z80_cycle_debt -= cycles as f32;
            }
        }

        // 4. Update APU
        {
            let mut bus = bus_rc.bus.borrow_mut();
            bus.apu.tick_cycles(m68k_cycles);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn run_cpu_batch_static(
        cpu: &mut Cpu,
        bus_rc: &SharedBus,
        max_cycles: u32,
        z80: &mut Z80<Z80Bus, Z80Bus>,
        z80_cycle_debt: &mut f32,
        line: u16,
        active_lines: u16,
        internal_frame_count: u64,
        z80_last_bus_req: &mut bool,
        z80_last_reset: &mut bool,
        z80_trace_count: &mut u32,
        debug: bool,
    ) -> CpuBatchResult {
        let (initial_req, initial_rst) = {
            let bus = bus_rc.bus.borrow();
            (bus.z80_bus_request, bus.z80_reset)
        };
        let mut pending_cycles = 0;
        loop {
            if pending_cycles >= max_cycles {
                return CpuBatchResult {
                    cycles: pending_cycles,
                    z80_change: None,
                };
            }
            let m68k_cycles = {
                let mut bus = bus_rc.bus.borrow_mut();
                let cycles = if bus.dma_active() {
                    2
                } else {
                    cpu.step_instruction(&mut *bus)
                };

                if cpu.pending_interrupt == 6 && !bus.vdp.vblank_pending() {
                    cpu.cancel_interrupt(6);
                }

                cycles
            };

            let trigger_vint = line == active_lines && pending_cycles < 10;
            Self::sync_components(
                bus_rc,
                m68k_cycles,
                z80,
                z80_cycle_debt,
                trigger_vint,
                internal_frame_count,
                z80_last_bus_req,
                z80_last_reset,
                z80_trace_count,
                debug,
            );

            let (req, rst) = {
                let bus = bus_rc.bus.borrow();
                (bus.z80_bus_request, bus.z80_reset)
            };

            if req != initial_req || rst != initial_rst {
                return CpuBatchResult {
                    cycles: pending_cycles,
                    z80_change: Some(Z80Change {
                        instruction_cycles: m68k_cycles,
                        new_req: req,
                        new_rst: rst,
                    }),
                };
            }
            pending_cycles += m68k_cycles;
        }
    }
    fn run_cpu_loop(&mut self, line: u16, active_lines: u16, z80_cycle_debt: &mut f32) {
        const CYCLES_PER_LINE: u32 = 488;
        const BATCH_SIZE: u32 = 64;
        let mut cycles_scanline: u32 = 0;
        let bus_rc = SharedBus::new(self.bus.clone());

        while cycles_scanline < CYCLES_PER_LINE {
            let remaining = CYCLES_PER_LINE - cycles_scanline;
            let limit = std::cmp::min(remaining, BATCH_SIZE);
            let result = Self::run_cpu_batch_static(
                &mut self.cpu,
                &bus_rc,
                limit,
                &mut self.z80,
                z80_cycle_debt,
                line,
                active_lines,
                self.internal_frame_count,
                &mut self.z80_last_bus_req,
                &mut self.z80_last_reset,
                &mut self.z80_trace_count,
                self.debug,
            );

            cycles_scanline += result.cycles;

            if let Some(change) = result.z80_change {
                {
                    let mut bus = self.bus.borrow_mut();
                    bus.z80_bus_request = change.new_req;
                    bus.z80_reset = change.new_rst;
                }

                let trigger_vint = line == active_lines && cycles_scanline < 10;
                Self::sync_components(
                    &bus_rc,
                    change.instruction_cycles,
                    &mut self.z80,
                    z80_cycle_debt,
                    trigger_vint,
                    self.internal_frame_count,
                    &mut self.z80_last_bus_req,
                    &mut self.z80_last_reset,
                    &mut self.z80_trace_count,
                    self.debug,
                );
                cycles_scanline += change.instruction_cycles;
            }
        }
    }
    fn generate_audio_samples(&mut self) {
        let mut bus = self.bus.borrow_mut();
        
        while let Some((l, r)) = bus.apu.generate_sample() {
            if let Some(writer) = &mut self.wav_writer {
                let _ = writer.write_samples(&[l, r]);
            }
            if self.audio_buffer.len() < crate::audio::samples_per_frame() * 4 {
                self.audio_buffer.push(l);
                self.audio_buffer.push(r);
            }
        }
    }
    fn handle_interrupts(&mut self, line: u16, active_lines: u16) {
        self.cpu.cancel_interrupt(4);

        let mut bus = self.bus.borrow_mut();
        if line == active_lines {
            if (bus.vdp.mode2() & 0x20) != 0 {
                self.cpu.request_interrupt(6);
            }
        }
        if line <= active_lines {
            if bus.vdp.line_counter == 0 {
                bus.vdp.line_counter = bus.vdp.registers[10] as u16;
                if (bus.vdp.mode1() & 0x10) != 0 {
                    self.cpu.request_interrupt(4);
                }
            } else {
                bus.vdp.line_counter -= 1;
            }
        } else {
            bus.vdp.line_counter = bus.vdp.registers[10] as u16;
        }
    }
    /// Run headless for N frames
    pub fn run(
        &mut self,
        frames: Option<u32>,
        screenshot_path: Option<String>,
        record_path: Option<String>,
    ) {
        if let Some(_path) = &record_path {
            self.input.start_recording();
        }

        let start_time = std::time::Instant::now();
        let mut current = 0;
        loop {
            if let Some(n) = frames {
                if current >= n { break; }
            } else if self.input.is_complete() {
                break;
            }

            self.step_frame(None);
            self.audio_buffer.clear();
            current += 1;
        }

        let elapsed = start_time.elapsed();
        if let Some(path) = screenshot_path {
            let _ = self.save_screenshot(&path);
        }

        if let Some(path) = record_path {
            let script: InputScript = self.input.stop_recording();
            let _ = script.save(&path);
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
        // Wait for connection
        while !gdb.accept() {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let mut stepping = false;
        let mut running = false;
        let mut mem_access = BusGdbMemory { bus: &self.bus };
        loop {
            if let Some(cmd) = gdb.receive_packet() {
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
                    "CONTINUE" => { running = true; stepping = false; }
                    "STEP" => { stepping = true; running = true; }
                    _ if !response.is_empty() => { gdb.send_packet(&response).ok(); }
                    _ => {}
                }
            }
            if running {
                let mut bus = self.bus.borrow_mut();
                self.cpu.step_instruction(&mut *bus);
                drop(bus);
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
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            if !gdb.is_connected() && !gdb.accept() {
                break;
            }
        }
        Ok(())
    }
    #[cfg(feature = "gui")]
    pub(crate) fn log_debug(&self, frame_count: u64) {
        let bus = self.bus.borrow();
        let disp_en = if bus.vdp.display_enabled() { "ON " } else { "OFF" };
        let dma_en = if bus.vdp.dma_enabled() { "ON " } else { "OFF" };
        let cram_val = if bus.vdp.cram.len() >= 2 {
            ((bus.vdp.cram[0] as u16) << 8) | (bus.vdp.cram[1] as u16)
        } else {
            0
        };
        let z80_pc = self.z80.pc;
        let z80_reset = if bus.z80_reset { "RST" } else { "RUN" };
        let z80_req = if bus.z80_bus_request { "BUS" } else { "---" };
        let z80_op = if (z80_pc as usize) < bus.z80_ram.len() { bus.z80_ram[z80_pc as usize] } else { 0 };
        println_safe!(
            "FRAME {:05} | 68k: PC={:06X} SR={:04X} | VDP: Disp={} DMA={} CRAM={:04X} | Z80: PC={:04X} OP={:02X} St={} Req={}",
            frame_count, self.cpu.pc, self.cpu.sr, disp_en, dma_en, cram_val, z80_pc, z80_op, z80_reset, z80_req
        );
    }

    #[cfg(feature = "gui")]
    pub fn run_with_frontend(self, record_path: Option<String>) -> Result<(), String> {
        gui::run(self, record_path)
    }
}
struct BusGdbMemory<'a> {
    bus: &'a std::cell::RefCell<Bus>,
}
impl<'a> GdbMemory for BusGdbMemory<'a> {
    fn read_byte(&mut self, addr: u32) -> u8 { self.bus.borrow_mut().read_byte(addr) }
    fn write_byte(&mut self, addr: u32, value: u8) { self.bus.borrow_mut().write_byte(addr, value); }
}
fn main() {
    env_logger::init();
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Usage: genteel <ROM>");
        return;
    }
    let mut emulator = Emulator::new();
    let rom_path = &args[1];
    if let Ok(canonical) = std::path::Path::new(rom_path).canonicalize() {
        if let Some(parent) = canonical.parent() {
            let _ = emulator.add_allowed_path(parent);
        }
    }
    if let Err(e) = emulator.load_rom(rom_path) {
        eprintln!("Failed to load ROM: {}", e);
        return;
    }
    #[cfg(feature = "gui")]
    if let Err(e) = emulator.run_with_frontend(None) {
        eprintln!("Frontend error: {}", e);
    }
}
