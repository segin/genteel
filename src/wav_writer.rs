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
