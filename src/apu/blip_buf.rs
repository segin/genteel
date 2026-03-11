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
    for i in 0..(KERNEL_SIZE * RES) {
        let x = (i as f64 / RES as f64) - (KERNEL_SIZE as f64 / 2.0);
        if x.abs() < 1e-9 {
            kernel[i] = 32767;
        } else {
            // Sinc function with Blackman window
            let sinc = (std::f64::consts::PI * x).sin() / (std::f64::consts::PI * x);
            let a = 0.42;
            let b = 0.50;
            let c = 0.08;
            let window = a - b * (2.0 * std::f64::consts::PI * i as f64 / (KERNEL_SIZE * RES) as f64).cos()
                + c * (4.0 * std::f64::consts::PI * i as f64 / (KERNEL_SIZE * RES) as f64).cos();
            kernel[i] = (sinc * window * 32767.0) as i32;
        }
    }
    kernel
});

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BlipBuf {
    /// Internal integration buffer
    buffer: Vec<i32>,
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
}

impl BlipBuf {
    pub fn new(clock_rate: u32, sample_rate: u32) -> Self {
        Self {
            buffer: vec![0; (sample_rate as usize / 10) + KERNEL_SIZE + 2], // Large enough for >100ms
            sample_rate,
            clock_rate,
            last_clock: 0,
            clock_ptr: 0.0,
            accumulator: 0,
        }
    }

    /// Set the source clock rate
    pub fn set_clock_rate(&mut self, rate: u32) {
        self.clock_rate = rate;
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.buffer.fill(0);
        self.accumulator = 0;
    }

    /// Add a delta (amplitude change) at a specific clock time
    pub fn add_delta(&mut self, clock: u64, delta: i32) {
        if delta == 0 { return; }

        let time_in_samples = (clock as f64 * self.sample_rate as f64) / self.clock_rate as f64;
        let sample_idx = time_in_samples as usize;
        let fract = time_in_samples - sample_idx as f64;

        if sample_idx + KERNEL_SIZE >= self.buffer.len() {
            // Should not happen if read frequently, but safety first
            return;
        }

        // Apply band-limited step
        let offset = (fract * RES as f64) as usize;
        for i in 0..KERNEL_SIZE {
            let kernel_val = KERNEL[i * RES + offset];
            self.buffer[sample_idx + i] += (delta * kernel_val) >> 15;
        }
        
        // Update DC accumulator for integration
        self.accumulator += delta;
    }

    /// Read generated samples into a buffer
    pub fn read_samples(&mut self, samples: &mut [i16]) -> usize {
        let count = samples.len().min(self.buffer.len() - KERNEL_SIZE);
        
        let mut current = 0;
        for i in 0..count {
            current += self.buffer[i];
            samples[i] = (current.clamp(-32768, 32767)) as i16;
            self.buffer[i] = 0;
        }

        // Shift remaining data (the "tails" of the kernels)
        self.buffer.rotate_left(count);
        
        count
    }

    /// Return the current integrated amplitude immediately (ignoring kernel latency)
    pub fn read_instant(&self) -> i16 {
        self.accumulator.clamp(-32768, 32767) as i16
    }
}
