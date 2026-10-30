//! Mega EverDrive "Core" I/O: the command mailbox, USB serial bridge, SD
//! card file access and the 1 kHz millisecond timer at `$A130D0-$A130DF`.
//!
//! The device answers only while the SSF mapper is enabled (a `"SEGA SSF"`
//! ROM). It follows the QEMU `md_everdrive` model used to develop LinuxMD
//! (github.com/fifteenhex/qemu, `hw/misc/md_everdrive.c`).
//!
//! | Address   | Read                                        | Write                    |
//! |-----------|---------------------------------------------|--------------------------|
//! | `$A130D0` | pop the next RX byte (low byte, 0 if empty) | push the low byte to the command parser |
//! | `$A130D2` | bits 0-10 RX level, bit 15 RX non-empty     | ignored                  |
//! | `$A130D6` | free-running 16-bit millisecond counter     | set the counter          |
//!
//! Byte accesses: a byte read of `$A130D1` pops the FIFO and a read of
//! `$A130D0` returns the (zero) high byte without popping. The 68000 drives
//! a byte write on both data lanes, so a byte write to either `$A130D0` or
//! `$A130D1` pushes that byte. The status and timer registers read as their
//! high (even address) and low (odd address) halves; a timer byte write sets
//! that half.
//!
//! Commands are framed as `'+'`, `~'+'`, `cmd`, `~cmd`, followed by their
//! arguments. Multi-byte values are big-endian; strings are a u16 length and
//! that many bytes. Disk commands read files below a host directory (the "SD
//! card"), which is read-only and never escaped: a leading `/` is stripped,
//! `..` and absolute components are rejected, and the canonical result must
//! stay below the canonical root (so symlinks cannot leave it).
//!
//! Everything the guest controls is bounded: strings keep at most
//! [`MAX_STRING_LEN`] bytes, staged responses at most [`MAX_PENDING_BYTES`],
//! and an `F_FRD` of any length is streamed from the file into the RX FIFO
//! as the guest drains it. The timer counts emulated master clocks, so runs
//! are deterministic. Host file handles are never serialized; a restored
//! state reopens the open file by its path.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::VecDeque;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};

/// First address of the mailbox register window.
pub const MAILBOX_BASE: u32 = 0xA130D0;
/// Last address of the mailbox register window.
pub const MAILBOX_END: u32 = 0xA130DF;

/// Mask of the RX level field in the status register.
pub const FIFO_RXF_MSK: u16 = 0x7FF;
/// Status bit set while the RX FIFO holds data.
pub const FIFO_CPU_RXF: u16 = 0x8000;
/// RX FIFO capacity: the largest level the 11-bit status field can report,
/// so the reported level is always exact.
pub const RX_FIFO_CAPACITY: usize = FIFO_RXF_MSK as usize;
/// Most bytes of a guest string (path) that are kept; longer strings are
/// consumed but fail the command.
pub const MAX_STRING_LEN: usize = 1024;
/// Most response bytes staged behind the RX FIFO.
pub const MAX_PENDING_BYTES: usize = 64 * 1024;
/// Most USB serial bytes buffered for the host before they are dropped.
pub const MAX_USB_OUTPUT: usize = 64 * 1024;
/// Most directory entries listed.
const MAX_DIR_ENTRIES: usize = u16::MAX as usize;

/// Command status reported by `STATUS` after a successful command.
pub const STATUS_OK: u16 = 0xA500;
/// Command status reported after a failed command.
pub const STATUS_ERR: u16 = 0x0000;

/// First byte of a command frame.
pub const CMD_PREAMBLE: u8 = b'+';
/// Read the status of the last command (u16 response).
pub const CMD_STATUS: u8 = 0x10;
/// Send bytes to the USB serial port (u16 length, then the bytes).
pub const CMD_USB_WRITE: u8 = 0x22;
/// Initialise the SD card.
pub const CMD_DISK_INIT: u8 = 0xC0;
/// Select a directory (u8 flags, string path).
pub const CMD_F_DIR_LD: u8 = 0xC5;
/// Number of entries in the selected directory (u16 response).
pub const CMD_F_DIR_SIZE: u8 = 0xC6;
/// List directory entries (u16 start, u16 count, u16 max name length).
pub const CMD_F_DIR_GET: u8 = 0xC8;
/// Open a file (u8 mode, string path).
pub const CMD_F_FOPN: u8 = 0xC9;
/// Read from the open file (u32 length; u8 result then the data).
pub const CMD_F_FRD: u8 = 0xCA;
/// Close the open file.
pub const CMD_F_FCLOSE: u8 = 0xCE;
/// Set the file position (u32 offset from the start).
pub const CMD_F_FPTR: u8 = 0xCF;
/// Size of the open file (u64 response).
pub const CMD_F_AVB: u8 = 0xD5;

/// File mode bit: open for reading.
pub const FILE_MODE_READ: u8 = 0x01;
/// File mode bits that need write access (write, create, append).
const FILE_MODE_WRITE_BITS: u8 = 0x3E;

/// Command parser state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum Parser {
    /// Collecting the 4-byte frame; `pos` bytes matched so far.
    Header { pos: u8 },
    /// USB_WRITE: collecting the u16 length.
    UsbLen { got: u8, len: u16 },
    /// USB_WRITE: forwarding `left` more payload bytes.
    UsbData { left: u16 },
    /// Collecting `need` fixed argument bytes.
    Args { need: u8, got: u8 },
    /// Collecting a u16 string length.
    StrLen { got: u8, len: u16 },
    /// Collecting `left` more string bytes.
    StrData { left: u16 },
}

impl Default for Parser {
    fn default() -> Self {
        Parser::Header { pos: 0 }
    }
}

/// Largest fixed argument block (F_DIR_GET: three u16 values).
const MAX_ARGS: usize = 6;

/// A queued part of a response, moved into the RX FIFO as space allows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum Segment {
    /// Literal response bytes.
    Bytes(VecDeque<u8>),
    /// `left` bytes of the open file starting at `offset`.
    File { offset: u64, left: u64 },
    /// `left` zero bytes (the padding of a short or failed read).
    Zeros { left: u64 },
}

/// Most queued segments (a streamed read adds at most two).
const MAX_SEGMENTS: usize = 64;

/// The serializable device state. Every field defaults, so states written
/// before the device existed load with it idle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct IoState {
    parser: Parser,
    /// Command byte of the frame being parsed or executed.
    cmd: u8,
    /// Fixed arguments collected so far.
    args: [u8; MAX_ARGS],
    /// String argument (at most `MAX_STRING_LEN` bytes kept).
    string: Vec<u8>,
    /// The string was longer than `MAX_STRING_LEN`.
    string_overflow: bool,
    /// Result of the last command, read by `STATUS`.
    status: u16,
    /// `DISK_INIT` succeeded.
    disk_ready: bool,
    /// Directory selected by `F_DIR_LD`, as the guest wrote it.
    dir_path: Vec<u8>,
    /// Path of the open file, as the guest wrote it.
    file_path: Option<Vec<u8>>,
    /// Position of the open file.
    file_pos: u64,
    /// Bytes the guest can read now.
    rx: VecDeque<u8>,
    /// Response data waiting for room in `rx`.
    pending: VecDeque<Segment>,
    /// Literal bytes held in `pending`.
    pending_bytes: usize,
    /// Emulated milliseconds since power-on.
    timer_ms: u64,
    /// Master clocks x 1000 not yet worth a whole millisecond.
    timer_frac: u64,
    /// Added to `timer_ms` to give the counter value.
    timer_offset: u16,
}

impl Default for IoState {
    fn default() -> Self {
        Self {
            parser: Parser::default(),
            cmd: 0,
            args: [0; MAX_ARGS],
            string: Vec::new(),
            string_overflow: false,
            status: STATUS_OK,
            disk_ready: false,
            dir_path: Vec::new(),
            file_path: None,
            file_pos: 0,
            rx: VecDeque::new(),
            pending: VecDeque::new(),
            pending_bytes: 0,
            timer_ms: 0,
            timer_frac: 0,
            timer_offset: 0,
        }
    }
}

impl IoState {
    /// Clamp everything a malformed save state could make unbounded or
    /// inconsistent.
    fn sanitize(&mut self) {
        let parser_ok = match self.parser {
            Parser::Header { pos } => pos < 4,
            Parser::UsbLen { got, .. } | Parser::StrLen { got, .. } => got < 2,
            Parser::Args { need, got } => need as usize <= MAX_ARGS && got < need,
            Parser::UsbData { left } | Parser::StrData { left } => left > 0,
        };
        if !parser_ok {
            self.parser = Parser::default();
        }
        self.string.truncate(MAX_STRING_LEN);
        self.dir_path.truncate(MAX_STRING_LEN);
        if let Some(path) = self.file_path.as_mut() {
            path.truncate(MAX_STRING_LEN);
        }
        self.rx.truncate(RX_FIFO_CAPACITY);
        self.pending.truncate(MAX_SEGMENTS);
        let mut budget = MAX_PENDING_BYTES;
        for segment in &mut self.pending {
            if let Segment::Bytes(bytes) = segment {
                bytes.truncate(budget);
                budget -= bytes.len();
            }
        }
        self.pending_bytes = MAX_PENDING_BYTES - budget;
    }
}

/// Host side of the device: the SD directory, the open file handle and the
/// USB serial output. None of it is part of a save state.
#[derive(Debug, Default)]
struct Host {
    /// Canonical SD card directory.
    sd_root: Option<PathBuf>,
    /// Handle of the open file (`IoState::file_path`).
    file: Option<File>,
    /// USB serial bytes written by the guest, not yet taken by the host.
    usb_out: Vec<u8>,
    /// Some USB output was dropped because the host did not take it.
    usb_overflow_logged: bool,
}

/// The Mega EverDrive mailbox, SD card and timer device.
#[derive(Debug, Default)]
pub struct EverdriveIo {
    st: IoState,
    host: Host,
}

impl Serialize for EverdriveIo {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.st.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for EverdriveIo {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut st = IoState::deserialize(deserializer)?;
        st.sanitize();
        Ok(Self {
            st,
            host: Host::default(),
        })
    }
}

/// Master clock rate of the console the timer runs on.
#[inline]
fn master_clock_hz(pal: bool) -> u64 {
    if pal {
        crate::audio::PAL_MCLK as u64
    } else {
        crate::audio::NTSC_MCLK as u64
    }
}

impl EverdriveIo {
    /// Create an idle device with no SD card.
    pub fn new() -> Self {
        Self::default()
    }

    /// True for an address in the `$A130D0-$A130DF` register window.
    #[inline]
    pub fn is_register(addr: u32) -> bool {
        (MAILBOX_BASE..=MAILBOX_END).contains(&addr)
    }

    /// Power-on/reset state: empty FIFOs, idle parser, disk not initialised,
    /// file closed and the timer reading 0. The SD directory and the host
    /// serial link are kept.
    pub fn reset(&mut self) {
        let (timer_ms, timer_frac) = (self.st.timer_ms, self.st.timer_frac);
        self.st = IoState {
            timer_ms,
            timer_frac,
            timer_offset: 0u16.wrapping_sub(timer_ms as u16),
            ..IoState::default()
        };
        self.host.file = None;
    }

    /// Use `dir` (or nothing) as the SD card. The directory is canonicalized
    /// once here; disk commands fail while no directory is set.
    pub fn set_sd_root(&mut self, dir: Option<&Path>) -> std::io::Result<()> {
        self.host.sd_root = match dir {
            Some(dir) => {
                let root = dir.canonicalize()?;
                if !root.is_dir() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("{} is not a directory", root.display()),
                    ));
                }
                Some(root)
            }
            None => None,
        };
        self.host.file = None;
        self.reopen_file();
        Ok(())
    }

    /// The canonical SD card directory, if one is set.
    pub fn sd_root(&self) -> Option<&Path> {
        self.host.sd_root.as_deref()
    }

    /// Take over the host side (SD directory, unread USB output) of `old`,
    /// the device this one replaces after a save-state restore, and reopen
    /// the file the restored state had open.
    pub fn adopt_host(&mut self, old: &mut EverdriveIo) {
        self.host = std::mem::take(&mut old.host);
        self.host.file = None;
        self.reopen_file();
    }

    /// Take the bytes the guest sent with USB_WRITE since the last call.
    pub fn take_usb_output(&mut self) -> Vec<u8> {
        self.host.usb_overflow_logged = false;
        std::mem::take(&mut self.host.usb_out)
    }

    /// Push host serial input into the RX FIFO, as the USB port does.
    /// Input is accepted only between commands (parser idle, no response
    /// queued), so it never splits a response. Returns how many bytes were
    /// taken; the caller keeps the rest for later.
    pub fn push_host_input(&mut self, data: &[u8]) -> usize {
        if self.st.parser != Parser::default() || !self.st.pending.is_empty() {
            return 0;
        }
        let n = data.len().min(RX_FIFO_CAPACITY - self.st.rx.len());
        self.st.rx.extend(&data[..n]);
        n
    }

    /// Bytes the guest can read from the RX FIFO now.
    pub fn rx_level(&self) -> usize {
        self.st.rx.len()
    }

    /// Status of the last command, as `STATUS` would report it.
    pub fn status(&self) -> u16 {
        self.st.status
    }

    /// Sizes of the guest-controlled buffers: kept string bytes, staged
    /// literal response bytes, RX FIFO level and queued segments.
    #[cfg(test)]
    pub(crate) fn buffer_sizes(&self) -> (usize, usize, usize, usize) {
        (
            self.st.string.len(),
            self.st.pending_bytes,
            self.st.rx.len(),
            self.st.pending.len(),
        )
    }

    /// Advance the millisecond timer by `mclk` master clocks.
    #[inline]
    pub fn tick(&mut self, mclk: u32, pal: bool) {
        self.st.timer_frac += mclk as u64 * 1000;
        let hz = master_clock_hz(pal);
        if self.st.timer_frac >= hz {
            self.st.timer_ms += self.st.timer_frac / hz;
            self.st.timer_frac %= hz;
        }
    }

    /// The 16-bit millisecond counter.
    pub fn timer(&self) -> u16 {
        (self.st.timer_ms as u16).wrapping_add(self.st.timer_offset)
    }

    fn set_timer(&mut self, value: u16) {
        self.st.timer_offset = value.wrapping_sub(self.st.timer_ms as u16);
    }

    fn status_register(&self) -> u16 {
        let level = self.st.rx.len().min(RX_FIFO_CAPACITY) as u16;
        if level > 0 {
            level | FIFO_CPU_RXF
        } else {
            0
        }
    }

    fn pop_rx(&mut self) -> u8 {
        let byte = self.st.rx.pop_front().unwrap_or(0);
        if self.st.rx.len() <= RX_FIFO_CAPACITY / 2 {
            self.refill();
        }
        byte
    }

    /// 68k word read of a register.
    pub fn read_word(&mut self, addr: u32) -> u16 {
        match addr & 0xE {
            0x0 => self.pop_rx() as u16,
            0x2 => self.status_register(),
            0x6 => self.timer(),
            _ => 0xFFFF,
        }
    }

    /// 68k word write of a register.
    pub fn write_word(&mut self, addr: u32, value: u16) {
        match addr & 0xE {
            0x0 => self.write_mailbox(value as u8),
            0x6 => self.set_timer(value),
            _ => {}
        }
    }

    /// 68k byte read of a register.
    pub fn read_byte(&mut self, addr: u32) -> u8 {
        match addr & 0xF {
            0x0 => 0,
            0x1 => self.pop_rx(),
            0x2 => (self.status_register() >> 8) as u8,
            0x3 => self.status_register() as u8,
            0x6 => (self.timer() >> 8) as u8,
            0x7 => self.timer() as u8,
            _ => 0xFF,
        }
    }

    /// 68k byte write of a register.
    pub fn write_byte(&mut self, addr: u32, value: u8) {
        match addr & 0xF {
            0x0 | 0x1 => self.write_mailbox(value),
            0x6 => self.set_timer((self.timer() & 0x00FF) | (u16::from(value) << 8)),
            0x7 => self.set_timer((self.timer() & 0xFF00) | u16::from(value)),
            _ => {}
        }
    }

    /// Feed one byte to the command parser.
    pub fn write_mailbox(&mut self, b: u8) {
        let st = &mut self.st;
        match st.parser {
            Parser::Header { pos } => match pos {
                0 => {
                    st.parser = Parser::Header {
                        pos: u8::from(b == CMD_PREAMBLE),
                    }
                }
                1 => {
                    let next = if b == !CMD_PREAMBLE {
                        2
                    } else {
                        u8::from(b == CMD_PREAMBLE)
                    };
                    st.parser = Parser::Header { pos: next };
                }
                2 => {
                    st.cmd = b;
                    st.parser = Parser::Header { pos: 3 };
                }
                _ => {
                    st.parser = Parser::default();
                    if b == !st.cmd {
                        self.start_command();
                    } else {
                        log::debug!("everdrive: bad complement for command {:#04x}", st.cmd);
                    }
                }
            },
            Parser::UsbLen { got, len } => {
                let len = (len << 8) | u16::from(b);
                st.parser = if got == 0 {
                    Parser::UsbLen { got: 1, len }
                } else if len == 0 {
                    Parser::default()
                } else {
                    Parser::UsbData { left: len }
                };
            }
            Parser::UsbData { left } => {
                st.parser = if left > 1 {
                    Parser::UsbData { left: left - 1 }
                } else {
                    Parser::default()
                };
                self.usb_write(b);
            }
            Parser::Args { need, got } => {
                st.args[got as usize] = b;
                if got + 1 == need {
                    st.parser = Parser::default();
                    self.args_done();
                } else {
                    st.parser = Parser::Args { need, got: got + 1 };
                }
            }
            Parser::StrLen { got, len } => {
                let len = (len << 8) | u16::from(b);
                if got == 0 {
                    st.parser = Parser::StrLen { got: 1, len };
                } else if len == 0 {
                    st.parser = Parser::default();
                    self.string_done();
                } else {
                    st.parser = Parser::StrData { left: len };
                }
            }
            Parser::StrData { left } => {
                if st.string.len() < MAX_STRING_LEN {
                    st.string.push(b);
                } else {
                    st.string_overflow = true;
                }
                if left > 1 {
                    st.parser = Parser::StrData { left: left - 1 };
                } else {
                    st.parser = Parser::default();
                    self.string_done();
                }
            }
        }
    }

    fn usb_write(&mut self, b: u8) {
        if self.host.usb_out.len() < MAX_USB_OUTPUT {
            self.host.usb_out.push(b);
        } else if !self.host.usb_overflow_logged {
            self.host.usb_overflow_logged = true;
            log::warn!("everdrive: USB output not drained by the host, dropping bytes");
        }
    }

    fn collect_args(&mut self, need: u8) {
        self.st.parser = Parser::Args { need, got: 0 };
    }

    /// A complete frame arrived: run the command or start collecting its
    /// arguments.
    fn start_command(&mut self) {
        match self.st.cmd {
            CMD_STATUS => {
                let status = self.st.status;
                self.respond(&status.to_be_bytes());
            }
            CMD_USB_WRITE => self.st.parser = Parser::UsbLen { got: 0, len: 0 },
            CMD_DISK_INIT => {
                self.st.disk_ready = self.host.sd_root.is_some();
                self.st.status = if self.st.disk_ready {
                    STATUS_OK
                } else {
                    STATUS_ERR
                };
            }
            CMD_F_DIR_SIZE => self.dir_size(),
            CMD_F_AVB => self.file_avb(),
            CMD_F_FCLOSE => self.file_close(),
            CMD_F_FPTR | CMD_F_FRD => self.collect_args(4),
            CMD_F_DIR_GET => self.collect_args(6),
            CMD_F_DIR_LD | CMD_F_FOPN => self.collect_args(1),
            other => log::debug!("everdrive: unhandled command {:#04x}", other),
        }
    }

    /// The fixed arguments are complete.
    fn args_done(&mut self) {
        match self.st.cmd {
            CMD_F_DIR_GET => self.dir_get(),
            CMD_F_FRD => self.file_read(),
            CMD_F_FPTR => self.file_seek(),
            CMD_F_DIR_LD | CMD_F_FOPN => {
                self.st.string.clear();
                self.st.string_overflow = false;
                self.st.parser = Parser::StrLen { got: 0, len: 0 };
            }
            _ => {}
        }
    }

    /// The string argument is complete.
    fn string_done(&mut self) {
        match self.st.cmd {
            CMD_F_DIR_LD => self.dir_load(),
            CMD_F_FOPN => self.file_open(),
            _ => {}
        }
        self.st.string.clear();
    }

    /// False (with an error status) until DISK_INIT has succeeded.
    fn disk_ready(&mut self) -> bool {
        if !self.st.disk_ready {
            self.st.status = STATUS_ERR;
        }
        self.st.disk_ready
    }

    /// Resolve a guest path below the SD directory. Returns `None` for
    /// non-UTF-8 text, `..`, absolute or drive components, missing files,
    /// and anything whose canonical path leaves the directory.
    fn resolve(&self, raw: &[u8]) -> Option<PathBuf> {
        let root = self.host.sd_root.as_ref()?;
        let text = std::str::from_utf8(raw).ok()?;
        if text.contains(['\0', '\\']) {
            return None;
        }
        let mut path = root.clone();
        for part in text.trim_start_matches('/').split('/') {
            if part.is_empty() || part == "." {
                continue;
            }
            let mut components = Path::new(part).components();
            match (components.next(), components.next()) {
                (Some(Component::Normal(name)), None) => path.push(name),
                _ => return None,
            }
        }
        let canonical = path.canonicalize().ok()?;
        canonical.starts_with(root).then_some(canonical)
    }

    /// Reopen the file named by the state (after a restore or a new SD
    /// directory). A file that cannot be reopened reads as missing.
    fn reopen_file(&mut self) {
        let Some(raw) = self.st.file_path.clone() else {
            return;
        };
        self.host.file = self.open_regular(&raw);
        if self.host.file.is_none() {
            log::warn!(
                "everdrive: cannot reopen {:?} from the save state",
                String::from_utf8_lossy(&raw)
            );
        }
    }

    fn open_regular(&self, raw: &[u8]) -> Option<File> {
        let path = self.resolve(raw)?;
        let file = File::open(path).ok()?;
        file.metadata().ok()?.is_file().then_some(file)
    }

    /// Entries of the selected directory, sorted by name so listings do not
    /// depend on the host file system's order. Entries that resolve outside
    /// the SD directory are left out.
    fn dir_entries(&self) -> Option<Vec<(String, u64, bool)>> {
        let dir = self.resolve(&self.st.dir_path)?;
        let root = self.host.sd_root.as_ref()?;
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&dir).ok()?.flatten() {
            if entries.len() >= MAX_DIR_ENTRIES {
                break;
            }
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let Ok(target) = entry.path().canonicalize() else {
                continue;
            };
            if !target.starts_with(root) {
                continue;
            }
            let Ok(meta) = std::fs::metadata(&target) else {
                continue;
            };
            entries.push((name, meta.len(), meta.is_dir()));
        }
        entries.sort();
        Some(entries)
    }

    fn dir_load(&mut self) {
        if !self.disk_ready() {
            return;
        }
        let ok = !self.st.string_overflow
            && self
                .resolve(&self.st.string)
                .is_some_and(|path| path.is_dir());
        if ok {
            self.st.dir_path = std::mem::take(&mut self.st.string);
            self.st.status = STATUS_OK;
        } else {
            self.st.status = STATUS_ERR;
        }
    }

    fn dir_size(&mut self) {
        if !self.disk_ready() {
            self.respond(&[0, 0]);
            return;
        }
        let count = self
            .dir_entries()
            .map_or(0, |e| e.len())
            .min(MAX_DIR_ENTRIES) as u16;
        self.respond(&count.to_be_bytes());
        self.st.status = STATUS_OK;
    }

    fn dir_get(&mut self) {
        let a = self.st.args;
        let start = u16::from_be_bytes([a[0], a[1]]) as usize;
        let num = u16::from_be_bytes([a[2], a[3]]) as usize;
        let max_name = u16::from_be_bytes([a[4], a[5]]) as usize;
        let entries = if self.disk_ready() {
            self.dir_entries()
        } else {
            None
        };
        let mut out = Vec::new();
        for (name, size, is_dir) in entries.iter().flatten().skip(start).take(num) {
            let name = &name.as_bytes()[..name.len().min(max_name)];
            /* 0 (entry follows), u32 size, u16 date, u16 time, u8 is_dir,
             * then the name as a string. */
            let entry_len = 12 + name.len();
            if self.st.pending_bytes + out.len() + entry_len + 1 > MAX_PENDING_BYTES {
                log::warn!("everdrive: directory listing truncated");
                break;
            }
            out.push(0);
            out.extend_from_slice(&(*size).min(u32::MAX as u64).to_be_bytes()[4..]);
            out.extend_from_slice(&[0, 0, 0, 0, u8::from(*is_dir)]);
            out.extend_from_slice(&(name.len() as u16).to_be_bytes());
            out.extend_from_slice(name);
        }
        out.push(1);
        self.respond(&out);
    }

    fn file_open(&mut self) {
        if !self.disk_ready() {
            return;
        }
        self.host.file = None;
        self.st.file_path = None;
        self.st.file_pos = 0;
        let mode = self.st.args[0];
        let file = if mode & FILE_MODE_WRITE_BITS != 0 || self.st.string_overflow {
            None
        } else {
            self.open_regular(&self.st.string)
        };
        if let Some(file) = file {
            self.host.file = Some(file);
            self.st.file_path = Some(self.st.string.clone());
            self.st.status = STATUS_OK;
        } else {
            self.st.status = STATUS_ERR;
        }
    }

    fn file_size(&self) -> u64 {
        self.host
            .file
            .as_ref()
            .and_then(|f| f.metadata().ok())
            .map_or(0, |m| m.len())
    }

    fn file_avb(&mut self) {
        let size = if self.disk_ready() {
            self.file_size()
        } else {
            0
        };
        self.respond(&size.to_be_bytes());
    }

    fn file_read(&mut self) {
        let a = self.st.args;
        let amount = u64::from(u32::from_be_bytes([a[0], a[1], a[2], a[3]]));
        let from_file = if self.disk_ready() && self.host.file.is_some() {
            self.file_size()
                .saturating_sub(self.st.file_pos)
                .min(amount)
        } else {
            0
        };
        let offset = self.st.file_pos;
        self.st.file_pos += from_file;
        self.respond(&[u8::from(from_file != amount)]);
        if self.st.pending.len() + 2 > MAX_SEGMENTS {
            log::warn!("everdrive: too many queued reads, dropping one");
            return;
        }
        if from_file > 0 {
            self.st.pending.push_back(Segment::File {
                offset,
                left: from_file,
            });
        }
        if amount > from_file {
            self.st.pending.push_back(Segment::Zeros {
                left: amount - from_file,
            });
        }
        self.refill();
    }

    fn file_close(&mut self) {
        if !self.disk_ready() {
            return;
        }
        self.host.file = None;
        self.st.file_path = None;
        self.st.file_pos = 0;
        self.st.status = STATUS_OK;
    }

    fn file_seek(&mut self) {
        if !self.disk_ready() {
            return;
        }
        if self.host.file.is_some() {
            let a = self.st.args;
            self.st.file_pos = u64::from(u32::from_be_bytes([a[0], a[1], a[2], a[3]]));
        }
    }

    /// Queue response bytes behind anything already queued.
    fn respond(&mut self, bytes: &[u8]) {
        let room = MAX_PENDING_BYTES - self.st.pending_bytes;
        let bytes = if bytes.len() > room {
            log::warn!("everdrive: response queue full, dropping bytes");
            &bytes[..room]
        } else {
            bytes
        };
        if bytes.is_empty() {
            return;
        }
        let segments = self.st.pending.len();
        match self.st.pending.back_mut() {
            Some(Segment::Bytes(queued)) => queued.extend(bytes),
            _ if segments < MAX_SEGMENTS => {
                self.st
                    .pending
                    .push_back(Segment::Bytes(bytes.iter().copied().collect()));
            }
            _ => {
                log::warn!("everdrive: response queue full, dropping bytes");
                return;
            }
        }
        self.st.pending_bytes += bytes.len();
        self.refill();
    }

    /// Move queued response data into the RX FIFO while it has room.
    fn refill(&mut self) {
        while self.st.rx.len() < RX_FIFO_CAPACITY {
            let room = RX_FIFO_CAPACITY - self.st.rx.len();
            let Some(segment) = self.st.pending.front_mut() else {
                break;
            };
            let done = match segment {
                Segment::Bytes(bytes) => {
                    let n = room.min(bytes.len());
                    self.st.rx.extend(bytes.drain(..n));
                    self.st.pending_bytes -= n;
                    bytes.is_empty()
                }
                Segment::Zeros { left } => {
                    let n = (room as u64).min(*left);
                    self.st.rx.extend(std::iter::repeat_n(0, n as usize));
                    *left -= n;
                    *left == 0
                }
                Segment::File { offset, left } => {
                    let n = (room as u64).min(*left) as usize;
                    let mut chunk = [0u8; RX_FIFO_CAPACITY];
                    let got = read_at(self.host.file.as_mut(), *offset, &mut chunk[..n]);
                    /* A file that shrank or failed reads as zeros, so the
                     * guest still gets the length it asked for. */
                    chunk[got..n].fill(0);
                    self.st.rx.extend(&chunk[..n]);
                    *offset += n as u64;
                    *left -= n as u64;
                    *left == 0
                }
            };
            if done {
                self.st.pending.pop_front();
            }
        }
    }
}

/// Read up to `buf.len()` bytes at `offset`; returns the count read.
fn read_at(file: Option<&mut File>, offset: u64, buf: &mut [u8]) -> usize {
    let Some(file) = file else {
        return 0;
    };
    if file.seek(SeekFrom::Start(offset)).is_err() {
        return 0;
    }
    let mut got = 0;
    while got < buf.len() {
        match file.read(&mut buf[got..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => got += n,
        }
    }
    got
}
