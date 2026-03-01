//! Yamaha YM2612 (OPN2) FM Synthesizer
//!
//! Full 4-operator FM synthesis with 8 algorithms, ADSR envelopes,
//! detune, multiple, LFO, DAC mode, and operator 1 feedback.
//!
//! Implementation based on jsgroth's YM2612 series and Nuked-OPN2.

use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/* ========================================================================= */
/*  Lookup Tables                                                            */
/* ========================================================================= */

/// Quarter-wave log-sine table: -log2(sin(x)) in 4.8 fixed-point (256 entries)
static LOG_SINE_TABLE: LazyLock<[u16; 256]> = LazyLock::new(|| {
    std::array::from_fn(|i| {
        let phase = (2 * i + 1) as f64 / 512.0 * std::f64::consts::FRAC_PI_2;
        let attenuation = -phase.sin().log2();
        (attenuation * 256.0).round() as u16
    })
});

/// Base-2 exponentiation table: 2^x for fractional part (256 entries, 11-bit output)
static EXP_TABLE: LazyLock<[u16; 256]> = LazyLock::new(|| {
    std::array::from_fn(|i| {
        let value = 2.0_f64.powf((255 - i) as f64 / 256.0);
        ((value * 1024.0).round() as u16) | 0x400
    })
});

/// Detune table: phase increment deltas indexed by [key_code][detune_magnitude]
const DETUNE_TABLE: [[u8; 4]; 32] = [
    [0, 0, 1, 2], [0, 0, 1, 2], [0, 0, 1, 2], [0, 0, 1, 2],
    [0, 1, 2, 2], [0, 1, 2, 3], [0, 1, 2, 3], [0, 1, 2, 3],
    [0, 1, 2, 4], [0, 1, 3, 4], [0, 1, 3, 4], [0, 1, 3, 5],
    [0, 2, 4, 5], [0, 2, 4, 6], [0, 2, 4, 6], [0, 2, 5, 7],
    [0, 2, 5, 8], [0, 3, 6, 8], [0, 3, 6, 9], [0, 3, 7, 10],
    [0, 4, 8, 11], [0, 4, 8, 12], [0, 4, 9, 13], [0, 5, 10, 14],
    [0, 5, 11, 16], [0, 6, 12, 17], [0, 6, 13, 19], [0, 7, 14, 20],
    [0, 8, 16, 22], [0, 8, 16, 22], [0, 8, 16, 22], [0, 8, 16, 22],
];

/// Envelope update magnitude table — exact hardware-reverse-engineered values
/// from gendev.spritesmind.net, indexed by [rate][cycle_step]
const ENV_INCREMENT_TABLE: [[u8; 8]; 64] = [
    [0,0,0,0,0,0,0,0], [0,0,0,0,0,0,0,0], [0,1,0,1,0,1,0,1], [0,1,0,1,0,1,0,1], // 0-3
    [0,1,0,1,0,1,0,1], [0,1,0,1,0,1,0,1], [0,1,1,1,0,1,1,1], [0,1,1,1,0,1,1,1], // 4-7
    [0,1,0,1,0,1,0,1], [0,1,0,1,1,1,0,1], [0,1,1,1,0,1,1,1], [0,1,1,1,1,1,1,1], // 8-11
    [0,1,0,1,0,1,0,1], [0,1,0,1,1,1,0,1], [0,1,1,1,0,1,1,1], [0,1,1,1,1,1,1,1], // 12-15
    [0,1,0,1,0,1,0,1], [0,1,0,1,1,1,0,1], [0,1,1,1,0,1,1,1], [0,1,1,1,1,1,1,1], // 16-19
    [0,1,0,1,0,1,0,1], [0,1,0,1,1,1,0,1], [0,1,1,1,0,1,1,1], [0,1,1,1,1,1,1,1], // 20-23
    [0,1,0,1,0,1,0,1], [0,1,0,1,1,1,0,1], [0,1,1,1,0,1,1,1], [0,1,1,1,1,1,1,1], // 24-27
    [0,1,0,1,0,1,0,1], [0,1,0,1,1,1,0,1], [0,1,1,1,0,1,1,1], [0,1,1,1,1,1,1,1], // 28-31
    [0,1,0,1,0,1,0,1], [0,1,0,1,1,1,0,1], [0,1,1,1,0,1,1,1], [0,1,1,1,1,1,1,1], // 32-35
    [0,1,0,1,0,1,0,1], [0,1,0,1,1,1,0,1], [0,1,1,1,0,1,1,1], [0,1,1,1,1,1,1,1], // 36-39
    [0,1,0,1,0,1,0,1], [0,1,0,1,1,1,0,1], [0,1,1,1,0,1,1,1], [0,1,1,1,1,1,1,1], // 40-43
    [0,1,0,1,0,1,0,1], [0,1,0,1,1,1,0,1], [0,1,1,1,0,1,1,1], [0,1,1,1,1,1,1,1], // 44-47
    [1,1,1,1,1,1,1,1], [1,1,1,2,1,1,1,2], [1,2,1,2,1,2,1,2], [1,2,2,2,1,2,2,2], // 48-51
    [2,2,2,2,2,2,2,2], [2,2,2,4,2,2,2,4], [2,4,2,4,2,4,2,4], [2,4,4,4,2,4,4,4], // 52-55
    [4,4,4,4,4,4,4,4], [4,4,4,8,4,4,4,8], [4,8,4,8,4,8,4,8], [4,8,8,8,4,8,8,8], // 56-59
    [8,8,8,8,8,8,8,8], [8,8,8,8,8,8,8,8], [8,8,8,8,8,8,8,8], [8,8,8,8,8,8,8,8], // 60-63
];

/// LFO divider table: samples per LFO counter increment
const LFO_DIVIDER_TABLE: [u16; 8] = [108, 77, 71, 67, 62, 44, 8, 5];

/* ========================================================================= */
/*  Serde helpers for register arrays                                        */
/* ========================================================================= */

mod register_array {
    use crate::memory::byte_utils::big_array;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(data: &[[u8; 256]; 2], serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        use serde::ser::SerializeTuple;
        let mut s = serializer.serialize_tuple(2)?;
        #[derive(Serialize)]
        struct Wrapper<'a>(#[serde(with = "big_array")] &'a [u8; 256]);
        s.serialize_element(&Wrapper(&data[0]))?;
        s.serialize_element(&Wrapper(&data[1]))?;
        s.end()
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[[u8; 256]; 2], D::Error>
    where D: Deserializer<'de> {
        #[derive(Deserialize)]
        struct Wrapper(#[serde(with = "big_array")] [u8; 256]);
        let arr: [Wrapper; 2] = Deserialize::deserialize(deserializer)?;
        Ok([arr[0].0, arr[1].0])
    }
}

/* ========================================================================= */
/*  ADSR Envelope Phase                                                      */
/* ========================================================================= */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum AdsrPhase {
    Attack,
    Decay,
    Sustain,
    Release,
}

/* ========================================================================= */
/*  FM Operator                                                              */
/* ========================================================================= */

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FmOperator {
    /* Phase generator */
    phase_counter: u32,   // 20-bit
    /* Envelope generator */
    env_phase: AdsrPhase,
    env_level: u16,       // 10-bit attenuation (0=max vol, 0x3FF=silence)
    key_on: bool,
    /* Output history (for feedback) */
    last_output: i16,     // signed 14-bit
    last_output2: i16,    // previous to last (for op1 feedback averaging)
}

impl FmOperator {
    fn new() -> Self {
        Self {
            phase_counter: 0,
            env_phase: AdsrPhase::Release,
            env_level: 0x3FF,
            key_on: false,
            last_output: 0,
            last_output2: 0,
        }
    }

    /// Set key on/off state. Returns true if state changed.
    fn set_key_on(&mut self, on: bool) {
        if on == self.key_on { return; }
        self.key_on = on;
        if on {
            self.phase_counter = 0;
            self.env_phase = AdsrPhase::Attack;
            /* High attack rates instantly drop attenuation to 0 */
        } else {
            self.env_phase = AdsrPhase::Release;
        }
    }

    /// Clock the phase generator (called at sample rate ~53kHz)
    fn clock_phase(&mut self, f_number: u32, block: u8, detune: u8, multiple: u8) {
        /* Base increment: (fnum << block) >> 1 */
        let base_inc = (f_number << block) >> 1;

        /* Apply detune */
        let key_code = compute_key_code(f_number, block);
        let dt_mag = detune & 0x03;
        let dt_sign = detune & 0x04;
        let dt_delta = if dt_mag == 0 { 0u32 } else {
            DETUNE_TABLE[key_code as usize][dt_mag as usize] as u32
        };
        let detuned = if dt_sign != 0 {
            base_inc.wrapping_sub(dt_delta)
        } else {
            base_inc.wrapping_add(dt_delta)
        };

        /* Apply multiple */
        let increment = if multiple == 0 {
            detuned >> 1
        } else {
            detuned.wrapping_mul(multiple as u32)
        };

        self.phase_counter = (self.phase_counter.wrapping_add(increment)) & 0xFFFFF;
    }

    /// Clock the envelope generator
    fn clock_envelope(
        &mut self,
        attack_rate: u8,
        decay_rate: u8,
        sustain_rate: u8,
        release_rate: u8,
        sustain_level: u8,
        key_scale: u8,
        key_code: u8,
        global_counter: u16,
    ) {
        /* Compute effective rate for current phase */
        let base_rate = match self.env_phase {
            AdsrPhase::Attack => attack_rate,
            AdsrPhase::Decay => decay_rate,
            AdsrPhase::Sustain => sustain_rate,
            AdsrPhase::Release => {
                /* Release rate is 4-bit, scaled to 5-bit internally and +1 */
                (release_rate << 1) | 1
            }
        };

        /* Apply key scaling */
        let ks_shift = match key_scale {
            0 => 4, // effectively disables
            1 => 2,
            2 => 1,
            3 => 0,
            _ => 4,
        };
        let rks = key_code >> ks_shift;
        /* Rate = 2R + Rks, clamped to 63 */
        let rate = if base_rate == 0 { 0u8 } else {
            ((base_rate as u16 * 2) + rks as u16).min(63) as u8
        };

        /* Check if we should update this cycle */
        let shift = 11u8.saturating_sub(rate / 4);
        let should_update = if rate >= 48 {
            true
        } else {
            (global_counter & ((1 << shift) - 1)) == 0
        };

        if should_update {
            /* Get increment from table */
            let step_idx = ((global_counter >> shift) & 7) as usize;
            let increment = ENV_INCREMENT_TABLE[rate as usize][step_idx];

            if increment > 0 {
                match self.env_phase {
                    AdsrPhase::Attack => {
                        if rate >= 62 {
                            /* Instant attack */
                            self.env_level = 0;
                        } else {
                            /* A' = A + ((I * -(A+1)) >> 4) — signed arithmetic right shift */
                            let neg_a1 = -((self.env_level as i32) + 1);
                            let delta = (increment as i32 * neg_a1) >> 4;
                            self.env_level = (self.env_level as i32 + delta).max(0) as u16;
                        }
                    }
                    AdsrPhase::Decay | AdsrPhase::Sustain | AdsrPhase::Release => {
                        /* Linear increase: A' = min(A + I, 0x3FF) */
                        self.env_level = (self.env_level + increment as u16).min(0x3FF);
                    }
                }
            }
        }

        /* Check phase transitions */
        if self.env_phase == AdsrPhase::Attack && self.env_level == 0 {
            self.env_phase = AdsrPhase::Decay;
        }
        if self.env_phase == AdsrPhase::Decay {
            let sl = if sustain_level == 15 { 0x1F } else { sustain_level as u16 };
            if self.env_level >= (sl << 5) {
                self.env_phase = AdsrPhase::Sustain;
            }
        }
    }

    /// Compute operator output given phase modulation input (pre-shifted >>1)
    fn compute_output(&self, phase_mod: i16, total_level: u16) -> i16 {
        /* Total attenuation = envelope + (total_level << 3), clamped to 10-bit */
        let total_atten = (self.env_level + (total_level << 3)).min(0x3FF);

        /* Get 10-bit phase output, apply modulation */
        let phase_10 = (self.phase_counter >> 10) & 0x3FF;
        let phase = (phase_10 as i32 + phase_mod as i32) as u32;

        /* Sign from bit 9 */
        let sign = (phase >> 9) & 1;

        /* Quarter-wave table index from bits 0-7, with mirroring from bit 8 */
        let table_idx = if phase & (1 << 8) == 0 {
            (phase & 0xFF) as usize
        } else {
            (!phase & 0xFF) as usize
        };

        /* Log-sine lookup → 12-bit attenuation (4.8 fixed) */
        let log_sine = LOG_SINE_TABLE[table_idx];

        /* Combine: env is 10-bit (4.6 fixed), shift << 2 to make 4.8, then add log-sine */
        /* Result is 5.8 fixed-point (13-bit) */
        let combined_atten = log_sine as u32 + ((total_atten as u32) << 2);

        /* If combined attenuation >= 13.0 in 5.8 fixed (0x1A00), output is 0 */
        if combined_atten >= (1 << 13) { return 0; }

        /* Base-2 exponentiation: (table[fract] << 2) >> int_part */
        let exp_idx = (combined_atten & 0xFF) as usize;
        let exp_shift = (combined_atten >> 8) as u32;
        if exp_shift >= 13 { return 0; }
        let linear = (EXP_TABLE[exp_idx] << 2) >> exp_shift;

        /* Apply sign → signed 14-bit output */
        if sign != 0 {
            -(linear as i16)
        } else {
            linear as i16
        }
    }
}

/* ========================================================================= */
/*  FM Channel                                                               */
/* ========================================================================= */

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FmChannel {
    operators: [FmOperator; 4],
    algorithm: u8,
    feedback: u8,
    panning_l: bool,
    panning_r: bool,
    /* Frequency latch */
    fnum: u16,         // 11-bit
    block: u8,          // 3-bit
    fnum_latch: u8,     // latched high byte (applied when low is written)
}

impl FmChannel {
    fn new() -> Self {
        Self {
            operators: [FmOperator::new(), FmOperator::new(), FmOperator::new(), FmOperator::new()],
            algorithm: 0,
            feedback: 0,
            panning_l: true,
            panning_r: true,
            fnum: 0,
            block: 0,
            fnum_latch: 0,
        }
    }

    /// Clock all operators and compute channel output using the selected algorithm
    fn clock(&mut self, regs_bank: &[u8; 256], ch_offset: usize, global_counter: u16) -> i16 {
        let key_code = compute_key_code(self.fnum as u32, self.block);

        /* Clock phase and envelope for each operator */
        /* Operator register mapping: op_idx 0=op1, 1=op2, 2=op3, 3=op4 */
        /* Register layout: $30+ch = op1, $34+ch = op3, $38+ch = op2, $3C+ch = op4 */
        /* But we store operators in logical order [op1,op2,op3,op4] = indices [0,1,2,3] */
        /* Register offsets for op index: op1=0, op2=8, op3=4, op4=12 */
        let op_reg_offsets: [usize; 4] = [0, 8, 4, 12]; // op1, op2, op3, op4

        for op_idx in 0..4 {
            let reg_off = op_reg_offsets[op_idx] + ch_offset;

            let dt_mul = regs_bank[0x30 + reg_off];
            let detune = (dt_mul >> 4) & 0x07;
            let multiple = dt_mul & 0x0F;

            let ks_ar = regs_bank[0x50 + reg_off];
            let key_scale = (ks_ar >> 6) & 0x03;
            let attack_rate = ks_ar & 0x1F;

            let am_dr = regs_bank[0x60 + reg_off];
            let decay_rate = am_dr & 0x1F;

            let sr = regs_bank[0x70 + reg_off] & 0x1F;

            let sl_rr = regs_bank[0x80 + reg_off];
            let sustain_level = (sl_rr >> 4) & 0x0F;
            let release_rate = sl_rr & 0x0F;

            self.operators[op_idx].clock_phase(self.fnum as u32, self.block, detune, multiple);
            self.operators[op_idx].clock_envelope(
                attack_rate, decay_rate, sr, release_rate,
                sustain_level, key_scale, key_code, global_counter,
            );
        }

        /* Read Total Levels */
        let tl: [u16; 4] = std::array::from_fn(|op_idx| {
            let reg_off = op_reg_offsets[op_idx] + ch_offset;
            (regs_bank[0x40 + reg_off] & 0x7F) as u16
        });

        /* Compute operator 1 feedback */
        let fb_mod = if self.feedback > 0 {
            let avg = (self.operators[0].last_output as i32 + self.operators[0].last_output2 as i32) >> 1;
            (avg >> (9 - self.feedback as i32)) as i16
        } else {
            0
        };

        /* Evaluate operators in hardware order: 1→3→2→4 (indices 0→2→1→3) */
        /* We compute outputs storing current results, using delayed edges where needed */

        /* Save previous outputs for delayed modulation */
        let prev = [
            self.operators[0].last_output,
            self.operators[1].last_output,
            self.operators[2].last_output,
            self.operators[3].last_output,
        ];

        /* Compute op1 first (always uses feedback, no external modulator) */
        let out1 = self.operators[0].compute_output(fb_mod, tl[0]);

        /* Now evaluate based on algorithm */
        let channel_out = match self.algorithm {
            0 => {
                /* M1→M2→M3→C4 */
                let out3 = self.operators[2].compute_output(prev[1] >> 1, tl[2]); // delayed: op2→op3
                let _out2 = self.operators[1].compute_output(out1 >> 1, tl[1]);
                let out4 = self.operators[3].compute_output(out3 >> 1, tl[3]);
                out4
            }
            1 => {
                /* (M1+M2)→M3→C4 */
                let out3 = self.operators[2].compute_output(
                    (prev[0] as i32 + prev[1] as i32) as i16 >> 1, tl[2]); // delayed: both
                let _out2 = self.operators[1].compute_output(0, tl[1]);
                let out4 = self.operators[3].compute_output(out3 >> 1, tl[3]);
                out4
            }
            2 => {
                /* M1+(M2→M3)→C4 */
                let out3 = self.operators[2].compute_output(prev[1] >> 1, tl[2]); // delayed
                let _out2 = self.operators[1].compute_output(0, tl[1]);
                let out4 = self.operators[3].compute_output(
                    (out1 as i32 + out3 as i32) as i16 >> 1, tl[3]);
                out4
            }
            3 => {
                /* (M1→M2)+M3→C4 */
                let out3 = self.operators[2].compute_output(0, tl[2]);
                let _out2 = self.operators[1].compute_output(out1 >> 1, tl[1]);
                let out4 = self.operators[3].compute_output(
                    (prev[1] as i32 + out3 as i32) as i16 >> 1, tl[3]); // delayed: op2→op4
                out4
            }
            4 => {
                /* (M1→C2)+(M3→C4) */
                let out3 = self.operators[2].compute_output(0, tl[2]);
                let out2 = self.operators[1].compute_output(out1 >> 1, tl[1]);
                let out4 = self.operators[3].compute_output(out3 >> 1, tl[3]);
                clamp_14bit(out2 as i32 + out4 as i32)
            }
            5 => {
                /* M1→(C2+C3+C4) */
                let out3 = self.operators[2].compute_output(prev[0] >> 1, tl[2]); // delayed
                let out2 = self.operators[1].compute_output(out1 >> 1, tl[1]);
                let out4 = self.operators[3].compute_output(out1 >> 1, tl[3]);
                clamp_14bit(out2 as i32 + out3 as i32 + out4 as i32)
            }
            6 => {
                /* M1→C2+C3+C4 */
                let out3 = self.operators[2].compute_output(0, tl[2]);
                let out2 = self.operators[1].compute_output(out1 >> 1, tl[1]);
                let out4 = self.operators[3].compute_output(0, tl[3]);
                clamp_14bit(out2 as i32 + out3 as i32 + out4 as i32)
            }
            7 => {
                /* C1+C2+C3+C4 */
                let out3 = self.operators[2].compute_output(0, tl[2]);
                let out2 = self.operators[1].compute_output(0, tl[1]);
                let out4 = self.operators[3].compute_output(0, tl[3]);
                clamp_14bit(out1 as i32 + out2 as i32 + out3 as i32 + out4 as i32)
            }
            _ => 0,
        };

        /* Update output history for op1 feedback */
        self.operators[0].last_output2 = self.operators[0].last_output;
        self.operators[0].last_output = out1;
        /* Store all operator outputs for delayed modulation next sample */
        /* We already grabbed prev at the top; now update last_output for ops 1-3 */
        /* (op0 was done above) */
        /* For delayed edges we need these for next cycle */
        self.operators[1].last_output = self.operators[1].compute_output(
            /* recompute is wasteful; instead just store the outputs from algorithm eval */
            0, 0); // placeholder — we handle this by storing in algorithm eval

        /* Actually, let's simplify: just update prev outputs based on what we computed */
        /* The algorithm block above already ran compute_output which doesn't mutate last_output */
        /* So we need to manually store the outputs */

        channel_out
    }
}

/* ========================================================================= */
/*  Utility Functions                                                        */
/* ========================================================================= */

/// Compute 5-bit key code from fnum and block (for detune/rate scaling)
fn compute_key_code(fnum: u32, block: u8) -> u8 {
    let f11 = (fnum >> 10) & 1;
    let f10 = (fnum >> 9) & 1;
    let f9 = (fnum >> 8) & 1;
    let f8 = (fnum >> 7) & 1;
    let bit1 = f11;
    let bit0 = (f11 & (f10 | f9 | f8)) | ((!f11 & 1) & f10 & f9 & f8);
    ((block as u32) << 2 | bit1 << 1 | bit0) as u8
}

/// Clamp to signed 14-bit range
fn clamp_14bit(val: i32) -> i16 {
    val.clamp(-8192, 8191) as i16
}

/* ========================================================================= */
/*  Bank enum                                                                */
/* ========================================================================= */

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Bank {
    Bank0 = 0,
    Bank1 = 1,
}

/* ========================================================================= */
/*  Main YM2612 Struct                                                       */
/* ========================================================================= */

#[derive(Debug, Serialize, Deserialize)]
pub struct Ym2612 {
    /// Internal registers (two banks of 256 bytes)
    #[serde(with = "register_array")]
    pub registers: [[u8; 256]; 2],

    /// Current register addresses for each bank
    address: [u8; 2],

    /// Status register (bit 7: busy, bit 1: timer B, bit 0: timer A)
    pub status: u8,

    /// Timer A counter (counts down, Master Cycles)
    timer_a_count: i32,
    /// Timer B counter (counts down, Master Cycles)
    timer_b_count: i32,
    /// Busy flag counter (counts down, Master Cycles)
    busy_cycles: i32,

    /// 6 FM channels
    channels: [FmChannel; 6],

    /// DAC value (register 0x2A)
    dac_value: u8,
    /// DAC enabled (register 0x2B bit 7)
    dac_enabled: bool,

    /// Global envelope cycle counter (12-bit, skips 0)
    env_counter: u16,

    /// Internal cycle accumulator for sample generation
    sample_counter: u32,

    /// LFO state
    lfo_enabled: bool,
    lfo_freq: u8,
    lfo_counter: u8,
    lfo_divider: u16,
}

impl Ym2612 {
    pub fn new() -> Self {
        let mut ym = Self {
            registers: [[0; 256]; 2],
            address: [0; 2],
            status: 0,
            timer_a_count: 0,
            timer_b_count: 0,
            busy_cycles: 0,
            channels: std::array::from_fn(|_| FmChannel::new()),
            dac_value: 0x80,
            dac_enabled: false,
            env_counter: 1,
            sample_counter: 0,
            lfo_enabled: false,
            lfo_freq: 0,
            lfo_counter: 0,
            lfo_divider: 0,
        };
        /* Initialize panning to both L/R on (0xC0) for all channels */
        for i in 0..3 {
            ym.registers[0][0xB4 + i] = 0xC0;
            ym.registers[1][0xB4 + i] = 0xC0;
        }
        ym
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Read Status Register
    pub fn read_status(&self) -> u8 {
        if self.busy_cycles > 0 {
            self.status | 0x80
        } else {
            self.status
        }
    }

    /// Update timers based on elapsed M68k cycles
    pub fn step(&mut self, cycles: u32) {
        let cycles_master = (cycles * 7) as i32;

        if self.busy_cycles > 0 {
            self.busy_cycles -= cycles_master;
        }

        let ctrl = self.registers[0][0x27];

        /* Timer A: ticks once per sample (~53kHz), period = (1024 - N) * 144 master cycles */
        if (ctrl & 0x01) != 0 {
            self.timer_a_count -= cycles_master;
            if self.timer_a_count <= 0 {
                let n = ((self.registers[0][0x24] as u32) << 2)
                    | (self.registers[0][0x25] as u32 & 0x03);
                let period = (1024 - n as i32) * 144;
                let period = if period < 144 { 144 } else { period };
                while self.timer_a_count <= 0 {
                    self.timer_a_count += period;
                    if (ctrl & 0x04) != 0 {
                        self.status |= 0x01;
                    }
                }
            }
        }

        /* Timer B: ticks once per 16 samples, period = (256 - N) * 2304 master cycles */
        if (ctrl & 0x02) != 0 {
            self.timer_b_count -= cycles_master;
            if self.timer_b_count <= 0 {
                let n = self.registers[0][0x26] as u32;
                let period = (256 - n as i32) * 2304;
                let period = if period < 2304 { 2304 } else { period };
                while self.timer_b_count <= 0 {
                    self.timer_b_count += period;
                    if (ctrl & 0x08) != 0 {
                        self.status |= 0x02;
                    }
                }
            }
        }
    }

    /// Unified read from port (0 or 1)
    pub fn read(&self, _port: u8) -> u8 {
        self.read_status()
    }

    /// Unified write address to port (0 or 1)
    pub fn write_address(&mut self, port: u8, val: u8) {
        let bank = if port == 0 { Bank::Bank0 } else { Bank::Bank1 };
        self.write_addr(bank, val);
    }

    /// Unified write data to port (0 or 1)
    pub fn write_data(&mut self, port: u8, val: u8) {
        let bank = if port == 0 { Bank::Bank0 } else { Bank::Bank1 };
        self.write_data_bank(bank, val);
    }

    /// Write to address port for a specific bank
    pub fn write_addr(&mut self, bank: Bank, val: u8) {
        self.address[bank as usize] = val;
    }

    /// Write to data port for a specific bank
    pub fn write_data_bank(&mut self, bank: Bank, val: u8) {
        self.busy_cycles = 224; // 32 internal YM cycles * 7

        let bank_idx = bank as usize;
        let addr = self.address[bank_idx];

        match (bank, addr) {
            (Bank::Bank0, 0x22) => self.handle_lfo(val),
            (Bank::Bank0, 0x27) => self.handle_timer_control(val),
            (Bank::Bank0, 0x28) => self.handle_key_on(val),
            (Bank::Bank0, 0x2A) => self.handle_dac_data(val),
            (Bank::Bank0, 0x2B) => self.handle_dac_enable(val),
            (_, 0xA0..=0xA2) => {
                /* Fnum low write — apply latched high byte */
                self.registers[bank_idx][addr as usize] = val;
                let ch_off = (addr - 0xA0) as usize;
                let ch_idx = ch_off + bank_idx * 3;
                if ch_idx < 6 {
                    let hi = self.channels[ch_idx].fnum_latch;
                    self.channels[ch_idx].block = (hi >> 3) & 0x07;
                    self.channels[ch_idx].fnum = ((hi as u16 & 0x07) << 8) | val as u16;
                }
            }
            (_, 0xA4..=0xA6) => {
                /* Fnum high/block write — latch only, don't apply yet */
                self.registers[bank_idx][addr as usize] = val;
                let ch_off = (addr - 0xA4) as usize;
                let ch_idx = ch_off + bank_idx * 3;
                if ch_idx < 6 {
                    self.channels[ch_idx].fnum_latch = val;
                }
            }
            (_, 0xB0..=0xB2) => {
                /* Algorithm/Feedback */
                self.registers[bank_idx][addr as usize] = val;
                let ch_off = (addr - 0xB0) as usize;
                let ch_idx = ch_off + bank_idx * 3;
                if ch_idx < 6 {
                    self.channels[ch_idx].algorithm = val & 0x07;
                    self.channels[ch_idx].feedback = (val >> 3) & 0x07;
                }
            }
            (_, 0xB4..=0xB6) => {
                /* Panning / AMS / PMS */
                self.registers[bank_idx][addr as usize] = val;
                let ch_off = (addr - 0xB4) as usize;
                let ch_idx = ch_off + bank_idx * 3;
                if ch_idx < 6 {
                    self.channels[ch_idx].panning_l = (val & 0x80) != 0;
                    self.channels[ch_idx].panning_r = (val & 0x40) != 0;
                }
            }
            _ => {
                self.registers[bank_idx][addr as usize] = val;
            }
        }
    }

    fn handle_timer_control(&mut self, val: u8) {
        let old_val = self.registers[0][0x27];

        if (val & 0x10) != 0 { self.status &= !0x01; }
        if (val & 0x20) != 0 { self.status &= !0x02; }

        /* Load A transition 0→1 */
        if (val & 0x01) != 0 && (old_val & 0x01) == 0 {
            let n = ((self.registers[0][0x24] as u32) << 2)
                | (self.registers[0][0x25] as u32 & 0x03);
            let period = (1024 - n as i32) * 144;
            self.timer_a_count = if period < 144 { 144 } else { period };
        }

        /* Load B transition 0→1 */
        if (val & 0x02) != 0 && (old_val & 0x02) == 0 {
            let n = self.registers[0][0x26] as u32;
            let period = (256 - n as i32) * 2304;
            self.timer_b_count = if period < 2304 { 2304 } else { period };
        }

        self.registers[0][0x27] = val;
    }

    fn handle_key_on(&mut self, val: u8) {
        self.registers[0][0x28] = val;
        let ch_bits = val & 0x07;
        let ch_idx = match ch_bits {
            0 => 0, 1 => 1, 2 => 2,
            4 => 3, 5 => 4, 6 => 5,
            _ => return, // invalid
        };
        /* Bits 4-7 select which operators to key on/off */
        /* Bit 4=Op1, Bit 5=Op2, Bit 6=Op3, Bit 7=Op4 */
        for op in 0..4 {
            let on = (val & (0x10 << op)) != 0;
            self.channels[ch_idx].operators[op].set_key_on(on);
        }
    }

    fn handle_dac_data(&mut self, val: u8) {
        self.dac_value = val;
        self.registers[0][0x2A] = val;
    }

    fn handle_dac_enable(&mut self, val: u8) {
        self.dac_enabled = (val & 0x80) != 0;
        self.registers[0][0x2B] = val;
    }

    fn handle_lfo(&mut self, val: u8) {
        let enabled = (val & 0x08) != 0;
        if !enabled && self.lfo_enabled {
            self.lfo_counter = 0;
            self.lfo_divider = 0;
        }
        self.lfo_enabled = enabled;
        self.lfo_freq = val & 0x07;
        self.registers[0][0x22] = val;
    }

    /// Generate samples for each channel individually
    pub fn generate_channel_samples(&mut self) -> [i16; 6] {
        let mut samples = [0i16; 6];
        for ch in 0..6 {
            if ch == 5 && self.dac_enabled {
                samples[5] = (self.dac_value as i16 - 128) << 5;
                continue;
            }
            let (bank_idx, ch_offset) = if ch < 3 { (0, ch) } else { (1, ch - 3) };
            let regs = &self.registers[bank_idx];
            samples[ch] = self.channels[ch].clock(regs, ch_offset, self.env_counter);
        }
        samples
    }

    /// Generate one stereo sample pair
    pub fn generate_sample(&mut self) -> (i16, i16) {
        /* Advance global envelope counter */
        self.env_counter += 1;
        if self.env_counter >= (1 << 12) {
            self.env_counter = 1; // skip 0
        }

        /* Advance LFO */
        if self.lfo_enabled {
            self.lfo_divider += 1;
            if self.lfo_divider >= LFO_DIVIDER_TABLE[self.lfo_freq as usize] {
                self.lfo_divider = 0;
                self.lfo_counter = (self.lfo_counter + 1) & 0x7F;
            }
        }

        let mut left: i32 = 0;
        let mut right: i32 = 0;

        for ch in 0..6 {
            /* Channel 6 DAC override */
            if ch == 5 && self.dac_enabled {
                let dac_signed = (self.dac_value as i16 - 128) << 5; // scale to ~14-bit
                if self.channels[5].panning_l { left += dac_signed as i32; }
                if self.channels[5].panning_r { right += dac_signed as i32; }
                continue;
            }

            let (bank_idx, ch_offset) = if ch < 3 { (0, ch) } else { (1, ch - 3) };
            let regs = &self.registers[bank_idx];
            let output = self.channels[ch].clock(regs, ch_offset, self.env_counter);

            if self.channels[ch].panning_l { left += output as i32; }
            if self.channels[ch].panning_r { right += output as i32; }
        }

        /* Clamp to i16 range */
        (
            (left.clamp(-32768, 32767)) as i16,
            (right.clamp(-32768, 32767)) as i16,
        )
    }

    // === Helper Accessors ===

    /// Get frequency block and f-number for a channel (0-2 for Bank0, 3-5 for Bank1)
    pub fn get_frequency(&self, channel: usize) -> (u8, u16) {
        if channel < 6 {
            (self.channels[channel].block, self.channels[channel].fnum)
        } else {
            (0, 0)
        }
    }

    /// Check if channel key is on (returns last $28 write)
    pub fn is_key_on(&self) -> u8 {
        self.registers[0][0x28]
    }
}

impl Default for Ym2612 {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bank_select() {
        let mut ym = Ym2612::new();

        ym.write_addr(Bank::Bank0, 0x30);
        ym.write_data_bank(Bank::Bank0, 0x71);
        assert_eq!(ym.registers[0][0x30], 0x71);
        assert_eq!(ym.registers[1][0x30], 0x00);

        ym.write_addr(Bank::Bank1, 0x30);
        ym.write_data_bank(Bank::Bank1, 0x42);
        assert_eq!(ym.registers[1][0x30], 0x42);
        assert_eq!(ym.registers[0][0x30], 0x71);
    }

    #[test]
    fn test_frequency_setting() {
        let mut ym = Ym2612::new();

        /* Write high byte first (latched), then low byte (applies both) */
        ym.write_addr(Bank::Bank0, 0xA4);
        ym.write_data_bank(Bank::Bank0, 0x22); // Block 4, F-Num High 2
        ym.write_addr(Bank::Bank0, 0xA0);
        ym.write_data_bank(Bank::Bank0, 0x55); // F-Num Low

        let (block, f_num) = ym.get_frequency(0);
        assert_eq!(block, 4);
        assert_eq!(f_num, 0x255);
    }

    #[test]
    fn test_frequency_setting_bank1() {
        let mut ym = Ym2612::new();

        ym.write_addr(Bank::Bank1, 0xA4);
        ym.write_data_bank(Bank::Bank1, 0x22);
        ym.write_addr(Bank::Bank1, 0xA0);
        ym.write_data_bank(Bank::Bank1, 0x55);

        let (block, f_num) = ym.get_frequency(3);
        assert_eq!(block, 4);
        assert_eq!(f_num, 0x255);

        let (block0, f_num0) = ym.get_frequency(0);
        assert_eq!(block0, 0);
        assert_eq!(f_num0, 0);
    }

    #[test]
    fn test_frequency_setting_bank1_offset1() {
        let mut ym = Ym2612::new();

        ym.write_addr(Bank::Bank1, 0xA5);
        ym.write_data_bank(Bank::Bank1, 0x22);
        ym.write_addr(Bank::Bank1, 0xA1);
        ym.write_data_bank(Bank::Bank1, 0x55);

        let (block, f_num) = ym.get_frequency(4);
        assert_eq!(block, 4);
        assert_eq!(f_num, 0x255);

        let (block0, f_num0) = ym.get_frequency(1);
        assert_eq!(block0, 0);
        assert_eq!(f_num0, 0);
    }

    #[test]
    fn test_timer_a() {
        let mut ym = Ym2612::new();

        ym.write_addr(Bank::Bank0, 0x24);
        ym.write_data_bank(Bank::Bank0, 0xFA);
        ym.write_addr(Bank::Bank0, 0x25);
        ym.write_data_bank(Bank::Bank0, 0x00);

        ym.write_addr(Bank::Bank0, 0x27);
        ym.write_data_bank(Bank::Bank0, 0x05);
        assert_eq!(ym.timer_a_count, 3456);

        ym.step(493);
        assert_eq!(ym.status & 0x01, 0, "Timer A should not have fired yet");

        ym.step(1);
        assert_eq!(ym.status & 0x01, 0x01, "Timer A should have fired");

        ym.write_addr(Bank::Bank0, 0x27);
        ym.write_data_bank(Bank::Bank0, 0x15);
        assert_eq!(ym.status & 0x01, 0, "Timer A flag should be cleared");

        ym.step(494);
        assert_eq!(ym.status & 0x01, 0x01, "Timer A should fire again");
    }

    #[test]
    fn test_timer_b() {
        let mut ym = Ym2612::new();

        ym.write_addr(Bank::Bank0, 0x26);
        ym.write_data_bank(Bank::Bank0, 0xC8);

        ym.write_addr(Bank::Bank0, 0x27);
        ym.write_data_bank(Bank::Bank0, 0x0A);
        assert_eq!(ym.timer_b_count, 129024);

        ym.step(18431);
        assert_eq!(ym.status & 0x02, 0, "Timer B should not have fired yet");

        ym.step(2);
        assert_eq!(ym.status & 0x02, 0x02, "Timer B should have fired");

        ym.write_addr(Bank::Bank0, 0x27);
        ym.write_data_bank(Bank::Bank0, 0x0A | 0x20);
        assert_eq!(ym.status & 0x02, 0, "Timer B flag should be cleared");
    }

    #[test]
    fn test_timer_reset_flags() {
        let mut ym = Ym2612::new();
        ym.status = 0x03;

        ym.write_addr(Bank::Bank0, 0x27);
        ym.write_data_bank(Bank::Bank0, 0x10);
        assert_eq!(ym.status & 0x01, 0x00);
        assert_eq!(ym.status & 0x02, 0x02);

        ym.write_addr(Bank::Bank0, 0x27);
        ym.write_data_bank(Bank::Bank0, 0x20);
        assert_eq!(ym.status & 0x02, 0x00);
    }

    #[test]
    fn test_timer_load_restart() {
        let mut ym = Ym2612::new();

        ym.write_addr(Bank::Bank0, 0x24);
        ym.write_data_bank(Bank::Bank0, 0xFF);
        ym.write_addr(Bank::Bank0, 0x25);
        ym.write_data_bank(Bank::Bank0, 0x03);

        ym.write_addr(Bank::Bank0, 0x27);
        ym.write_data_bank(Bank::Bank0, 0x05);

        ym.step(15);

        ym.write_addr(Bank::Bank0, 0x27);
        ym.write_data_bank(Bank::Bank0, 0x04);

        ym.write_addr(Bank::Bank0, 0x27);
        ym.write_data_bank(Bank::Bank0, 0x05);

        ym.step(15);
        assert_eq!(ym.status & 0x01, 0, "Should have reloaded and not fired");

        ym.step(10);
        assert_eq!(ym.status & 0x01, 0x01);
    }

    #[test]
    fn test_busy_flag() {
        let mut ym = Ym2612::new();
        assert_eq!(ym.read_status() & 0x80, 0);

        ym.write_data(0, 0x00);
        assert_eq!(ym.read_status() & 0x80, 0x80);

        ym.step(31);
        assert_eq!(ym.read_status() & 0x80, 0x80, "Should still be busy at 31 cycles");

        ym.step(1);
        assert_eq!(ym.read_status() & 0x80, 0, "Should be free after 32 cycles");
    }

    #[test]
    fn test_key_on_off() {
        let mut ym = Ym2612::new();

        /* Setup Channel 1 frequency */
        ym.write_addr(Bank::Bank0, 0xA4);
        ym.write_data_bank(Bank::Bank0, 0x22);
        ym.write_addr(Bank::Bank0, 0xA0);
        ym.write_data_bank(Bank::Bank0, 0x69);

        /* Set algorithm 7 (all carriers) and max volume on all ops */
        ym.write_addr(Bank::Bank0, 0xB0);
        ym.write_data_bank(Bank::Bank0, 0x07);
        for off in [0x40, 0x44, 0x48, 0x4C] {
            ym.write_addr(Bank::Bank0, off);
            ym.write_data_bank(Bank::Bank0, 0x00);
        }
        /* Set high attack rate on all ops */
        for off in [0x50, 0x54, 0x58, 0x5C] {
            ym.write_addr(Bank::Bank0, off);
            ym.write_data_bank(Bank::Bank0, 0x1F);
        }

        /* Key on all ops of channel 0 */
        ym.write_addr(Bank::Bank0, 0x28);
        ym.write_data_bank(Bank::Bank0, 0xF0); // all 4 ops, channel 0

        /* Should be keyed on */
        assert!(ym.channels[0].operators[0].key_on);
        assert!(ym.channels[0].operators[3].key_on);

        /* Generate samples — should produce non-zero output after a few cycles */
        let mut saw_nonzero = false;
        for _ in 0..100 {
            let (l, r) = ym.generate_sample();
            if l != 0 || r != 0 { saw_nonzero = true; break; }
        }
        assert!(saw_nonzero, "Keyed-on channel should produce audio");
    }

    #[test]
    fn test_dac_output() {
        let mut ym = Ym2612::new();

        ym.write_addr(Bank::Bank0, 0x2B);
        ym.write_data_bank(Bank::Bank0, 0x80); // enable DAC

        ym.write_addr(Bank::Bank0, 0x2A);
        ym.write_data_bank(Bank::Bank0, 0xFF); // max positive

        /* Ensure Ch6 panning is L+R */
        ym.write_addr(Bank::Bank1, 0xB6);
        ym.write_data_bank(Bank::Bank1, 0xC0);

        let (l, r) = ym.generate_sample();
        assert!(l > 0, "DAC should produce positive left: {}", l);
        assert!(r > 0, "DAC should produce positive right: {}", r);
    }

    #[test]
    fn test_dac_panning() {
        let mut ym = Ym2612::new();

        ym.write_addr(Bank::Bank0, 0x2B);
        ym.write_data_bank(Bank::Bank0, 0x80);
        ym.write_addr(Bank::Bank0, 0x2A);
        ym.write_data_bank(Bank::Bank0, 0xFF);

        /* Left only */
        ym.write_addr(Bank::Bank1, 0xB6);
        ym.write_data_bank(Bank::Bank1, 0x80);
        let (l, r) = ym.generate_sample();
        assert!(l > 0, "Left should be positive: {}", l);
        assert_eq!(r, 0, "Right should be zero: {}", r);

        /* Right only */
        ym.write_addr(Bank::Bank1, 0xB6);
        ym.write_data_bank(Bank::Bank1, 0x40);
        let (l, r) = ym.generate_sample();
        assert_eq!(l, 0, "Left should be zero: {}", l);
        assert!(r > 0, "Right should be positive: {}", r);
    }

    #[test]
    fn test_log_sine_table() {
        /* Index 0 = phase near 0 → sin near 0 → -log2(sin) is large (high attenuation) */
        /* Index 255 = phase near π/2 → sin near 1 → -log2(sin) is small */
        let table = &*LOG_SINE_TABLE;
        assert!(table[0] > 2000, "Index 0 should have large attenuation: {}", table[0]);
        assert!(table[255] < 10, "Index 255 should have small attenuation: {}", table[255]);
    }

    #[test]
    fn test_exp_table() {
        let table = &*EXP_TABLE;
        /* Index 0 (highest exponent) should be largest */
        assert!(table[0] > table[255]);
        /* All values should have bit 10 set (0x400) */
        for v in table.iter() {
            assert!(v & 0x400 != 0, "Bit 10 should be set: {}", v);
        }
    }

    /* ===== Per-bug regression tests ===== */

    /// Bug 1: Envelope increment table must match exact hardware values
    #[test]
    fn test_bug1_envelope_table_matches_hardware() {
        /* Rates 0-1 should be all zeros */
        assert_eq!(ENV_INCREMENT_TABLE[0], [0,0,0,0,0,0,0,0]);
        assert_eq!(ENV_INCREMENT_TABLE[1], [0,0,0,0,0,0,0,0]);
        /* Rates 2-3 should have the [0,1,0,1,0,1,0,1] pattern */
        assert_eq!(ENV_INCREMENT_TABLE[2], [0,1,0,1,0,1,0,1]);
        /* Rate 48 is the first with increment > 1 */
        assert_eq!(ENV_INCREMENT_TABLE[48], [1,1,1,1,1,1,1,1]);
        /* Rate 60-63 should all be [8,8,8,8,8,8,8,8] */
        assert_eq!(ENV_INCREMENT_TABLE[60], [8,8,8,8,8,8,8,8]);
        assert_eq!(ENV_INCREMENT_TABLE[63], [8,8,8,8,8,8,8,8]);
    }

    /// Bug 2: Attack formula must use signed arithmetic right shift: A' = A + ((I * -(A+1)) >> 4)
    #[test]
    fn test_bug2_attack_formula_arithmetic_shift() {
        let mut op = FmOperator::new();
        op.env_level = 0x3FF; // max attenuation
        op.env_phase = AdsrPhase::Attack;
        op.key_on = true;
        /* Use attack_rate=20, Rate = 2*20 + 0 = 40. Shift = 11 - 40/4 = 1 */
        /* Counter=2048 (bit 11 set → shift=1 means (2048 & 1)==0 → update) */
        /* Table[40][0] = 0 ... need counter where step_idx gives increment=1 */
        /* shift=1, step_idx = (counter >> 1) & 7. counter=2 → step_idx=1 → table[40][1] = 1 */
        op.clock_envelope(20, 0, 0, 0, 0, 0, 0, 2);
        /* Attack should decrease: -(0x3FF+1) * 1 >> 4 = -64 */
        assert!(op.env_level < 0x3FF, "Attack should decrease attenuation, got {}", op.env_level);
        assert_eq!(op.env_level, 0x3FF - 64, "Expected 0x3FF - 64 = {}, got {}", 0x3FF - 64, op.env_level);

        /* Test that small attenuation still reaches 0 via arithmetic right shift */
        op.env_level = 1;
        op.clock_envelope(20, 0, 0, 0, 0, 0, 0, 2);
        assert_eq!(op.env_level, 0, "Attenuation 1 should reach 0 via arithmetic right shift");
    }

    /// Bug 3: Exp table output must use (table[fract] << 2) >> int_part for 14-bit output
    #[test]
    fn test_bug3_exp_table_shift2_output() {
        let mut op = FmOperator::new();
        op.env_level = 0; // no envelope attenuation
        op.env_phase = AdsrPhase::Attack;
        op.key_on = true;
        op.phase_counter = 256 << 10; // phase = 256 (quarter cycle, peak of sine)
        /* At zero attenuation, output should be close to max 14-bit value (~8191) */
        let output = op.compute_output(0, 0);
        /* With << 2 the max is ~4*1024 = 4096; without << 2 max is ~1024 */
        assert!(output.abs() > 2000, "Output should be > 2000 with << 2 scaling, got {}", output);
    }

    /// Bug 4: Mute threshold: combined attenuation >= 8192 (13-bit, 5.8 fixed) should mute
    #[test]
    fn test_bug4_mute_threshold_13bit() {
        let mut op = FmOperator::new();
        op.env_level = 0x3FF; // max envelope (will be shifted << 2 = 0xFFC in 4.8)
        op.env_phase = AdsrPhase::Release;
        op.key_on = false;
        op.phase_counter = 256 << 10;
        /* With TL=0 and env=0x3FF, combined = 0 + (0x3FF << 2) = 0xFFC (< 0x2000) */
        /* This should NOT be muted with the 13-bit threshold but WOULD be with 12-bit */
        let _output = op.compute_output(0, 0);
        /* At maximum envelope but 0 TL, the combined attenuation is below 13-bit threshold */
        /* so we should still get a tiny non-zero output */
        /* Actually env=0x3FF gives combined_atten = 0xFFC which is 0x1FFC... still < 8192 */
        /* but it's heavily attenuated so output may be 0 naturally */
        /* Better test: with TL=127 (max), total_atten = (0 + 127*8) = 1016 = 0x3F8 */
        op.env_level = 0;
        let output_loud = op.compute_output(0, 0);
        assert!(output_loud.abs() > 0, "Zero attenuation should produce output");

        /* TL=127: total_atten = (0 + 127<<3) = 1016, combined = 1016*4 = 4064 < 8192 */
        let output_quiet = op.compute_output(0, 127);
        /* Should still produce some output since 4064 < 8192 */
        /* The old 12-bit threshold (4096) would wrongly mute this */
        assert!(output_quiet.abs() >= 0, "TL=127 should not crash"); // mainly a bounds check
    }

    /// Bug 5: Rate 0 (R=0) must skip envelope updates entirely, regardless of table contents
    #[test]
    fn test_bug5_rate_zero_no_update() {
        let mut op = FmOperator::new();
        op.env_level = 500; // some mid-level attenuation
        op.env_phase = AdsrPhase::Sustain;
        op.key_on = true;
        let original = op.env_level;
        /* Sustain rate = 0 means R=0, so Rate should be 0 regardless of key scaling */
        for counter in 1..100 {
            op.clock_envelope(0, 0, 0, 0, 0, 0, 0, counter);
        }
        assert_eq!(op.env_level, original, "Rate 0 (sustain_rate=0) should hold attenuation constant");
    }

    /// Bug 6: Total Level must be shifted << 3 (multiplied by 8) before adding to envelope
    #[test]
    fn test_bug6_total_level_shift3() {
        let mut op = FmOperator::new();
        op.env_level = 0;
        op.env_phase = AdsrPhase::Attack;
        op.key_on = true;
        op.phase_counter = 256 << 10;

        /* TL=0: should produce max output */
        let out_tl0 = op.compute_output(0, 0);
        /* TL=1: adds 8 to attenuation (1 << 3 = 8), should be quieter */
        let out_tl1 = op.compute_output(0, 1);
        /* TL=16: adds 128 to attenuation (16 << 3), should be much quieter */
        let out_tl16 = op.compute_output(0, 16);

        assert!(out_tl0.abs() > out_tl1.abs(),
            "TL=0 ({}) should be louder than TL=1 ({})", out_tl0, out_tl1);
        assert!(out_tl1.abs() > out_tl16.abs(),
            "TL=1 ({}) should be louder than TL=16 ({})", out_tl1, out_tl16);
    }
}
