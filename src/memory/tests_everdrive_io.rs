//! Tests for the EverDrive mailbox, SD card and timer device, driven the
//! way LinuxMD drives it: one `move.w` per byte to `$A130D0`, polling the
//! level in `$A130D2` before every read.

use super::bus::Bus;
use super::everdrive_io::*;
use proptest::prelude::*;
use std::path::{Path, PathBuf};

const DATA: u32 = 0xA130D0;
const STAT: u32 = 0xA130D2;
const TIMER: u32 = 0xA130D6;

/// A directory under the system temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("genteel-edio-{}-{}-{}", tag, std::process::id(), n));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn file(&self, name: &str, data: &[u8]) {
        let path = self.0.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, data).unwrap();
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A small `"SEGA SSF"` ROM image.
fn ssf_rom() -> Vec<u8> {
    let mut rom = vec![0u8; 0x1000];
    rom[0x100..0x110].copy_from_slice(b"SEGA SSF        ");
    rom
}

/// A bus running an SSF ROM with the mapper unlocked as u-boot does it.
fn ssf_bus(sd: Option<&TempDir>) -> Bus {
    let mut bus = Bus::new();
    bus.load_rom(&ssf_rom());
    bus.write_word(0xA130F0, 0xA000);
    bus.everdrive_io
        .set_sd_root(sd.map(|d| d.path()))
        .expect("SD root");
    bus
}

fn send(bus: &mut Bus, bytes: &[u8]) {
    for &b in bytes {
        bus.write_word(DATA, b as u16);
    }
}

fn cmd(bus: &mut Bus, c: u8) {
    send(bus, &[CMD_PREAMBLE, !CMD_PREAMBLE, c, !c]);
}

fn send_str(bus: &mut Bus, s: &[u8]) {
    send(bus, &(s.len() as u16).to_be_bytes());
    send(bus, s);
}

/// Read `n` bytes, polling the level before each read like the guest.
fn recv(bus: &mut Bus, n: usize) -> Vec<u8> {
    (0..n)
        .map(|i| {
            let level = bus.read_word(STAT) & FIFO_RXF_MSK;
            assert!(level > 0, "RX FIFO empty after {i} of {n} bytes");
            bus.read_word(DATA) as u8
        })
        .collect()
}

fn recv_u16(bus: &mut Bus) -> u16 {
    let b = recv(bus, 2);
    u16::from_be_bytes([b[0], b[1]])
}

fn status(bus: &mut Bus) -> u16 {
    cmd(bus, CMD_STATUS);
    recv_u16(bus)
}

fn rx_empty(bus: &mut Bus) -> bool {
    bus.read_word(STAT) == 0
}

fn disk_init(bus: &mut Bus) {
    cmd(bus, CMD_DISK_INIT);
    assert_eq!(status(bus), STATUS_OK);
}

fn fopen(bus: &mut Bus, mode: u8, path: &[u8]) -> u16 {
    cmd(bus, CMD_F_FOPN);
    send(bus, &[mode]);
    send_str(bus, path);
    status(bus)
}

/// F_FRD: returns the result byte and the data.
fn fread(bus: &mut Bus, n: u32) -> (u8, Vec<u8>) {
    cmd(bus, CMD_F_FRD);
    send(bus, &n.to_be_bytes());
    let resp = recv(bus, 1)[0];
    (resp, recv(bus, n as usize))
}

fn favb(bus: &mut Bus) -> u64 {
    cmd(bus, CMD_F_AVB);
    let b = recv(bus, 8);
    u64::from_be_bytes(b.try_into().unwrap())
}

fn dir_ld(bus: &mut Bus, path: &[u8]) -> u16 {
    cmd(bus, CMD_F_DIR_LD);
    send(bus, &[0]);
    send_str(bus, path);
    status(bus)
}

/// F_DIR_GET, parsed: (name, size, is_dir) per entry.
fn dir_get(bus: &mut Bus, start: u16, num: u16, max_name: u16) -> Vec<(Vec<u8>, u32, bool)> {
    cmd(bus, CMD_F_DIR_GET);
    for v in [start, num, max_name] {
        send(bus, &v.to_be_bytes());
    }
    let mut out = Vec::new();
    loop {
        if recv(bus, 1)[0] != 0 {
            break;
        }
        let head = recv(bus, 9);
        let size = u32::from_be_bytes(head[0..4].try_into().unwrap());
        assert_eq!(&head[4..8], &[0, 0, 0, 0], "date and time are zero");
        let len = recv_u16(bus) as usize;
        out.push((recv(bus, len), size, head[8] != 0));
    }
    out
}

#[test]
fn registers_absent_without_ssf_mapper() {
    let mut bus = Bus::new();
    let mut rom = ssf_rom();
    rom[0x100..0x110].copy_from_slice(b"SEGA GENESIS    ");
    bus.load_rom(&rom);
    send(
        &mut bus,
        &[CMD_PREAMBLE, !CMD_PREAMBLE, CMD_STATUS, !CMD_STATUS],
    );
    assert_eq!(bus.read_word(STAT), 0xFFFF);
    assert_eq!(bus.everdrive_io.rx_level(), 0);
}

#[test]
fn status_defaults_to_ok() {
    let mut bus = ssf_bus(None);
    assert!(rx_empty(&mut bus));
    assert_eq!(status(&mut bus), STATUS_OK);
    assert!(rx_empty(&mut bus));
}

#[test]
fn status_register_reports_level_and_non_empty_bit() {
    let mut bus = ssf_bus(None);
    cmd(&mut bus, CMD_STATUS);
    assert_eq!(bus.read_word(STAT), FIFO_CPU_RXF | 2);
    assert_eq!(bus.read_byte(STAT), 0x80);
    assert_eq!(bus.read_byte(STAT + 1), 2);
    bus.read_word(DATA);
    assert_eq!(bus.read_word(STAT), FIFO_CPU_RXF | 1);
    bus.read_word(DATA);
    assert_eq!(bus.read_word(STAT), 0);
    // An empty FIFO reads as zero.
    assert_eq!(bus.read_word(DATA), 0);
}

#[test]
fn byte_access_uses_the_low_lane() {
    let mut bus = ssf_bus(None);
    // Byte writes to either address push the byte.
    for (i, b) in [CMD_PREAMBLE, !CMD_PREAMBLE, CMD_STATUS, !CMD_STATUS]
        .into_iter()
        .enumerate()
    {
        bus.write_byte(DATA + (i as u32 & 1), b);
    }
    // The even byte is the zero high half and does not pop.
    assert_eq!(bus.read_byte(DATA), 0);
    assert_eq!(bus.everdrive_io.rx_level(), 2);
    assert_eq!(bus.read_byte(DATA + 1), 0xA5);
    assert_eq!(bus.read_byte(DATA + 1), 0x00);
}

#[test]
fn bad_frames_are_ignored() {
    let mut bus = ssf_bus(None);
    // Wrong complement, wrong second preamble byte, stray bytes.
    send(&mut bus, &[CMD_PREAMBLE, !CMD_PREAMBLE, CMD_STATUS, 0x00]);
    send(&mut bus, &[CMD_PREAMBLE, 0x00, CMD_STATUS, !CMD_STATUS]);
    send(&mut bus, &[0x55, 0xAA]);
    assert!(rx_empty(&mut bus));
    // A repeated preamble resynchronises.
    send(
        &mut bus,
        &[
            CMD_PREAMBLE,
            CMD_PREAMBLE,
            !CMD_PREAMBLE,
            CMD_STATUS,
            !CMD_STATUS,
        ],
    );
    assert_eq!(recv_u16(&mut bus), STATUS_OK);
    // Unknown commands have no response.
    cmd(&mut bus, 0x40);
    assert!(rx_empty(&mut bus));
}

#[test]
fn usb_write_goes_to_the_host() {
    let mut bus = ssf_bus(None);
    cmd(&mut bus, CMD_USB_WRITE);
    send_str(&mut bus, b"hi");
    // u-boot's putc: one byte per command.
    cmd(&mut bus, CMD_USB_WRITE);
    send(&mut bus, &[0, 1, b'!']);
    // A zero-length write is complete after its length.
    cmd(&mut bus, CMD_USB_WRITE);
    send(&mut bus, &[0, 0]);
    assert_eq!(status(&mut bus), STATUS_OK);
    assert_eq!(bus.everdrive_io.take_usb_output(), b"hi!");
    assert!(bus.everdrive_io.take_usb_output().is_empty());
    assert!(rx_empty(&mut bus));
}

#[test]
fn usb_output_is_bounded() {
    let mut bus = ssf_bus(None);
    for _ in 0..(MAX_USB_OUTPUT / 0xFFFF) + 2 {
        cmd(&mut bus, CMD_USB_WRITE);
        send(&mut bus, &[0xFF, 0xFF]);
        for _ in 0..0xFFFF {
            bus.write_word(DATA, b'x' as u16);
        }
    }
    assert_eq!(bus.everdrive_io.take_usb_output().len(), MAX_USB_OUTPUT);
}

#[test]
fn host_input_reaches_the_rx_fifo_between_commands() {
    let mut bus = ssf_bus(None);
    assert_eq!(bus.everdrive_io.push_host_input(b"ls\n"), 3);
    assert_eq!(recv(&mut bus, 3), b"ls\n");
    // Not while a command frame is half written.
    send(&mut bus, &[CMD_PREAMBLE, !CMD_PREAMBLE]);
    assert_eq!(bus.everdrive_io.push_host_input(b"x"), 0);
    send(&mut bus, &[CMD_STATUS, !CMD_STATUS]);
    assert_eq!(recv_u16(&mut bus), STATUS_OK);
    // Only as much as fits.
    let big = vec![b'a'; RX_FIFO_CAPACITY + 10];
    assert_eq!(bus.everdrive_io.push_host_input(&big), RX_FIFO_CAPACITY);
    assert_eq!(bus.read_word(STAT), FIFO_CPU_RXF | FIFO_RXF_MSK);
}

#[test]
fn disk_commands_fail_before_disk_init() {
    let sd = TempDir::new("noinit");
    sd.file("f.bin", b"data");
    let mut bus = ssf_bus(Some(&sd));
    assert_eq!(fopen(&mut bus, FILE_MODE_READ, b"f.bin"), STATUS_ERR);
    cmd(&mut bus, CMD_F_DIR_SIZE);
    assert_eq!(recv_u16(&mut bus), 0);
    assert_eq!(status(&mut bus), STATUS_ERR);
    assert_eq!(favb(&mut bus), 0);
    assert_eq!(fread(&mut bus, 4), (1, vec![0; 4]));
    assert!(dir_get(&mut bus, 0, 10, 64).is_empty());
    assert!(rx_empty(&mut bus));
}

#[test]
fn disk_init_fails_without_an_sd_directory() {
    let mut bus = ssf_bus(None);
    cmd(&mut bus, CMD_DISK_INIT);
    assert_eq!(status(&mut bus), STATUS_ERR);
    assert_eq!(fopen(&mut bus, FILE_MODE_READ, b"x"), STATUS_ERR);
    // USB serial still works.
    cmd(&mut bus, CMD_USB_WRITE);
    send_str(&mut bus, b"ok");
    assert_eq!(bus.everdrive_io.take_usb_output(), b"ok");
}

#[test]
fn directory_listing() {
    let sd = TempDir::new("dir");
    sd.file("b.bin", &[0; 300]);
    sd.file("a.txt", b"abc");
    sd.file("sub/inner.txt", b"x");
    sd.file("a-very-long-file-name.txt", b"");
    let mut bus = ssf_bus(Some(&sd));
    disk_init(&mut bus);

    assert_eq!(dir_ld(&mut bus, b"/"), STATUS_OK);
    cmd(&mut bus, CMD_F_DIR_SIZE);
    assert_eq!(recv_u16(&mut bus), 4);
    assert_eq!(status(&mut bus), STATUS_OK);

    let all = dir_get(&mut bus, 0, 4, 64);
    let names: Vec<&[u8]> = all.iter().map(|e| e.0.as_slice()).collect();
    assert_eq!(
        names,
        [
            &b"a-very-long-file-name.txt"[..],
            b"a.txt",
            b"b.bin",
            b"sub"
        ]
    );
    assert_eq!((all[1].1, all[1].2), (3, false));
    assert_eq!((all[2].1, all[2].2), (300, false));
    assert!(all[3].2, "sub is a directory");

    // Start/count select a window; names are cut to the maximum length.
    let window = dir_get(&mut bus, 1, 2, 4);
    assert_eq!(window.len(), 2);
    assert_eq!(window[0].0, b"a.tx");
    assert_eq!(window[1].0, b"b.bi");
    assert!(dir_get(&mut bus, 10, 5, 64).is_empty());

    assert_eq!(dir_ld(&mut bus, b"sub"), STATUS_OK);
    let sub = dir_get(&mut bus, 0, 100, 64);
    assert_eq!(sub.len(), 1);
    assert_eq!(sub[0].0, b"inner.txt");

    // A file or a missing path is not a directory.
    assert_eq!(dir_ld(&mut bus, b"a.txt"), STATUS_ERR);
    assert_eq!(dir_ld(&mut bus, b"nope"), STATUS_ERR);
    assert!(rx_empty(&mut bus));
}

#[test]
fn file_open_read_seek_close() {
    let sd = TempDir::new("file");
    let data: Vec<u8> = (0..1500u32).map(|i| (i * 7) as u8).collect();
    sd.file("dir/data.bin", &data);
    let mut bus = ssf_bus(Some(&sd));
    disk_init(&mut bus);

    assert_eq!(fopen(&mut bus, FILE_MODE_READ, b"/dir/data.bin"), STATUS_OK);
    assert_eq!(favb(&mut bus), 1500);
    assert_eq!(fread(&mut bus, 512), (0, data[..512].to_vec()));
    assert_eq!(fread(&mut bus, 512), (0, data[512..1024].to_vec()));
    // The size is the whole file, not what is left (as in QEMU).
    assert_eq!(favb(&mut bus), 1500);
    // A short read flags it and pads with zeros.
    let (resp, tail) = fread(&mut bus, 512);
    assert_eq!(resp, 1);
    assert_eq!(&tail[..476], &data[1024..]);
    assert!(tail[476..].iter().all(|&b| b == 0));

    // Block-device pattern: F_FPTR then F_FRD.
    cmd(&mut bus, CMD_F_FPTR);
    send(&mut bus, &100u32.to_be_bytes());
    assert_eq!(fread(&mut bus, 10), (0, data[100..110].to_vec()));
    assert_eq!(status(&mut bus), STATUS_OK);

    cmd(&mut bus, CMD_F_FCLOSE);
    assert_eq!(status(&mut bus), STATUS_OK);
    assert_eq!(favb(&mut bus), 0);
    assert_eq!(fread(&mut bus, 3), (1, vec![0; 3]));
    assert!(rx_empty(&mut bus));
}

#[test]
fn read_larger_than_the_fifo_streams() {
    let sd = TempDir::new("stream");
    let data: Vec<u8> = (0..10_000u32).map(|i| (i ^ (i >> 8)) as u8).collect();
    sd.file("big.bin", &data);
    let mut bus = ssf_bus(Some(&sd));
    disk_init(&mut bus);
    assert_eq!(fopen(&mut bus, FILE_MODE_READ, b"big.bin"), STATUS_OK);
    cmd(&mut bus, CMD_F_FRD);
    send(&mut bus, &10_000u32.to_be_bytes());
    assert_eq!(bus.read_word(STAT) & FIFO_RXF_MSK, FIFO_RXF_MSK);
    assert_eq!(recv(&mut bus, 1), [0]);
    assert_eq!(recv(&mut bus, 10_000), data);
    assert!(rx_empty(&mut bus));
}

#[test]
fn huge_read_does_not_allocate() {
    let sd = TempDir::new("huge");
    sd.file("small.bin", &[0x5A; 3000]);
    let mut bus = ssf_bus(Some(&sd));
    disk_init(&mut bus);
    assert_eq!(fopen(&mut bus, FILE_MODE_READ, b"small.bin"), STATUS_OK);
    cmd(&mut bus, CMD_F_FRD);
    send(&mut bus, &u32::MAX.to_be_bytes());
    let (_, pending, rx, segments) = bus.everdrive_io.buffer_sizes();
    assert_eq!(rx, RX_FIFO_CAPACITY);
    assert_eq!(pending, 0, "file data is streamed, not staged");
    assert!(segments <= 2);
    assert_eq!(recv(&mut bus, 1), [1], "a short read");
    assert!(recv(&mut bus, 3000).iter().all(|&b| b == 0x5A));
    assert!(recv(&mut bus, 5000).iter().all(|&b| b == 0));
    let (_, _, rx, _) = bus.everdrive_io.buffer_sizes();
    assert!(rx <= RX_FIFO_CAPACITY);
}

#[test]
fn long_strings_are_consumed_but_bounded() {
    let sd = TempDir::new("longstr");
    let mut bus = ssf_bus(Some(&sd));
    disk_init(&mut bus);
    cmd(&mut bus, CMD_F_FOPN);
    send(&mut bus, &[FILE_MODE_READ, 0xFF, 0xFF]);
    for _ in 0..0xFFFF {
        bus.write_word(DATA, b'a' as u16);
    }
    // The whole string was consumed: the next frame parses.
    assert_eq!(status(&mut bus), STATUS_ERR);
    let (string, pending, _, _) = bus.everdrive_io.buffer_sizes();
    assert!(string <= MAX_STRING_LEN);
    assert_eq!(pending, 0);
}

#[test]
fn directory_listing_is_bounded() {
    let sd = TempDir::new("manyfiles");
    for i in 0..1500 {
        sd.file(&format!("file-with-a-longish-name-{i:05}.bin"), b"");
    }
    let mut bus = ssf_bus(Some(&sd));
    disk_init(&mut bus);
    assert_eq!(dir_ld(&mut bus, b""), STATUS_OK);
    let entries = dir_get(&mut bus, 0, u16::MAX, u16::MAX);
    assert!(
        !entries.is_empty() && entries.len() < 1500,
        "listing truncated"
    );
    let (_, pending, _, _) = bus.everdrive_io.buffer_sizes();
    assert!(pending <= MAX_PENDING_BYTES);
}

#[test]
fn path_escapes_are_rejected() {
    let outer = TempDir::new("escape");
    outer.file("secret.txt", b"secret");
    outer.file("sd/inside.txt", b"inside");
    let sd_path = outer.path().join("sd");
    let mut bus = Bus::new();
    bus.load_rom(&ssf_rom());
    bus.everdrive_io.set_sd_root(Some(&sd_path)).unwrap();
    disk_init(&mut bus);

    for bad in [
        &b"../secret.txt"[..],
        b"/../secret.txt",
        b"sub/../../secret.txt",
        b"./../secret.txt",
        b"..",
        b"inside.txt/..",
        b"..\\secret.txt",
        b"inside.txt\0",
        &[0xFF, 0xFE],
    ] {
        assert_eq!(fopen(&mut bus, FILE_MODE_READ, bad), STATUS_ERR, "{bad:?}");
        assert_eq!(dir_ld(&mut bus, bad), STATUS_ERR, "{bad:?}");
    }
    // Absolute paths are taken relative to the SD directory.
    let abs = outer.path().join("secret.txt");
    assert_eq!(
        fopen(&mut bus, FILE_MODE_READ, abs.to_str().unwrap().as_bytes()),
        STATUS_ERR
    );
    assert_eq!(fopen(&mut bus, FILE_MODE_READ, b"//inside.txt"), STATUS_OK);
    // The directory itself is not a file.
    assert_eq!(fopen(&mut bus, FILE_MODE_READ, b"/"), STATUS_ERR);
}

#[cfg(unix)]
#[test]
fn symlinks_must_stay_inside_the_sd_directory() {
    let outer = TempDir::new("symlink");
    outer.file("secret.txt", b"secret");
    outer.file("sd/real.txt", b"real");
    let sd_path = outer.path().join("sd");
    std::os::unix::fs::symlink(outer.path().join("secret.txt"), sd_path.join("out.txt")).unwrap();
    std::os::unix::fs::symlink(outer.path(), sd_path.join("outdir")).unwrap();
    std::os::unix::fs::symlink(sd_path.join("real.txt"), sd_path.join("alias.txt")).unwrap();
    let mut bus = Bus::new();
    bus.load_rom(&ssf_rom());
    bus.everdrive_io.set_sd_root(Some(&sd_path)).unwrap();
    disk_init(&mut bus);

    assert_eq!(fopen(&mut bus, FILE_MODE_READ, b"out.txt"), STATUS_ERR);
    assert_eq!(
        fopen(&mut bus, FILE_MODE_READ, b"outdir/secret.txt"),
        STATUS_ERR
    );
    assert_eq!(dir_ld(&mut bus, b"outdir"), STATUS_ERR);
    assert_eq!(fopen(&mut bus, FILE_MODE_READ, b"alias.txt"), STATUS_OK);
    assert_eq!(fread(&mut bus, 4), (0, b"real".to_vec()));
    // Escaping links are left out of listings.
    assert_eq!(dir_ld(&mut bus, b"/"), STATUS_OK);
    let names: Vec<Vec<u8>> = dir_get(&mut bus, 0, 10, 64)
        .into_iter()
        .map(|e| e.0)
        .collect();
    assert_eq!(names, [b"alias.txt".to_vec(), b"real.txt".to_vec()]);
}

#[test]
fn the_sd_card_is_read_only() {
    let sd = TempDir::new("ro");
    sd.file("f.txt", b"keep");
    let mut bus = ssf_bus(Some(&sd));
    disk_init(&mut bus);
    for mode in [0x02, 0x03, 0x04, 0x08, 0x10, 0x30] {
        assert_eq!(
            fopen(&mut bus, mode, b"f.txt"),
            STATUS_ERR,
            "mode {mode:#x}"
        );
        assert_eq!(
            fopen(&mut bus, mode, b"new.txt"),
            STATUS_ERR,
            "mode {mode:#x}"
        );
    }
    assert!(!sd.path().join("new.txt").exists());
    assert_eq!(std::fs::read(sd.path().join("f.txt")).unwrap(), b"keep");
}

#[test]
fn timer_counts_emulated_milliseconds() {
    let mut bus = ssf_bus(None);
    assert_eq!(bus.read_word(TIMER), 0);
    // One NTSC second of master clocks, in scheduler-sized slices.
    let mut left = crate::audio::NTSC_MCLK;
    while left > 0 {
        let slice = left.min(3420);
        bus.tick(slice);
        left -= slice;
    }
    assert_eq!(bus.read_word(TIMER), 1000);
    // Writable, and it wraps at 16 bits.
    bus.write_word(TIMER, 0xFFFE);
    assert_eq!(bus.read_word(TIMER), 0xFFFE);
    bus.tick(crate::audio::NTSC_MCLK / 1000 * 3 + 10);
    assert_eq!(bus.read_word(TIMER), 1);
    assert_eq!(bus.read_byte(TIMER), 0);
    assert_eq!(bus.read_byte(TIMER + 1), 1);
    bus.write_byte(TIMER, 0x12);
    assert_eq!(bus.read_word(TIMER), 0x1201);
    // The 68k cycle path: 7 MCLK per cycle.
    bus.write_word(TIMER, 0);
    for _ in 0..53_694 {
        bus.tick(7 * 7);
    }
    assert_eq!(bus.read_word(TIMER), 49);
}

#[test]
fn timer_does_not_run_without_the_mapper() {
    let mut bus = Bus::new();
    bus.tick(crate::audio::NTSC_MCLK);
    assert_eq!(bus.everdrive_io.timer(), 0);
}

#[test]
fn reset_clears_the_device_but_keeps_the_sd_directory() {
    let sd = TempDir::new("reset");
    sd.file("f", b"1");
    let mut bus = ssf_bus(Some(&sd));
    disk_init(&mut bus);
    assert_eq!(fopen(&mut bus, FILE_MODE_READ, b"f"), STATUS_OK);
    bus.tick(crate::audio::NTSC_MCLK / 100);
    cmd(&mut bus, CMD_STATUS);
    bus.reset();
    bus.write_word(0xA130F0, 0xA000);
    assert!(rx_empty(&mut bus));
    assert_eq!(bus.read_word(TIMER), 0);
    assert_eq!(favb(&mut bus), 0, "disk needs DISK_INIT again");
    disk_init(&mut bus);
    assert_eq!(fopen(&mut bus, FILE_MODE_READ, b"f"), STATUS_OK);
}

#[test]
fn save_state_round_trip_reopens_the_file() {
    // Bus (de)serialization needs more stack than the default test thread.
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(save_state_round_trip)
        .unwrap()
        .join()
        .unwrap();
}

fn save_state_round_trip() {
    let sd = TempDir::new("state");
    let data: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
    sd.file("disk.img", &data);
    let mut bus = ssf_bus(Some(&sd));
    disk_init(&mut bus);
    assert_eq!(fopen(&mut bus, FILE_MODE_READ, b"disk.img"), STATUS_OK);
    cmd(&mut bus, CMD_F_FPTR);
    send(&mut bus, &1000u32.to_be_bytes());
    // Stop part-way through a read longer than the FIFO, mid-frame.
    cmd(&mut bus, CMD_F_FRD);
    send(&mut bus, &3000u32.to_be_bytes());
    assert_eq!(recv(&mut bus, 101)[1..], data[1000..1100]);
    bus.tick(crate::audio::NTSC_MCLK / 1000 * 42 + 1000);
    cmd(&mut bus, CMD_USB_WRITE);
    send(&mut bus, &[0, 1, b'z']);

    let json = serde_json::to_string(&bus).unwrap();
    let mut restored: Bus = serde_json::from_str(&json).unwrap();
    restored.restore_rom(bus.take_rom_image());
    restored.everdrive_io.adopt_host(&mut bus.everdrive_io);
    drop(bus);

    assert_eq!(restored.read_word(TIMER), 42);
    assert_eq!(restored.everdrive_io.take_usb_output(), b"z");
    assert_eq!(recv(&mut restored, 2900), data[1100..4000]);
    assert!(rx_empty(&mut restored));
    // The reopened handle keeps working at the restored position.
    assert_eq!(fread(&mut restored, 1000), (0, data[4000..].to_vec()));
    assert_eq!(favb(&mut restored), 5000);
}

#[test]
fn old_states_load_with_an_idle_device() {
    let io: EverdriveIo = serde_json::from_str("{}").unwrap();
    assert_eq!(io.status(), STATUS_OK);
    assert_eq!(io.rx_level(), 0);
    assert_eq!(io.timer(), 0);
}

#[test]
fn malformed_states_are_clamped() {
    let rx: Vec<u8> = vec![1; 5000];
    let string: Vec<u8> = vec![b'a'; 5000];
    let json = serde_json::json!({
        "rx": rx,
        "string": string,
        "parser": {"Args": {"need": 200, "got": 3}},
        "pending": [{"Bytes": vec![0u8; 100_000]}],
    });
    let mut io: EverdriveIo = serde_json::from_value(json).unwrap();
    let (string, pending, rx, _) = io.buffer_sizes();
    assert_eq!(rx, RX_FIFO_CAPACITY);
    assert!(string <= MAX_STRING_LEN);
    assert!(pending <= MAX_PENDING_BYTES);
    // The parser was reset, so writes cannot index out of range.
    for b in 0..=255u8 {
        io.write_mailbox(b);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Arbitrary register traffic never panics and keeps every buffer
    /// inside its bound.
    #[test]
    fn random_mailbox_traffic_is_safe(
        ops in proptest::collection::vec((0u8..8, any::<u8>(), any::<u16>()), 0..600),
        prefix_cmd in prop::sample::select(vec![
            CMD_STATUS, CMD_USB_WRITE, CMD_DISK_INIT, CMD_F_DIR_LD, CMD_F_DIR_SIZE,
            CMD_F_DIR_GET, CMD_F_FOPN, CMD_F_FRD, CMD_F_FCLOSE, CMD_F_FPTR, CMD_F_AVB,
        ]),
    ) {
        let sd = TempDir::new("prop");
        sd.file("a.bin", &[7; 700]);
        let mut bus = ssf_bus(Some(&sd));
        disk_init(&mut bus);
        assert_eq!(fopen(&mut bus, FILE_MODE_READ, b"a.bin"), STATUS_OK);
        cmd(&mut bus, prefix_cmd);
        for (op, byte, word) in ops {
            let addr = DATA + u32::from(byte & 0xF);
            match op {
                0 | 1 => bus.write_word(DATA, u16::from(byte)),
                2 => send(&mut bus, &[CMD_PREAMBLE, !CMD_PREAMBLE, byte, !byte]),
                3 => { bus.read_word(addr & !1); }
                4 => { bus.read_byte(addr); }
                5 => bus.write_byte(addr, byte),
                6 => bus.write_word(addr & !1, word),
                _ => bus.tick(u32::from(word)),
            }
            let (string, pending, rx, segments) = bus.everdrive_io.buffer_sizes();
            prop_assert!(string <= MAX_STRING_LEN);
            prop_assert!(pending <= MAX_PENDING_BYTES);
            prop_assert!(rx <= RX_FIFO_CAPACITY);
            prop_assert!(segments <= 64);
        }
        prop_assert!(bus.everdrive_io.take_usb_output().len() <= MAX_USB_OUTPUT);
        prop_assert_eq!(std::fs::read(sd.path().join("a.bin")).unwrap(), vec![7; 700]);
    }
}

/// A hand-assembled 68k program in a `"SEGA SSF"` ROM: unlock the mapper,
/// print "hi" over USB, open a file on the SD card and read it into
/// cartridge RAM, then spin.
#[test]
fn m68k_program_boots_and_reads_a_file() {
    let sd = TempDir::new("e2e");
    sd.file("hello.txt", b"hello world");

    let mut table = Vec::new();
    let frame = |c: u8| [CMD_PREAMBLE, !CMD_PREAMBLE, c, !c];
    table.extend(frame(CMD_USB_WRITE));
    table.extend([0, 2, b'h', b'i']);
    table.extend(frame(CMD_DISK_INIT));
    table.extend(frame(CMD_F_FOPN));
    table.extend([FILE_MODE_READ, 0, 9]);
    table.extend(b"hello.txt");
    table.extend(frame(CMD_STATUS));
    table.extend(frame(CMD_F_FRD));
    table.extend(5u32.to_be_bytes());
    // Responses: STATUS (2), F_FRD result (1) and data (5).
    let reply_len: u16 = 8;

    let code: &[u16] = &[
        0x33FC,
        0xA000,
        0x00A1,
        0x30F0, // 200: move.w #$A000,$A130F0
        0x41F9,
        0x00A1,
        0x30D0, //         208: lea $A130D0,a0
        0x43FA,
        0x002E, //                 20E: lea table(pc),a1
        0x303C,
        table.len() as u16 - 1, // 212: move.w #len-1,d0
        0x7200,                 //                         216: moveq #0,d1
        0x1219,                 //                         218: move.b (a1)+,d1
        0x3081,                 //                         21A: move.w d1,(a0)
        0x51C8,
        0xFFFA, //                 21C: dbra d0,$218
        0x45F9,
        0x0010,
        0x0000, //         220: lea $100000,a2
        0x303C,
        reply_len - 1, //          226: move.w #7,d0
        0x3228,
        0x0002, //                 22A: move.w 2(a0),d1
        0x0241,
        0x07FF, //                 22E: andi.w #$7FF,d1
        0x67F6, //                         232: beq.s $22A
        0x3210, //                         234: move.w (a0),d1
        0x14C1, //                         236: move.b d1,(a2)+
        0x51C8,
        0xFFF0, //                 238: dbra d0,$22A
        0x60FE, //                         23C: bra.s *
    ];
    const SPIN: u32 = 0x23C;

    let mut rom = ssf_rom();
    rom[0..4].copy_from_slice(&0x00FF_8000u32.to_be_bytes());
    rom[4..8].copy_from_slice(&0x0000_0200u32.to_be_bytes());
    for (i, w) in code.iter().enumerate() {
        rom[0x200 + 2 * i..0x202 + 2 * i].copy_from_slice(&w.to_be_bytes());
    }
    assert_eq!(0x200 + 2 * code.len(), 0x23E, "table follows the code");
    rom[0x23E..0x23E + table.len()].copy_from_slice(&table);

    let mut bus = Bus::new();
    bus.load_rom(&rom);
    bus.everdrive_io.set_sd_root(Some(sd.path())).unwrap();
    let mut cpu = crate::cpu::Cpu::new(&mut bus);
    let mut steps = 0;
    while cpu.pc != SPIN {
        let cycles = cpu.step_instruction(&mut bus);
        bus.tick(cycles * 7);
        steps += 1;
        assert!(steps < 100_000, "program did not finish (PC {:#x})", cpu.pc);
    }

    assert!(bus.everdrive.write_enable, "$A000 unlocked the mapper RAM");
    assert_eq!(bus.everdrive_io.take_usb_output(), b"hi");
    let ram: Vec<u8> = (0..8).map(|i| bus.read_byte(0x100000 + i)).collect();
    assert_eq!(ram, [0xA5, 0x00, 0x00, b'h', b'e', b'l', b'l', b'o']);
    assert!(rx_empty(&mut bus));
}
