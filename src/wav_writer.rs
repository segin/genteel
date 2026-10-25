//! 16-bit PCM WAV writer used by `--dump-audio`.
//!
//! The writer keeps the RIFF header consistent with the bytes actually
//! written: it stops recording (and reports why) on the first I/O error or
//! when the 4 GiB RIFF size limit is reached, and the header written on
//! finalization always describes exactly the data that reached the writer.

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, ErrorKind, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

/// Bytes of RIFF payload preceding the sample data:
/// "WAVE" (4) + fmt chunk (24) + data chunk header (8).
const RIFF_HEADER_BYTES: u64 = 36;
/// Largest data chunk whose RIFF size (`36 + data`) still fits in a u32.
const MAX_DATA_BYTES: u64 = u32::MAX as u64 - RIFF_HEADER_BYTES;

pub struct WavWriter<W: Write + Seek = BufWriter<File>> {
    writer: W,
    /// Sample-data bytes accepted by `writer`.
    data_size: u64,
    channels: u16,
    /// Largest data chunk this writer will produce.
    max_data_bytes: u64,
    /// Set once recording stopped (I/O error or size limit).
    stopped: Option<String>,
    finalized: bool,
}

pub type FileWavWriter = WavWriter<BufWriter<File>>;

fn denied(path: &str) -> std::io::Error {
    std::io::Error::new(
        ErrorKind::PermissionDenied,
        format!(
            "Access denied to path: {}. Audio dumps must be within the current directory.",
            path
        ),
    )
}

/// Resolve `path` to a new file inside `base_dir` (which must be canonical).
///
/// Only plain relative components are accepted; the parent directory is
/// canonicalized so a symlinked directory cannot lead outside `base_dir`.
fn resolve_dump_path(base_dir: &Path, path: &str) -> std::io::Result<PathBuf> {
    let mut relative = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            Component::Normal(c) => relative.push(c),
            Component::CurDir => {}
            _ => return Err(denied(path)),
        }
    }
    let file_name = relative
        .file_name()
        .map(|n| n.to_os_string())
        .ok_or_else(|| {
            std::io::Error::new(
                ErrorKind::InvalidInput,
                "Invalid path: must specify a file name",
            )
        })?;
    let parent = base_dir.join(relative.parent().unwrap_or(Path::new("")));
    let parent = parent.canonicalize()?;
    if !parent.starts_with(base_dir) {
        return Err(denied(path));
    }
    Ok(parent.join(file_name))
}

impl WavWriter<BufWriter<File>> {
    /// Create a new WAV file at `path`, which must name a file that does not
    /// exist yet inside the current directory (or one of its subdirectories).
    pub fn new(path: &str, sample_rate: u32, channels: u16) -> std::io::Result<Self> {
        let base_dir = std::env::current_dir()?.canonicalize()?;
        Self::new_in(&base_dir, path, sample_rate, channels)
    }

    /// Like [`WavWriter::new`] but relative to an explicit base directory.
    pub fn new_in(
        base_dir: &Path,
        path: &str,
        sample_rate: u32,
        channels: u16,
    ) -> std::io::Result<Self> {
        let base_dir = base_dir.canonicalize()?;
        let full_path = resolve_dump_path(&base_dir, path)?;
        /* create_new refuses existing files and (dangling) symlinks instead
         * of truncating or following them. */
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&full_path)?;
        Self::new_with_writer(BufWriter::new(file), sample_rate, channels)
    }
}

impl<W: Write + Seek> WavWriter<W> {
    pub fn new_with_writer(
        mut writer: W,
        sample_rate: u32,
        channels: u16,
    ) -> std::io::Result<Self> {
        // RIFF header
        writer.write_all(b"RIFF")?;
        writer.write_all(&[0; 4])?; // Placeholder for file size
        writer.write_all(b"WAVE")?;

        // fmt chunk
        writer.write_all(b"fmt ")?;
        writer.write_all(&16u32.to_le_bytes())?; // Chunk size (16 for PCM)
        writer.write_all(&1u16.to_le_bytes())?; // AudioFormat (1 = PCM)
        writer.write_all(&channels.to_le_bytes())?;
        writer.write_all(&sample_rate.to_le_bytes())?;

        let byte_rate = sample_rate.saturating_mul(u32::from(channels) * 2); // 16-bit = 2 bytes
        writer.write_all(&byte_rate.to_le_bytes())?;

        let block_align = channels.saturating_mul(2);
        writer.write_all(&block_align.to_le_bytes())?;
        writer.write_all(&16u16.to_le_bytes())?; // BitsPerSample

        // data chunk
        writer.write_all(b"data")?;
        writer.write_all(&[0; 4])?; // Placeholder for data size

        Ok(Self {
            writer,
            data_size: 0,
            channels,
            max_data_bytes: MAX_DATA_BYTES,
            stopped: None,
            finalized: false,
        })
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// Sample-data bytes written so far.
    pub fn data_bytes(&self) -> u64 {
        self.data_size
    }

    /// Why recording stopped, if it did.
    pub fn stopped_reason(&self) -> Option<&str> {
        self.stopped.as_deref()
    }

    /// Whether recording is still active.
    pub fn is_recording(&self) -> bool {
        self.stopped.is_none() && !self.finalized
    }

    #[cfg(test)]
    fn set_max_data_bytes(&mut self, max: u64) {
        self.max_data_bytes = max.min(MAX_DATA_BYTES);
    }

    fn stop(&mut self, reason: String) -> std::io::Error {
        log::error!("WAV dump stopped: {}", reason);
        self.stopped = Some(reason.clone());
        std::io::Error::other(reason)
    }

    /// Hand `bytes` to the writer, counting exactly what it accepted.
    fn write_counted(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        let mut rest = bytes;
        while !rest.is_empty() {
            match self.writer.write(rest) {
                Ok(0) => {
                    return Err(self.stop("write accepted no bytes".to_string()));
                }
                Ok(n) => {
                    self.data_size += n as u64;
                    rest = &rest[n..];
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(self.stop(format!("write failed: {}", e))),
            }
        }
        Ok(())
    }

    pub fn write_samples(&mut self, samples: &[i16]) -> std::io::Result<()> {
        if let Some(reason) = &self.stopped {
            return Err(std::io::Error::other(format!(
                "recording already stopped: {}",
                reason
            )));
        }
        let channels = self.channels.max(1) as usize;
        if !samples.len().is_multiple_of(channels) {
            return Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                "Sample count not aligned with channel count",
            ));
        }

        /* Never let the data chunk outgrow the RIFF u32 size fields: keep
         * whole frames up to the limit, then stop recording. */
        let frame_bytes = channels as u64 * 2;
        let room = self.max_data_bytes.saturating_sub(self.data_size);
        let room_samples = ((room / frame_bytes) * channels as u64) as usize;
        let (to_write, truncated) = if samples.len() > room_samples {
            (&samples[..room_samples], true)
        } else {
            (samples, false)
        };

        // Use a stack-allocated buffer to batch writes and reduce call overhead
        let mut buffer = [0u8; 2048];
        for chunk in to_write.chunks(buffer.len() / 2) {
            for (i, &sample) in chunk.iter().enumerate() {
                buffer[2 * i..2 * i + 2].copy_from_slice(&sample.to_le_bytes());
            }
            self.write_counted(&buffer[..chunk.len() * 2])?;
        }

        if truncated {
            return Err(self.stop("WAV size limit (4 GiB) reached".to_string()));
        }
        Ok(())
    }

    fn finalize(&mut self) -> std::io::Result<()> {
        if self.finalized {
            return Ok(());
        }
        self.finalized = true;
        self.writer.flush()?;

        // RIFF Size = File size - 8 = 36 + data_size
        let data_size = self.data_size.min(MAX_DATA_BYTES) as u32;
        let file_size = RIFF_HEADER_BYTES as u32 + data_size;

        // Update RIFF size (offset 4)
        self.writer.seek(SeekFrom::Start(4))?;
        self.writer.write_all(&file_size.to_le_bytes())?;

        // Update data size (offset 40)
        self.writer.seek(SeekFrom::Start(40))?;
        self.writer.write_all(&data_size.to_le_bytes())?;

        self.writer.seek(SeekFrom::End(0))?;
        self.writer.flush()
    }

    /// Finalize the header now, reporting any error.
    pub fn finish(mut self) -> std::io::Result<()> {
        self.finalize()
    }
}

impl<W: Write + Seek> Drop for WavWriter<W> {
    fn drop(&mut self) {
        if let Err(e) = self.finalize() {
            log::error!("Failed to finalize WAV dump: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_wav_header_generation() {
        let mut buffer = Vec::new();
        {
            let writer = Cursor::new(&mut buffer);
            let wav = WavWriter::new_with_writer(writer, 44100, 2).unwrap();
            assert_eq!(wav.channels(), 2);
        } // wav dropped

        // Check RIFF header
        assert_eq!(&buffer[0..4], b"RIFF");
        assert_eq!(&buffer[8..12], b"WAVE");

        // Check fmt chunk
        assert_eq!(&buffer[12..16], b"fmt ");
        assert_eq!(&buffer[16..20], &16u32.to_le_bytes()); // Chunk size
        assert_eq!(&buffer[20..22], &1u16.to_le_bytes()); // PCM
        assert_eq!(&buffer[22..24], &2u16.to_le_bytes()); // Channels
        assert_eq!(&buffer[24..28], &44100u32.to_le_bytes()); // Sample rate

        // Byte rate = 44100 * 2 * 2 = 176400
        let expected_byte_rate: u32 = 44100 * 2 * 2;
        assert_eq!(&buffer[28..32], &expected_byte_rate.to_le_bytes());

        // Block align = 2 * 2 = 4
        assert_eq!(&buffer[32..34], &4u16.to_le_bytes());

        // Bits per sample = 16
        assert_eq!(&buffer[34..36], &16u16.to_le_bytes());

        // Check data chunk header
        assert_eq!(&buffer[36..40], b"data");
        // Empty data chunk
        assert_eq!(&buffer[40..44], &[0; 4]);
    }

    #[test]
    fn test_wav_sample_writing() {
        let mut buffer = Vec::new();
        let samples = vec![i16::MIN, 0, i16::MAX];

        {
            let writer = Cursor::new(&mut buffer);
            let mut wav = WavWriter::new_with_writer(writer, 44100, 1).unwrap();

            wav.write_samples(&samples).unwrap();
            assert_eq!(wav.data_bytes(), 6); // 3 samples * 2 bytes
        } // wav dropped here

        // Verify samples are written after header (44 bytes)
        let data = &buffer[44..];
        let expected_bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        assert_eq!(data, expected_bytes.as_slice());
    }

    #[test]
    fn test_large_write_spans_batches() {
        let mut buffer = Vec::new();
        let samples: Vec<i16> = (0..5000).map(|i| i as i16).collect();
        {
            let mut wav = WavWriter::new_with_writer(Cursor::new(&mut buffer), 8000, 2).unwrap();
            wav.write_samples(&samples).unwrap();
        }
        let expected: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        assert_eq!(&buffer[44..], expected.as_slice());
        assert_eq!(&buffer[40..44], &10_000u32.to_le_bytes());
    }

    #[test]
    fn test_write_samples_unaligned() {
        let mut buffer = Vec::new();
        let cursor = Cursor::new(&mut buffer);
        let mut writer = WavWriter::new_with_writer(cursor, 44100, 2).unwrap();
        let samples = vec![0, 0, 100]; // 3 samples, 2 channels => unaligned
        let result = writer.write_samples(&samples);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::InvalidInput);
        assert!(writer.is_recording());
    }

    #[test]
    fn test_wav_finalize() {
        let mut buffer = Vec::new();
        let writer = Cursor::new(&mut buffer);
        {
            let mut wav = WavWriter::new_with_writer(writer, 44100, 2).unwrap();
            let samples = vec![0; 10]; // 20 bytes
            wav.write_samples(&samples).unwrap();
        } // wav dropped here, finalize called

        // File size = 36 + data_size = 36 + 20 = 56
        assert_eq!(&buffer[4..8], &56u32.to_le_bytes());
        assert_eq!(&buffer[40..44], &20u32.to_le_bytes());
    }

    #[test]
    fn test_size_limit_finalizes_without_overflow() {
        let mut buffer = Vec::new();
        {
            let mut wav = WavWriter::new_with_writer(Cursor::new(&mut buffer), 44100, 2).unwrap();
            wav.set_max_data_bytes(18); // room for 4 whole stereo frames (16 bytes)
            wav.write_samples(&[1, 2, 3, 4]).unwrap();
            let err = wav.write_samples(&[5, 6, 7, 8, 9, 10]).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::Other);
            assert!(!wav.is_recording());
            assert_eq!(wav.data_bytes(), 16);
            assert!(wav.write_samples(&[0, 0]).is_err());
            assert_eq!(wav.data_bytes(), 16);
        }
        assert_eq!(&buffer[4..8], &(36u32 + 16).to_le_bytes());
        assert_eq!(&buffer[40..44], &16u32.to_le_bytes());
        assert_eq!(buffer.len(), 44 + 16);
    }

    #[test]
    fn test_real_limit_keeps_riff_size_in_u32() {
        let wav = WavWriter::new_with_writer(Cursor::new(Vec::new()), 44100, 2).unwrap();
        assert_eq!(wav.max_data_bytes, u32::MAX as u64 - 36);
        assert!(RIFF_HEADER_BYTES + wav.max_data_bytes <= u32::MAX as u64);
    }

    /// Writer that accepts `budget` bytes and then fails.
    struct FailingWriter {
        inner: Cursor<Vec<u8>>,
        budget: usize,
    }

    impl Write for FailingWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.budget == 0 {
                return Err(std::io::Error::other("disk full"));
            }
            let n = buf.len().min(self.budget);
            self.budget -= n;
            self.inner.write(&buf[..n])
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Seek for FailingWriter {
        fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
            self.inner.seek(pos)
        }
    }

    #[test]
    fn test_write_error_stops_recording_and_keeps_header_consistent() {
        let writer = FailingWriter {
            inner: Cursor::new(Vec::new()),
            budget: 44 + 10,
        };
        let mut wav = WavWriter::new_with_writer(writer, 22050, 2).unwrap();
        let err = wav.write_samples(&[1; 8]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Other);
        assert!(!wav.is_recording());
        assert!(wav.stopped_reason().unwrap().contains("disk full"));
        assert_eq!(wav.data_bytes(), 10);
        assert!(wav.write_samples(&[1, 1]).is_err());

        /* Restore budget so the header rewrite can succeed. */
        wav.writer.budget = usize::MAX;
        wav.finalize().unwrap();
        let bytes = wav.writer.inner.get_ref().clone();
        assert_eq!(bytes.len(), 44 + 10);
        assert_eq!(&bytes[4..8], &(36u32 + 10).to_le_bytes());
        assert_eq!(&bytes[40..44], &10u32.to_le_bytes());
    }

    #[test]
    fn test_wav_writer_channels() {
        for channels in [1u16, 2] {
            let wav = WavWriter::new_with_writer(Cursor::new(Vec::new()), 44100, channels).unwrap();
            assert_eq!(wav.channels(), channels);
        }
    }

    /// Fresh scratch directory under `target/` (inside the crate root).
    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::current_dir()
            .unwrap()
            .join("target")
            .join("wav_writer_tests")
            .join(format!("{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.canonicalize().unwrap()
    }

    #[test]
    fn test_wav_writer_new_in_relative_path() {
        let dir = scratch_dir("relative");
        std::fs::create_dir(dir.join("sub")).unwrap();
        {
            let wav = WavWriter::new_in(&dir, "./sub/out.wav", 44100, 2).unwrap();
            assert_eq!(wav.channels(), 2);
        }
        assert_eq!(
            std::fs::metadata(dir.join("sub/out.wav")).unwrap().len(),
            44
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_wav_writer_new_cwd_relative() {
        std::fs::create_dir_all("target").unwrap();
        let file_path = format!("target/test_wav_writer_new_{}.wav", std::process::id());
        let _ = std::fs::remove_file(&file_path);
        {
            let wav = WavWriter::new(&file_path, 44100, 2).unwrap();
            assert_eq!(wav.channels(), 2);
        }
        assert_eq!(std::fs::metadata(&file_path).unwrap().len(), 44);
        std::fs::remove_file(&file_path).unwrap();
    }

    #[test]
    fn test_refuses_existing_file() {
        let dir = scratch_dir("existing");
        let target = dir.join("keep.wav");
        std::fs::write(&target, b"precious").unwrap();
        let err = WavWriter::new_in(&dir, "keep.wav", 44100, 2)
            .err()
            .expect("existing file must be refused");
        assert_eq!(err.kind(), ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(&target).unwrap(), b"precious");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn test_refuses_symlink_escape() {
        let dir = scratch_dir("symlink_base");
        let outside = scratch_dir("symlink_outside");
        std::os::unix::fs::symlink(&outside, dir.join("escape")).unwrap();
        let err = WavWriter::new_in(&dir, "escape/out.wav", 44100, 2)
            .err()
            .expect("symlinked directory leading outside must be refused");
        assert_eq!(err.kind(), ErrorKind::PermissionDenied);
        assert!(!outside.join("out.wav").exists());

        /* A symlinked file target is refused rather than followed. */
        let victim = outside.join("victim.wav");
        std::fs::write(&victim, b"keep").unwrap();
        std::os::unix::fs::symlink(&victim, dir.join("link.wav")).unwrap();
        assert!(WavWriter::new_in(&dir, "link.wav", 44100, 2).is_err());
        assert_eq!(std::fs::read(&victim).unwrap(), b"keep");

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn test_wav_writer_new_invalid_path() {
        let result = WavWriter::new("non_existent_dir/test.wav", 44100, 2);
        match result {
            Err(e) => {
                assert!(e.kind() == ErrorKind::NotFound || e.kind() == ErrorKind::PermissionDenied)
            }
            _ => panic!("Expected error"),
        }
    }

    #[test]
    fn test_wav_writer_new_directory_error() {
        // An existing directory is never truncated or replaced.
        assert!(WavWriter::new("src", 44100, 2).is_err());
        assert!(WavWriter::new(".", 44100, 2).is_err());
    }

    #[test]
    fn test_wav_writer_new_path_traversal_protection() {
        let traversal_path = "../traversal_test.wav";
        match WavWriter::new(traversal_path, 44100, 2) {
            Err(e) => assert_eq!(e.kind(), ErrorKind::PermissionDenied),
            Ok(_) => panic!("Should have failed with PermissionDenied"),
        }
        assert!(!Path::new(traversal_path).exists());

        let absolute_path = if cfg!(windows) {
            "C:\\Windows\\test.wav"
        } else {
            "/etc/test.wav"
        };
        match WavWriter::new(absolute_path, 44100, 2) {
            Err(e) => assert_eq!(e.kind(), ErrorKind::PermissionDenied),
            Ok(_) => panic!("Should have failed with PermissionDenied"),
        }
    }
}
