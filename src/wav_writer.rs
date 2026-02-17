use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};

pub struct WavWriter<W: Write + Seek = BufWriter<File>> {
    writer: W,
    data_size: u32,
    #[allow(dead_code)] // Stored for potential future use or debugging
    channels: u16,
}

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

    pub fn write_samples(&mut self, samples: &[i16]) -> std::io::Result<()> {
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
        // RIFF header size (4) + Size (4) are not included in the Size field value itself,
        // but the Size field covers everything after it.
        // File size = 44 + data_size
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
    fn test_wav_header_format() {
        let mut buffer = Vec::new();
        let cursor = Cursor::new(&mut buffer);
        let sample_rate = 44100;
        let channels = 2;

        {
            let _writer = WavWriter::new_with_writer(cursor, sample_rate, channels).unwrap();
            // Header is written on creation
        }

        // Verify content
        // 0-3: "RIFF"
        assert_eq!(&buffer[0..4], b"RIFF");
        // 4-7: File size (placeholder initially, updated on drop)
        // 8-11: "WAVE"
        assert_eq!(&buffer[8..12], b"WAVE");
        // 12-15: "fmt "
        assert_eq!(&buffer[12..16], b"fmt ");
        // 16-19: Chunk size (16)
        assert_eq!(&buffer[16..20], &16u32.to_le_bytes());
        // 20-21: AudioFormat (1)
        assert_eq!(&buffer[20..22], &1u16.to_le_bytes());
        // 22-23: Channels (2)
        assert_eq!(&buffer[22..24], &channels.to_le_bytes());
        // 24-27: Sample Rate (44100)
        assert_eq!(&buffer[24..28], &sample_rate.to_le_bytes());

        // 28-31: Byte Rate (SampleRate * Channels * BitsPerSample/8) = 44100 * 2 * 2 = 176400
        let expected_byte_rate = sample_rate * u32::from(channels) * 2;
        assert_eq!(&buffer[28..32], &expected_byte_rate.to_le_bytes());

        // 32-33: Block Align (Channels * BitsPerSample/8) = 2 * 2 = 4
        let expected_block_align = channels * 2;
        assert_eq!(&buffer[32..34], &expected_block_align.to_le_bytes());

        // 34-35: BitsPerSample (16)
        assert_eq!(&buffer[34..36], &16u16.to_le_bytes());

        // 36-39: "data"
        assert_eq!(&buffer[36..40], b"data");
        // 40-43: Data size (placeholder initially, updated on drop)
    }

    #[test]
    fn test_write_samples() {
        let mut buffer = Vec::new();
        let cursor = Cursor::new(&mut buffer);
        let sample_rate = 44100;
        let channels = 1; // Mono for simplicity

        {
            let mut writer = WavWriter::new_with_writer(cursor, sample_rate, channels).unwrap();
            let samples = vec![i16::MIN, 0, i16::MAX];
            writer.write_samples(&samples).unwrap();
        }

        // Check data after header (header is 44 bytes)
        assert_eq!(buffer.len(), 44 + 3 * 2); // 3 samples, 2 bytes each

        // Check samples
        let sample_data = &buffer[44..];
        assert_eq!(&sample_data[0..2], &i16::MIN.to_le_bytes());
        assert_eq!(&sample_data[2..4], &0i16.to_le_bytes());
        assert_eq!(&sample_data[4..6], &i16::MAX.to_le_bytes());
    }

    #[test]
    fn test_finalize_updates_sizes() {
        let mut buffer = Vec::new();
        let cursor = Cursor::new(&mut buffer);

        {
            let mut writer = WavWriter::new_with_writer(cursor, 44100, 2).unwrap();
            let samples = vec![0; 10];
            writer.write_samples(&samples).unwrap();
        } // Drop happens here, calling finalize

        // Check File Size at offset 4
        // File size = 36 + data_size = 36 + 20 = 56
        let file_size_bytes = &buffer[4..8];
        let file_size = u32::from_le_bytes(file_size_bytes.try_into().unwrap());
        assert_eq!(file_size, 56);

        // Check Data Size at offset 40
        // Data size = 20
        let data_size_bytes = &buffer[40..44];
        let data_size = u32::from_le_bytes(data_size_bytes.try_into().unwrap());
        assert_eq!(data_size, 20);
    }
}
