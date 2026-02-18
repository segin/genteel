use genteel::{AudioBuffer, audio::BUFFER_SIZE};
use std::time::Instant;

#[test]
fn bench_audio_push() {
    // Buffer size is BUFFER_SIZE * 2 (stereo) = 4096 samples
    // We want to exercise the wrap-around logic, so we use a chunk size that isn't a factor of buffer size
    let chunk_size = 1000;
    let mut buf = AudioBuffer::new(BUFFER_SIZE);

    // Create a chunk of samples
    let samples = vec![100i16; chunk_size];

    // Total iterations
    let iterations = 1_000_000;

    // Buffer to pop into to make space
    let mut pop_buf = vec![0i16; chunk_size];

    let start = Instant::now();

    for _ in 0..iterations {
        // Push samples
        buf.push(&samples);

        // Pop samples to make space for the next push
        // This keeps the buffer moving and forces wrapping
        buf.pop(&mut pop_buf);
    }

    let duration = start.elapsed();

    println!("Audio Push Benchmark ({} iterations) took: {:?}", iterations, duration);
    println!("Samples pushed: {}", iterations * chunk_size);

    let seconds = duration.as_secs_f64();
    if seconds > 0.0 {
        let samples_per_sec = (iterations * chunk_size) as f64 / seconds;
        println!("Throughput: {:.2} M samples/sec", samples_per_sec / 1_000_000.0);
    }
}
