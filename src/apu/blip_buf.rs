//! Band-limited synthesis buffer (BlipBuf)
//!
//! Based on the algorithm by Shayne Powell and Blip_Buffer by Blargg.
//! This allows generating high-quality audio from signals with fast transitions
//! (square waves, noise, FM synthesis) by treating transitions as band-limited steps.

use serde::{Deserialize, Serialize};

/// Number of points in the sinc-like kernel
const KERNEL_SIZE: usize = 16;
/// Oversampling factor for kernel lookup
const RES: usize = 512;

/// Band-limited step kernel (Pre-computed)
static KERNEL: std::sync::LazyLock<[i32; KERNEL_SIZE * RES]> = std::sync::LazyLock::new(|| {
    let mut kernel = [0i32; KERNEL_SIZE * RES];
    for (i, sample) in kernel.iter_mut().enumerate().take(KERNEL_SIZE * RES) {
        let x = (i as f64 / RES as f64) - (KERNEL_SIZE as f64 / 2.0);
        if x.abs() < 1e-9 {
            *sample = 32767;
        } else {
            // Sinc function with Blackman window
            let sinc = (std::f64::consts::PI * x).sin() / (std::f64::consts::PI * x);
            let a = 0.42;
            let b = 0.50;
            let c = 0.08;
            let window = a - b
                * (2.0 * std::f64::consts::PI * i as f64 / (KERNEL_SIZE * RES) as f64).cos()
                + c * (4.0 * std::f64::consts::PI * i as f64 / (KERNEL_SIZE * RES) as f64).cos();
            *sample = (sinc * window * 32767.0) as i32;
        }
    }
    kernel
});

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BlipBuf {
    /// Internal integration buffer
    buffer: Vec<i32>,
    /// Logical start of unread samples in the ring buffer
    start: usize,
    /// Target sample rate (e.g. 44100)
    sample_rate: u32,
    /// Source clock rate (e.g. 53267 for FM, 3579545 for PSG)
    clock_rate: u32,
    /// Time of the last sample generated (in source clocks)
    last_clock: u64,
    /// Fractional clock remainder
    clock_ptr: f64,
    /// Current DC offset
    accumulator: i32,
    /// Running integrated output state between read calls.
    integrator: i32,
}

impl BlipBuf {
    pub fn new(clock_rate: u32, sample_rate: u32) -> Self {
        Self {
            buffer: vec![0; (sample_rate as usize / 10) + KERNEL_SIZE + 2], // Large enough for >100ms
            start: 0,
            sample_rate,
            clock_rate,
            last_clock: 0,
            clock_ptr: 0.0,
            accumulator: 0,
            integrator: 0,
        }
    }

    /// Return the source clock rate used when scheduling deltas.
    pub fn clock_rate(&self) -> u32 {
        self.clock_rate
    }

    /// Return the output sample rate used by the buffer.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Set the source clock rate
    pub fn set_clock_rate(&mut self, rate: u32) {
        self.clock_rate = rate;
    }

    /// Reconfigure both source and output timing.
    ///
    /// This clears any queued transitions because they were scheduled
    /// against the previous timing domain.
    pub fn set_timing(&mut self, clock_rate: u32, sample_rate: u32) {
        if self.clock_rate == clock_rate && self.sample_rate == sample_rate {
            return;
        }
        self.clock_rate = clock_rate;
        self.sample_rate = sample_rate;
        self.buffer
            .resize((sample_rate as usize / 10) + KERNEL_SIZE + 2, 0);
        self.clear();
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.buffer.fill(0);
        self.start = 0;
        self.last_clock = 0;
        self.clock_ptr = 0.0;
        self.accumulator = 0;
        self.integrator = 0;
    }

    /// Add a delta (amplitude change) at a specific clock time
    pub fn add_delta(&mut self, clock: u64, delta: i32) {
        if delta == 0 {
            return;
        }

        let clock_delta = clock.saturating_sub(self.last_clock);
        self.last_clock = clock;

        let samples_per_clock = self.sample_rate as f64 / self.clock_rate as f64;
        let time_in_samples = self.clock_ptr + clock_delta as f64 * samples_per_clock;
        let sample_idx = time_in_samples.floor() as usize;
        let fract = time_in_samples - sample_idx as f64;

        if sample_idx + KERNEL_SIZE >= self.buffer.len() {
            // Out of bounds (producer ran too far ahead of the consumer)
            return;
        }

        // Apply band-limited step
        let offset = (fract * RES as f64) as usize;
        for i in 0..KERNEL_SIZE {
            let idx = (self.start + sample_idx + i) % self.buffer.len();
            let kernel_val = KERNEL[i * RES + offset];
            self.buffer[idx] += (delta * kernel_val) >> 15;
        }

        // Update DC accumulator for integration
        self.accumulator += delta;
        self.clock_ptr = time_in_samples;
    }

    /// Read generated samples into a buffer
    pub fn read_samples(&mut self, samples: &mut [i16]) -> usize {
        let count = samples.len().min(self.buffer.len() - KERNEL_SIZE);

        let mut current = self.integrator;
        for (i, sample) in samples.iter_mut().enumerate().take(count) {
            let idx = (self.start + i) % self.buffer.len();
            current += self.buffer[idx];
            *sample = (current.clamp(-32768, 32767)) as i16;
            self.buffer[idx] = 0;
        }
        self.integrator = current;

        self.start = (self.start + count) % self.buffer.len();
        self.clock_ptr = (self.clock_ptr - count as f64).max(0.0);

        count
    }

    /// Return the current integrated amplitude immediately (ignoring kernel latency)
    pub fn read_instant(&self) -> i16 {
        self.accumulator.clamp(-32768, 32767) as i16
    }
}
