//! Yamaha YM2612 (OPN2) FM Synthesizer
//!
//! Refactored to use band-limited synthesis via BlipBuf for high quality.

use crate::apu::blip_buf::BlipBuf;
use crate::audio;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

fn default_master_clock() -> u32 {
    audio::NTSC_MCLK
}

fn default_sample_rate() -> u32 {
    audio::SAMPLE_RATE
}

const YM2612_INTERNAL_CYCLE_MCLK_DIVIDER: u32 = 7 * 6;
const YM2612_SAMPLE_SUBCYCLES: u8 = 24;
const YM2612_CHANNEL_SUBCYCLE_STRIDE: u8 = 4;
const LFO_SAMPLES_PER_STEP: [u32; 8] = [108, 77, 71, 67, 62, 44, 8, 5];
const LFO_AMS_DEPTH_SHIFT: [u8; 4] = [8, 3, 1, 0];
const DT_MASK: u32 = (1 << 17) - 1;
const TL_RES_LEN: usize = 256;
const TL_TAB_LEN: usize = 13 * 2 * TL_RES_LEN;
const ENV_QUIET: u32 = (TL_TAB_LEN >> 3) as u32;
const SLOT1: usize = 0;
const SLOT3: usize = 1;
const SLOT2: usize = 2;
const SLOT4: usize = 3;
const LFO_PM_OUTPUT: [[[u8; 8]; 8]; 7] = [
    [
        [0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 1, 1, 1, 1],
    ],
    [
        [0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 1, 1, 1, 1],
        [0, 0, 1, 1, 2, 2, 2, 3],
    ],
    [
        [0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 1],
        [0, 0, 0, 0, 1, 1, 1, 1],
        [0, 0, 1, 1, 2, 2, 2, 3],
        [0, 0, 2, 3, 4, 4, 5, 6],
    ],
    [
        [0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 1, 1],
        [0, 0, 0, 0, 1, 1, 1, 1],
        [0, 0, 0, 1, 1, 1, 1, 2],
        [0, 0, 1, 1, 2, 2, 2, 3],
        [0, 0, 2, 3, 4, 4, 5, 6],
        [0, 0, 4, 6, 8, 8, 10, 12],
    ],
    [
        [0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 1, 1, 1, 1],
        [0, 0, 0, 1, 1, 1, 2, 2],
        [0, 0, 1, 1, 2, 2, 3, 3],
        [0, 0, 1, 2, 2, 2, 3, 4],
        [0, 0, 2, 3, 4, 4, 5, 6],
        [0, 0, 4, 6, 8, 8, 10, 12],
        [0, 0, 8, 12, 16, 16, 20, 24],
    ],
    [
        [0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 2, 2, 2, 2],
        [0, 0, 0, 2, 2, 2, 4, 4],
        [0, 0, 2, 2, 4, 4, 6, 6],
        [0, 0, 2, 4, 4, 4, 6, 8],
        [0, 0, 4, 6, 8, 8, 10, 12],
        [0, 0, 8, 12, 16, 16, 20, 24],
        [0, 0, 16, 24, 32, 32, 40, 48],
    ],
    [
        [0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 4, 4, 4, 4],
        [0, 0, 0, 4, 4, 4, 8, 8],
        [0, 0, 4, 4, 8, 8, 12, 12],
        [0, 0, 4, 8, 8, 8, 12, 16],
        [0, 0, 8, 12, 16, 16, 20, 24],
        [0, 0, 16, 24, 32, 32, 40, 48],
        [0, 0, 32, 48, 64, 64, 80, 96],
    ],
];
const EG_RATE_SELECT: [u8; 128] = [
    144, 144, 144, 144, 144, 144, 144, 144, 144, 144, 144, 144, 144, 144, 144, 144, 144, 144, 144,
    144, 144, 144, 144, 144, 144, 144, 144, 144, 144, 144, 144, 144, 144, 144, 16, 24, 0, 8, 16,
    24, 0, 8, 16, 24, 0, 8, 16, 24, 0, 8, 16, 24, 0, 8, 16, 24, 0, 8, 16, 24, 0, 8, 16, 24, 0, 8,
    16, 24, 0, 8, 16, 24, 0, 8, 16, 24, 0, 8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112,
    120, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128,
    128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128,
];
const EG_RATE_SHIFT: [u8; 128] = [
    11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11,
    11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 10, 10, 10, 10, 9, 9, 9, 9, 8, 8, 8, 8, 7, 7,
    7, 7, 6, 6, 6, 6, 5, 5, 5, 5, 4, 4, 4, 4, 3, 3, 3, 3, 2, 2, 2, 2, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/* ========================================================================= */
/*  Lookup Tables                                                            */
/* ========================================================================= */

static TL_TABLE: LazyLock<[i16; TL_TAB_LEN]> = LazyLock::new(|| {
    let mut table = [0i16; TL_TAB_LEN];
    for x in 0..TL_RES_LEN {
        let mut m = (1u32 << 16) as f64 / 2.0_f64.powf((x as f64 + 1.0) * (0.125 / 4.0) / 8.0);
        m = m.floor();
        let mut n = m as i32;
        n >>= 4;
        if (n & 1) != 0 {
            n = (n >> 1) + 1;
        } else {
            n >>= 1;
        }
        n <<= 2;
        table[x * 2] = n as i16;
        table[x * 2 + 1] = -(n as i16);
        for i in 1..13 {
            let shifted = (table[x * 2] as i32) >> i;
            table[x * 2 + i * 2 * TL_RES_LEN] = shifted as i16;
            table[x * 2 + 1 + i * 2 * TL_RES_LEN] = -(shifted as i16);
        }
    }
    table
});

static SIN_TABLE: LazyLock<[u16; 1024]> = LazyLock::new(|| {
    let mut table = [0u16; 1024];
    for (i, slot) in table.iter_mut().enumerate() {
        let m = (((i * 2) + 1) as f64 * std::f64::consts::PI / 1024.0).sin();
        let o = 8.0 * (1.0 / m.abs()).log2();
        let o = o / (0.125 / 4.0);
        let mut n = (2.0 * o) as i32;
        if (n & 1) != 0 {
            n = (n >> 1) + 1;
        } else {
            n >>= 1;
        }
        *slot = (n * 2 + if m >= 0.0 { 0 } else { 1 }) as u16;
    }
    table
});

static LFO_PM_TABLE: LazyLock<[i16; 128 * 8 * 32]> = LazyLock::new(|| {
    let mut table = [0i16; 128 * 8 * 32];
    for depth in 0..8 {
        for fnum in 0..128 {
            for step in 0..8 {
                let mut value = 0i16;
                for bit in 0..7 {
                    if (fnum & (1 << bit)) != 0 {
                        value += LFO_PM_OUTPUT[bit][depth][step] as i16;
                    }
                }
                let base = (fnum * 32 * 8) + (depth * 32);
                table[base + step] = value;
                table[base + ((step ^ 7) + 8)] = value;
                table[base + step + 16] = -value;
                table[base + ((step ^ 7) + 24)] = -value;
            }
        }
    }
    table
});

/// Detune table
const DETUNE_TABLE: [[u8; 4]; 32] = [
    [0, 0, 1, 2],
    [0, 0, 1, 2],
    [0, 0, 1, 2],
    [0, 0, 1, 2],
    [0, 1, 2, 2],
    [0, 1, 2, 3],
    [0, 1, 2, 3],
    [0, 1, 2, 3],
    [0, 1, 2, 4],
    [0, 1, 3, 4],
    [0, 1, 3, 4],
    [0, 1, 3, 5],
    [0, 2, 4, 5],
    [0, 2, 4, 6],
    [0, 2, 4, 6],
    [0, 2, 5, 7],
    [0, 2, 5, 8],
    [0, 3, 6, 8],
    [0, 3, 6, 9],
    [0, 3, 7, 10],
    [0, 4, 8, 11],
    [0, 4, 8, 12],
    [0, 4, 9, 13],
    [0, 5, 10, 14],
    [0, 5, 11, 16],
    [0, 6, 12, 17],
    [0, 6, 13, 19],
    [0, 7, 14, 20],
    [0, 8, 16, 22],
    [0, 8, 16, 22],
    [0, 8, 16, 22],
    [0, 8, 16, 22],
];

/// Envelope increment patterns (GPGX/MAME `eg_inc`): 19 rows of 8 steps,
/// selected by `EG_RATE_SELECT[rate] / 8`. Rows 0-3 cover rates 0-11
/// (0/1 steps), 4-16 the fast rates (1/2/4/8), 17 the attack-only 16s,
/// and 18 the "infinity" rates that never advance.
const ENV_INCREMENT_TABLE: [[u8; 8]; 19] = [
    [0, 1, 0, 1, 0, 1, 0, 1],
    [0, 1, 0, 1, 1, 1, 0, 1],
    [0, 1, 1, 1, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 1, 1],
    [1, 1, 1, 2, 1, 1, 1, 2],
    [1, 2, 1, 2, 1, 2, 1, 2],
    [1, 2, 2, 2, 1, 2, 2, 2],
    [2, 2, 2, 2, 2, 2, 2, 2],
    [2, 2, 2, 4, 2, 2, 2, 4],
    [2, 4, 2, 4, 2, 4, 2, 4],
    [2, 4, 4, 4, 2, 4, 4, 4],
    [4, 4, 4, 4, 4, 4, 4, 4],
    [4, 4, 4, 8, 4, 4, 4, 8],
    [4, 8, 4, 8, 4, 8, 4, 8],
    [4, 8, 8, 8, 4, 8, 8, 8],
    [8, 8, 8, 8, 8, 8, 8, 8],
    [16, 16, 16, 16, 16, 16, 16, 16],
    [0, 0, 0, 0, 0, 0, 0, 0],
];

mod register_array {
    use crate::memory::byte_utils::big_array;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(data: &[[u8; 256]; 2], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeTuple;
        let mut s = serializer.serialize_tuple(2)?;
        #[derive(Serialize)]
        struct Wrapper<'a>(#[serde(with = "big_array")] &'a [u8; 256]);
        s.serialize_element(&Wrapper(&data[0]))?;
        s.serialize_element(&Wrapper(&data[1]))?;
        s.end()
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[[u8; 256]; 2], D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wrapper(#[serde(with = "big_array")] [u8; 256]);
        let arr: [Wrapper; 2] = Deserialize::deserialize(deserializer)?;
        Ok([arr[0].0, arr[1].0])
    }
}

/* ========================================================================= */
/*  FM Operator                                                              */
/* ========================================================================= */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum AdsrPhase {
    Attack,
    Decay,
    Sustain,
    Release,
    Off,
}

struct EnvelopeParams {
    ar: u8,
    dr: u8,
    sr: u8,
    rr: u8,
    sl: u8,
    ks: u8,
    kc: u8,
    counter: u16,
    ssg_eg: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FmOperator {
    phase_counter: u32,
    env_phase: AdsrPhase,
    env_level: u16,
    #[serde(default)]
    ssg_invert: bool,
    key_on: bool,
    last_output: i16,
    last_output2: i16,
}

impl FmOperator {
    fn effective_attack_rate(params: &EnvelopeParams) -> u16 {
        let ks_shift = match params.ks {
            0 => 3,
            1 => 2,
            2 => 1,
            3 => 0,
            _ => 3,
        };
        if params.ar == 0 {
            0
        } else {
            (expand_eg_rate(params.ar) + (params.kc >> ks_shift) as u16).min(94)
        }
    }

    fn attack_transition_phase(params: &EnvelopeParams, env_level: u16) -> (u16, AdsrPhase) {
        if Self::effective_attack_rate(params) >= 94 {
            (
                0,
                if sustain_level(params.sl) == 0 {
                    AdsrPhase::Sustain
                } else {
                    AdsrPhase::Decay
                },
            )
        } else if env_level == 0 {
            (
                env_level,
                if sustain_level(params.sl) == 0 {
                    AdsrPhase::Sustain
                } else {
                    AdsrPhase::Decay
                },
            )
        } else {
            (env_level, AdsrPhase::Attack)
        }
    }

    fn new() -> Self {
        Self {
            phase_counter: 0,
            env_phase: AdsrPhase::Off,
            env_level: 0x3FF,
            ssg_invert: false,
            key_on: false,
            last_output: 0,
            last_output2: 0,
        }
    }

    fn set_key_on(&mut self, on: bool, params: &EnvelopeParams, csm_active: bool) {
        if on == self.key_on {
            return;
        }
        self.key_on = on;
        /* While the CSM key window holds the combined key line (register OR
         * CSM) high, register key changes only latch state: no retrigger on
         * key-on and no release on key-off. The deferred release fires when
         * the CSM window ends (set_key_off_csm checks !key_on). */
        if csm_active {
            return;
        }
        if on {
            self.phase_counter = 0;
            self.ssg_invert = false;
            let (env_level, env_phase) = Self::attack_transition_phase(params, self.env_level);
            self.env_level = env_level;
            self.env_phase = env_phase;
        } else if self.env_phase != AdsrPhase::Off && self.env_phase != AdsrPhase::Release {
            self.env_phase = AdsrPhase::Release;
            if (params.ssg_eg & 0x08) != 0 {
                if self.ssg_invert ^ ((params.ssg_eg & 0x04) != 0) {
                    self.env_level = (0x200u16.wrapping_sub(self.env_level)) & 0x03ff;
                }
                if self.env_level >= 0x200 {
                    self.env_level = 0x3ff;
                    self.env_phase = AdsrPhase::Off;
                }
            }
        }
    }

    fn set_key_on_csm(&mut self, params: &EnvelopeParams) {
        if !self.key_on {
            self.phase_counter = 0;
            self.ssg_invert = false;
            let (env_level, env_phase) = Self::attack_transition_phase(params, self.env_level);
            self.env_level = env_level;
            self.env_phase = env_phase;
        }
    }

    fn set_key_off_csm(&mut self, params: &EnvelopeParams) {
        if !self.key_on && self.env_phase != AdsrPhase::Off && self.env_phase != AdsrPhase::Release
        {
            self.env_phase = AdsrPhase::Release;
            if (params.ssg_eg & 0x08) != 0 {
                if self.ssg_invert ^ ((params.ssg_eg & 0x04) != 0) {
                    self.env_level = (0x200u16.wrapping_sub(self.env_level)) & 0x03ff;
                }
                if self.env_level >= 0x200 {
                    self.env_level = 0x3ff;
                    self.env_phase = AdsrPhase::Off;
                }
            }
        }
    }

    fn update_ssg_eg(&mut self, params: &EnvelopeParams) {
        if (params.ssg_eg & 0x08) == 0
            || self.env_phase == AdsrPhase::Release
            || self.env_phase == AdsrPhase::Off
        {
            return;
        }
        if self.env_level < 0x200 {
            return;
        }

        if (params.ssg_eg & 0x01) != 0 {
            if (params.ssg_eg & 0x02) != 0 {
                self.ssg_invert = true;
            }
            if self.env_phase != AdsrPhase::Attack
                && !(self.ssg_invert ^ ((params.ssg_eg & 0x04) != 0))
            {
                self.env_level = 0x3FF;
            }
            return;
        }

        if (params.ssg_eg & 0x02) != 0 {
            self.ssg_invert = !self.ssg_invert;
        } else {
            self.phase_counter = 0;
        }

        if self.env_phase != AdsrPhase::Attack {
            let (env_level, env_phase) = Self::attack_transition_phase(params, self.env_level);
            self.env_level = env_level;
            self.env_phase = env_phase;
        }
    }

    fn apply_sl_rr_write(&mut self, value: u8) {
        if self.env_phase == AdsrPhase::Decay && self.env_level >= sustain_level(value >> 4) {
            self.env_phase = AdsrPhase::Sustain;
        }
    }

    fn apply_ssg_write(&mut self, params: &EnvelopeParams) {
        self.update_ssg_eg(params);
    }

    fn clock_phase(
        &mut self,
        fnum: u16,
        block: u8,
        key_code: u8,
        detune: u8,
        multiple: u8,
        lfo_offset: i16,
    ) {
        let base_inc = if lfo_offset == 0 {
            ((fnum as u32) << block) >> 1
        } else {
            ((((fnum as i32) << 1) + lfo_offset as i32) & 0x0fff) as u32
        };
        let base_inc = if lfo_offset == 0 {
            base_inc
        } else {
            (base_inc << block) >> 2
        };
        let dt_mag = detune & 0x03;
        let dt_sign = detune & 0x04;
        let dt_delta = if dt_mag == 0 {
            0
        } else {
            DETUNE_TABLE[key_code as usize][dt_mag as usize] as u32
        };
        let detuned = if dt_sign != 0 {
            base_inc.wrapping_sub(dt_delta)
        } else {
            base_inc.wrapping_add(dt_delta)
        } & DT_MASK;
        let increment = if multiple == 0 {
            detuned >> 1
        } else {
            detuned.wrapping_mul(multiple as u32)
        };
        self.phase_counter = (self.phase_counter.wrapping_add(increment)) & 0xFFFFF;
    }

    fn clock_envelope(&mut self, params: &EnvelopeParams) {
        let base_rate = match self.env_phase {
            AdsrPhase::Attack => expand_eg_rate(params.ar),
            AdsrPhase::Decay => expand_eg_rate(params.dr),
            AdsrPhase::Sustain => expand_eg_rate(params.sr),
            AdsrPhase::Release => 34 + ((params.rr as u16) << 2),
            AdsrPhase::Off => 0,
        };
        if self.env_phase == AdsrPhase::Off {
            self.env_level = 0x3FF;
            return;
        }
        let ks_shift = match params.ks {
            0 => 3,
            1 => 2,
            2 => 1,
            3 => 0,
            _ => 3,
        };
        let rate = if base_rate == 0 {
            0
        } else {
            (base_rate + (params.kc >> ks_shift) as u16).min(94) as u8
        };
        let (shift, select) = eg_rate_params(rate);
        let mask = if shift >= 16 {
            u16::MAX
        } else {
            (1u16 << shift).saturating_sub(1)
        };
        if (params.counter & mask) == 0 {
            let step_idx = ((params.counter >> shift) & 7) as usize;
            let increment = ENV_INCREMENT_TABLE[(select as usize / 8).min(18)][step_idx];
            if increment > 0 {
                match self.env_phase {
                    AdsrPhase::Attack => {
                        if rate >= 94 {
                            self.env_level = 0;
                        } else {
                            let delta = (increment as i32 * -((self.env_level as i32) + 1)) >> 4;
                            self.env_level = (self.env_level as i32 + delta).max(0) as u16;
                        }
                    }
                    _ => {
                        /* SSG-EG runs the envelope 4x fast in decay, sustain
                         * AND release (GPGX/Nuked; the release snap to 0x3FF
                         * at 0x200 is handled below). Outside release, SSG
                         * increments apply only below 0x200: at or above, the
                         * per-sample transition logic (hold/alternate/
                         * retrigger) owns the level, so e.g. held-inverted
                         * shapes hold steady instead of ramping to 0x3FF. */
                        let ssg_on = (params.ssg_eg & 0x08) != 0;
                        let frozen = ssg_on
                            && self.env_phase != AdsrPhase::Release
                            && self.env_level >= 0x200;
                        if !frozen {
                            let step = if ssg_on {
                                (increment as u16) * 4
                            } else {
                                increment as u16
                            };
                            self.env_level = (self.env_level + step).min(0x3FF);
                        }
                    }
                }
            }
        }
        if self.env_phase == AdsrPhase::Attack && self.env_level == 0 {
            self.env_phase = AdsrPhase::Decay;
        }
        if self.env_phase == AdsrPhase::Decay && self.env_level >= sustain_level(params.sl) {
            self.env_phase = AdsrPhase::Sustain;
        }
        if self.env_phase == AdsrPhase::Release {
            if (params.ssg_eg & 0x08) != 0 {
                if self.env_level >= 0x200 {
                    self.env_level = 0x3FF;
                    self.env_phase = AdsrPhase::Off;
                }
            } else if self.env_level >= 0x3FF {
                self.env_phase = AdsrPhase::Off;
            }
        }
    }

    fn compute_output(
        &self,
        phase_mod: i32,
        total_level: u16,
        lfo_am: u16,
        ssg_eg: u8,
        full_phase_mod: bool,
    ) -> i16 {
        let env_level = if (ssg_eg & 0x08) != 0
            && self.env_phase != AdsrPhase::Release
            && self.env_phase != AdsrPhase::Off
            && (self.ssg_invert ^ ((ssg_eg & 0x04) != 0))
        {
            /* Inverted SSG output: internal level 0x200 maps to attenuation 0
             * (full volume), level 0 to 0x200, per GPGX/Nuked. */
            (0x200u16.wrapping_sub(self.env_level)) & 0x03ff
        } else {
            self.env_level
        };
        let total_atten = (env_level + (total_level << 3) + lfo_am).min(0x3FF);
        if total_atten as u32 >= ENV_QUIET {
            return 0;
        }
        let mod_input = if full_phase_mod {
            phase_mod
        } else {
            phase_mod >> 1
        };
        let phase = (((self.phase_counter >> 10) & 0x3ff) as i32 + mod_input) as u32 & 0x3ff;
        let combined_atten = ((total_atten as u32) << 3) + SIN_TABLE[phase as usize] as u32;
        if combined_atten >= TL_TAB_LEN as u32 {
            return 0;
        }
        TL_TABLE[combined_atten as usize]
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
    #[serde(default)]
    ams_shift: u8,
    #[serde(default)]
    pms: u8,
    fnum: u16,
    block: u8,
    fnum_latch: u8,
    #[serde(default)]
    mem_value: i32,
    last_sample: i16,
}

impl FmChannel {
    fn clamp_output(sample: i32) -> i16 {
        sample.clamp(-8192, 8191) as i16
    }

    fn quantize_output_mask(&self, slot: usize, hardware_profile: Ym2612HardwareProfile) -> bool {
        if hardware_profile != Ym2612HardwareProfile::DiscreteYm2612 {
            return false;
        }
        let quantize_9bit = match self.algorithm {
            0..=3 => slot == SLOT4,
            4 => slot == SLOT2 || slot == SLOT4,
            5 | 6 => slot == SLOT2 || slot == SLOT3 || slot == SLOT4,
            7 => true,
            _ => false,
        };
        quantize_9bit
    }

    fn mask_carrier_output(
        &self,
        slot: usize,
        sample: i16,
        hardware_profile: Ym2612HardwareProfile,
    ) -> i16 {
        let quantize_9bit = self.quantize_output_mask(slot, hardware_profile);
        if quantize_9bit {
            ((sample as i32) & !31) as i16
        } else {
            sample
        }
    }

    fn new() -> Self {
        Self {
            operators: std::array::from_fn(|_| FmOperator::new()),
            algorithm: 0,
            feedback: 0,
            panning_l: true,
            panning_r: true,
            ams_shift: LFO_AMS_DEPTH_SHIFT[0],
            pms: 0,
            fnum: 0,
            block: 0,
            fnum_latch: 0,
            mem_value: 0,
            last_sample: 0,
        }
    }

    fn update_ssg(
        &mut self,
        regs: &[u8; 256],
        ch_off: usize,
        special_frequencies: Option<[(u16, u8); 4]>,
    ) {
        /* Register-order slot offsets indexed by the SLOT constants
         * (SLOT1=0, SLOT3=1, SLOT2=2, SLOT4=3): hardware register order is
         * S1(+0), S3(+4), S2(+8), S4(+12), matching
         * `operator_index_from_register`. */
        let op_offsets: [usize; 4] = [0, 4, 8, 12];
        let slot_freqs: [(u16, u8); 4] = std::array::from_fn(|i| {
            special_frequencies
                .map(|freqs| freqs[i])
                .unwrap_or((self.fnum, self.block))
        });

        for (i, op_offset) in op_offsets.iter().enumerate() {
            let off = *op_offset + ch_off;
            let (fnum, block) = slot_freqs[i];
            let kc = compute_key_code(fnum as u32, block);
            self.operators[i].update_ssg_eg(&EnvelopeParams {
                ar: regs[0x50 + off] & 0x1F,
                dr: regs[0x60 + off] & 0x1F,
                sr: regs[0x70 + off] & 0x1F,
                rr: regs[0x80 + off] & 0xF,
                sl: (regs[0x80 + off] >> 4) & 0xF,
                ks: (regs[0x50 + off] >> 6) & 3,
                kc,
                counter: 0,
                ssg_eg: regs[0x90 + off] & 0x0F,
            });
        }
    }

    fn advance_envelopes(
        &mut self,
        regs: &[u8; 256],
        ch_off: usize,
        counter: u16,
        special_frequencies: Option<[(u16, u8); 4]>,
    ) {
        /* Register-order slot offsets indexed by the SLOT constants
         * (SLOT1=0, SLOT3=1, SLOT2=2, SLOT4=3): hardware register order is
         * S1(+0), S3(+4), S2(+8), S4(+12), matching
         * `operator_index_from_register`. */
        let op_offsets: [usize; 4] = [0, 4, 8, 12];
        let slot_freqs: [(u16, u8); 4] = std::array::from_fn(|i| {
            special_frequencies
                .map(|freqs| freqs[i])
                .unwrap_or((self.fnum, self.block))
        });

        for (i, op_offset) in op_offsets.iter().enumerate() {
            let off = *op_offset + ch_off;
            let (base_fnum, block) = slot_freqs[i];
            let eg_kc = compute_key_code(base_fnum as u32, block);
            self.operators[i].clock_envelope(&EnvelopeParams {
                ar: regs[0x50 + off] & 0x1F,
                dr: regs[0x60 + off] & 0x1F,
                sr: regs[0x70 + off] & 0x1F,
                rr: regs[0x80 + off] & 0xF,
                sl: (regs[0x80 + off] >> 4) & 0xF,
                ks: (regs[0x50 + off] >> 6) & 3,
                kc: eg_kc,
                counter,
                ssg_eg: regs[0x90 + off] & 0x0F,
            });
        }
    }

    fn clock(
        &mut self,
        regs: &[u8; 256],
        ch_off: usize,
        lfo_am: u8,
        lfo_pm: u8,
        hardware_profile: Ym2612HardwareProfile,
        special_frequencies: Option<[(u16, u8); 4]>,
    ) -> i16 {
        /* Register-order slot offsets indexed by the SLOT constants
         * (SLOT1=0, SLOT3=1, SLOT2=2, SLOT4=3): hardware register order is
         * S1(+0), S3(+4), S2(+8), S4(+12), matching
         * `operator_index_from_register`. */
        let op_offsets: [usize; 4] = [0, 4, 8, 12];
        let slot_freqs: [(u16, u8); 4] = std::array::from_fn(|i| {
            special_frequencies
                .map(|freqs| freqs[i])
                .unwrap_or((self.fnum, self.block))
        });
        let am_atten = (lfo_am as u16) >> self.ams_shift;
        let tl: [u16; 4] =
            std::array::from_fn(|i| (regs[0x40 + op_offsets[i] + ch_off] & 0x7F) as u16);
        let ssg: [u8; 4] = std::array::from_fn(|i| regs[0x90 + op_offsets[i] + ch_off] & 0x0F);
        let fb = if self.feedback > 0 {
            ((self.operators[SLOT1].last_output as i32 + self.operators[SLOT1].last_output2 as i32)
                >> 1)
                >> (9 - self.feedback as i32)
        } else {
            0
        };
        let out1 = self.operators[SLOT1].compute_output(
            fb,
            tl[SLOT1],
            if (regs[0x60 + ch_off] & 0x80) != 0 {
                am_atten
            } else {
                0
            },
            ssg[SLOT1],
            true,
        );
        let out1 = self.mask_carrier_output(SLOT1, out1, hardware_profile);
        let mut m2 = 0i32;
        let mut c1 = 0i32;
        let mut c2 = 0i32;
        let mut mem = 0i32;
        match self.algorithm {
            0..=2 | 5 => m2 = self.mem_value,
            3 => c2 = self.mem_value,
            _ => {}
        }

        if self.algorithm == 5 {
            mem = out1 as i32;
            c1 = out1 as i32;
            c2 = out1 as i32;
        } else {
            match self.algorithm {
                0 | 3 | 4 | 6 => c1 = out1 as i32,
                1 => mem = out1 as i32,
                2 => c2 = out1 as i32,
                7 => {}
                _ => {}
            }
        }

        let out3 = self.operators[SLOT3].compute_output(
            m2,
            tl[SLOT3],
            if (regs[0x60 + 4 + ch_off] & 0x80) != 0 {
                am_atten
            } else {
                0
            },
            ssg[SLOT3],
            false,
        );
        let out3 = self.mask_carrier_output(SLOT3, out3, hardware_profile);
        match self.algorithm {
            /* Algorithms 0-4 all route OP3 into OP4's modulation input
             * (alg 4 is the two-stack S1->S2 + S3->S4 configuration). In
             * 5/6/7 OP3 is a carrier and goes to the output sum instead. */
            0..=4 => c2 += out3 as i32,
            _ => {}
        }

        let out2 = self.operators[SLOT2].compute_output(
            c1,
            tl[SLOT2],
            if (regs[0x60 + 8 + ch_off] & 0x80) != 0 {
                am_atten
            } else {
                0
            },
            ssg[SLOT2],
            false,
        );
        let out2 = self.mask_carrier_output(SLOT2, out2, hardware_profile);
        match self.algorithm {
            0 | 1 | 2 | 3 => mem += out2 as i32,
            4 | 5 | 6 | 7 => {}
            _ => {}
        }

        let mut carrier = match self.algorithm {
            4 | 5 | 6 | 7 => 0i32,
            _ => 0i32,
        };
        match self.algorithm {
            /* Carrier sets (OP4 is added below for every algorithm):
             * alg 4 -> OP2+OP4; algs 5/6 -> OP2+OP3+OP4; alg 7 -> all four.
             * OP1 is a modulator in every algorithm except 7. */
            4 => carrier += self.mask_carrier_output(SLOT2, out2, hardware_profile) as i32,
            5 | 6 => {
                carrier += self.mask_carrier_output(SLOT2, out2, hardware_profile) as i32;
                carrier += self.mask_carrier_output(SLOT3, out3, hardware_profile) as i32;
            }
            7 => {
                carrier += self.mask_carrier_output(SLOT1, out1, hardware_profile) as i32;
                carrier += self.mask_carrier_output(SLOT2, out2, hardware_profile) as i32;
                carrier += self.mask_carrier_output(SLOT3, out3, hardware_profile) as i32;
            }
            _ => {}
        }

        let out4 = self.operators[SLOT4].compute_output(
            c2,
            tl[SLOT4],
            if (regs[0x60 + 12 + ch_off] & 0x80) != 0 {
                am_atten
            } else {
                0
            },
            ssg[SLOT4],
            false,
        );
        let out4 = self.mask_carrier_output(SLOT4, out4, hardware_profile);
        carrier += out4 as i32;

        self.operators[SLOT1].last_output2 = self.operators[SLOT1].last_output;
        self.operators[SLOT1].last_output = out1;
        self.operators[SLOT2].last_output = out2;
        self.operators[SLOT3].last_output = out3;
        self.operators[SLOT4].last_output = out4;
        self.mem_value = mem;

        let channel_out = match self.algorithm {
            4 | 5 | 6 | 7 => Self::clamp_output(carrier),
            _ => out4,
        };
        for (i, op_offset) in op_offsets.iter().enumerate() {
            let off = *op_offset + ch_off;
            let (base_fnum, block) = slot_freqs[i];
            /* Key code always derives from the slot's own frequency, also in
             * CH3 special mode with LFO PM active (Nuked kcode_3ch; GPGX
             * update_phase_lfo_slot recomputes kc per slot). */
            let base_key_code = compute_key_code(base_fnum as u32, block);
            let lfo_offset = if self.pms == 0 {
                0
            } else {
                compute_lfo_phase_mod(base_fnum, self.pms, lfo_pm)
            };
            self.operators[i].clock_phase(
                base_fnum,
                block,
                base_key_code,
                (regs[0x30 + off] >> 4) & 7,
                regs[0x30 + off] & 0xF,
                lfo_offset,
            );
        }

        self.last_sample = channel_out;
        channel_out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Bank {
    Bank0 = 0,
    Bank1 = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ym2612HardwareProfile {
    DiscreteYm2612,
    Ym3438,
}

fn default_hardware_profile() -> Ym2612HardwareProfile {
    Ym2612HardwareProfile::DiscreteYm2612
}

fn default_selected_data_bank() -> Bank {
    Bank::Bank0
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct LatchedChannelState {
    algorithm: u8,
    feedback: u8,
    panning_l: bool,
    panning_r: bool,
    ams_shift: u8,
    pms: u8,
    fnum: u16,
    block: u8,
}

impl LatchedChannelState {
    fn from_channel(channel: &FmChannel) -> Self {
        Self {
            algorithm: channel.algorithm,
            feedback: channel.feedback,
            panning_l: channel.panning_l,
            panning_r: channel.panning_r,
            ams_shift: channel.ams_shift,
            pms: channel.pms,
            fnum: channel.fnum,
            block: channel.block,
        }
    }
}

fn default_latched_registers() -> [[u8; 256]; 2] {
    [[0; 256]; 2]
}

fn default_latched_channels() -> [LatchedChannelState; 6] {
    std::array::from_fn(|_| LatchedChannelState {
        algorithm: 0,
        feedback: 0,
        panning_l: true,
        panning_r: true,
        ams_shift: LFO_AMS_DEPTH_SHIFT[0],
        pms: 0,
        fnum: 0,
        block: 0,
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Ym2612 {
    #[serde(with = "register_array")]
    pub registers: [[u8; 256]; 2],
    address: [u8; 2],
    pub status: u8,
    timer_a: i32,
    timer_b: i32,
    #[serde(default)]
    timer_b_divider: u8,
    busy: i32,
    channels: [FmChannel; 6],
    #[serde(default)]
    mode: u8,
    #[serde(default)]
    ch3_fnum: [u16; 3],
    #[serde(default)]
    ch3_block: [u8; 3],
    #[serde(default)]
    ch3_fnum_latch: [u8; 3],
    #[serde(default)]
    csm_key_state: u8,
    dac_val: u8,
    dac_en: bool,
    env_counter: u16,
    #[serde(default)]
    eg_timer: u8,
    #[serde(default)]
    lfo_counter: u8,
    #[serde(default)]
    lfo_timer: u32,
    #[serde(default)]
    lfo_timer_overflow: u32,
    #[serde(default)]
    lfo_am: u8,
    #[serde(default)]
    lfo_pm: u8,
    #[serde(default)]
    sample_subcycle: u8,
    #[serde(default)]
    channel_outputs: [i32; 6],
    #[serde(with = "register_array", default = "default_latched_registers")]
    sample_registers: [[u8; 256]; 2],
    #[serde(default = "default_latched_channels")]
    sample_channels: [LatchedChannelState; 6],
    #[serde(default)]
    sample_mode: u8,
    #[serde(default)]
    sample_ch3_fnum: [u16; 3],
    #[serde(default)]
    sample_ch3_block: [u8; 3],
    #[serde(default)]
    sample_dac_val: u8,
    #[serde(default)]
    sample_dac_en: bool,
    #[serde(default = "default_master_clock")]
    pub master_clock: u32,
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
    pub total_clocks: u64,
    pub blip_l: BlipBuf,
    pub blip_r: BlipBuf,
    pub last_left: i32,
    pub last_right: i32,
    pub total_mclocks: u64,
    pub mclk_debt: u32,
    #[serde(default = "default_hardware_profile")]
    hardware_profile: Ym2612HardwareProfile,
    #[serde(default)]
    last_status_read: u8,
    #[serde(default)]
    last_status_read_clock: u64,
    #[serde(default = "default_selected_data_bank")]
    selected_data_bank: Bank,
}

impl Default for Ym2612 {
    fn default() -> Self {
        Self::new()
    }
}

impl Ym2612 {
    pub fn new() -> Self {
        let mut ym = Self {
            registers: [[0; 256]; 2],
            address: [0; 2],
            status: 0,
            timer_a: 0,
            timer_b: 0,
            timer_b_divider: 0,
            busy: 0,
            channels: std::array::from_fn(|_| FmChannel::new()),
            mode: 0,
            ch3_fnum: [0; 3],
            ch3_block: [0; 3],
            ch3_fnum_latch: [0; 3],
            csm_key_state: 0,
            dac_val: 0x80,
            dac_en: false,
            env_counter: 0,
            eg_timer: 0,
            lfo_counter: 0,
            lfo_timer: 0,
            lfo_timer_overflow: 0,
            lfo_am: 126,
            lfo_pm: 0,
            sample_subcycle: 0,
            channel_outputs: [0; 6],
            sample_registers: [[0; 256]; 2],
            sample_channels: default_latched_channels(),
            sample_mode: 0,
            sample_ch3_fnum: [0; 3],
            sample_ch3_block: [0; 3],
            sample_dac_val: 0x80,
            sample_dac_en: false,
            master_clock: default_master_clock(),
            sample_rate: default_sample_rate(),
            total_clocks: 0,
            blip_l: BlipBuf::new(default_master_clock(), default_sample_rate()),
            blip_r: BlipBuf::new(default_master_clock(), default_sample_rate()),
            last_left: 0,
            last_right: 0,
            total_mclocks: 0,
            mclk_debt: 0,
            hardware_profile: default_hardware_profile(),
            last_status_read: 0,
            last_status_read_clock: 0,
            selected_data_bank: Bank::Bank0,
        };
        for i in 0..3 {
            ym.registers[0][0xB4 + i] = 0xC0;
            ym.registers[1][0xB4 + i] = 0xC0;
        }
        ym
    }

    pub fn reset(&mut self) {
        let master_clock = self.master_clock;
        let sample_rate = self.sample_rate;
        *self = Self::new();
        self.set_timing(master_clock, sample_rate);
    }

    /// Reconfigure YM2612 timing for the active video region and output sample rate.
    pub fn set_timing(&mut self, master_clock: u32, sample_rate: u32) {
        if self.master_clock == master_clock
            && self.sample_rate == sample_rate
            && self.blip_l.clock_rate() == master_clock
            && self.blip_r.clock_rate() == master_clock
            && self.blip_l.sample_rate() == sample_rate
            && self.blip_r.sample_rate() == sample_rate
        {
            return;
        }
        self.master_clock = master_clock;
        self.sample_rate = sample_rate;
        self.blip_l.set_timing(master_clock, sample_rate);
        self.blip_r.set_timing(master_clock, sample_rate);
    }

    pub fn read_status(&self) -> u8 {
        let mut res = self.status;
        if self.busy > 0 {
            res |= 0x80;
        }
        res
    }

    fn current_mclk(&self) -> u64 {
        self.total_mclocks + self.mclk_debt as u64
    }

    fn undefined_read_decay_constant(&self) -> u64 {
        (self.master_clock.max(4) / 4) as u64
    }

    fn address_write_busy_cycles(&self) -> i32 {
        match self.hardware_profile {
            Ym2612HardwareProfile::DiscreteYm2612 => 0,
            // step() consumes busy in Genesis MCLKs, so YM3438 wait-cycle constants need conversion.
            Ym2612HardwareProfile::Ym3438 => 17 * 7,
        }
    }

    fn data_write_busy_cycles(&self, address: u8) -> i32 {
        match self.hardware_profile {
            // Baseline: ~32 internal YM cycles after a data write, in the
            // divide-by-6 internal-cycle domain (1 internal cycle = 42 MCLK).
            Ym2612HardwareProfile::DiscreteYm2612 => 32 * 42,
            Ym2612HardwareProfile::Ym3438 => {
                if (0xA0..=0xB6).contains(&address) {
                    47 * 7
                } else {
                    83 * 7
                }
            }
        }
    }

    fn decayed_undefined_read(&self) -> u8 {
        let elapsed = self
            .current_mclk()
            .saturating_sub(self.last_status_read_clock);
        let decay = self.undefined_read_decay_constant();
        if elapsed >= decay {
            return 0;
        }
        (((self.last_status_read as u64) * (decay - elapsed)) / decay) as u8
    }

    pub fn set_hardware_profile(&mut self, profile: Ym2612HardwareProfile) {
        self.hardware_profile = profile;
    }

    pub fn read(&mut self, p: u8) -> u8 {
        if p == 0 {
            let status = self.read_status();
            self.last_status_read = status;
            self.last_status_read_clock = self.current_mclk();
            return status;
        }
        match self.hardware_profile {
            Ym2612HardwareProfile::Ym3438 => self.read_status(),
            Ym2612HardwareProfile::DiscreteYm2612 => self.decayed_undefined_read(),
        }
    }

    /// Step the YM2612 by a number of M68K cycles.
    /// Convert internally to MCLK (1 M68K cycle = 7 MCLK).
    pub fn step(&mut self, cycles: u32) {
        let mclocks = cycles * 7;
        if self.busy > 0 {
            self.busy = self.busy.saturating_sub(mclocks as i32);
        }

        // YM2612 generates one native output sample every 144 chip clocks.
        // On Genesis hardware the chip clock is MCLK/7, so one sample occurs
        // every 1008 master clocks.
        self.mclk_debt += mclocks;

        while self.mclk_debt >= YM2612_INTERNAL_CYCLE_MCLK_DIVIDER {
            self.total_mclocks += YM2612_INTERNAL_CYCLE_MCLK_DIVIDER as u64;
            self.internal_step();
            self.mclk_debt -= YM2612_INTERNAL_CYCLE_MCLK_DIVIDER;
        }
    }

    fn timer_a_period(&self) -> i32 {
        let n = ((self.registers[0][0x24] as u32) << 2) | (self.registers[0][0x25] as u32 & 0x03);
        (1024 - n as i32).max(1)
    }

    fn timer_b_period(&self) -> i32 {
        /* Timer B counts once per 16 samples (timer_b_divider), so the
         * period is (256 - NB) in those /16 units. Applying the <<4 here
         * as well double-counted the prescaler and made Timer B 16x slow. */
        (256 - self.registers[0][0x26] as i32).max(1)
    }

    fn set_timers(&mut self, value: u8) {
        let previous = self.mode;
        self.mode = value;

        if ((previous ^ value) & 0xC0) != 0 && (value & 0xC0) != 0x80 && self.csm_key_state != 0 {
            self.apply_csm_key_off();
            self.csm_key_state = 0;
        }

        if (value & 0x01) != 0 && (previous & 0x01) == 0 {
            self.timer_a = self.timer_a_period();
        }
        if (value & 0x02) != 0 && (previous & 0x02) == 0 {
            self.timer_b = self.timer_b_period();
            self.timer_b_divider = 0;
        }

        if (value & 0x10) != 0 {
            self.status &= !0x01;
        }
        if (value & 0x20) != 0 {
            self.status &= !0x02;
        }
    }

    fn set_lfo(&mut self, value: u8) {
        if (value & 0x08) != 0 {
            self.lfo_timer_overflow = LFO_SAMPLES_PER_STEP[(value & 0x07) as usize];
        } else {
            self.lfo_timer_overflow = 0;
            self.lfo_timer = 0;
            self.lfo_counter = 0;
            self.lfo_pm = 0;
            self.lfo_am = 126;
        }
    }

    fn advance_lfo(&mut self) {
        if self.lfo_timer_overflow == 0 {
            return;
        }

        self.lfo_timer += 1;
        if self.lfo_timer >= self.lfo_timer_overflow {
            self.lfo_timer = 0;
            self.lfo_counter = self.lfo_counter.wrapping_add(1) & 127;
            self.lfo_am = if self.lfo_counter < 64 {
                (self.lfo_counter ^ 63) << 1
            } else {
                (self.lfo_counter & 63) << 1
            };
            self.lfo_pm = self.lfo_counter >> 2;
        }
    }

    fn operator_params_for_channel(&self, channel: usize, slot: usize) -> EnvelopeParams {
        let bank_idx = if channel < 3 { 0 } else { 1 };
        let ch_off = channel % 3;
        /* Register-order slot offsets indexed by the SLOT constants
         * (SLOT1=0, SLOT3=1, SLOT2=2, SLOT4=3): hardware register order is
         * S1(+0), S3(+4), S2(+8), S4(+12), matching
         * `operator_index_from_register`. */
        let op_offsets: [usize; 4] = [0, 4, 8, 12];
        let off = op_offsets[slot] + ch_off;
        /* Any non-zero $27 bits 7-6 value (special mode or CSM) selects the
         * per-operator CH3 frequencies, as in Nuked-OPN2/MAME/GPGX. */
        let (fnum, block) = if channel == 2 && (self.mode & 0xC0) != 0 {
            match slot {
                SLOT1 => (self.ch3_fnum[1], self.ch3_block[1]),
                SLOT2 => (self.ch3_fnum[2], self.ch3_block[2]),
                SLOT3 => (self.ch3_fnum[0], self.ch3_block[0]),
                _ => (self.channels[channel].fnum, self.channels[channel].block),
            }
        } else {
            (self.channels[channel].fnum, self.channels[channel].block)
        };
        EnvelopeParams {
            ar: self.registers[bank_idx][0x50 + off] & 0x1f,
            dr: self.registers[bank_idx][0x60 + off] & 0x1f,
            sr: self.registers[bank_idx][0x70 + off] & 0x1f,
            rr: self.registers[bank_idx][0x80 + off] & 0x0f,
            sl: (self.registers[bank_idx][0x80 + off] >> 4) & 0x0f,
            ks: (self.registers[bank_idx][0x50 + off] >> 6) & 3,
            kc: compute_key_code(fnum as u32, block),
            counter: self.env_counter,
            ssg_eg: self.registers[bank_idx][0x90 + off] & 0x0f,
        }
    }

    fn operator_index_from_register(a: u8) -> Option<usize> {
        if (a & 0x03) == 0x03 {
            return None;
        }
        Some(match (a >> 2) & 0x03 {
            0 => SLOT1,
            1 => SLOT3,
            2 => SLOT2,
            3 => SLOT4,
            _ => SLOT1,
        })
    }

    fn apply_key_on_write(&mut self, channel: usize, value: u8) {
        let slot_params = [
            self.operator_params_for_channel(channel, SLOT1),
            self.operator_params_for_channel(channel, SLOT2),
            self.operator_params_for_channel(channel, SLOT3),
            self.operator_params_for_channel(channel, SLOT4),
        ];
        let csm_active = channel == 2 && self.csm_key_state != 0;
        self.channels[channel].operators[SLOT1].set_key_on(
            (value & 0x10) != 0,
            &slot_params[SLOT1],
            csm_active,
        );
        self.channels[channel].operators[SLOT2].set_key_on(
            (value & 0x20) != 0,
            &slot_params[SLOT2],
            csm_active,
        );
        self.channels[channel].operators[SLOT3].set_key_on(
            (value & 0x40) != 0,
            &slot_params[SLOT3],
            csm_active,
        );
        self.channels[channel].operators[SLOT4].set_key_on(
            (value & 0x80) != 0,
            &slot_params[SLOT4],
            csm_active,
        );
    }

    fn apply_csm_key_on(&mut self) {
        if self.csm_key_state == 0 {
            let slot_params = [
                self.operator_params_for_channel(2, SLOT1),
                self.operator_params_for_channel(2, SLOT2),
                self.operator_params_for_channel(2, SLOT3),
                self.operator_params_for_channel(2, SLOT4),
            ];
            self.channels[2].operators[SLOT1].set_key_on_csm(&slot_params[SLOT1]);
            self.channels[2].operators[SLOT2].set_key_on_csm(&slot_params[SLOT2]);
            self.channels[2].operators[SLOT3].set_key_on_csm(&slot_params[SLOT3]);
            self.channels[2].operators[SLOT4].set_key_on_csm(&slot_params[SLOT4]);
        }
        self.csm_key_state = 1;
    }

    fn apply_csm_key_off(&mut self) {
        let slot_params = [
            self.operator_params_for_channel(2, SLOT1),
            self.operator_params_for_channel(2, SLOT2),
            self.operator_params_for_channel(2, SLOT3),
            self.operator_params_for_channel(2, SLOT4),
        ];
        self.channels[2].operators[SLOT1].set_key_off_csm(&slot_params[SLOT1]);
        self.channels[2].operators[SLOT2].set_key_off_csm(&slot_params[SLOT2]);
        self.channels[2].operators[SLOT3].set_key_off_csm(&slot_params[SLOT3]);
        self.channels[2].operators[SLOT4].set_key_off_csm(&slot_params[SLOT4]);
    }

    fn sample_special_frequencies_for_channel(&self, channel: usize) -> Option<[(u16, u8); 4]> {
        if channel == 2 && (self.sample_mode & 0xC0) != 0 {
            Some([
                (self.sample_ch3_fnum[1], self.sample_ch3_block[1]),
                (self.sample_ch3_fnum[0], self.sample_ch3_block[0]),
                (self.sample_ch3_fnum[2], self.sample_ch3_block[2]),
                (
                    self.sample_channels[channel].fnum,
                    self.sample_channels[channel].block,
                ),
            ])
        } else {
            None
        }
    }

    fn latch_sample_state(&mut self) {
        self.sample_registers = self.registers;
        self.sample_channels = std::array::from_fn(|channel| {
            LatchedChannelState::from_channel(&self.channels[channel])
        });
        self.sample_mode = self.mode;
        self.sample_ch3_fnum = self.ch3_fnum;
        self.sample_ch3_block = self.ch3_block;
        self.sample_dac_val = self.dac_val;
        self.sample_dac_en = self.dac_en;
    }

    fn update_ssg_for_sample(&mut self) {
        for channel in 0..6 {
            let bank_idx = if channel < 3 { 0 } else { 1 };
            let ch_off = channel % 3;
            let special_frequencies = self.sample_special_frequencies_for_channel(channel);
            let live_fnum = self.channels[channel].fnum;
            let live_block = self.channels[channel].block;
            self.channels[channel].fnum = self.sample_channels[channel].fnum;
            self.channels[channel].block = self.sample_channels[channel].block;
            self.channels[channel].update_ssg(
                &self.sample_registers[bank_idx],
                ch_off,
                special_frequencies,
            );
            self.channels[channel].fnum = live_fnum;
            self.channels[channel].block = live_block;
        }
    }

    fn clock_channel_output(&mut self, channel: usize) {
        let bank_idx = if channel < 3 { 0 } else { 1 };
        let ch_off = channel % 3;
        let special_frequencies = self.sample_special_frequencies_for_channel(channel);
        let live_state = LatchedChannelState::from_channel(&self.channels[channel]);
        self.channels[channel].algorithm = self.sample_channels[channel].algorithm;
        self.channels[channel].feedback = self.sample_channels[channel].feedback;
        self.channels[channel].panning_l = self.sample_channels[channel].panning_l;
        self.channels[channel].panning_r = self.sample_channels[channel].panning_r;
        self.channels[channel].ams_shift = self.sample_channels[channel].ams_shift;
        self.channels[channel].pms = self.sample_channels[channel].pms;
        self.channels[channel].fnum = self.sample_channels[channel].fnum;
        self.channels[channel].block = self.sample_channels[channel].block;
        /* The CH6 FM pipeline keeps running while DAC mode is enabled (phase
         * counters, feedback history, envelopes); only the output value is
         * replaced at this stage, as on hardware. */
        let fm_out = self.channels[channel].clock(
            &self.sample_registers[bank_idx],
            ch_off,
            self.lfo_am,
            self.lfo_pm,
            self.hardware_profile,
            special_frequencies,
        ) as i32;
        let out = if channel == 5 && self.sample_dac_en {
            (self.sample_dac_val as i32 - 128) << 6
        } else {
            fm_out
        };
        self.channels[channel].algorithm = live_state.algorithm;
        self.channels[channel].feedback = live_state.feedback;
        self.channels[channel].panning_l = live_state.panning_l;
        self.channels[channel].panning_r = live_state.panning_r;
        self.channels[channel].ams_shift = live_state.ams_shift;
        self.channels[channel].pms = live_state.pms;
        self.channels[channel].fnum = live_state.fnum;
        self.channels[channel].block = live_state.block;
        self.channel_outputs[channel] = out.clamp(-8192, 8191);
    }

    fn finish_sample(&mut self) {
        let mut left = 0i32;
        let mut right = 0i32;
        for (i, out) in self.channel_outputs.iter().copied().enumerate() {
            if self.sample_channels[i].panning_l {
                left += out;
            }
            if self.sample_channels[i].panning_r {
                right += out;
            }
        }
        if self.hardware_profile == Ym2612HardwareProfile::DiscreteYm2612 {
            for (i, out) in self.channel_outputs.iter().copied().enumerate() {
                if out < 0 {
                    left -= (4 - i32::from(self.sample_channels[i].panning_l)) << 5;
                    right -= (4 - i32::from(self.sample_channels[i].panning_r)) << 5;
                } else {
                    left += 4 << 5;
                    right += 4 << 5;
                }
            }
        }
        /* Scale to output range. BlastEm scales each channel by 79/120
         * (ym_enable_zero_offset); the previous `>> 3` (x0.125) left the FM
         * ~5x quieter than hardware relative to the PSG and the i16 range.
         * Worst case: 6 channels x +-8192 x 79/120 = +-32.4k, still in i16. */
        left = left * 79 / 120;
        right = right * 79 / 120;
        let dl = left - self.last_left;
        if dl != 0 {
            self.blip_l.add_delta(self.total_mclocks, dl);
            self.last_left = left;
        }
        let dr = right - self.last_right;
        if dr != 0 {
            self.blip_r.add_delta(self.total_mclocks, dr);
            self.last_right = right;
        }

        self.advance_lfo();

        self.eg_timer = self.eg_timer.wrapping_add(1);
        if self.eg_timer >= 3 {
            self.eg_timer = 0;
            self.env_counter = (self.env_counter + 1) & 0x0FFF;
            if self.env_counter == 0 {
                self.env_counter = 1;
            }

            for channel in 0..6 {
                let special_frequencies = self.sample_special_frequencies_for_channel(channel);
                self.channels[channel].advance_envelopes(
                    &self.sample_registers[if channel < 3 { 0 } else { 1 }],
                    channel % 3,
                    self.env_counter,
                    special_frequencies,
                );
            }
        }

        let timer_ctrl = self.registers[0][0x27];
        self.csm_key_state <<= 1;
        if (timer_ctrl & 0x01) != 0 {
            self.timer_a -= 1;
            if self.timer_a <= 0 {
                if (timer_ctrl & 0x04) != 0 {
                    self.status |= 0x01;
                }
                self.timer_a += self.timer_a_period();
                if (self.mode & 0xC0) == 0x80 {
                    self.apply_csm_key_on();
                }
            }
        }
        if (timer_ctrl & 0x02) != 0 {
            self.timer_b_divider = self.timer_b_divider.wrapping_add(1) & 0x0f;
            if self.timer_b_divider == 0 {
                self.timer_b -= 1;
                if self.timer_b <= 0 {
                    if (timer_ctrl & 0x08) != 0 {
                        self.status |= 0x02;
                    }
                    while self.timer_b <= 0 {
                        self.timer_b += self.timer_b_period();
                    }
                }
            }
        }
        if (self.csm_key_state & 0x02) != 0 {
            self.apply_csm_key_off();
            self.csm_key_state = 0;
        }

        self.total_clocks += 1;
    }

    /// Advance internal FM synthesis by one internal YM2612 cycle.
    fn internal_step(&mut self) {
        if self.sample_subcycle == 0 {
            self.latch_sample_state();
            self.update_ssg_for_sample();
        }

        if self.sample_subcycle % YM2612_CHANNEL_SUBCYCLE_STRIDE == 0 {
            let channel = (self.sample_subcycle / YM2612_CHANNEL_SUBCYCLE_STRIDE) as usize;
            self.clock_channel_output(channel);
        }

        self.sample_subcycle = (self.sample_subcycle + 1) % YM2612_SAMPLE_SUBCYCLES;
        if self.sample_subcycle == 0 {
            self.finish_sample();
        }
    }

    /// Catch up the YM2612 to the current cycle. Useful before register writes.
    pub fn catch_up(&mut self, current_cycles: u32) {
        if current_cycles > 0 {
            self.step(current_cycles);
        }
    }

    pub fn write_address(&mut self, p: u8, v: u8) {
        let bank = if (p & 1) == 0 {
            Bank::Bank0
        } else {
            Bank::Bank1
        };
        self.address[bank as usize] = v;
        self.selected_data_bank = bank;
        self.busy = self.busy.max(self.address_write_busy_cycles());
    }
    pub fn write_addr(&mut self, b: Bank, v: u8) {
        self.address[b as usize] = v;
        self.selected_data_bank = b;
        self.busy = self.busy.max(self.address_write_busy_cycles());
    }
    pub fn write_data(&mut self, _p: u8, v: u8) {
        // There is one physical data port: writes via $4001 or $4003 both
        // commit to the group selected by the last address-port write, so
        // the data port's A1 bit is ignored (Nuked-OPN2 latches the bank at
        // address-write time; some drivers select via $4002 but stream data
        // through $4001).
        // No busy gating: the chip is stepped in batches, so `busy` (set by
        // the preceding address/data write and only drained in `step`) is
        // still high when the CPU writes the data port on the very next
        // instruction. Dropping the write would lose nearly every FM register
        // write in real games (BlastEm likewise never drops data writes —
        // busy only affects the status read).
        self.write_data_bank(self.selected_data_bank, v);
    }

    pub fn write_data_selected_bank(&mut self, b: Bank, v: u8) {
        self.selected_data_bank = b;
        self.write_data(b as u8, v);
    }

    pub fn write_data_bank(&mut self, b: Bank, v: u8) {
        let bank_idx = b as usize;
        let a = self.address[bank_idx];
        self.busy = self.busy.max(self.data_write_busy_cycles(a));
        self.registers[bank_idx][a as usize] = v;
        match (b, a) {
            (Bank::Bank0, 0x22) => self.set_lfo(v),
            (Bank::Bank0, 0x28) => {
                let c = match v & 7 {
                    0..=2 => v & 7,
                    4..=6 => (v & 7) - 1,
                    _ => 7,
                } as usize;
                if c < 6 {
                    self.apply_key_on_write(c, v);
                }
            }
            (_, 0xA0..=0xA2) => {
                let c = (a - 0xA0) as usize + bank_idx * 3;
                if c < 6 {
                    self.channels[c].fnum_latch = v;
                    let high = self.registers[bank_idx][a as usize + 4];
                    self.channels[c].block = (high >> 3) & 0x07;
                    self.channels[c].fnum = (((high as u16) & 0x07) << 8) | v as u16;
                }
            }
            (Bank::Bank0, 0xA8..=0xAA) => {
                let slot = (a - 0xA8) as usize;
                self.ch3_fnum_latch[slot] = v;
                let high = self.registers[0][a as usize + 4];
                self.ch3_block[slot] = (high >> 3) & 0x07;
                self.ch3_fnum[slot] = (((high as u16) & 0x07) << 8) | v as u16;
            }
            (_, 0xB0..=0xB2) => {
                let c = (a - 0xB0) as usize + bank_idx * 3;
                if c < 6 {
                    self.channels[c].feedback = (v >> 3) & 0x07;
                    self.channels[c].algorithm = v & 0x07;
                }
            }
            (_, 0x80..=0x8E) => {
                let op_index = Self::operator_index_from_register(a).unwrap_or(SLOT1);
                let c = (a & 0x03) as usize + bank_idx * 3;
                if c < 6 && (a & 0x03) != 0x03 {
                    self.channels[c].operators[op_index].apply_sl_rr_write(v);
                }
            }
            (_, 0x90..=0x9E) => {
                let c = (a & 0x03) as usize + bank_idx * 3;
                if c < 6 {
                    if let Some(op_index) = Self::operator_index_from_register(a) {
                        let params = self.operator_params_for_channel(c, op_index);
                        self.channels[c].operators[op_index].apply_ssg_write(&params);
                    }
                }
            }
            (Bank::Bank0, 0x27) => self.set_timers(v),
            (Bank::Bank0, 0x2A) => self.dac_val = v,
            (Bank::Bank0, 0x2B) => self.dac_en = (v & 0x80) != 0,
            (_, 0xB4..=0xB6) => {
                let c = (a - 0xB4) as usize + bank_idx * 3;
                if c < 6 {
                    self.channels[c].panning_l = (v & 0x80) != 0;
                    self.channels[c].panning_r = (v & 0x40) != 0;
                    self.channels[c].ams_shift = LFO_AMS_DEPTH_SHIFT[((v >> 4) & 0x03) as usize];
                    self.channels[c].pms = v & 0x07;
                }
            }
            _ => {}
        }
    }

    pub fn generate_sample(&mut self) -> (i16, i16) {
        // Band-limited output: the ~53 kHz chip waveform is resampled to the
        // host rate through the BlipBuf sinc kernel, avoiding the aliasing a
        // nearest-sample hold of the raw step waveform produces.
        let mut l = [0i16; 1];
        let mut r = [0i16; 1];
        self.blip_l.read_samples(&mut l[..]);
        self.blip_r.read_samples(&mut r[..]);
        (l[0], r[0])
    }

    pub fn generate_channel_samples(&mut self) -> [i16; 6] {
        std::array::from_fn(|i| self.channels[i].last_sample)
    }

    #[cfg(test)]
    pub(crate) fn lfo_debug_state(&self) -> (u32, u8, u8) {
        (self.lfo_timer_overflow, self.lfo_am, self.lfo_pm)
    }

    #[cfg(test)]
    pub(crate) fn channel_lfo_debug(&self, channel: usize) -> Option<(u8, u8)> {
        self.channels.get(channel).map(|ch| (ch.ams_shift, ch.pms))
    }

    #[cfg(test)]
    pub(crate) fn channel_frequency_debug(&self, channel: usize) -> Option<(u16, u8)> {
        self.channels.get(channel).map(|ch| (ch.fnum, ch.block))
    }

    #[cfg(test)]
    pub(crate) fn ch3_slot_frequency_debug(&self, slot: usize) -> Option<(u16, u8)> {
        self.ch3_fnum
            .get(slot)
            .zip(self.ch3_block.get(slot))
            .map(|(fnum, block)| (*fnum, *block))
    }

    #[cfg(test)]
    pub(crate) fn channel_key_state(&self, channel: usize) -> Option<[bool; 4]> {
        self.channels.get(channel).map(|ch| {
            [
                ch.operators[SLOT1].key_on,
                ch.operators[SLOT2].key_on,
                ch.operators[SLOT3].key_on,
                ch.operators[SLOT4].key_on,
            ]
        })
    }

    #[cfg(test)]
    pub(crate) fn operator_phase_debug(&self, channel: usize, slot: usize) -> Option<u32> {
        self.channels
            .get(channel)
            .and_then(|ch| ch.operators.get(slot))
            .map(|op| op.phase_counter)
    }

    #[cfg(test)]
    pub(crate) fn operator_last_output_debug(&self, channel: usize, slot: usize) -> Option<i16> {
        self.channels
            .get(channel)
            .and_then(|ch| ch.operators.get(slot))
            .map(|op| op.last_output)
    }

    #[cfg(test)]
    pub(crate) fn operator_feedback_history_debug(
        &self,
        channel: usize,
        slot: usize,
    ) -> Option<(i16, i16)> {
        self.channels
            .get(channel)
            .and_then(|ch| ch.operators.get(slot))
            .map(|op| (op.last_output, op.last_output2))
    }

    #[cfg(test)]
    pub(crate) fn channel_mem_value_debug(&self, channel: usize) -> Option<i32> {
        self.channels.get(channel).map(|ch| ch.mem_value)
    }

    #[cfg(test)]
    pub(crate) fn phase_mod_debug(fnum: u16, pms: u8, lfo_pm: u8) -> i16 {
        compute_lfo_phase_mod(fnum, pms, lfo_pm)
    }

    #[cfg(test)]
    pub(crate) fn eg_debug(raw_rate: u8, raw_sl: u8) -> (u16, u16) {
        (expand_eg_rate(raw_rate), sustain_level(raw_sl))
    }

    #[cfg(test)]
    pub(crate) fn eg_rate_table_debug(rate: u8) -> (u8, u8) {
        eg_rate_params(rate)
    }

    #[cfg(test)]
    pub(crate) fn operator_envelope_debug(
        &self,
        channel: usize,
        slot: usize,
    ) -> Option<(u16, &'static str, bool)> {
        self.channels
            .get(channel)
            .and_then(|ch| ch.operators.get(slot))
            .map(|op| {
                let phase = match op.env_phase {
                    AdsrPhase::Attack => "attack",
                    AdsrPhase::Decay => "decay",
                    AdsrPhase::Sustain => "sustain",
                    AdsrPhase::Release => "release",
                    AdsrPhase::Off => "off",
                };
                (op.env_level, phase, op.ssg_invert)
            })
    }

    #[cfg(test)]
    pub(crate) fn force_operator_envelope(
        &mut self,
        channel: usize,
        slot: usize,
        level: u16,
        phase: &str,
    ) {
        if let Some(ch) = self.channels.get_mut(channel) {
            if let Some(op) = ch.operators.get_mut(slot) {
                op.env_level = level;
                op.env_phase = match phase {
                    "attack" => AdsrPhase::Attack,
                    "decay" => AdsrPhase::Decay,
                    "sustain" => AdsrPhase::Sustain,
                    "release" => AdsrPhase::Release,
                    _ => AdsrPhase::Off,
                };
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn force_lfo_debug(&mut self, am: u8, pm: u8) {
        self.lfo_am = am;
        self.lfo_pm = pm;
    }

    #[cfg(test)]
    pub(crate) fn force_operator_ssg_invert(&mut self, channel: usize, slot: usize, invert: bool) {
        if let Some(ch) = self.channels.get_mut(channel) {
            if let Some(op) = ch.operators.get_mut(slot) {
                op.ssg_invert = invert;
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn operator_output_debug(
        &self,
        channel: usize,
        slot: usize,
        phase_mod: i32,
        total_level: u16,
        lfo_am: u16,
        ssg_eg: u8,
        full_phase_mod: bool,
    ) -> Option<i16> {
        self.channels
            .get(channel)
            .and_then(|ch| ch.operators.get(slot))
            .map(|op| op.compute_output(phase_mod, total_level, lfo_am, ssg_eg, full_phase_mod))
    }

    #[cfg(test)]
    pub(crate) fn phase_increment_debug(
        fnum: u16,
        block: u8,
        key_code: u8,
        detune: u8,
        multiple: u8,
    ) -> u32 {
        let mut op = FmOperator::new();
        op.clock_phase(fnum, block, key_code, detune, multiple, 0);
        op.phase_counter
    }

    #[cfg(test)]
    pub(crate) fn phase_increment_pm_debug(
        fnum: u16,
        block: u8,
        key_code: u8,
        detune: u8,
        multiple: u8,
        lfo_offset: i16,
    ) -> u32 {
        let mut op = FmOperator::new();
        op.clock_phase(fnum, block, key_code, detune, multiple, lfo_offset);
        op.phase_counter
    }

    #[cfg(test)]
    pub(crate) fn force_operator_phase(&mut self, channel: usize, slot: usize, phase: u32) {
        if let Some(ch) = self.channels.get_mut(channel) {
            if let Some(op) = ch.operators.get_mut(slot) {
                op.phase_counter = phase;
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn force_channel_mem_value(&mut self, channel: usize, value: i32) {
        if let Some(ch) = self.channels.get_mut(channel) {
            ch.mem_value = value;
        }
    }

    #[cfg(test)]
    pub(crate) fn force_csm_key_state(&mut self, state: u8) {
        self.csm_key_state = state;
    }

    #[cfg(test)]
    pub(crate) fn debug_apply_csm_key_on(&mut self) {
        self.apply_csm_key_on();
    }
}

fn compute_key_code(f: u32, b: u8) -> u8 {
    let f11 = (f >> 10) & 1;
    let f10 = (f >> 9) & 1;
    let f9 = (f >> 8) & 1;
    let f8 = (f >> 7) & 1;
    let bit0 = (f11 & (f10 | f9 | f8)) | ((1 - f11) & f10 & f9 & f8);
    ((b << 2) as u32 | (f11 << 1) | bit0) as u8
}

fn expand_eg_rate(raw: u8) -> u16 {
    if raw == 0 {
        0
    } else {
        32 + ((raw as u16) << 1)
    }
}

fn sustain_level(raw: u8) -> u16 {
    if raw == 0x0F {
        31 << 5
    } else {
        (raw as u16) << 5
    }
}

fn eg_rate_params(rate: u8) -> (u8, u8) {
    let idx = rate.min(127) as usize;
    (EG_RATE_SHIFT[idx], EG_RATE_SELECT[idx])
}

fn compute_lfo_phase_mod(fnum: u16, pms: u8, lfo_pm: u8) -> i16 {
    if pms == 0 {
        return 0;
    }
    let fnum_index = ((fnum >> 4) & 0x7f) as usize;
    let pm_index = (lfo_pm & 31) as usize;
    LFO_PM_TABLE[(fnum_index * 32 * 8) + (pms as usize * 32) + pm_index]
}
