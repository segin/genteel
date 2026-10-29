#![forbid(unsafe_code)]
/// Graceful println that ignores broken pipe errors (for `| head` usage)
#[allow(unused_macros)]
macro_rules! println_safe {
    ($($arg:tt)*) => {{
        use std::io::Write;
        let _ = writeln!(std::io::stdout(), $($arg)*);
    }};
}
/* The emulator core comes from the library crate; only the GUI front end
lives in the binary. Importing the modules at the crate root keeps
`crate::cpu`-style paths working inside the binary's own modules. */
#[cfg(feature = "gui")]
use genteel::io;
use genteel::{apu, audio, cpu, debugger, frontend, input, memory, vdp, wav_writer, z80};

#[cfg(feature = "gui")]
mod gui;
#[cfg(all(feature = "gui", test))]
mod tests_gui;

pub const SLOT_EXTS: [&str; 10] = ["s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9"];

/* Maximum save state size in bytes, to prevent OOM. The machine state is
under 7 MB; an SSF cartridge can add up to 16 MB of written mapper RAM,
which is saved as hex (32 MB). */
const MAX_STATE_SIZE: u64 = 48 * 1024 * 1024;

/// Maximum SRAM size in bytes (2MB) to prevent OOM/DoS
const MAX_SRAM_SIZE: u64 = 2 * 1024 * 1024;

/// Maximum ROM size in bytes (32MB) to prevent OOM/DoS
const MAX_ROM_SIZE: u64 = 32 * 1024 * 1024;

use apu::{Apu, Region};
use cpu::Cpu;
use debugger::{GdbMemory, GdbRegisters, GdbServer, RunRequest, StopReason};
use frontend::InputMapping;
use input::{InputManager, InputScript};
use memory::bus::Bus;
use memory::{SharedBus, Z80Bus, Z80BusView, Z80Ports};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::rc::Rc;
use z80::Z80;

/// Master clocks per scanline (Genesis Plus GX `MCYCLES_PER_LINE`).
pub const MCLK_PER_LINE: u32 = 3420;
/// The 68000 is clocked at MCLK / 7.
const MCLK_PER_M68K_CYCLE: u32 = 7;

struct SystemContext<'a> {
    cpu: &'a mut Cpu,
    bus_rc: &'a Rc<RefCell<Bus>>,
    z80: &'a mut Z80<Z80Bus, Z80Bus>,
    z80_cycle_debt: &'a mut f32,
    z80_last_bus_req: &'a mut bool,
    z80_last_reset: &'a mut bool,
    z80_trace_count: &'a mut u32,
    master_clock: &'a mut u64,
    internal_frame_count: u64,
    debug: bool,
}

#[derive(Debug, Clone, Copy)]
struct CpuBatchResult {
    stopped: bool,
}

#[derive(Debug, Clone, Copy)]
struct ScanlineOutcome {
    frame_done: bool,
    stopped: bool,
}

/// Result of a debugger-limited run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    /// The VDP reached the next frame boundary.
    FrameComplete,
    /// Execution stopped on a breakpoint or instruction budget.
    Stopped,
}

/// A validated ROM image and its battery save, read off the emulator so
/// the file I/O can run on a worker thread.
#[derive(Debug)]
pub struct LoadedRom {
    /// Canonical path of the file that was read.
    pub path: std::path::PathBuf,
    /// Linear ROM bytes (zip extracted, SMD converted).
    pub data: Vec<u8>,
    /// Contents of the matching `.srm`, if one exists.
    pub sram: Option<Vec<u8>>,
}

/// 68k run state while a GDB client is attached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GdbExecState {
    /// Stopped; waiting for a continue or step request.
    #[default]
    Halted,
    /// Running until a breakpoint or interrupt.
    Running,
    /// Executing a single instruction.
    Stepping,
}

/// Stop conditions checked before each 68k instruction.
#[derive(Debug, Default)]
pub struct ExecLimit<'a> {
    /// Stop before executing an instruction at one of these addresses.
    pub breakpoints: Option<&'a std::collections::HashSet<u32>>,
    /// Stop after this many instructions.
    pub instructions_left: Option<u32>,
    /// Ignore a breakpoint at the current PC until one instruction has run
    /// (used when resuming from a breakpoint).
    pub skip_breakpoint_once: bool,
    /// Set when execution stopped on a breakpoint.
    pub hit_breakpoint: std::cell::Cell<bool>,
}

impl ExecLimit<'_> {
    fn should_stop(&self, pc: u32) -> bool {
        if self.instructions_left == Some(0) {
            return true;
        }
        if !self.skip_breakpoint_once && self.breakpoints.is_some_and(|b| b.contains(&pc)) {
            self.hit_breakpoint.set(true);
            return true;
        }
        false
    }

    fn instruction_executed(&mut self) {
        self.skip_breakpoint_once = false;
        if let Some(n) = self.instructions_left.as_mut() {
            *n = n.saturating_sub(1);
        }
    }
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
    /// Session state, not part of a save state.
    #[serde(skip)]
    pub allowed_paths: Vec<std::path::PathBuf>,
    /// Session state, not part of a save state.
    #[serde(skip)]
    pub current_rom_path: Option<std::path::PathBuf>,
    pub z80_cycle_debt: f32,
    /// Total master clocks executed (68k cycles x 7).
    #[serde(default)]
    pub master_clock: u64,
    /// Z80 VINT is due at the start of the next CPU batch.
    #[serde(default)]
    pub z80_vint_pending: bool,
    /// Run state under an attached GDB client.
    #[serde(skip)]
    pub gdb_exec: GdbExecState,
    /// Execute the instruction at a breakpoint once when resuming.
    #[serde(skip)]
    gdb_skip_breakpoint: bool,
    /// The input mapping was given on the command line and takes
    /// precedence over the stored GUI configuration.
    #[serde(skip)]
    pub input_mapping_from_cli: bool,
    /// WAV dump requested but not started yet (it starts once the output
    /// sample rate is final, see `start_pending_wav_dump`).
    #[serde(skip)]
    pub pending_wav_dump: Option<String>,
}
impl Default for Emulator {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Emulator {
    /// Battery RAM is written on every way out (window close, Escape, Quit,
    /// headless completion, GDB session end), not only on Close ROM.
    fn drop(&mut self) {
        self.save_sram();
    }
}

/// Write `data` to `path` atomically: write a temporary file in the same
/// directory, sync it, then rename it over the destination, so a crash or a
/// concurrent reader never sees a truncated file.
pub fn write_atomic(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let dir = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => std::path::PathBuf::from("."),
    };
    let file_name = path
        .file_name()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no file name"))?;
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut tmp_name = std::ffi::OsString::from(".");
    tmp_name.push(file_name);
    tmp_name.push(format!(".{}.{}.tmp", std::process::id(), unique));
    let tmp_path = dir.join(tmp_name);
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)?;
        file.write_all(data)?;
        file.sync_all()?;
        std::fs::rename(&tmp_path, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp_path);
    }
    result
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
            master_clock: 0,
            z80_vint_pending: false,
            gdb_exec: GdbExecState::Halted,
            gdb_skip_breakpoint: false,
            input_mapping_from_cli: false,
            pending_wav_dump: None,
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

    /// Start a requested WAV dump. The header records the bus output
    /// sample rate in effect now, which is the rate samples are produced
    /// at, so call this after the frontend has set the device rate.
    pub fn start_pending_wav_dump(&mut self) {
        let Some(path) = self.pending_wav_dump.take() else {
            return;
        };
        let rate = self.bus.borrow().sample_rate;
        println!("Dumping audio to: {} ({} Hz)", path, rate);
        match wav_writer::WavWriter::new(&path, rate, 2) {
            Ok(writer) => self.wav_writer = Some(writer),
            Err(e) => eprintln!("Failed to create audio dump file: {}", e),
        }
    }

    /// Close current ROM and return to default state
    pub fn close_rom(&mut self) {
        if let Some((path, image)) = self.close_rom_deferred() {
            Self::write_sram_file(&path, &image);
        }
    }

    /// Close the current ROM without touching the disk and return the
    /// pending SRAM write (path and image), so a frontend can perform it
    /// on a worker thread.
    pub fn close_rom_deferred(&mut self) -> Option<(std::path::PathBuf, Vec<u8>)> {
        let job = self.sram_save_job();

        let allowed_paths = self.allowed_paths.clone();
        let mapping = self.input_mapping;
        let sample_rate = self.bus.borrow().sample_rate;
        /* The ROM is gone; nothing more may be written to its .srm. */
        self.current_rom_path = None;

        *self = Self::new();

        self.allowed_paths = allowed_paths;
        self.input_mapping = mapping;
        self.bus.borrow_mut().sample_rate = sample_rate;
        job
    }

    /// Path of a file derived from the ROM path by swapping the extension
    /// (`game.bin` -> `game.srm`). If that would name the ROM itself (a ROM
    /// called `game.srm`), the extension is appended instead
    /// (`game.srm.srm`) so the ROM can never be overwritten.
    pub fn derived_path(rom_path: &std::path::Path, ext: &str) -> std::path::PathBuf {
        let swapped = rom_path.with_extension(ext);
        if swapped != rom_path {
            return swapped;
        }
        let mut name = rom_path.as_os_str().to_os_string();
        name.push(".");
        name.push(ext);
        std::path::PathBuf::from(name)
    }

    fn sram_path(&self) -> Option<std::path::PathBuf> {
        self.current_rom_path
            .as_deref()
            .map(|p| Self::derived_path(p, "srm"))
    }

    /// Path of save-state `slot` (0-9) for the current ROM.
    pub fn state_path(&self, slot: u8) -> Result<std::path::PathBuf, String> {
        let ext = SLOT_EXTS
            .get(slot as usize)
            .ok_or_else(|| format!("Invalid save slot {} (valid slots are 0-9)", slot))?;
        let rom = self
            .current_rom_path
            .as_deref()
            .ok_or_else(|| "No ROM loaded".to_string())?;
        Ok(Self::derived_path(rom, ext))
    }

    /// Read a `.srm` file with a size limit. Touches no emulator state.
    fn read_sram_file(sram_path: &std::path::Path) -> Option<Vec<u8>> {
        let file = std::fs::File::open(sram_path).ok()?;

        // 1. Check metadata size
        if let Ok(metadata) = file.metadata() {
            if metadata.len() > MAX_SRAM_SIZE {
                eprintln!("SRAM file too large: {} bytes", metadata.len());
                return None;
            }
        }

        // 2. Read with limit to prevent OOM
        use std::io::Read;
        let mut data = Vec::new();
        if let Err(e) = file.take(MAX_SRAM_SIZE + 1).read_to_end(&mut data) {
            eprintln!("Failed to read SRAM: {}", e);
            return None;
        }

        if data.len() as u64 > MAX_SRAM_SIZE {
            eprintln!("SRAM exceeds size limit");
            return None;
        }
        println!("Loading SRAM from {:?}", sram_path);
        Some(data)
    }

    pub fn load_sram(&mut self) {
        if let Some(data) = self.sram_path().and_then(|p| Self::read_sram_file(&p)) {
            self.bus.borrow_mut().sram.load_image(&data);
        }
    }

    /// The pending SRAM write: `.srm` path and image. `None` when no ROM is
    /// loaded, the cartridge has no SRAM, or the game never wrote to it.
    pub fn sram_save_job(&self) -> Option<(std::path::PathBuf, Vec<u8>)> {
        let sram_path = self.sram_path()?;
        let image = self.bus.borrow().sram.image();
        (!image.is_empty()).then_some((sram_path, image))
    }

    /// Write an SRAM image atomically, reporting the outcome.
    pub fn write_sram_file(sram_path: &std::path::Path, image: &[u8]) {
        if let Err(e) = write_atomic(sram_path, image) {
            eprintln!("Failed to save SRAM to {:?}: {}", sram_path, e);
        } else {
            println!("Saved SRAM to {:?}", sram_path);
        }
    }

    /// Write battery RAM to the `.srm` file. Nothing is written when the
    /// cartridge has no SRAM or the game never wrote to it.
    pub fn save_sram(&self) {
        if let Some((path, image)) = self.sram_save_job() {
            Self::write_sram_file(&path, &image);
        }
    }

    pub fn save_state(&self, slot: u8) -> Result<(), String> {
        let state_path = self.state_path(slot)?;
        self.save_state_to_path(state_path);
        Ok(())
    }

    /// Serialize the machine state for a save state.
    pub fn serialize_state(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(self).map_err(|e| format!("Failed to serialize state: {}", e))
    }

    /// Write serialized state bytes atomically, reporting the outcome.
    pub fn write_state_file(state_path: &std::path::Path, data: &[u8]) {
        if let Err(e) = write_atomic(state_path, data) {
            eprintln!("Failed to save state to {:?}: {}", state_path, e);
        } else {
            println!("Saved state to {:?}", state_path);
        }
    }

    pub fn save_state_to_path(&self, state_path: std::path::PathBuf) {
        match self.serialize_state() {
            Ok(data) => Self::write_state_file(&state_path, &data),
            Err(e) => eprintln!("{}", e),
        }
    }

    pub fn load_state(&mut self, slot: u8) -> Result<(), String> {
        let state_path = self.state_path(slot)?;
        self.load_state_from_path(state_path);
        Ok(())
    }

    pub fn delete_state(&self, slot: u8) -> Result<(), String> {
        let state_path = self.state_path(slot)?;

        if state_path.exists() {
            if let Err(e) = std::fs::remove_file(&state_path) {
                eprintln!("Failed to delete state {:?}: {}", state_path, e);
            } else {
                println!("Deleted state {:?}", state_path);
            }
        }
        Ok(())
    }

    /// Read a save-state file with a size limit. Touches no emulator
    /// state, so it can run on a worker thread.
    pub fn read_state_file(state_path: &std::path::Path) -> Result<String, String> {
        let file = std::fs::File::open(state_path)
            .map_err(|e| format!("Failed to open save state {:?}: {}", state_path, e))?;

        // 1. Check metadata size
        if let Ok(metadata) = file.metadata() {
            if metadata.len() > MAX_STATE_SIZE {
                return Err(format!("Save state too large: {} bytes", metadata.len()));
            }
        }

        // 2. Read with limit to prevent OOM
        use std::io::Read;
        let mut json = String::new();
        file.take(MAX_STATE_SIZE + 1)
            .read_to_string(&mut json)
            .map_err(|e| format!("Failed to read save state: {}", e))?;

        if json.len() as u64 > MAX_STATE_SIZE {
            return Err("Save state exceeds size limit".to_string());
        }
        Ok(json)
    }

    /// Apply a serialized save state read by `read_state_file`.
    pub fn apply_state_json(&mut self, json: &str) -> Result<(), String> {
        let new_emulator = serde_json::from_str::<Self>(json)
            .map_err(|e| format!("Failed to parse save state: {}", e))?;
        self.adopt_state(new_emulator);
        Ok(())
    }

    pub fn load_state_from_path(&mut self, state_path: std::path::PathBuf) {
        match Self::read_state_file(&state_path).and_then(|json| self.apply_state_json(&json)) {
            Ok(()) => println!("Loaded state from {:?}", state_path),
            Err(e) => eprintln!("{}", e),
        }
    }

    /// Replace the machine state with a deserialized one while keeping
    /// everything that belongs to the session rather than the machine.
    fn adopt_state(&mut self, mut new_emulator: Self) {
        /* The ROM image is not serialized: re-attach the bytes that are in
         * memory now (they came through the validated loader), as loaded
         * rather than as a program may have rewritten SSF mapper memory.
         * restore_rom keeps the SRAM, mapper banks, mapper RAM and region
         * from the state. */
        let rom = self.bus.borrow_mut().take_rom_image();
        new_emulator.bus.borrow_mut().restore_rom(rom);
        let sample_rate = self.bus.borrow().sample_rate;
        new_emulator.bus.borrow_mut().sample_rate = sample_rate;

        /* The Z80 deserializes with a detached default bus; connect it to
         * the restored bus. */
        let z80_bus = Z80Bus::new(SharedBus::new(new_emulator.bus.clone()));
        new_emulator.z80.memory = z80_bus.clone();
        new_emulator.z80.io = z80_bus;

        /* Session state: frontend, debugger, recording and dump state. */
        std::mem::swap(&mut new_emulator.gdb, &mut self.gdb);
        std::mem::swap(&mut new_emulator.wav_writer, &mut self.wav_writer);
        std::mem::swap(&mut new_emulator.input, &mut self.input);
        std::mem::swap(&mut new_emulator.audio_buffer, &mut self.audio_buffer);
        std::mem::swap(&mut new_emulator.allowed_paths, &mut self.allowed_paths);
        std::mem::swap(
            &mut new_emulator.current_rom_path,
            &mut self.current_rom_path,
        );
        std::mem::swap(
            &mut new_emulator.pending_wav_dump,
            &mut self.pending_wav_dump,
        );
        new_emulator.input_mapping_from_cli = self.input_mapping_from_cli;
        new_emulator.input_mapping = self.input_mapping;
        new_emulator.debug = self.debug;
        new_emulator.paused = self.paused;
        new_emulator.single_step = self.single_step;
        new_emulator.gdb_exec = self.gdb_exec;
        new_emulator.gdb_skip_breakpoint = self.gdb_skip_breakpoint;

        /* The old machine must not write its SRAM over the restored one
         * when it is dropped below. */
        self.current_rom_path = None;
        *self = new_emulator;
    }

    /// Restrict ROM loading to files under `path` (canonicalized).
    ///
    /// With no roots configured, any file the user names may be loaded:
    /// the frontends load exactly the file given on the command line or
    /// picked in the file dialog. Embedders that load ROM paths from less
    /// trusted sources can configure roots, and paths outside every root
    /// are then denied.
    pub fn add_allowed_path<P: AsRef<std::path::Path>>(&mut self, path: P) -> std::io::Result<()> {
        let canonical = path.as_ref().canonicalize()?;
        self.allowed_paths.push(canonical);
        Ok(())
    }

    pub fn load_rom(&mut self, path: &str) -> std::io::Result<()> {
        let rom = Self::read_rom_image(path, &self.allowed_paths)?;
        self.install_loaded_rom(rom);
        Ok(())
    }

    /// Read and validate a ROM (allowed roots, zip extraction, size limit,
    /// SMD conversion) and its `.srm`, without touching emulator state, so
    /// it can run on a worker thread. Finish with `install_loaded_rom`.
    pub fn read_rom_image(
        path: &str,
        allowed_paths: &[std::path::PathBuf],
    ) -> std::io::Result<LoadedRom> {
        let canonical_path = std::path::Path::new(path).canonicalize()?;

        let allowed = allowed_paths.is_empty()
            || allowed_paths
                .iter()
                .any(|allowed_base| canonical_path.starts_with(allowed_base));
        if !allowed {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("Access denied to ROM path: {:?}", canonical_path),
            ));
        }

        /* Open the canonical path that was validated, once, and read
         * everything (archive or raw image) from that handle. */
        let mut file = std::fs::File::open(&canonical_path)?;
        let data = if has_extension(&canonical_path.to_string_lossy(), ".zip") {
            Self::load_rom_from_zip_reader(file)?
        } else {
            let size = file.metadata()?.len();
            let raw = Self::read_rom_with_limit(&mut file, size)?;
            let is_smd = has_extension(&canonical_path.to_string_lossy(), ".smd");
            Self::deinterleave_smd_if_needed(raw, is_smd)
        };
        let sram = Self::read_sram_file(&Self::derived_path(&canonical_path, "srm"));
        Ok(LoadedRom {
            path: canonical_path,
            data,
            sram,
        })
    }

    /// Install a ROM read by `read_rom_image` and reset the CPUs.
    pub fn install_loaded_rom(&mut self, rom: LoadedRom) {
        let mut bus = self.bus.borrow_mut();
        bus.load_rom(&rom.data);
        if let Some(image) = &rom.sram {
            bus.sram.load_image(image);
        }
        drop(bus);

        self.current_rom_path = Some(rom.path);

        // Reset again to load initial PC/SP from ROM vectors
        let mut bus = self.bus.borrow_mut();
        self.cpu.reset(&mut *bus);
        self.z80.reset();
    }

    /// Convert a Super Magic Drive (`.smd`) dump to a linear image.
    ///
    /// An SMD file is a 512-byte header (bytes 8 and 9 are 0xAA 0xBB)
    /// followed by 16 KiB blocks, each holding the block's odd bytes in its
    /// first half and its even bytes in the second half (Genesis Plus GX
    /// `deinterleave_block`). Images that do not match are returned as is.
    fn deinterleave_smd_if_needed(data: Vec<u8>, smd_extension: bool) -> Vec<u8> {
        const HEADER: usize = 512;
        const BLOCK: usize = 16 * 1024;
        let sized = data.len() > HEADER && data.len() % BLOCK == HEADER;
        let marked = data.len() > 9 && data[8] == 0xAA && data[9] == 0xBB;
        if !sized || !(marked || smd_extension) {
            return data;
        }
        let body = &data[HEADER..];
        let mut out = vec![0u8; body.len()];
        for (src, dst) in body.chunks_exact(BLOCK).zip(out.chunks_exact_mut(BLOCK)) {
            let (odd, even) = src.split_at(BLOCK / 2);
            for (i, pair) in dst.chunks_exact_mut(2).enumerate() {
                pair[0] = even[i];
                pair[1] = odd[i];
            }
        }
        out
    }

    fn read_rom_with_limit<R: std::io::Read>(
        reader: &mut R,
        size: u64,
    ) -> std::io::Result<Vec<u8>> {
        use std::io::Read;
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

        // Security: Do NOT use Vec::with_capacity(size) as 'size' comes from untrusted metadata
        // (e.g. ZIP file header) and could be used to trigger an OOM before any data is read.
        let mut data = Vec::new();
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
    #[cfg(test)]
    fn load_rom_from_zip(path: &str) -> std::io::Result<Vec<u8>> {
        Self::load_rom_from_zip_reader(std::fs::File::open(path)?)
    }

    /// Extract the first ROM entry (.bin, .md, .gen, .smd, .32x) from an
    /// already opened zip archive.
    fn load_rom_from_zip_reader(file: std::fs::File) -> std::io::Result<Vec<u8>> {
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        // ROM file extensions to look for
        let rom_extensions = [".bin", ".md", ".gen", ".smd", ".32x"];
        // Find first ROM file in archive
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

            let name = entry.name().to_string();
            let is_rom = rom_extensions.iter().any(|&ext| has_extension(&name, ext));

            if is_rom {
                let size = entry.size();
                if size > MAX_ROM_SIZE {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!(
                            "ROM size exceeds limit of {}MB",
                            MAX_ROM_SIZE / (1024 * 1024)
                        ),
                    ));
                }
                let data = Self::read_rom_with_limit(&mut entry, size)?;
                println!("Extracted ROM: {} ({} bytes)", name, data.len());
                return Ok(Self::deinterleave_smd_if_needed(
                    data,
                    has_extension(&name, ".smd"),
                ));
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No ROM file found in zip archive",
        ))
    }
    /// Step one frame with current input state
    ///
    /// While a GDB client is attached, the debugger controls execution
    /// instead of the pause/single-step flags (see `run_gdb_slice`); live
    /// controller input is still applied.
    pub fn step_frame(&mut self, input: Option<&input::FrameInput>) {
        if self.gdb_attached() {
            if let Some(i) = input {
                self.apply_controllers(i);
            }
            self.run_gdb_slice();
            return;
        }
        if self.paused && !self.single_step {
            return;
        }
        // Reset single_step if it was set
        self.single_step = false;

        // Apply inputs from script or live input (recorded when recording)
        let frame_input = self.input.next_frame(input);
        self.apply_controllers(&frame_input);

        // Handle commands (e.g., SCREENSHOT <path>)
        if let Some(cmd) = &frame_input.command {
            self.execute_script_command(cmd);
        }

        self.step_frame_internal();
    }

    fn apply_controllers(&mut self, input: &input::FrameInput) {
        let mut bus = self.bus.borrow_mut();
        if let Some(ctrl) = bus.io.controller(1) {
            *ctrl = input.p1;
        }
        if let Some(ctrl) = bus.io.controller(2) {
            *ctrl = input.p2;
        }
    }

    fn execute_script_command(&self, cmd: &str) {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() {
            return;
        }

        let cmd_upper = parts[0].to_uppercase();
        if let Some((op, width)) = ScriptMemOp::parse(&cmd_upper) {
            self.handle_memory_cmd(op, width, &parts);
            return;
        }
        match cmd_upper.as_str() {
            "SCREENSHOT" => self.handle_screenshot_cmd(&parts),
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
            // 1. Only allow saving to current directory by using only the file name component
            let file_name = std::path::Path::new(raw_path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("screenshot.png");

            // 2. Enforce .png extension to prevent writing to non-image files
            let mut path_buf = std::path::PathBuf::from(file_name);
            if path_buf.extension().and_then(|s| s.to_str()) != Some("png") {
                path_buf.set_extension("png");
            }
            let sanitized_path = path_buf.to_string_lossy().into_owned();
            let path = &sanitized_path;

            if path != raw_path {
                eprintln!(
                    "Script Warning: Sanitized screenshot path '{}' to '{}'",
                    raw_path, path
                );
            }

            // 3. Never overwrite existing files: the file is created with
            // create_new, so there is no check-then-create window.
            match self.save_screenshot_new(path) {
                Ok(()) => println!("Script: Saved screenshot to {}", path),
                Err(e) => eprintln!("Script Error: Failed to save screenshot to {}: {}", path, e),
            }
        }
    }

    /// READ/WRITE/ASSERT of a byte, word or long through the 68k bus.
    fn handle_memory_cmd(&self, op: ScriptMemOp, width: ScriptWidth, parts: &[&str]) {
        let parse = |s: &str| u32::from_str_radix(s.trim_start_matches("0x"), 16).ok();
        let Some(addr) = parts.get(1).and_then(|s| parse(s)) else {
            return;
        };
        let value = parts
            .get(2)
            .and_then(|s| parse(s))
            .filter(|&v| v <= width.max());
        let name = width.name();
        let digits = width.digits();
        let mut bus = self.bus.borrow_mut();
        match (op, value) {
            (ScriptMemOp::Read, _) => {
                let val = width.read(&mut bus, addr);
                println!(
                    "Script: READ_{} 0x{:06X} = 0x{:0digits$X}",
                    name,
                    addr,
                    val,
                    digits = digits
                );
            }
            (ScriptMemOp::Write, Some(val)) => {
                width.write(&mut bus, addr, val);
                println!(
                    "Script: WRITE_{} 0x{:06X} = 0x{:0digits$X}",
                    name,
                    addr,
                    val,
                    digits = digits
                );
            }
            (ScriptMemOp::Assert, Some(expected)) => {
                let actual = width.read(&mut bus, addr);
                if actual != expected {
                    eprintln!(
                        "Script Assertion Failed: [0x{:06X}] == 0x{:0digits$X} (Expected 0x{:0digits$X})",
                        addr,
                        actual,
                        expected,
                        digits = digits
                    );
                } else {
                    println!(
                        "Script: ASSERT_{} 0x{:06X} == 0x{:0digits$X} OK",
                        name,
                        addr,
                        expected,
                        digits = digits
                    );
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
    /// Run until the VDP reaches the next frame boundary.
    pub fn step_frame_internal(&mut self) {
        self.run_system(None);
    }

    /// Run the whole system (68k, VDP, Z80, APU, interrupts) until the VDP
    /// wraps to the next frame, or until `limit` stops execution.
    ///
    /// Scanline and frame boundaries are taken from the VDP's own
    /// master-clock counters, so resuming after a mid-frame stop continues
    /// exactly where execution left off.
    fn run_system(&mut self, mut limit: Option<&mut ExecLimit>) -> RunOutcome {
        loop {
            let line = self.run_scanline(limit.as_deref_mut());
            if line.frame_done {
                self.finish_frame();
                return if line.stopped {
                    RunOutcome::Stopped
                } else {
                    RunOutcome::FrameComplete
                };
            }
            if line.stopped {
                return RunOutcome::Stopped;
            }
        }
    }

    fn finish_frame(&mut self) {
        self.internal_frame_count += 1;
        if self.debug && self.internal_frame_count.is_multiple_of(60) {
            self.log_debug(self.internal_frame_count);
        }
        self.generate_audio_samples();
    }

    /// Execute until the VDP leaves its current scanline (or `limit` stops).
    ///
    /// Each CPU batch is sized to the master clocks remaining in the VDP's
    /// current line (MCLK_PER_LINE per line, 7 MCLK per 68k cycle); the
    /// VDP carries any overshoot of the last instruction into the next line.
    fn run_scanline(&mut self, mut limit: Option<&mut ExecLimit>) -> ScanlineOutcome {
        let start_line = self.bus.borrow().vdp.v_counter;
        let mut stopped = false;
        {
            let mut ctx = SystemContext {
                cpu: &mut self.cpu,
                bus_rc: &self.bus,
                z80: &mut self.z80,
                z80_cycle_debt: &mut self.z80_cycle_debt,
                z80_last_bus_req: &mut self.z80_last_bus_req,
                z80_last_reset: &mut self.z80_last_reset,
                z80_trace_count: &mut self.z80_trace_count,
                master_clock: &mut self.master_clock,
                internal_frame_count: self.internal_frame_count,
                debug: self.debug,
            };
            loop {
                let (line, line_clocks) = {
                    let bus = ctx.bus_rc.borrow();
                    (bus.vdp.v_counter, bus.vdp.mclk_line_clocks)
                };
                if line != start_line {
                    break;
                }
                let remaining = MCLK_PER_LINE
                    .saturating_sub(line_clocks)
                    .div_ceil(MCLK_PER_M68K_CYCLE)
                    .max(1);
                // Z80 VINT fires at the start of the first blanking line.
                let trigger_vint = std::mem::take(&mut self.z80_vint_pending);
                let result = Self::run_cpu_batch_static(
                    &mut ctx,
                    remaining,
                    trigger_vint,
                    limit.as_deref_mut(),
                );
                if result.stopped {
                    stopped = true;
                    if ctx.bus_rc.borrow().vdp.v_counter == start_line {
                        return ScanlineOutcome {
                            frame_done: false,
                            stopped,
                        };
                    }
                    break;
                }
            }
        }
        self.handle_interrupts();
        let (end_line, active_lines) = {
            let bus = self.bus.borrow();
            (bus.vdp.v_counter, bus.vdp.screen_height())
        };
        if end_line == active_lines {
            self.z80_vint_pending = true;
        }
        ScanlineOutcome {
            frame_done: end_line < start_line,
            stopped,
        }
    }

    fn sync_audio_z80(ctx: &mut SystemContext, m68k_cycles: u32, trigger_vint: bool) {
        if m68k_cycles == 0 && !trigger_vint {
            return;
        }

        let mclk = m68k_cycles * MCLK_PER_M68K_CYCLE;

        let (z80_can_run, z80_is_reset, cycles_per_sample) = {
            let bus = ctx.bus_rc.borrow();
            let prev = *ctx.z80_last_bus_req;
            if ctx.debug && bus.z80_bus_request != prev {
                log::debug!(
                    "Bus Req Changed: {} -> {} at 68k PC={:06X}",
                    prev,
                    bus.z80_bus_request,
                    ctx.cpu.pc
                );
            }
            *ctx.z80_last_bus_req = bus.z80_bus_request;

            let z80_can_run = bus.z80_can_run();
            let z80_is_reset = bus.z80_reset;
            let cycles_per_sample = if bus.vdp.is_pal {
                audio::PAL_MCLK as f64 / bus.sample_rate as f64
            } else {
                audio::NTSC_MCLK as f64 / bus.sample_rate as f64
            };
            (z80_can_run, z80_is_reset, cycles_per_sample)
        };

        if z80_is_reset && !*ctx.z80_last_reset {
            ctx.z80.reset();
        }
        *ctx.z80_last_reset = z80_is_reset;

        if z80_can_run && ctx.internal_frame_count > 0 {
            ctx.z80.debug = ctx.debug && *ctx.z80_trace_count < 5000;
            if ctx.z80.debug {
                *ctx.z80_trace_count += 1;
            }
        } else {
            ctx.z80.debug = false;
        }

        /* The VDP holds the Z80 INT line low for about one scanline; the Z80
        samples it at instruction boundaries inside step(), which also returns
        the acknowledge T-states so they are charged to the cycle budget. */
        const Z80_INT_HOLD_CYCLES: u32 = 228;
        if trigger_vint && !z80_is_reset {
            ctx.z80.assert_int_for(Z80_INT_HOLD_CYCLES, 0xFF);
        }

        const Z80_CYCLES_PER_M68K_CYCLE: f32 = 3579545.0 / 7670453.0;
        if z80_can_run {
            *ctx.z80_cycle_debt += m68k_cycles as f32 * Z80_CYCLES_PER_M68K_CYCLE;

            if *ctx.z80_cycle_debt >= 1.0 {
                /* Borrow the bus once for the whole run of Z80 instructions
                rather than once per memory access. */
                let mut bus = ctx.bus_rc.borrow_mut();
                let debt = &mut *ctx.z80_cycle_debt;
                ctx.z80
                    .with_bus(Z80BusView::new(&mut bus), Z80Ports, |z80| {
                        while *debt >= 1.0 {
                            *debt -= z80.step() as f32;
                        }
                    });
            }
        } else {
            /* While the 68000 holds BUSREQ or reset, the Z80 cannot respond,
            but the INT line still times out. */
            ctx.z80
                .advance_int_line((m68k_cycles as f32 * Z80_CYCLES_PER_M68K_CYCLE) as u32);
        }

        let mut bus = ctx.bus_rc.borrow_mut();
        let region = if bus.vdp.is_pal {
            Region::Pal
        } else {
            Region::Ntsc
        };
        let output_sample_rate = bus.sample_rate;
        bus.apu.set_timing(region, output_sample_rate);
        bus.apu.tick_cycles(m68k_cycles);
        bus.audio_accumulator += mclk as f64;

        while bus.audio_accumulator >= cycles_per_sample {
            let (l, r) = bus.apu.generate_sample();
            bus.audio_buffer.push(l);
            bus.audio_buffer.push(r);
            bus.audio_accumulator -= cycles_per_sample;
        }
    }

    /// Raise or cancel the 68k VINT (level 6) and HINT (level 4) requests
    /// to match the VDP's pending flags.
    fn update_m68k_irqs(cpu: &mut Cpu, bus: &Bus) {
        if bus.vdp.vblank_pending() {
            cpu.request_interrupt(6);
        } else {
            cpu.cancel_interrupt(6);
        }
        if bus.vdp.hint_pending() {
            cpu.request_interrupt(4);
        } else {
            cpu.cancel_interrupt(4);
        }
    }

    /// Bring the bus/VDP, IRQ lines, Z80 and APU up to date with
    /// `m68k_cycles` of 68k execution.
    fn sync_system(ctx: &mut SystemContext, m68k_cycles: u32, trigger_vint: bool) {
        if m68k_cycles > 0 {
            let mut bus = ctx.bus_rc.borrow_mut();
            bus.tick(m68k_cycles * MCLK_PER_M68K_CYCLE);
            Self::update_m68k_irqs(ctx.cpu, &bus);
        }
        Self::sync_audio_z80(ctx, m68k_cycles, trigger_vint);
    }

    fn run_cpu_batch_static(
        ctx: &mut SystemContext,
        max_cycles: u32,
        trigger_vint: bool,
        mut limit: Option<&mut ExecLimit>,
    ) -> CpuBatchResult {
        // R3: tighter HINT/VINT sensing during active display so the CPU
        // can ack HINT in time to update the road palette before line render.
        const Z80_AUDIO_SYNC_SLICE_ACTIVE: u32 = 8;
        const Z80_AUDIO_SYNC_SLICE_VBLANK: u32 = 32;
        let (initial_req, initial_rst) = {
            let bus = ctx.bus_rc.borrow();
            (bus.z80_bus_request, bus.z80_reset)
        };
        let mut pending_cycles = 0;
        let mut deferred_cycles = 0;
        let mut trigger_vint = trigger_vint;
        loop {
            let stopped = limit.as_deref().is_some_and(|l| l.should_stop(ctx.cpu.pc));
            if pending_cycles >= max_cycles || stopped {
                // Final sync for the batch
                Self::sync_system(ctx, deferred_cycles, std::mem::take(&mut trigger_vint));
                return CpuBatchResult { stopped };
            }

            let m68k_cycles = {
                let mut bus = ctx.bus_rc.borrow_mut();
                if bus.dma_active() {
                    2 // Yield 2 cycles to let the bus step during DMA
                } else {
                    let cycles = ctx.cpu.step_instruction(&mut *bus);
                    if let Some(l) = limit.as_deref_mut() {
                        l.instruction_executed();
                    }
                    cycles
                }
            };

            match ctx.cpu.last_interrupt_level {
                6 => ctx.bus_rc.borrow_mut().vdp.acknowledge_vint(),
                4 => ctx.bus_rc.borrow_mut().vdp.acknowledge_hint(),
                _ => {}
            }

            deferred_cycles += m68k_cycles;
            pending_cycles += m68k_cycles;
            *ctx.master_clock += u64::from(m68k_cycles * MCLK_PER_M68K_CYCLE);

            let slice = if ctx.bus_rc.borrow().vdp.in_active_display() {
                Z80_AUDIO_SYNC_SLICE_ACTIVE
            } else {
                Z80_AUDIO_SYNC_SLICE_VBLANK
            };
            let should_sync_audio =
                deferred_cycles >= slice || trigger_vint || ctx.bus_rc.borrow().dma_active();
            if should_sync_audio {
                Self::sync_system(ctx, deferred_cycles, std::mem::take(&mut trigger_vint));
                deferred_cycles = 0;
            }

            // Check for Z80 state change (rare but needs early exit)
            let (req, rst) = {
                let bus = ctx.bus_rc.borrow();
                (bus.z80_bus_request, bus.z80_reset)
            };
            if req != initial_req || rst != initial_rst {
                Self::sync_system(ctx, deferred_cycles, std::mem::take(&mut trigger_vint));
                return CpuBatchResult { stopped: false };
            }
        }
    }
    fn generate_audio_samples(&mut self) {
        let mut bus = self.bus.borrow_mut();
        if bus.audio_buffer.is_empty() {
            return;
        }
        if let Some(writer) = &mut self.wav_writer {
            let _ = writer.write_samples(&bus.audio_buffer);
        }
        // Keep a few frames of slack so transient frontend stalls do not
        // collapse audio into repeated whole-frame dropouts.
        const MAX_BUFFERED_AUDIO_FRAMES: usize = 8;
        let max_samples =
            audio::samples_per_frame_for_rate_and_region(bus.sample_rate, bus.vdp.is_pal)
                * 2
                * MAX_BUFFERED_AUDIO_FRAMES;
        let needed = self.audio_buffer.len() + bus.audio_buffer.len();
        if needed > max_samples {
            let to_drop = ((needed - max_samples) + 1) & !1;
            self.audio_buffer
                .drain(..to_drop.min(self.audio_buffer.len() & !1));
        }
        if (bus.audio_buffer.len() & 1) != 0 {
            bus.audio_buffer.pop();
        }
        self.audio_buffer.extend_from_slice(&bus.audio_buffer);
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

    /// The visible display as packed RGB8 plus its size. The framebuffer
    /// has a fixed 320-pixel stride; only the active width (256 in H32,
    /// 320 in H40) and height (224 or 240 lines) are returned.
    pub fn screenshot_rgb(&self) -> (Vec<u8>, u32, u32) {
        let bus = self.bus.borrow();
        let stride = frontend::GENESIS_WIDTH as usize;
        let width = usize::from(bus.vdp.screen_width()).min(stride);
        let height = usize::from(bus.vdp.screen_height()).min(bus.vdp.framebuffer.len() / stride);
        let mut rgb_data = Vec::with_capacity(width * height * 3);
        for row in bus.vdp.framebuffer.chunks_exact(stride).take(height) {
            for &pixel in &row[..width] {
                let r5 = ((pixel >> 11) & 0x1F) as u8;
                let g6 = ((pixel >> 5) & 0x3F) as u8;
                let b5 = (pixel & 0x1F) as u8;
                rgb_data.push((r5 << 3) | (r5 >> 2));
                rgb_data.push((g6 << 2) | (g6 >> 4));
                rgb_data.push((b5 << 3) | (b5 >> 2));
            }
        }
        (rgb_data, width as u32, height as u32)
    }

    fn write_screenshot(&self, file: std::fs::File) -> Result<(), String> {
        let (rgb, width, height) = self.screenshot_rgb();
        let mut writer = std::io::BufWriter::new(file);
        image::write_buffer_with_format(
            &mut writer,
            &rgb,
            width,
            height,
            image::ExtendedColorType::Rgb8,
            image::ImageFormat::Png,
        )
        .map_err(|e| e.to_string())
    }

    /// Save the visible display as PNG, replacing `path` if it exists.
    pub fn save_screenshot(&self, path: &str) -> Result<(), String> {
        let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
        self.write_screenshot(file)
    }

    /// Save the visible display as PNG to a new file; fails if `path`
    /// already exists (checked atomically by the open itself).
    pub fn save_screenshot_new(&self, path: &str) -> Result<(), String> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        self.write_screenshot(file)
    }
    /// Whether a GDB client is currently attached.
    pub fn gdb_attached(&self) -> bool {
        self.gdb.as_ref().is_some_and(|g| g.is_connected())
    }

    /// Accept a GDB client if none is attached, process its pending packets
    /// and apply run-control requests. Halts the target when a client
    /// attaches; an interrupt (0x03) halts a running target and sends a
    /// stop reply.
    pub fn poll_gdb(&mut self) {
        let Some(gdb) = self.gdb.as_mut() else {
            return;
        };

        if !gdb.is_connected() {
            if gdb.accept() {
                println!("GDB client connected");
                self.gdb_exec = GdbExecState::Halted;
            } else {
                return;
            }
        }

        let mut regs = GdbRegisters {
            d: self.cpu.d,
            a: self.cpu.a,
            sr: self.cpu.sr,
            pc: self.cpu.pc,
        };
        let mut mem_access = BusGdbMemory { bus: &self.bus };
        let request = gdb.service(&mut regs, &mut mem_access);
        self.cpu.d = regs.d;
        self.cpu.a = regs.a;
        if regs.sr != self.cpu.sr {
            self.cpu.set_sr(regs.sr);
        }
        self.cpu.pc = regs.pc;

        match request {
            Some(RunRequest::Continue) => {
                self.gdb_exec = GdbExecState::Running;
                /* Resuming from a breakpoint must execute that instruction. */
                self.gdb_skip_breakpoint = true;
            }
            Some(RunRequest::Step) => self.gdb_exec = GdbExecState::Stepping,
            Some(RunRequest::Interrupt) if self.gdb_exec != GdbExecState::Halted => {
                self.gdb_exec = GdbExecState::Halted;
                gdb.report_stop(StopReason::Interrupt);
            }
            Some(RunRequest::Interrupt) | None => {}
        }
    }

    /// Advance the full system under GDB control: nothing while halted, one
    /// 68k instruction when stepping, otherwise up to the next frame
    /// boundary, stopping before any breakpoint. Sends the stop reply when
    /// execution halts. Returns true when a frame boundary was reached.
    pub fn run_gdb_slice(&mut self) -> bool {
        let mode = self.gdb_exec;
        if mode == GdbExecState::Halted {
            return false;
        }
        let gdb = self.gdb.take();
        let (outcome, hit_breakpoint) = {
            let mut limit = ExecLimit {
                breakpoints: gdb.as_ref().map(|g| &g.breakpoints),
                instructions_left: (mode == GdbExecState::Stepping).then_some(1),
                skip_breakpoint_once: self.gdb_skip_breakpoint,
                hit_breakpoint: std::cell::Cell::new(false),
            };
            let outcome = self.run_system(Some(&mut limit));
            self.gdb_skip_breakpoint = limit.skip_breakpoint_once;
            (outcome, limit.hit_breakpoint.get())
        };
        self.gdb = gdb;
        if outcome == RunOutcome::Stopped {
            let reason = if hit_breakpoint {
                StopReason::Breakpoint
            } else {
                StopReason::Step
            };
            self.gdb_exec = GdbExecState::Halted;
            if let Some(gdb) = self.gdb.as_mut() {
                gdb.report_stop(reason);
            }
        }
        outcome == RunOutcome::FrameComplete
    }

    /// Run with GDB debugger attached (blocking loop)
    pub fn run_with_gdb(&mut self, port: u16, password: Option<String>) -> std::io::Result<()> {
        self.gdb = Some(GdbServer::new(port, password)?);
        println!("Waiting for GDB connection on port {}...", port);
        println!(
            "Connect with: m68k-elf-gdb -ex \"target remote :{}\" <elf_file>",
            port
        );
        while !self.gdb_attached() {
            self.poll_gdb();
            if !self.gdb_attached() {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        loop {
            self.poll_gdb();
            if !self.gdb_attached() {
                println!("GDB client disconnected");
                break;
            }
            if self.gdb_exec == GdbExecState::Halted {
                std::thread::sleep(std::time::Duration::from_millis(5));
            } else {
                self.run_gdb_slice();
                // Headless: nobody consumes audio.
                self.audio_buffer.clear();
            }
        }
        self.save_sram();
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
        let dma_pending = bus.vdp.command.dma_pending;
        let fifo_len = bus.vdp.fifo.len();
        let mode3 = bus.vdp.registers[crate::vdp::REG_MODE3];
        let mode4 = bus.vdp.registers[crate::vdp::REG_MODE4];
        let reg2 = bus.vdp.registers[crate::vdp::REG_PLANE_A];
        let reg4 = bus.vdp.registers[crate::vdp::REG_PLANE_B];
        let reg16 = bus.vdp.registers[crate::vdp::REG_PLANE_SIZE];
        let hscroll = bus.vdp.registers[crate::vdp::REG_HSCROLL];
        let win_h = bus.vdp.registers[crate::vdp::REG_WINDOW_H_POS];
        let win_v = bus.vdp.registers[crate::vdp::REG_WINDOW_V_POS];
        let hs_base = bus.vdp.hscroll_address();
        let plane_a = bus.vdp.plane_a_address();
        let plane_b = bus.vdp.plane_b_address();
        let window = bus.vdp.window_address();
        let hs_word = |addr: usize| -> u16 {
            let hi = bus.vdp.vram[addr & 0xFFFF];
            let lo = bus.vdp.vram[(addr + 1) & 0xFFFF];
            ((hi as u16) << 8) | (lo as u16)
        };
        let hs0a = hs_word(hs_base);
        let hs0b = hs_word(hs_base + 2);
        let hs80a = hs_word(hs_base + (80 * 4));
        let hs80b = hs_word(hs_base + (80 * 4) + 2);
        let z80_pc = self.z80.pc;
        let z80_reset = if bus.z80_reset { "RST" } else { "RUN" };
        let z80_req = if bus.z80_bus_request { "BUS" } else { "---" };
        let z80_op = if (z80_pc as usize) < bus.z80_ram.len() {
            bus.z80_ram[z80_pc as usize]
        } else {
            0
        };
        println_safe!(
            "FRAME {:05} | 68k: PC={:06X} SR={:04X} | VDP: Disp={} DMA={} DMAP={} FIFO={} CRAM={:04X} M3={:02X} M4={:02X} R2={:02X} R4={:02X} R16={:02X} HS={:02X} WH={:02X} WV={:02X} PA={:04X} PB={:04X} WN={:04X} HS0A={:04X} HS0B={:04X} HS80A={:04X} HS80B={:04X} | Z80: PC={:04X} OP={:02X} St={} Req={}",
            frame_count,
            self.cpu.pc,
            self.cpu.sr,
            disp_en,
            dma_en,
            dma_pending,
            fifo_len,
            cram_val,
            mode3,
            mode4,
            reg2,
            reg4,
            reg16,
            hscroll,
            win_h,
            win_v,
            plane_a,
            plane_b,
            window,
            hs0a,
            hs0b,
            hs80a,
            hs80b,
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
/// Input-script memory command kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptMemOp {
    Read,
    Write,
    Assert,
}

/// Access width of an input-script memory command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptWidth {
    Byte,
    Word,
    Long,
}

impl ScriptMemOp {
    /// Parse `READ_BYTE`, `WRITE_WORD`, `ASSERT_LONG`, ... (upper case).
    fn parse(cmd: &str) -> Option<(Self, ScriptWidth)> {
        let (op, width) = cmd.split_once('_')?;
        let op = match op {
            "READ" => Self::Read,
            "WRITE" => Self::Write,
            "ASSERT" => Self::Assert,
            _ => return None,
        };
        let width = match width {
            "BYTE" => ScriptWidth::Byte,
            "WORD" => ScriptWidth::Word,
            "LONG" => ScriptWidth::Long,
            _ => return None,
        };
        Some((op, width))
    }
}

impl ScriptWidth {
    fn name(self) -> &'static str {
        match self {
            Self::Byte => "BYTE",
            Self::Word => "WORD",
            Self::Long => "LONG",
        }
    }
    fn digits(self) -> usize {
        match self {
            Self::Byte => 2,
            Self::Word => 4,
            Self::Long => 8,
        }
    }
    fn max(self) -> u32 {
        match self {
            Self::Byte => 0xFF,
            Self::Word => 0xFFFF,
            Self::Long => u32::MAX,
        }
    }
    fn read(self, bus: &mut Bus, addr: u32) -> u32 {
        match self {
            Self::Byte => u32::from(bus.read_byte(addr)),
            Self::Word => u32::from(bus.read_word(addr)),
            Self::Long => bus.read_long(addr),
        }
    }
    fn write(self, bus: &mut Bus, addr: u32, value: u32) {
        match self {
            Self::Byte => bus.write_byte(addr, value as u8),
            Self::Word => bus.write_word(addr, value as u16),
            Self::Long => bus.write_long(addr, value),
        }
    }
}

/// Case-insensitive file-extension test that compares bytes, so names with
/// multi-byte UTF-8 characters are handled without slicing a character.
fn has_extension(name: &str, ext: &str) -> bool {
    let (n, e) = (name.as_bytes(), ext.as_bytes());
    n.len() >= e.len() && n[n.len() - e.len()..].eq_ignore_ascii_case(e)
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
    println!("  --gdb-password <pass>  Set password for GDB server authentication");
    println!("  --gdb-password-file <path> Read GDB password from a file");
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
/// GDB memory accessor for Bus. Uses the side-effect-free debugger accessors
/// so inspecting memory never disturbs VDP, I/O or Z80 bus state.
struct BusGdbMemory<'a> {
    bus: &'a std::cell::RefCell<Bus>,
}
impl<'a> GdbMemory for BusGdbMemory<'a> {
    fn read_byte(&mut self, addr: u32) -> u8 {
        self.bus.borrow().peek_byte(addr)
    }
    fn write_byte(&mut self, addr: u32, value: u8) {
        self.bus.borrow_mut().poke_byte(addr, value);
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
    /// Set only when `--input-mapping` was given.
    input_mapping: Option<InputMapping>,
    debug: bool,
    show_help: bool,
    /// Usage errors found while parsing.
    errors: Vec<String>,
}
impl Config {
    fn from_args<I>(args: I) -> Self
    where
        I: IntoIterator<Item = String>,
    {
        let mut config = Config::default();
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
                "--gdb-password" => {
                    config.gdb_password = iter.next();
                    current_opt = iter.next();
                }
                "--gdb-password-file" => {
                    match iter.next() {
                        Some(path) => match std::fs::read_to_string(&path) {
                            Ok(password) => config.gdb_password = Some(password.trim().to_string()),
                            Err(e) => config
                                .errors
                                .push(format!("cannot read GDB password file {}: {}", path, e)),
                        },
                        None => config
                            .errors
                            .push("--gdb-password-file needs a path".to_string()),
                    }
                    current_opt = iter.next();
                }
                "--dump-audio" => {
                    config.dump_audio_path = iter.next();
                    current_opt = iter.next();
                }
                "--input-mapping" => {
                    if let Some(mapping_str) = iter.next() {
                        config.input_mapping = Some(match mapping_str.to_lowercase().as_str() {
                            "ergonomic" | "modern" | "wasd" => InputMapping::Ergonomic,
                            _ => InputMapping::Original,
                        });
                    }
                    current_opt = iter.next();
                }
                "--debug" => {
                    config.debug = true;
                    current_opt = iter.next();
                }
                arg if !arg.starts_with('-') => {
                    if let Some(ref first) = config.rom_path {
                        config.errors.push(format!(
                            "more than one ROM path given ({:?} and {:?}); quote paths that contain spaces",
                            first, arg
                        ));
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
        if config.headless
            && config.headless_frames.is_none()
            && config.script_path.is_none()
            && config.gdb_port.is_none()
        {
            config.errors.push(
                "--headless needs a frame count (--headless <N>), a --script, or --gdb".to_string(),
            );
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
    if !config.errors.is_empty() {
        for e in &config.errors {
            eprintln!("Error: {}", e);
        }
        eprintln!("Run with --help for usage.");
        std::process::exit(2);
    }
    let rom_path = config.rom_path;
    let script_path = config.script_path;
    let gdb_port = config.gdb_port;
    let mut emulator = Emulator::new();
    emulator.input_mapping = config.input_mapping.unwrap_or_default();
    emulator.input_mapping_from_cli = config.input_mapping.is_some();
    emulator.debug = config.debug;
    emulator.pending_wav_dump = config.dump_audio_path;
    // Load ROM if provided
    if let Some(ref path) = rom_path {
        println!("Loading ROM: {}", path);
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
            emulator.start_pending_wav_dump();
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
        emulator.start_pending_wav_dump();
        emulator.run(
            config.headless_frames,
            config.screenshot_path,
            config.record_path,
        );
    } else {
        // Interactive mode; the WAV dump starts once the audio device rate
        // is known.
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
        let z80_code = [0x3E, 0x80, 0x32, 0xFD, 0x1F, 0x76];
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

        // Test GDB password flags
        let args = vec![
            "genteel".to_string(),
            "--gdb-password".to_string(),
            "secret_pwd".to_string(),
            "rom.bin".to_string(),
        ];
        let config = Config::from_args(args);
        assert_eq!(config.gdb_password, Some("secret_pwd".to_string()));

        let pwd_file = "test_gdb_pwd.txt";
        std::fs::write(pwd_file, " file_secret \n").unwrap();
        let args = vec![
            "genteel".to_string(),
            "--gdb-password-file".to_string(),
            pwd_file.to_string(),
            "rom.bin".to_string(),
        ];
        let config = Config::from_args(args);
        assert_eq!(config.gdb_password, Some("file_secret".to_string()));
        let _ = std::fs::remove_file(pwd_file);

        // Verify environment variable is NO LONGER used
        std::env::set_var("GENTEEL_GDB_PASSWORD", "should_be_ignored");
        let args = vec!["genteel".to_string(), "rom.bin".to_string()];
        let config = Config::from_args(args);
        assert_eq!(config.gdb_password, None);
        std::env::remove_var("GENTEEL_GDB_PASSWORD");

        let args = vec![
            "genteel".to_string(),
            "rom".to_string(),
            "part2.bin".to_string(),
        ];
        let config = Config::from_args(args);
        assert_eq!(config.rom_path, Some("rom".to_string()));
        assert_eq!(
            config.errors.len(),
            1,
            "extra positional must be a usage error"
        );
    }
    #[test]
    fn test_step_frame_basic() {
        let mut emulator = Emulator::new();
        emulator.step_frame(None);
        emulator.step_frame(None);
        assert_eq!(emulator.internal_frame_count, 2);
    }
    #[test]
    fn test_read_rom_with_limit_capacity_vulnerability_fixed() {
        let reported_size = MAX_ROM_SIZE;
        let actual_data = b"small rom data";
        let mut reader = std::io::Cursor::new(actual_data);

        // This should not fail, but we want to inspect the capacity of the returned Vec
        let result = Emulator::read_rom_with_limit(&mut reader, reported_size);
        let data = result.expect("Should succeed with small data");

        // After the fix, capacity should NOT be the pre-allocated reported_size
        assert!(
            data.capacity() < reported_size as usize,
            "Vulnerability STILL PRESENT: Capacity {} is >= reported size {}",
            data.capacity(),
            reported_size
        );
        assert_eq!(data.len(), actual_data.len());
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
        let inside = scratch_dir("allow-inside");
        let outside = scratch_dir("allow-outside");
        let rom_in = inside.join("in.bin");
        let rom_out = outside.join("out.bin");
        std::fs::write(&rom_in, idle_loop_rom()).unwrap();
        std::fs::write(&rom_out, idle_loop_rom()).unwrap();

        /* No roots configured: the file the user named is loaded. */
        let mut emulator = Emulator::new();
        assert!(emulator.load_rom(rom_out.to_str().unwrap()).is_ok());

        /* Roots configured: paths outside every root are denied, including
         * via `..` traversal out of an allowed root. */
        let mut emulator = Emulator::new();
        emulator.add_allowed_path(&inside).unwrap();
        let result = emulator.load_rom(rom_out.to_str().unwrap());
        assert_eq!(
            result.err().map(|e| e.kind()),
            Some(std::io::ErrorKind::PermissionDenied)
        );
        let traversal = inside
            .join("..")
            .join(outside.file_name().unwrap())
            .join("out.bin");
        assert!(emulator.load_rom(traversal.to_str().unwrap()).is_err());
        assert!(emulator.load_rom(rom_in.to_str().unwrap()).is_ok());

        let _ = std::fs::remove_dir_all(inside);
        let _ = std::fs::remove_dir_all(outside);
    }

    #[test]
    fn test_screenshot_path_sanitization() {
        let mut emulator = Emulator::new();

        // 1. Test directory traversal and extension enforcement
        let exploit_path = "/tmp/genteel_exploit.txt";
        let expected_path = "genteel_exploit.png";

        // Ensure files don't exist
        if std::path::Path::new(exploit_path).exists() {
            let _ = std::fs::remove_file(exploit_path);
        }
        if std::path::Path::new(expected_path).exists() {
            let _ = std::fs::remove_file(expected_path);
        }

        let input = crate::input::FrameInput {
            command: Some(format!("SCREENSHOT {}", exploit_path)),
            ..Default::default()
        };

        emulator.step_frame(Some(&input));

        // Check directory traversal is blocked
        if std::path::Path::new(exploit_path).exists() {
            let _ = std::fs::remove_file(exploit_path);
            panic!(
                "Vulnerability still present: file created at {}",
                exploit_path
            );
        }

        // Check sanitized behavior and extension enforcement
        if std::path::Path::new(expected_path).exists() {
            let _ = std::fs::remove_file(expected_path);
        } else {
            panic!("Sanitization failed: file not created at {}", expected_path);
        }

        // 2. Test overwrite prevention
        let protected_file = "protected.txt";
        let protected_content = "original content";
        std::fs::write(protected_file, protected_content).unwrap();

        // Try to overwrite it by requesting a screenshot with that name
        // (even if we request .png, if we request protected.txt it should be sanitized to protected.txt.png if we are not careful,
        // or if we request protected.txt it will be sanitized to protected.png.
        // Let's test with a name that won't be changed by sanitization to be sure we test overwrite prevention logic)
        let screenshot_name = "test_exists.png";
        std::fs::write(screenshot_name, "do not overwrite").unwrap();

        let input2 = crate::input::FrameInput {
            command: Some(format!("SCREENSHOT {}", screenshot_name)),
            ..Default::default()
        };

        emulator.step_frame(Some(&input2));

        // Content should still be "do not overwrite"
        let content = std::fs::read_to_string(screenshot_name).unwrap();
        assert_eq!(content, "do not overwrite", "Overwrite prevention failed!");

        // Cleanup
        let _ = std::fs::remove_file(protected_file);
        let _ = std::fs::remove_file(screenshot_name);
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

        let initial_sram_len = emulator.bus.borrow().sram.data.len();

        // Attempt to load - should fail/print warning and not change sram contents or crash
        emulator.load_sram();

        // SRAM size should not have changed to the large size
        assert_eq!(emulator.bus.borrow().sram.data.len(), initial_sram_len);

        // Cleanup
        let _ = std::fs::remove_file(srm_path);
    }

    #[test]
    fn test_large_state_prevention() {
        let path = std::path::PathBuf::from("test_large.state");
        // Create a file one byte larger than MAX_STATE_SIZE.
        // Using valid UTF-8 data (spaces) to exercise the size check specifically.
        let mut file = std::fs::File::create(&path).unwrap();
        let chunk = vec![b' '; 1024 * 1024]; // 1MB chunk
        for _ in 0..MAX_STATE_SIZE / chunk.len() as u64 {
            std::io::Write::write_all(&mut file, &chunk).unwrap();
        }
        std::io::Write::write_all(&mut file, b" ").unwrap(); // last byte
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

    /// Program that idles forever (BRA.S *) so frame timing does not depend
    /// on executing garbage.
    fn idle_loop_rom() -> Vec<u8> {
        let mut rom = vec![0u8; 0x400];
        rom[0..4].copy_from_slice(&0x00FF_FFF0u32.to_be_bytes());
        rom[4..8].copy_from_slice(&0x0000_0200u32.to_be_bytes());
        rom[0x200..0x202].copy_from_slice(&[0x60, 0xFE]);
        rom
    }

    fn emulator_with_rom(rom: &[u8]) -> Emulator {
        let mut emulator = Emulator::new();
        emulator.bus.borrow_mut().load_rom(rom);
        let mut bus = emulator.bus.borrow_mut();
        emulator.cpu.reset(&mut *bus);
        drop(bus);
        emulator
    }

    #[test]
    fn test_frames_end_on_vdp_frame_boundary() {
        for &pal in &[false, true] {
            let mut emulator = emulator_with_rom(&idle_loop_rom());
            emulator.bus.borrow_mut().vdp.set_pal(pal);
            for frame in 0..600 {
                emulator.step_frame_internal();
                let bus = emulator.bus.borrow();
                assert_eq!(
                    bus.vdp.v_counter, 0,
                    "frame {} (pal={}) ended off the VDP frame boundary",
                    frame, pal
                );
                /* Overshoot past the boundary is at most one instruction. */
                assert!(bus.vdp.mclk_line_clocks < 7 * 200);
            }
        }
    }

    #[test]
    fn test_scheduler_accumulates_master_clocks_per_line() {
        let mut emulator = emulator_with_rom(&idle_loop_rom());
        /* Ten emulated seconds: any per-line rounding loss would already
         * show as a mismatch, and a debug build runs this in seconds. */
        let frames = 600u64;
        for _ in 0..frames {
            emulator.step_frame_internal();
        }
        let lines = 262u64;
        let carried = u64::from(emulator.bus.borrow().vdp.mclk_line_clocks);
        /* Every master clock executed is accounted to the VDP: whole lines
         * plus the overshoot carried into the next line. */
        assert_eq!(
            emulator.master_clock,
            frames * lines * u64::from(MCLK_PER_LINE) + carried
        );
        assert_eq!(emulator.internal_frame_count, frames);
    }

    /// A per-test scratch directory under the system temp dir.
    fn scratch_dir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("genteel-test-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_zip(path: &std::path::Path, entry: &str, data: &[u8]) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        zip.start_file(entry, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(data).unwrap();
        zip.finish().unwrap();
    }

    #[test]
    fn test_zip_entry_with_multibyte_name_loads_without_panic() {
        let dir = scratch_dir("zipname");
        let zip_path = dir.join("game.zip");
        write_zip(&zip_path, "\u{e9}\u{e9}", b"not a rom");
        let zip2 = dir.join("game2.zip");
        write_zip(&zip2, "\u{e9}\u{e9}.md", &idle_loop_rom());
        let mut emulator = Emulator::new();
        emulator.add_allowed_path(&dir).unwrap();
        assert!(emulator.load_rom(zip_path.to_str().unwrap()).is_err());
        emulator.load_rom(zip2.to_str().unwrap()).unwrap();
        assert_eq!(emulator.cpu.pc, 0x200);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Interleave a linear image into SMD format (header + 16 KiB blocks of
    /// odd bytes then even bytes).
    fn to_smd(linear: &[u8]) -> Vec<u8> {
        let mut out = vec![0u8; 512];
        out[0] = (linear.len() / 16384) as u8;
        out[1] = 3;
        out[8] = 0xAA;
        out[9] = 0xBB;
        for block in linear.chunks(16384) {
            let odd: Vec<u8> = block.iter().skip(1).step_by(2).copied().collect();
            let even: Vec<u8> = block.iter().step_by(2).copied().collect();
            out.extend_from_slice(&odd);
            out.extend_from_slice(&even);
        }
        out
    }

    #[test]
    fn test_smd_rom_is_deinterleaved() {
        let mut linear = idle_loop_rom();
        linear.resize(32 * 1024, 0);
        for (i, b) in linear.iter_mut().enumerate().skip(0x400) {
            *b = (i * 7) as u8;
        }
        let dir = scratch_dir("smd");
        let path = dir.join("game.smd");
        std::fs::write(&path, to_smd(&linear)).unwrap();
        let zip_path = dir.join("game.zip");
        write_zip(&zip_path, "game.smd", &to_smd(&linear));
        for p in [&path, &zip_path] {
            let mut emulator = Emulator::new();
            emulator.add_allowed_path(&dir).unwrap();
            emulator.load_rom(p.to_str().unwrap()).unwrap();
            assert_eq!(emulator.bus.borrow().rom, linear, "{:?}", p);
            assert_eq!(emulator.cpu.pc, 0x200);
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_plain_bin_with_smd_like_size_is_not_deinterleaved() {
        let mut linear = idle_loop_rom();
        linear.resize(16 * 1024 + 512, 0x11);
        let dir = scratch_dir("notsmd");
        let path = dir.join("game.bin");
        std::fs::write(&path, &linear).unwrap();
        let mut emulator = Emulator::new();
        emulator.add_allowed_path(&dir).unwrap();
        emulator.load_rom(path.to_str().unwrap()).unwrap();
        assert_eq!(emulator.bus.borrow().rom, linear);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Run `f` on a thread with a large stack: deserializing an `Emulator`
    /// (inline VRAM and other large arrays) needs more than the default
    /// 2 MiB test-thread stack in unoptimized builds.
    fn with_big_stack(f: fn()) {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(f)
            .unwrap()
            .join()
            .unwrap();
    }

    /// Idle-loop ROM whose header declares battery-backed SRAM at
    /// $200001-$203FFF (odd lane).
    fn battery_rom() -> Vec<u8> {
        let mut rom = idle_loop_rom();
        rom[0x1B0..0x1B4].copy_from_slice(&[b'R', b'A', 0xF8, 0x20]);
        rom[0x1B4..0x1B8].copy_from_slice(&0x0020_0001u32.to_be_bytes());
        rom[0x1B8..0x1BC].copy_from_slice(&0x0020_3FFFu32.to_be_bytes());
        rom
    }

    fn load_from_dir(dir: &std::path::Path, name: &str, rom: &[u8]) -> Emulator {
        let path = dir.join(name);
        std::fs::write(&path, rom).unwrap();
        let mut emulator = Emulator::new();
        emulator.load_rom(path.to_str().unwrap()).unwrap();
        emulator
    }

    #[test]
    fn test_state_load_keeps_saved_sram() {
        with_big_stack(test_state_load_keeps_saved_sram_body);
    }

    fn test_state_load_keeps_saved_sram_body() {
        let dir = scratch_dir("state-sram");
        let mut emulator = load_from_dir(&dir, "game.bin", &battery_rom());
        emulator.bus.borrow_mut().write_byte(0x200001, 0x42);
        assert_eq!(emulator.bus.borrow_mut().read_byte(0x200001), 0x42);
        emulator.save_state(1).unwrap();
        emulator.bus.borrow_mut().write_byte(0x200001, 0x17);
        emulator.load_state(1).unwrap();
        assert_eq!(emulator.bus.borrow_mut().read_byte(0x200001), 0x42);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_state_load_restores_ssf_mapper_banks_and_ram() {
        with_big_stack(test_state_load_restores_ssf_mapper_banks_and_ram_body);
    }

    fn test_state_load_restores_ssf_mapper_banks_and_ram_body() {
        let dir = scratch_dir("state-ssf");
        let mut rom = idle_loop_rom();
        rom.resize(0x10_0000, 0x77);
        rom[0x100..0x108].copy_from_slice(b"SEGA SSF");
        let mut emulator = load_from_dir(&dir, "game.bin", &rom);
        {
            let mut bus = emulator.bus.borrow_mut();
            bus.write_byte(0xA130F0, 0xA0); // P + W: window becomes writable
            bus.write_byte(0xA130F3, 0x05); // slot 1 -> bank 5 (RAM)
            bus.write_byte(0x080010, 0x42); // RAM in bank 5
            bus.write_byte(0x000400, 0x99); // over a ROM byte
        }
        emulator.save_state(2).unwrap();
        {
            let mut bus = emulator.bus.borrow_mut();
            bus.write_byte(0xA130F3, 0x01);
            bus.write_byte(0x080010, 0x00);
            bus.write_byte(0x000400, 0x00);
            bus.write_byte(0x003000, 0x11); // a page the state never saw
        }
        emulator.load_state(2).unwrap();
        let mut bus = emulator.bus.borrow_mut();
        assert_eq!(bus.everdrive.banks[1], 5);
        assert_eq!(bus.read_byte(0x080010), 0x42);
        assert_eq!(bus.read_byte(0x000400), 0x99);
        assert_eq!(bus.read_byte(0x003000), 0x77, "unsaved write is undone");
        assert!(bus.rom.is_empty(), "still one copy of the image");
        drop(bus);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_state_load_keeps_rom_from_zip() {
        with_big_stack(test_state_load_keeps_rom_from_zip_body);
    }

    fn test_state_load_keeps_rom_from_zip_body() {
        let dir = scratch_dir("state-zip");
        let zip_path = dir.join("game.zip");
        write_zip(&zip_path, "game.bin", &idle_loop_rom());
        let mut emulator = Emulator::new();
        emulator.load_rom(zip_path.to_str().unwrap()).unwrap();
        let rom = emulator.bus.borrow().rom.clone();
        emulator.save_state(0).unwrap();
        emulator.load_state(0).unwrap();
        assert_eq!(emulator.bus.borrow().rom, rom);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_state_load_rewires_z80_to_the_live_bus() {
        with_big_stack(test_state_load_rewires_z80_to_the_live_bus_body);
    }

    fn test_state_load_rewires_z80_to_the_live_bus_body() {
        let dir = scratch_dir("state-z80");
        let mut emulator = load_from_dir(&dir, "game.bin", &idle_loop_rom());
        emulator.save_state(0).unwrap();
        emulator.load_state(0).unwrap();
        crate::memory::MemoryInterface::write_byte(&mut emulator.z80.memory, 0x0123, 0x99);
        assert_eq!(emulator.bus.borrow().z80_ram[0x0123], 0x99);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_state_load_preserves_session_state() {
        with_big_stack(test_state_load_preserves_session_state_body);
    }

    fn test_state_load_preserves_session_state_body() {
        let dir = scratch_dir("state-session");
        let mut emulator = load_from_dir(&dir, "game.bin", &idle_loop_rom());
        emulator.save_state(0).unwrap();
        emulator.input.start_recording();
        emulator.step_frame(None);
        emulator.debug = true;
        emulator.input_mapping = InputMapping::Ergonomic;
        emulator.paused = true;
        emulator.load_state(0).unwrap();
        assert!(emulator.debug);
        assert!(emulator.paused);
        assert_eq!(emulator.input_mapping, InputMapping::Ergonomic);
        emulator.paused = false;
        emulator.step_frame(None);
        let script = emulator.input.stop_recording();
        assert!(script.get(0).is_some() && script.get(1).is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_state_load_continues_wav_dump() {
        with_big_stack(test_state_load_continues_wav_dump_body);
    }

    fn test_state_load_continues_wav_dump_body() {
        let dir = scratch_dir("state-wav");
        let mut emulator = load_from_dir(&dir, "game.bin", &idle_loop_rom());
        let rate = emulator.bus.borrow().sample_rate;
        emulator.wav_writer =
            Some(wav_writer::WavWriter::new_in(&dir, "dump.wav", rate, 2).unwrap());
        emulator.save_state(0).unwrap();
        emulator.load_state(0).unwrap();
        assert!(
            emulator.wav_writer.is_some(),
            "WAV dump dropped by state load"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_slot_out_of_range_is_an_error() {
        let dir = scratch_dir("state-slot");
        let mut emulator = load_from_dir(&dir, "game.bin", &idle_loop_rom());
        assert!(emulator.save_state(10).is_err());
        assert!(emulator.load_state(255).is_err());
        assert!(emulator.delete_state(10).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_sram_and_state_paths_never_equal_rom_path() {
        let dir = scratch_dir("state-paths");
        for name in ["game.srm", "game.s3"] {
            let rom = battery_rom();
            let emulator = load_from_dir(&dir, name, &rom);
            emulator.bus.borrow_mut().write_byte(0x200001, 0x42);
            emulator.save_sram();
            emulator.save_state(3).unwrap();
            assert_eq!(
                std::fs::read(dir.join(name)).unwrap(),
                rom,
                "{} overwritten",
                name
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_write_atomic_replaces_file_without_leftovers() {
        let dir = scratch_dir("atomic");
        let target = dir.join("out.bin");
        std::fs::write(&target, b"old contents that are longer").unwrap();
        write_atomic(&target, b"new").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        let entries: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
        assert_eq!(entries.len(), 1, "temporary file left behind");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_no_srm_file_without_sram_writes() {
        let dir = scratch_dir("no-srm");
        let emulator = load_from_dir(&dir, "plain.bin", &idle_loop_rom());
        emulator.save_sram();
        assert!(!dir.join("plain.srm").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_sram_is_saved_when_emulator_is_dropped() {
        let dir = scratch_dir("srm-drop");
        {
            let emulator = load_from_dir(&dir, "game.bin", &battery_rom());
            emulator.bus.borrow_mut().write_byte(0x200001, 0x42);
        }
        assert!(dir.join("game.srm").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_largest_state_round_trips() {
        with_big_stack(test_largest_state_round_trips_body);
    }

    fn test_largest_state_round_trips_body() {
        let dir = scratch_dir("state-max");
        let mut emulator = load_from_dir(&dir, "game.bin", &battery_rom());
        {
            let mut bus = emulator.bus.borrow_mut();
            bus.work_ram.fill(0xFF);
            bus.z80_ram.fill(0xFF);
            bus.sram.data.fill(0xEE);
            bus.vdp.vram.fill(0xFF);
        }
        emulator.save_state(2).unwrap();
        let size = std::fs::metadata(emulator.state_path(2).unwrap())
            .unwrap()
            .len();
        assert!(size < MAX_STATE_SIZE / 4, "state is {} bytes", size);
        emulator.bus.borrow_mut().work_ram.fill(0);
        emulator.load_state(2).unwrap();
        assert!(emulator.bus.borrow().work_ram.iter().all(|&b| b == 0xFF));
        let _ = std::fs::remove_dir_all(dir);
    }

    mod gdb_session {
        use super::*;
        use std::io::Read;
        use std::net::TcpStream;
        use std::time::Duration;

        /// NOP sled at the reset vector followed by an idle loop.
        fn nop_rom() -> Vec<u8> {
            let mut rom = idle_loop_rom();
            for i in 0..8 {
                rom[0x200 + i * 2..0x202 + i * 2].copy_from_slice(&[0x4E, 0x71]);
            }
            rom[0x210..0x212].copy_from_slice(&[0x60, 0xFE]);
            rom
        }

        fn send(client: &mut TcpStream, data: &str) {
            client.write_all(&GdbServer::frame_packet(data)).unwrap();
        }

        /// Read the next `$...#xx` packet payload, skipping acks.
        fn read_packet(client: &mut TcpStream) -> String {
            client
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut byte = [0u8; 1];
            loop {
                client.read_exact(&mut byte).unwrap();
                if byte[0] == b'$' {
                    break;
                }
            }
            let mut payload = Vec::new();
            loop {
                client.read_exact(&mut byte).unwrap();
                if byte[0] == b'#' {
                    break;
                }
                payload.push(byte[0]);
            }
            let mut cs = [0u8; 2];
            client.read_exact(&mut cs).unwrap();
            String::from_utf8(payload).unwrap()
        }

        /// Poll until the emulator has consumed a request.
        fn poll_until<F: Fn(&Emulator) -> bool>(emulator: &mut Emulator, done: F) {
            for _ in 0..400 {
                emulator.poll_gdb();
                if done(emulator) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            panic!("GDB request not processed");
        }

        fn attach(rom: &[u8]) -> (Emulator, TcpStream) {
            let mut emulator = emulator_with_rom(rom);
            emulator.gdb = Some(GdbServer::new(0, Some("pw".to_string())).unwrap());
            let port = emulator.gdb.as_ref().unwrap().port();
            let mut client = TcpStream::connect(("127.0.0.1", port)).unwrap();
            poll_until(&mut emulator, |e| e.gdb_attached());
            let auth: String = "auth pw".bytes().map(|b| format!("{:02x}", b)).collect();
            send(&mut client, &format!("qRcmd,{}", auth));
            let mut peek = [0u8; 8];
            client.set_nonblocking(true).unwrap();
            for _ in 0..400 {
                emulator.poll_gdb();
                if client.peek(&mut peek).is_ok_and(|n| n >= 7) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            client.set_nonblocking(false).unwrap();
            assert_eq!(read_packet(&mut client), "OK");
            (emulator, client)
        }

        #[test]
        fn attach_halts_the_target() {
            let (mut emulator, _client) = attach(&nop_rom());
            assert_eq!(emulator.gdb_exec, GdbExecState::Halted);
            let pc = emulator.cpu.pc;
            for _ in 0..3 {
                emulator.step_frame(None);
            }
            assert_eq!(emulator.cpu.pc, pc);
            assert_eq!(emulator.internal_frame_count, 0);
        }

        #[test]
        fn step_executes_exactly_one_instruction_and_replies() {
            let (mut emulator, mut client) = attach(&nop_rom());
            assert_eq!(emulator.cpu.pc, 0x200);
            send(&mut client, "s");
            poll_until(&mut emulator, |e| e.gdb_exec == GdbExecState::Stepping);
            emulator.step_frame(None);
            assert_eq!(emulator.cpu.pc, 0x202);
            assert_eq!(emulator.gdb_exec, GdbExecState::Halted);
            assert_eq!(read_packet(&mut client), "S05");
        }

        #[test]
        fn continue_runs_full_system_and_stops_at_breakpoint() {
            let (mut emulator, mut client) = attach(&nop_rom());
            send(&mut client, "Z0,20a,2");
            poll_until(&mut emulator, |e| {
                e.gdb.as_ref().is_some_and(|g| g.is_breakpoint(0x20A))
            });
            assert_eq!(read_packet(&mut client), "OK");
            send(&mut client, "c");
            poll_until(&mut emulator, |e| e.gdb_exec == GdbExecState::Running);
            emulator.step_frame(None);
            assert_eq!(emulator.cpu.pc, 0x20A);
            assert_eq!(emulator.gdb_exec, GdbExecState::Halted);
            assert_eq!(read_packet(&mut client), "S05");

            /* Continuing from the breakpoint executes it and keeps running:
             * the whole system (VDP included) advances by frames. */
            send(&mut client, "z0,20a,2");
            poll_until(&mut emulator, |e| {
                e.gdb.as_ref().is_some_and(|g| !g.is_breakpoint(0x20A))
            });
            assert_eq!(read_packet(&mut client), "OK");
            send(&mut client, "c");
            poll_until(&mut emulator, |e| e.gdb_exec == GdbExecState::Running);
            for _ in 0..3 {
                emulator.step_frame(None);
            }
            assert_eq!(emulator.internal_frame_count, 3);
            assert_eq!(emulator.cpu.pc, 0x210);
            assert_eq!(emulator.bus.borrow().vdp.v_counter, 0);
        }

        #[test]
        fn interrupt_stops_running_target() {
            let (mut emulator, mut client) = attach(&nop_rom());
            send(&mut client, "c");
            poll_until(&mut emulator, |e| e.gdb_exec == GdbExecState::Running);
            emulator.step_frame(None);
            client.write_all(&[0x03]).unwrap();
            poll_until(&mut emulator, |e| e.gdb_exec == GdbExecState::Halted);
            assert_eq!(read_packet(&mut client), "S02");
            let frames = emulator.internal_frame_count;
            emulator.step_frame(None);
            assert_eq!(emulator.internal_frame_count, frames);
        }
    }

    #[test]
    fn test_recording_captures_every_frame_with_distinct_indices() {
        let mut emulator = emulator_with_rom(&idle_loop_rom());
        emulator.input.start_recording();
        for _ in 0..5 {
            emulator.step_frame(None);
        }
        let script = emulator.input.stop_recording();
        for frame in 0..5 {
            assert!(script.get(frame).is_some(), "frame {} not recorded", frame);
        }
        assert_eq!(script.max_frame, 4);

        let mut emulator = emulator_with_rom(&idle_loop_rom());
        emulator.input.start_recording();
        let mut live = crate::input::FrameInput::default();
        for frame in 0..5u64 {
            live.p1.a = frame % 2 == 1;
            emulator.step_frame(Some(&live));
        }
        let script = emulator.input.stop_recording();
        for frame in 0..5u64 {
            let rec = script.get(frame).expect("live frame recorded");
            assert_eq!(rec.p1.a, frame % 2 == 1);
        }
    }

    #[test]
    fn test_gdb_memory_access_has_no_hardware_side_effects() {
        let emulator = Emulator::new();
        {
            let mut bus = emulator.bus.borrow_mut();
            bus.vdp.command.pending = true;
            bus.vdp.status |= 0x0080; // VINT pending
            bus.z80_bus_request = false;
            bus.z80_ram[0x10] = 0x5A;
        }
        let mut mem = BusGdbMemory { bus: &emulator.bus };
        let _ = mem.read_byte(0xC00004);
        let _ = mem.read_byte(0xC00005);
        let _ = mem.read_byte(0xC00000);
        {
            let bus = emulator.bus.borrow();
            assert!(
                bus.vdp.command.pending,
                "status read reset the command latch"
            );
            assert_ne!(
                bus.vdp.status & 0x0080,
                0,
                "status read cleared VINT pending"
            );
        }
        /* Z80 RAM is visible to the debugger regardless of bus ownership. */
        assert_eq!(mem.read_byte(0xA00010), 0x5A);
        mem.write_byte(0xA00011, 0xA5);
        assert_eq!(emulator.bus.borrow().z80_ram[0x11], 0xA5);
        /* Writes to the VDP control port are ignored, not executed. */
        mem.write_byte(0xC00004, 0x81);
        assert!(emulator.bus.borrow().vdp.command.pending);
        mem.write_byte(0xFF0000, 0x12);
        assert_eq!(mem.read_byte(0xFF0000), 0x12);
    }

    #[test]
    fn test_script_assertion_no_panic() {
        let emulator = Emulator::new();

        // Setup memory to value 0x00 at 0x1000
        emulator.bus.borrow_mut().write_byte(0x1000, 0x00);

        emulator.execute_script_command("ASSERT_BYTE 0x1000 0xFF");

        // Test word assertion
        emulator.execute_script_command("ASSERT_WORD 0x1000 0xFFFF");

        // Test long assertion
        emulator.execute_script_command("ASSERT_LONG 0x1000 0xFFFFFFFF");

        // If we reach here without panicking, the test passes
    }

    #[test]
    fn test_script_memory_commands_round_trip() {
        let emulator = Emulator::new();
        emulator.execute_script_command("WRITE_LONG 0xFF0010 0x12345678");
        assert_eq!(emulator.bus.borrow_mut().read_long(0xFF0010), 0x12345678);
        emulator.execute_script_command("write_word 0xFF0010 0xBEEF");
        assert_eq!(emulator.bus.borrow_mut().read_word(0xFF0010), 0xBEEF);
        emulator.execute_script_command("WRITE_BYTE 0xFF0010 0x42");
        assert_eq!(emulator.bus.borrow_mut().read_byte(0xFF0010), 0x42);
        /* Values wider than the access are rejected, not truncated. */
        emulator.execute_script_command("WRITE_BYTE 0xFF0010 0x1FF");
        assert_eq!(emulator.bus.borrow_mut().read_byte(0xFF0010), 0x42);
        assert_eq!(
            ScriptMemOp::parse("ASSERT_LONG"),
            Some((ScriptMemOp::Assert, ScriptWidth::Long))
        );
        assert_eq!(ScriptMemOp::parse("READ_QUAD"), None);
    }

    fn args(list: &[&str]) -> Vec<String> {
        std::iter::once("genteel")
            .chain(list.iter().copied())
            .map(String::from)
            .collect()
    }

    #[test]
    fn test_cli_usage_errors() {
        assert!(!Config::from_args(args(&["--headless"])).errors.is_empty());
        assert!(!Config::from_args(args(&["--headless", "rom.bin"]))
            .errors
            .is_empty());
        assert!(Config::from_args(args(&["--headless", "5", "rom.bin"]))
            .errors
            .is_empty());
        assert!(
            Config::from_args(args(&["--headless", "--script", "s.txt", "rom.bin"]))
                .errors
                .is_empty()
        );
        assert!(Config::from_args(args(&["--headless", "--gdb", "rom.bin"]))
            .errors
            .is_empty());
        assert!(!Config::from_args(args(&["a.bin", "b.bin"]))
            .errors
            .is_empty());
        let missing = Config::from_args(args(&[
            "--gdb-password-file",
            "/nonexistent/genteel/pw",
            "rom.bin",
        ]));
        assert!(!missing.errors.is_empty());
        assert_eq!(missing.gdb_password, None);
    }

    #[test]
    fn test_cli_input_mapping_is_only_set_when_given() {
        assert_eq!(Config::from_args(args(&["rom.bin"])).input_mapping, None);
        assert_eq!(
            Config::from_args(args(&["--input-mapping", "wasd", "rom.bin"])).input_mapping,
            Some(InputMapping::Ergonomic)
        );
    }

    #[test]
    fn test_screenshot_matches_display_resolution() {
        let emulator = Emulator::new();
        {
            let mut bus = emulator.bus.borrow_mut();
            bus.vdp.registers[crate::vdp::REG_MODE4] = 0x00; // H32
        }
        let (rgb, w, h) = emulator.screenshot_rgb();
        assert_eq!((w, h), (256, 224));
        assert_eq!(rgb.len(), 256 * 224 * 3);
        emulator.bus.borrow_mut().vdp.registers[crate::vdp::REG_MODE4] = 0x81; // H40
        let (_, w, _) = emulator.screenshot_rgb();
        assert_eq!(w, 320);

        let dir = scratch_dir("shot");
        let path = dir.join("s.png");
        let path_str = path.to_str().unwrap();
        emulator.save_screenshot_new(path_str).unwrap();
        assert!(
            emulator.save_screenshot_new(path_str).is_err(),
            "existing file overwritten"
        );
        let img = image::open(&path).unwrap();
        assert_eq!((img.width(), img.height()), (320, 224));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_wav_header_uses_output_sample_rate() {
        let dir = scratch_dir("wavrate");
        let rel = dir
            .strip_prefix(std::env::current_dir().unwrap())
            .map(|p| p.to_path_buf());
        /* WavWriter::new only writes inside the current directory. */
        let name = format!("genteel-wavrate-{}.wav", std::process::id());
        let path = match rel {
            Ok(r) => r.join(&name),
            Err(_) => std::path::PathBuf::from(&name),
        };
        let mut emulator = Emulator::new();
        emulator.pending_wav_dump = Some(path.to_string_lossy().into_owned());
        emulator.bus.borrow_mut().sample_rate = 48_000; // device rate
        emulator.start_pending_wav_dump();
        assert!(emulator.wav_writer.is_some());
        emulator.wav_writer = None;
        let bytes = std::fs::read(&path).unwrap();
        let rate = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
        assert_eq!(rate, 48_000);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(dir);
    }
}
