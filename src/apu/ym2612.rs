//! Yamaha YM2612 (OPN2) FM Synthesizer
//!
//! Refactored to use band-limited synthesis via BlipBuf for high quality.

use crate::apu::blip_buf::BlipBuf;
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

/// Envelope update magnitude table
const ENV_INCREMENT_TABLE: [[u8; 8]; 64] = [
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 1, 0, 1, 0, 1, 0, 1],
    [0, 1, 0, 1, 0, 1, 0, 1],
    [0, 1, 0, 1, 0, 1, 0, 1],
    [0, 1, 0, 1, 0, 1, 0, 1],
    [0, 1, 1, 1, 0, 1, 1, 1],
    [0, 1, 1, 1, 0, 1, 1, 1],
    [0, 1, 0, 1, 0, 1, 0, 1],
    [0, 1, 0, 1, 1, 1, 0, 1],
    [0, 1, 1, 1, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 1],
    [0, 1, 0, 1, 0, 1, 0, 1],
    [0, 1, 0, 1, 1, 1, 0, 1],
    [0, 1, 1, 1, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 1],
    [0, 1, 0, 1, 0, 1, 0, 1],
    [0, 1, 0, 1, 1, 1, 0, 1],
    [0, 1, 1, 1, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 1],
    [0, 1, 0, 1, 0, 1, 0, 1],
    [0, 1, 0, 1, 1, 1, 0, 1],
    [0, 1, 1, 1, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 1],
    [0, 1, 0, 1, 0, 1, 0, 1],
    [0, 1, 0, 1, 1, 1, 0, 1],
    [0, 1, 1, 1, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 1],
    [0, 1, 0, 1, 0, 1, 0, 1],
    [0, 1, 0, 1, 1, 1, 0, 1],
    [0, 1, 1, 1, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 1],
    [0, 1, 0, 1, 0, 1, 0, 1],
    [0, 1, 0, 1, 1, 1, 0, 1],
    [0, 1, 1, 1, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 1],
    [0, 1, 0, 1, 0, 1, 0, 1],
    [0, 1, 0, 1, 1, 1, 0, 1],
    [0, 1, 1, 1, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 1],
    [0, 1, 0, 1, 0, 1, 0, 1],
    [0, 1, 0, 1, 1, 1, 0, 1],
    [0, 1, 1, 1, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 1],
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
    [8, 8, 8, 8, 8, 8, 8, 8],
    [8, 8, 8, 8, 8, 8, 8, 8],
    [8, 8, 8, 8, 8, 8, 8, 8],
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FmOperator {
    phase_counter: u32,
    env_phase: AdsrPhase,
    env_level: u16,
    key_on: bool,
    last_output: i16,
    last_output2: i16,
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

    fn set_key_on(&mut self, on: bool) {
        if on == self.key_on {
            return;
        }
        self.key_on = on;
        if on {
            self.phase_counter = 0;
            self.env_phase = AdsrPhase::Attack;
        } else {
            self.env_phase = AdsrPhase::Release;
        }
    }

    fn clock_phase(&mut self, fnum: u32, block: u8, detune: u8, multiple: u8) {
        let base_inc = (fnum << block) >> 1;
        let key_code = compute_key_code(fnum, block);
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
        };
        let increment = if multiple == 0 {
            detuned >> 1
        } else {
            detuned.wrapping_mul(multiple as u32)
        };
        self.phase_counter = (self.phase_counter.wrapping_add(increment)) & 0xFFFFF;
    }

    fn clock_envelope(&mut self, params: &EnvelopeParams) {
        let base_rate = match self.env_phase {
            AdsrPhase::Attack => params.ar,
            AdsrPhase::Decay => params.dr,
            AdsrPhase::Sustain => params.sr,
            AdsrPhase::Release => (params.rr << 1) | 1,
        };
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
            ((base_rate as u16 * 2) + (params.kc >> ks_shift) as u16).min(63) as u8
        };
        let shift = 11u8.saturating_sub(rate / 4);
        if rate >= 48 || (params.counter & ((1 << shift) - 1)) == 0 {
            let step_idx = ((params.counter >> shift) & 7) as usize;
            let increment = ENV_INCREMENT_TABLE[rate as usize][step_idx];
            if increment > 0 {
                match self.env_phase {
                    AdsrPhase::Attack => {
                        if rate >= 62 {
                            self.env_level = 0;
                        } else {
                            let delta = (increment as i32 * -((self.env_level as i32) + 1)) >> 4;
                            self.env_level = (self.env_level as i32 + delta).max(0) as u16;
                        }
                    }
                    _ => {
                        self.env_level = (self.env_level + increment as u16).min(0x3FF);
                    }
                }
            }
        }
        if self.env_phase == AdsrPhase::Attack && self.env_level == 0 {
            self.env_phase = AdsrPhase::Decay;
        }
        if self.env_phase == AdsrPhase::Decay && self.env_level >= ((params.sl as u16) << 5) {
            self.env_phase = AdsrPhase::Sustain;
        }
    }

    fn compute_output(&self, phase_mod: i16, total_level: u16) -> i16 {
        let total_atten = (self.env_level + (total_level << 3)).min(0x3FF);
        let phase = (((self.phase_counter >> 10) & 0x3FF) as i32 + phase_mod as i32) as u32;
        let sign = (phase >> 9) & 1;
        let table_idx = if phase & (1 << 8) == 0 {
            (phase & 0xFF) as usize
        } else {
            (!phase & 0xFF) as usize
        };
        let log_sine = LOG_SINE_TABLE[table_idx];
        let combined_atten = log_sine as u32 + ((total_atten as u32) << 2);
        if combined_atten >= 0x1E00 {
            return 0;
        }
        let linear =
            ((EXP_TABLE[(combined_atten & 0xFF) as usize] as u32) << 3) >> (combined_atten >> 8);
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
    fnum: u16,
    block: u8,
    fnum_latch: u8,
    last_sample: i16,
}

impl FmChannel {
    fn new() -> Self {
        Self {
            operators: std::array::from_fn(|_| FmOperator::new()),
            algorithm: 0,
            feedback: 0,
            panning_l: true,
            panning_r: true,
            fnum: 0,
            block: 0,
            fnum_latch: 0,
            last_sample: 0,
        }
    }

    fn clock(&mut self, regs: &[u8; 256], ch_off: usize, counter: u16) -> i16 {
        let op_offsets: [usize; 4] = [0, 8, 4, 12];
        let kc = compute_key_code(self.fnum as u32, self.block);
        for i in 0..4 {
            let off = op_offsets[i] + ch_off;
            self.operators[i].clock_phase(
                self.fnum as u32,
                self.block,
                (regs[0x30 + off] >> 4) & 7,
                regs[0x30 + off] & 0xF,
            );
            self.operators[i].clock_envelope(&EnvelopeParams {
                ar: regs[0x50 + off] & 0x1F,
                dr: regs[0x60 + off] & 0x1F,
                sr: regs[0x70 + off] & 0x1F,
                rr: regs[0x80 + off] & 0xF,
                sl: (regs[0x80 + off] >> 4) & 0xF,
                ks: (regs[0x50 + off] >> 6) & 3,
                kc,
                counter,
            });
        }
        let tl: [u16; 4] =
            std::array::from_fn(|i| (regs[0x40 + op_offsets[i] + ch_off] & 0x7F) as u16);
        let fb = if self.feedback > 0 {
            ((self.operators[0].last_output as i32 + self.operators[0].last_output2 as i32) >> 1)
                >> (9 - self.feedback as i32)
        } else {
            0
        } as i16;
        let out1 = self.operators[0].compute_output(fb, tl[0]);
        let (out2, out3, out4) = match self.algorithm {
            0 => {
                let o3 =
                    self.operators[2].compute_output(self.operators[1].last_output >> 1, tl[2]);
                (
                    self.operators[1].compute_output(out1 >> 1, tl[1]),
                    o3,
                    self.operators[3].compute_output(o3 >> 1, tl[3]),
                )
            }
            7 => (
                self.operators[1].compute_output(0, tl[1]),
                self.operators[2].compute_output(0, tl[2]),
                self.operators[3].compute_output(0, tl[3]),
            ),
            _ => (0, 0, self.operators[3].compute_output(0, tl[3])),
        };
        self.operators[0].last_output2 = self.operators[0].last_output;
        self.operators[0].last_output = out1;
        self.operators[1].last_output = out2;
        self.operators[2].last_output = out3;
        self.operators[3].last_output = out4;
        let channel_out = match self.algorithm {
            0 => out4,
            7 => out1
                .wrapping_add(out2)
                .wrapping_add(out3)
                .wrapping_add(out4),
            _ => out4,
        };
        self.last_sample = channel_out;
        channel_out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Bank {
    Bank0 = 0,
    Bank1 = 1,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Ym2612 {
    #[serde(with = "register_array")]
    pub registers: [[u8; 256]; 2],
    address: [u8; 2],
    pub status: u8,
    timer_a: i32,
    timer_b: i32,
    busy: i32,
    channels: [FmChannel; 6],
    dac_val: u8,
    dac_en: bool,
    env_counter: u16,
    pub total_clocks: u64,
    pub blip_l: BlipBuf,
    pub blip_r: BlipBuf,
    last_left: i32,
    last_right: i32,
}

impl Ym2612 {
    pub fn new() -> Self {
        let mut ym = Self {
            registers: [[0; 256]; 2],
            address: [0; 2],
            status: 0,
            timer_a: 0,
            timer_b: 0,
            busy: 0,
            channels: std::array::from_fn(|_| FmChannel::new()),
            dac_val: 0x80,
            dac_en: false,
            env_counter: 1,
            total_clocks: 0,
            blip_l: BlipBuf::new(7670453, 53267),
            blip_r: BlipBuf::new(7670453, 53267),
            last_left: 0,
            last_right: 0,
        };
        for i in 0..3 {
            ym.registers[0][0xB4 + i] = 0xC0;
            ym.registers[1][0xB4 + i] = 0xC0;
        }
        ym
    }

    pub fn reset(&mut self) {
        let (bl, br) = (self.blip_l.clone(), self.blip_r.clone());
        *self = Self::new();
        self.blip_l = bl;
        self.blip_r = br;
    }

    pub fn read_status(&self) -> u8 {
        let mut res = self.status;
        if self.busy > 0 {
            res |= 0x80;
        }
        res
    }
    pub fn read(&self, _p: u8) -> u8 {
        self.read_status()
    }

    pub fn step(&mut self, cycles: u32) {
        let mclks = (cycles * 7) as i32;
        if self.busy > 0 {
            self.busy -= mclks;
        }

        // Timer Logic
        // Timer A period: (1024 - NA) * 72 master clocks
        // Timer B period: (256 - NB) * 1152 master clocks
        for _ in 0..mclks {
            if (self.registers[0][0x27] & 0x01) != 0 {
                self.timer_a -= 1;
                if self.timer_a <= 0 {
                    let n = ((self.registers[0][0x24] as u32) << 2)
                        | (self.registers[0][0x25] as u32 & 0x03);
                    self.timer_a = (1024 - n as i32) * 72;
                    if (self.registers[0][0x27] & 0x04) != 0 {
                        self.status |= 0x01;
                    }
                }
            }
            if (self.registers[0][0x27] & 0x02) != 0 {
                self.timer_b -= 1;
                if self.timer_b <= 0 {
                    self.timer_b = (256 - self.registers[0][0x26] as i32) * 1152;
                    if (self.registers[0][0x27] & 0x08) != 0 {
                        self.status |= 0x02;
                    }
                }
            }
        }

        if cycles > 0 {
            self.total_clocks += cycles as u64;
            self.env_counter = (self.env_counter + 1) & 0xFFF;
            let mut left = 0i32;
            let mut right = 0i32;
            for i in 0..6 {
                let out = if i == 5 && self.dac_en {
                    (self.dac_val as i32 - 128) << 6
                } else {
                    self.channels[i].clock(
                        &self.registers[if i < 3 { 0 } else { 1 }],
                        i % 3,
                        self.env_counter,
                    ) as i32
                };
                if self.channels[i].panning_l {
                    left += out;
                }
                if self.channels[i].panning_r {
                    right += out;
                }
            }
            let dl = left - self.last_left;
            if dl != 0 {
                self.blip_l.add_delta(self.total_clocks, dl);
                self.last_left = left;
            }
            let dr = right - self.last_right;
            if dr != 0 {
                self.blip_r.add_delta(self.total_clocks, dr);
                self.last_right = right;
            }
        }
    }

    pub fn write_address(&mut self, p: u8, v: u8) {
        self.address[(p & 1) as usize] = v;
    }
    pub fn write_addr(&mut self, b: Bank, v: u8) {
        self.address[b as usize] = v;
    }
    pub fn write_data(&mut self, p: u8, v: u8) {
        let b = (p & 1) as usize;
        self.write_data_bank(if b == 0 { Bank::Bank0 } else { Bank::Bank1 }, v);
    }
    pub fn write_data_bank(&mut self, b: Bank, v: u8) {
        self.busy = 224;
        let bank_idx = b as usize;
        let a = self.address[bank_idx];
        self.registers[bank_idx][a as usize] = v;
        match (b, a) {
            (Bank::Bank0, 0x28) => {
                let c = match v & 7 {
                    0..=2 => v & 7,
                    4..=6 => (v & 7) - 1,
                    _ => 7,
                } as usize;
                if c < 6 {
                    for i in 0..4 {
                        self.channels[c].operators[i].set_key_on((v & (0x10 << i)) != 0);
                    }
                }
            }
            (Bank::Bank0, 0x27) => {
                if (v & 0x10) != 0 {
                    self.status &= !0x01;
                }
                if (v & 0x20) != 0 {
                    self.status &= !0x02;
                }
            }
            (Bank::Bank0, 0x2A) => self.dac_val = v,
            (Bank::Bank0, 0x2B) => self.dac_en = (v & 0x80) != 0,
            (_, 0xB4..=0xB6) => {
                let c = (a - 0xB4) as usize + bank_idx * 3;
                if c < 6 {
                    self.channels[c].panning_l = (v & 0x80) != 0;
                    self.channels[c].panning_r = (v & 0x40) != 0;
                }
            }
            _ => {}
        }
    }

    pub fn generate_sample(&mut self) -> (i16, i16) {
        let mut l = [0i16; 1];
        let mut r = [0i16; 1];
        if self.blip_l.read_samples(&mut l[..]) > 0 {
            self.blip_r.read_samples(&mut r[..]);
            (l[0], r[0])
        } else {
            (self.blip_l.read_instant(), self.blip_r.read_instant())
        }
    }

    pub fn generate_channel_samples(&mut self) -> [i16; 6] {
        std::array::from_fn(|i| self.channels[i].last_sample)
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
