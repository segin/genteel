use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};

pub struct WavWriter<W: Write + Seek = BufWriter<File>> {
    writer: W,
    data_size: u32,
    channels: u16,
}

pub type FileWavWriter = WavWriter<BufWriter<File>>;

impl WavWriter<BufWriter<File>> {
    pub fn new(path: &str, sample_rate: u32, channels: u16) -> std::io::Result<Self> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        Self::new_with_writer(writer, sample_rate, channels)
    }
}

impl<W: Write + Seek> WavWriter<W> {
    pub fn new_with_writer(mut writer: W, sample_rate: u32, channels: u16) -> std::io::Result<Self> {
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
            writer,
            data_size: 0,
            channels,
        })
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn write_samples(&mut self, samples: &[i16]) -> std::io::Result<()> {
        if samples.len() % (self.channels as usize) != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Sample count not aligned with channel count",
            ));
        }
        for &sample in samples {
            self.writer.write_all(&sample.to_le_bytes())?;
        }
        self.data_size += (samples.len() * 2) as u32;
        Ok(())
    }

    fn finalize(&mut self) -> std::io::Result<()> {
        self.writer.flush()?;

        // Total file size = 36 + data_size
        // 36 comes from: 4 (WAVE) + 24 (fmt chunk) + 8 (data header)
        // RIFF Size = File size - 8 = 36 + data_size
        let file_size = 36 + self.data_size;

        // Update RIFF size (offset 4)
        self.writer.seek(SeekFrom::Start(4))?;
        self.writer.write_all(&file_size.to_le_bytes())?;

        // Update data size (offset 40)
        self.writer.seek(SeekFrom::Start(40))?;
        self.writer.write_all(&self.data_size.to_le_bytes())?;

        self.writer.seek(SeekFrom::End(0))?;
        Ok(())
    }
}

impl<W: Write + Seek> Drop for WavWriter<W> {
    fn drop(&mut self) {
        let _ = self.finalize();
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
        assert_eq!(&buffer[20..22], &1u16.to_le_bytes());  // PCM
        assert_eq!(&buffer[22..24], &2u16.to_le_bytes());  // Channels
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
        // Size placeholder
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
            assert_eq!(wav.data_size, 6); // 3 samples * 2 bytes
        } // wav dropped here

        // Verify samples are written after header (44 bytes)
        let expected_data_start = 44;
        let data = &buffer[expected_data_start..];

        let mut expected_bytes = Vec::new();
        for s in &samples {
            expected_bytes.extend_from_slice(&s.to_le_bytes());
        }

        assert_eq!(data, expected_bytes.as_slice());
    }

    #[test]
    fn test_write_samples_unaligned() {
        let mut buffer = Vec::new();
        let cursor = Cursor::new(&mut buffer);
        let mut writer = WavWriter::new_with_writer(cursor, 44100, 2).unwrap();
        let samples = vec![0, 0, 100]; // 3 samples, 2 channels => unaligned
        let result = writer.write_samples(&samples);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
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

        // Check File Size at offset 4
        // File size = 36 + data_size = 36 + 20 = 56
        let expected_file_size = 56u32;
        assert_eq!(&buffer[4..8], &expected_file_size.to_le_bytes());

        // Check Data Size at offset 40
        // Data size = 20
        let expected_data_size = 20u32;
        assert_eq!(&buffer[40..44], &expected_data_size.to_le_bytes());
    }
}
