use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};

pub struct WavWriter {
    file: BufWriter<File>,
    data_size: u32,
}

impl WavWriter {
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
        })
    }

    pub fn write_samples(&mut self, samples: &[i16]) -> std::io::Result<()> {
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

    struct TempFile(String);
    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn test_wav_writer() {
        let path = "test_wav_writer_output.wav";
        let _temp_file = TempFile(path.to_string());

        {
            let mut writer = WavWriter::new(path, 44100, 2).unwrap();
            let samples = vec![0, 100, -100, 0];
            writer.write_samples(&samples).unwrap();
        } // Drop writer to finalize

        let mut file = File::open(path).unwrap();
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).unwrap();

        // Header Check
        assert_eq!(&buffer[0..4], b"RIFF");
        assert_eq!(&buffer[8..12], b"WAVE");
        assert_eq!(&buffer[12..16], b"fmt ");

        // Check channels (offset 22, 2 bytes)
        let channels = u16::from_le_bytes([buffer[22], buffer[23]]);
        assert_eq!(channels, 2);

        // Check sample rate (offset 24, 4 bytes)
        let sample_rate = u32::from_le_bytes([buffer[24], buffer[25], buffer[26], buffer[27]]);
        assert_eq!(sample_rate, 44100);

        // Check data chunk (offset 36)
        assert_eq!(&buffer[36..40], b"data");

        // Check data size (offset 40, 4 bytes)
        let data_size = u32::from_le_bytes([buffer[40], buffer[41], buffer[42], buffer[43]]);
        assert_eq!(data_size, 8); // 4 samples * 2 bytes/sample
    }
}
