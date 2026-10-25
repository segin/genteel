//! Band-limited synthesis buffer (BlipBuf)
//!
//! Based on the approach of blargg's `blip_buf`: a source (PSG, FM DAC)
//! reports amplitude *changes* at source-clock timestamps, and each change is
//! spread over a few output samples with a windowed-sinc impulse. Integrating
//! the buffer then yields a band-limited version of the stepped waveform.
//!
//! Time mapping is absolute: a source clock is converted to an output sample
//! position with one 64.64 fixed-point multiplication, relative to the clock
//! at which the timing was last configured. Elapsed time is therefore never
//! lost while a source is silent, and no division or transcendental function
//! is evaluated per amplitude change.

use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// Number of output samples each impulse is spread over.
const KERNEL_TAPS: usize = 16;
/// Tap index of the impulse centre; the band-limited step lags by this many samples.
const KERNEL_CENTER: usize = KERNEL_TAPS / 2 - 1;
/// log2 of the number of sub-sample phases in the kernel table.
const PHASE_BITS: u32 = 8;
const PHASES: usize = 1 << PHASE_BITS;
/// Every kernel phase sums to exactly `1 << KERNEL_UNIT_BITS`.
const KERNEL_UNIT_BITS: u32 = 15;
/// Kernel cutoff as a fraction of the output Nyquist frequency. Leaving a
/// transition band below Nyquist keeps the windowed sinc from passing
/// images that alias back into the audible band.
pub const KERNEL_CUTOFF: f64 = 0.875;
/// Number of fractional bits in the fixed-point sample time.
const TIME_FRAC_BITS: u32 = 64;
/// Upper bound on the output sample rate accepted from restored state.
const MAX_SAMPLE_RATE: u32 = 1_000_000;

/// Windowed-sinc impulse for a step at sub-sample offset `frac` in [0, 1).
///
/// Tap `i` sits at output sample `floor(t) + i`; the impulse centre is at
/// `t + KERNEL_CENTER`, so tap `i` samples the kernel at `i - KERNEL_CENTER - frac`.
/// A later step (larger `frac`) therefore lands later in the output.
fn kernel_phase(frac: f64) -> [i32; KERNEL_TAPS] {
    let half = (KERNEL_TAPS / 2) as f64;
    let mut taps = [0f64; KERNEL_TAPS];
    for (i, tap) in taps.iter_mut().enumerate() {
        let x = i as f64 - KERNEL_CENTER as f64 - frac;
        let arg = std::f64::consts::PI * KERNEL_CUTOFF * x;
        let sinc = if arg.abs() < 1e-12 {
            1.0
        } else {
            arg.sin() / arg
        };
        let w = std::f64::consts::PI * x / half;
        let window = (0.42 + 0.5 * w.cos() + 0.08 * (2.0 * w).cos()).max(0.0);
        *tap = sinc * window;
    }
    let sum: f64 = taps.iter().sum();
    let unit = (1i64 << KERNEL_UNIT_BITS) as f64;
    let mut out = [0i32; KERNEL_TAPS];
    let mut total = 0i32;
    for (o, t) in out.iter_mut().zip(taps.iter()) {
        *o = (t / sum * unit).round() as i32;
        total += *o;
    }
    /* Put the rounding residual on the largest tap so every phase has unit
     * gain exactly. */
    let largest = (0..KERNEL_TAPS)
        .max_by_key(|&i| out[i].abs())
        .unwrap_or(KERNEL_CENTER);
    out[largest] += (1 << KERNEL_UNIT_BITS) - total;
    out
}

static KERNEL: LazyLock<Box<[[i32; KERNEL_TAPS]; PHASES]>> = LazyLock::new(|| {
    let mut table = Box::new([[0i32; KERNEL_TAPS]; PHASES]);
    for (p, phase) in table.iter_mut().enumerate() {
        *phase = kernel_phase(p as f64 / PHASES as f64);
    }
    table
});

fn buffer_len_for(sample_rate: u32) -> usize {
    /* ~100 ms of look-ahead plus the kernel tail. */
    (sample_rate as usize / 10) + KERNEL_TAPS + 2
}

/// Samples per source clock in 64.64 fixed point.
fn time_factor(clock_rate: u32, sample_rate: u32) -> u128 {
    ((sample_rate as u128) << TIME_FRAC_BITS) / clock_rate.max(1) as u128
}

/// Serialized form of [`BlipBuf`]; validated on the way back in.
#[derive(Serialize, Deserialize)]
struct BlipBufState {
    buffer: Vec<i32>,
    start: usize,
    sample_rate: u32,
    clock_rate: u32,
    #[serde(default)]
    base_clock: u64,
    #[serde(default)]
    read_pos: u64,
    accumulator: i32,
    integrator: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(try_from = "BlipBufState", into = "BlipBufState")]
pub struct BlipBuf {
    /// Ring of pending per-sample deltas (the derivative of the output).
    buffer: Vec<i32>,
    /// Ring index of the next sample to be read.
    start: usize,
    /// Output sample rate (e.g. 48000)
    sample_rate: u32,
    /// Source clock rate (e.g. the Genesis master clock)
    clock_rate: u32,
    /// Source clock corresponding to output sample 0 of the current timebase.
    base_clock: u64,
    /// Number of samples read since `base_clock`.
    read_pos: u64,
    /// Samples per source clock, 64.64 fixed point (derived from the rates).
    factor: u128,
    /// Sum of every delta: the settled output level.
    accumulator: i32,
    /// Running integrated output state between read calls.
    integrator: i32,
}

impl From<BlipBuf> for BlipBufState {
    fn from(b: BlipBuf) -> Self {
        Self {
            buffer: b.buffer,
            start: b.start,
            sample_rate: b.sample_rate,
            clock_rate: b.clock_rate,
            base_clock: b.base_clock,
            read_pos: b.read_pos,
            accumulator: b.accumulator,
            integrator: b.integrator,
        }
    }
}

impl TryFrom<BlipBufState> for BlipBuf {
    type Error = String;

    fn try_from(s: BlipBufState) -> Result<Self, Self::Error> {
        if s.clock_rate == 0 || s.sample_rate == 0 || s.sample_rate > MAX_SAMPLE_RATE {
            return Err(format!(
                "invalid band-limited buffer timing: clock {} Hz, output {} Hz",
                s.clock_rate, s.sample_rate
            ));
        }
        let len = buffer_len_for(s.sample_rate);
        let mut blip = BlipBuf {
            buffer: s.buffer,
            start: s.start,
            sample_rate: s.sample_rate,
            clock_rate: s.clock_rate,
            base_clock: s.base_clock,
            read_pos: s.read_pos,
            factor: time_factor(s.clock_rate, s.sample_rate),
            accumulator: s.accumulator,
            integrator: s.integrator,
        };
        if blip.buffer.len() != len || blip.start >= len {
            /* In-flight kernel tails cannot be trusted: drop them but keep
             * the settled level so sources' delta baselines stay valid. */
            blip.buffer = vec![0; len];
            blip.start = 0;
            blip.integrator = blip.accumulator;
        }
        Ok(blip)
    }
}

impl BlipBuf {
    pub fn new(clock_rate: u32, sample_rate: u32) -> Self {
        let sample_rate = sample_rate.clamp(1, MAX_SAMPLE_RATE);
        let clock_rate = clock_rate.max(1);
        Self {
            buffer: vec![0; buffer_len_for(sample_rate)],
            start: 0,
            sample_rate,
            clock_rate,
            base_clock: 0,
            read_pos: 0,
            factor: time_factor(clock_rate, sample_rate),
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

    /// Reconfigure both source and output timing, starting a new timebase at
    /// source clock `now`.
    ///
    /// Queued kernel tails are discarded because they were scheduled against
    /// the previous timing domain, but the settled output level is kept so a
    /// source's delta baseline (its last reported amplitude) stays valid and
    /// no DC step is introduced.
    pub fn set_timing(&mut self, clock_rate: u32, sample_rate: u32, now: u64) {
        let sample_rate = sample_rate.clamp(1, MAX_SAMPLE_RATE);
        let clock_rate = clock_rate.max(1);
        if self.clock_rate == clock_rate && self.sample_rate == sample_rate {
            return;
        }
        self.clock_rate = clock_rate;
        self.sample_rate = sample_rate;
        self.factor = time_factor(clock_rate, sample_rate);
        let len = buffer_len_for(sample_rate);
        self.buffer.clear();
        self.buffer.resize(len, 0);
        self.start = 0;
        self.read_pos = 0;
        self.base_clock = now;
        self.integrator = self.accumulator;
    }

    /// Clear the buffer and reset the timebase to clock 0 and level 0.
    pub fn clear(&mut self) {
        self.buffer.fill(0);
        self.start = 0;
        self.base_clock = 0;
        self.read_pos = 0;
        self.accumulator = 0;
        self.integrator = 0;
    }

    /// Output sample position of `clock` in 64.64 fixed point.
    fn sample_time(&self, clock: u64) -> u128 {
        let rel = clock.saturating_sub(self.base_clock) as u128;
        rel.saturating_mul(self.factor)
    }

    /// Add a delta (amplitude change) at a specific source clock time.
    pub fn add_delta(&mut self, clock: u64, delta: i32) {
        if delta == 0 {
            return;
        }
        /* Track the DC level (sum of every delta) so `read_instant` stays
         * exact regardless of where the band-limited step lands. */
        self.accumulator = self.accumulator.saturating_add(delta);

        let time = self.sample_time(clock);
        let whole = u64::try_from(time >> TIME_FRAC_BITS).unwrap_or(u64::MAX);
        let phase = ((time >> (TIME_FRAC_BITS - PHASE_BITS)) as usize) & (PHASES - 1);

        /* Late deltas (already-read position) land on the next unread
         * sample; deltas too far ahead land at the last slot whose kernel
         * tail still fits, so the integral never loses a step. */
        let len = self.buffer.len();
        let max_offset = (len - KERNEL_TAPS) as u64;
        let offset = whole.saturating_sub(self.read_pos).min(max_offset) as usize;

        let mut idx = self.start + offset;
        if idx >= len {
            idx -= len;
        }
        let kernel = &KERNEL[phase];
        let delta64 = delta as i64;
        let mut placed = 0i64;
        let mut center_idx = idx;
        for (i, &k) in kernel.iter().enumerate() {
            let part = (delta64 * k as i64) >> KERNEL_UNIT_BITS;
            placed += part;
            self.buffer[idx] = self.buffer[idx].wrapping_add(part as i32);
            if i == KERNEL_CENTER {
                center_idx = idx;
            }
            idx += 1;
            if idx == len {
                idx = 0;
            }
        }
        /* The per-tap arithmetic shift rounds toward -inf; put the residual
         * on the centre tap so the taps sum to exactly `delta`. */
        let residual = (delta64 - placed) as i32;
        self.buffer[center_idx] = self.buffer[center_idx].wrapping_add(residual);
    }

    /// Read generated samples into a buffer
    pub fn read_samples(&mut self, samples: &mut [i16]) -> usize {
        let len = self.buffer.len();
        let count = samples.len().min(len - KERNEL_TAPS);

        let mut current = self.integrator;
        let mut idx = self.start;
        for sample in samples.iter_mut().take(count) {
            current = current.wrapping_add(self.buffer[idx]);
            *sample = current.clamp(-32768, 32767) as i16;
            self.buffer[idx] = 0;
            idx += 1;
            if idx == len {
                idx = 0;
            }
        }
        self.integrator = current;
        self.start = idx;
        self.read_pos = self.read_pos.saturating_add(count as u64);

        count
    }

    /// Return the current integrated amplitude immediately (ignoring kernel latency)
    pub fn read_instant(&self) -> i16 {
        self.accumulator.clamp(-32768, 32767) as i16
    }

    /// Number of output samples the band-limited step lags its timestamp.
    pub const fn latency_samples() -> usize {
        KERNEL_CENTER
    }

    /// Integer coefficients of the impulse for a given phase (tests/analysis).
    #[cfg(test)]
    pub(crate) fn kernel_for_phase(phase: usize) -> [i32; KERNEL_TAPS] {
        KERNEL[phase & (PHASES - 1)]
    }
}
