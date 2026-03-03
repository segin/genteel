use genteel::wav_writer::WavWriter;
use std::io::Cursor;
use std::time::Instant;

fn main() {
    let mut buffer = Vec::new();
    let cursor = Cursor::new(&mut buffer);
    let mut wav = WavWriter::new_with_writer(cursor, 44100, 2).unwrap();

    let samples: Vec<i16> = (0..44100 * 2 * 10).map(|i| (i % 1000) as i16).collect(); // 10 seconds of stereo audio

    let start = Instant::now();
    for _ in 0..100 {
        wav.write_samples(&samples).unwrap();
    }
    let duration = start.elapsed();
    println!("Time for 100 writes: {:?}", duration);
}
