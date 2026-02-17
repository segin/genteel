use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};

pub struct WavWriter {
    file: BufWriter<File>,
    data_size: u32,
    channels: u16,
}

impl WavWriter {
    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn new(path: &str, sample_rate: u32, channels: u16) -> std::io::Result<Self> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

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

        let byte_rate = sample_rate * u32::from(channels) * 2; // 16-bit = 2 bytes
        writer.write_all(&byte_rate.to_le_bytes())?;

        let block_align = channels * 2;
        writer.write_all(&block_align.to_le_bytes())?;
        writer.write_all(&16u16.to_le_bytes())?; // BitsPerSample

        // data chunk
        writer.write_all(b"data")?;
        writer.write_all(&[0; 4])?; // Placeholder for data size

        Ok(Self {
            file: writer,
            data_size: 0,
            channels,
        })
    }

    pub fn write_samples(&mut self, samples: &[i16]) -> std::io::Result<()> {
        if samples.len() % (self.channels as usize) != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Sample count not aligned with channel count",
            ));
        }
        for &sample in samples {
            self.file.write_all(&sample.to_le_bytes())?;
        }
        self.data_size += (samples.len() * 2) as u32;
        Ok(())
    }

    fn finalize(&mut self) -> std::io::Result<()> {
        self.file.flush()?;
        let file = self.file.get_mut();

        // Total file size = 36 + data_size
        // 36 comes from: 4 (WAVE) + 24 (fmt chunk) + 8 (data header)
        // RIFF header size (4) + Size (4) are not included in the Size field value itself,
        // but the Size field covers everything after it.
        // File size = 44 + data_size
        // RIFF Size = File size - 8 = 36 + data_size
        let file_size = 36 + self.data_size;

        // Update RIFF size (offset 4)
        file.seek(SeekFrom::Start(4))?;
        file.write_all(&file_size.to_le_bytes())?;

        // Update data size (offset 40)
        file.seek(SeekFrom::Start(40))?;
        file.write_all(&self.data_size.to_le_bytes())?;

        file.seek(SeekFrom::End(0))?;
        Ok(())
    }
}

impl Drop for WavWriter {
    fn drop(&mut self) {
        let _ = self.finalize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn test_wav_writer_creation() {
        let path = "test_output.wav";
        let _ = std::fs::remove_file(path); // Cleanup before test

        {
            let mut writer = WavWriter::new(path, 44100, 2).expect("Failed to create WavWriter");
            let samples = vec![0, 0, 100, -100];
            writer.write_samples(&samples).expect("Failed to write samples");
        } // writer is dropped here, finalizing the file

        // Verify file content
        let mut file = File::open(path).expect("Failed to open output file");
        let mut content = Vec::new();
        file.read_to_end(&mut content).expect("Failed to read file");

        // RIFF header
        assert_eq!(&content[0..4], b"RIFF");
        assert_eq!(&content[8..12], b"WAVE");

        // fmt chunk
        assert_eq!(&content[12..16], b"fmt ");

        // data chunk
        // The position depends on fmt chunk size (16) and header size.
        // 12 (RIFF) + 8 (fmt header) + 16 (fmt body) = 36
        assert_eq!(&content[36..40], b"data");

        // Cleanup
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_wav_writer_channels() {
        let path = "test_channels.wav";
        let _ = std::fs::remove_file(path);
        let writer = WavWriter::new(path, 44100, 2).expect("Failed to create");
        assert_eq!(writer.channels(), 2);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_write_samples_unaligned() {
        let path = "test_unaligned.wav";
        let _ = std::fs::remove_file(path);
        let mut writer = WavWriter::new(path, 44100, 2).expect("Failed to create");
        let samples = vec![0, 0, 100]; // 3 samples, 2 channels => unaligned
        let result = writer.write_samples(&samples);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
        let _ = std::fs::remove_file(path);
    }
}
