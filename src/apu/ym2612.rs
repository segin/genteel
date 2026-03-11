//! Refactored to use band-limited synthesis via BlipBuf for high quality.

use crate::apu::Bank;
use crate::apu::blip_buf::BlipBuf;
use serde::{Deserialize, Serialize};

/* ========================================================================= */
/*  Helper Tables                                                            */
/* ========================================================================= */

static SINE_TABLE: std::sync::LazyLock<[i16; 1024]> = std::sync::LazyLock::new(|| {
    let mut table = [0i16; 1024];
    for i in 0..1024 {
        table[i] = (f64::sin((i as f64 + 0.5) * std::f64::consts::PI / 512.0) * 4095.0) as i16;
    }
    table
});

static TL_TABLE: std::sync::LazyLock<[u16; 4096]> = std::sync::LazyLock::new(|| {
    let mut table = [0u16; 4096];
    for i in 0..4096 {
        table[i] = (f64::powf(2.0, (i as f64) * -1.0 / 256.0) * 4095.0) as u16;
    }
    table
});

/* ========================================================================= */
/*  FM Operator                                                              */
/* ========================================================================= */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum AdsrPhase { Attack, Decay, Sustain, Release }

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
        if on == self.key_on { return; }
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
        let dt = if dt_mag == 0 { 0 } else { (key_code as u32 * dt_mag as u32) >> 1 };
        let mut inc = if dt_sign != 0 { base_inc.wrapping_sub(dt) } else { base_inc.wrapping_add(dt) };
        if multiple > 0 { inc *= multiple as u32; } else { inc >>= 1; }
        self.phase_counter = self.phase_counter.wrapping_add(inc);
    }

    fn clock_envelope(&mut self, reg: &[u8], op_idx: usize) {
        let ar = reg[0x50 + op_idx] & 0x1F;
        let dr = reg[0x60 + op_idx] & 0x1F;
        let sr = reg[0x70 + op_idx] & 0x1F;
        let sl = (reg[0x80 + op_idx] >> 4) & 0x0F;
        let rr = reg[0x80 + op_idx] & 0x0F;

        match self.env_phase {
            AdsrPhase::Attack => {
                if ar == 31 { self.env_level = 0; self.env_phase = AdsrPhase::Decay; }
                else if self.env_level > 0 {
                    let step = (32 - ar as u16) * 4;
                    self.env_level = self.env_level.saturating_sub(step);
                } else { self.env_phase = AdsrPhase::Decay; }
            }
            AdsrPhase::Decay => {
                let limit = (sl as u16) << 5;
                if self.env_level < limit {
                    let step = (32 - dr as u16) / 2;
                    self.env_level = self.env_level.saturating_add(step.max(1));
                } else { self.env_phase = AdsrPhase::Sustain; }
            }
            AdsrPhase::Sustain => {
                if self.env_level < 0x3FF {
                    let step = (32 - sr as u16) / 4;
                    self.env_level = self.env_level.saturating_add(step.max(1));
                }
            }
            AdsrPhase::Release => {
                if self.env_level < 0x3FF {
                    let step = (16 - rr as u16) * 4;
                    self.env_level = self.env_level.saturating_add(step.max(1));
                }
            }
        }
    }

    fn get_output(&mut self, mod_in: i32, tl: u8, feedback: u8) -> i16 {
        let phase = ((self.phase_counter >> 10) as i32 + mod_in) & 0x3FF;
        let sine = SINE_TABLE[phase as usize] as i32;
        let level = (self.env_level as i32 + ((tl as i32) << 3)).clamp(0, 4095);
        let amp = (sine * TL_TABLE[level as usize] as i32) >> 12;
        
        let out = if feedback > 0 {
            let fb_out = (self.last_output as i32 + self.last_output2 as i32) >> (8 - feedback);
            self.last_output2 = self.last_output;
            self.last_output = amp as i16;
            (amp + fb_out).clamp(-32768, 32767) as i16
        } else {
            amp as i16
        };
        out
    }
}

/* ========================================================================= */
/*  FM Channel                                                               */
/* ========================================================================= */

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FmChannel {
    operators: [FmOperator; 4],
    pub panning_l: bool,
    pub panning_r: bool,
    pub last_sample: i16,
}

impl FmChannel {
    fn new() -> Self {
        Self {
            operators: std::array::from_fn(|_| FmOperator::new()),
            panning_l: true, panning_r: true, last_sample: 0,
        }
    }

    fn clock(&mut self, reg: &[u8], ch_idx: usize, env_clk: u16) -> i16 {
        let fnum = ((reg[0xA4 + ch_idx] as u32 & 0x07) << 8) | reg[0xA0 + ch_idx] as u32;
        let block = (reg[0xA4 + ch_idx] >> 3) & 0x07;
        let algo = reg[0xB0 + ch_idx] & 0x07;
        let fb = (reg[0xB0 + ch_idx] >> 3) & 0x07;

        for i in 0..4 {
            let op_idx = ch_idx + i * 4;
            let dt = (reg[0x30 + op_idx] >> 4) & 0x07;
            let multi = reg[0x30 + op_idx] & 0x0F;
            self.operators[i].clock_phase(fnum, block, dt, multi);
            if (env_clk % 3) == 0 { self.operators[i].clock_envelope(reg, op_idx); }
        }

        let out = match algo {
            0 => { // Op1 -> Op2 -> Op3 -> Op4 -> Out
                let o1 = self.operators[0].get_output(0, reg[0x40+ch_idx], fb);
                let o2 = self.operators[1].get_output(o1 as i32, reg[0x40+ch_idx+4], 0);
                let o3 = self.operators[2].get_output(o2 as i32, reg[0x40+ch_idx+8], 0);
                self.operators[3].get_output(o3 as i32, reg[0x40+ch_idx+12], 0)
            }
            1 => { // (Op1 + Op2) -> Op3 -> Op4 -> Out
                let o1 = self.operators[0].get_output(0, reg[0x40+ch_idx], fb);
                let o2 = self.operators[1].get_output(0, reg[0x40+ch_idx+4], 0);
                let o3 = self.operators[2].get_output(o1 as i32 + o2 as i32, reg[0x40+ch_idx+8], 0);
                self.operators[3].get_output(o3 as i32, reg[0x40+ch_idx+12], 0)
            }
            2 => { // Op1 -> (Op2 + Op3) -> Op4 -> Out
                let o1 = self.operators[0].get_output(0, reg[0x40+ch_idx], fb);
                let o2 = self.operators[1].get_output(o1 as i32, reg[0x40+ch_idx+4], 0);
                let o3 = self.operators[2].get_output(0, reg[0x40+ch_idx+8], 0);
                self.operators[3].get_output(o2 as i32 + o3 as i32, reg[0x40+ch_idx+12], 0)
            }
            3 => { // (Op1 -> Op2) + (Op3) -> Op4 -> Out
                let o1 = self.operators[0].get_output(0, reg[0x40+ch_idx], fb);
                let o2 = self.operators[1].get_output(o1 as i32, reg[0x40+ch_idx+4], 0);
                let o3 = self.operators[2].get_output(0, reg[0x40+ch_idx+8], 0);
                self.operators[3].get_output(o2 as i32 + o3 as i32, reg[0x40+ch_idx+12], 0)
            }
            4 => { // (Op1 -> Op2) + (Op3 -> Op4) -> Out
                let o1 = self.operators[0].get_output(0, reg[0x40+ch_idx], fb);
                let o2 = self.operators[1].get_output(o1 as i32, reg[0x40+ch_idx+4], 0);
                let o3 = self.operators[2].get_output(0, reg[0x40+ch_idx+8], 0);
                let o4 = self.operators[3].get_output(o3 as i32, reg[0x40+ch_idx+12], 0);
                o2 / 2 + o4 / 2
            }
            5 => { // Op1 -> (Op2 + Op3 + Op4) -> Out
                let o1 = self.operators[0].get_output(0, reg[0x40+ch_idx], fb);
                let o2 = self.operators[1].get_output(o1 as i32, reg[0x40+ch_idx+4], 0);
                let o3 = self.operators[2].get_output(o1 as i32, reg[0x40+ch_idx+8], 0);
                let o4 = self.operators[3].get_output(o1 as i32, reg[0x40+ch_idx+12], 0);
                (o2 as i32 + o3 as i32 + o4 as i32).clamp(-32768, 32767) as i16
            }
            6 => { // (Op1 -> Op2) + Op3 + Op4 -> Out
                let o1 = self.operators[0].get_output(0, reg[0x40+ch_idx], fb);
                let o2 = self.operators[1].get_output(o1 as i32, reg[0x40+ch_idx+4], 0);
                let o3 = self.operators[2].get_output(0, reg[0x40+ch_idx+8], 0);
                let o4 = self.operators[3].get_output(0, reg[0x40+ch_idx+12], 0);
                (o2 as i32 + o3 as i32 + o4 as i32).clamp(-32768, 32767) as i16
            }
            _ => { // All separate
                let o1 = self.operators[0].get_output(0, reg[0x40+ch_idx], fb);
                let o2 = self.operators[1].get_output(0, reg[0x40+ch_idx+4], 0);
                let o3 = self.operators[2].get_output(0, reg[0x40+ch_idx+8], 0);
                let o4 = self.operators[3].get_output(0, reg[0x40+ch_idx+12], 0);
                (o1 as i32 + o2 as i32 + o3 as i32 + o4 as i32).clamp(-32768, 32767) as i16
            }
        };
        self.last_sample = out;
        out
    }
}

/* ========================================================================= */
/*  YM2612 Core                                                              */
/* ========================================================================= */

#[derive(Debug, Serialize, Deserialize)]
pub struct Ym2612 {
    pub registers: Vec<Vec<u8>>,
    address: [u8; 2],
    pub status: u8,
    timer_a: i32, timer_b: i32, busy: i32,
    channels: [FmChannel; 6],
    dac_val: u8, dac_en: bool,
    env_counter: u16,
    pub total_clocks: u64,
    pub blip_l: BlipBuf,
    pub blip_r: BlipBuf,
    last_left: i32,
    last_right: i32,
    pub clock_accumulator: f32,
}

impl Ym2612 {
    pub fn new() -> Self {
        let mut ym = Self {
            registers: vec![vec![0; 256]; 2], address: [0; 2], status: 0, timer_a: 0, timer_b: 0, busy: 0,
            channels: std::array::from_fn(|_| FmChannel::new()), dac_val: 0x80, dac_en: false,
            env_counter: 1, total_clocks: 0,
            blip_l: BlipBuf::new(53267, 53267), blip_r: BlipBuf::new(53267, 53267),
            last_left: 0, last_right: 0,
            clock_accumulator: 0.0,
        };
        for i in 0..3 { ym.registers[0][0xB4+i] = 0xC0; ym.registers[1][0xB4+i] = 0xC0; }
        ym
    }

    pub fn reset(&mut self) { 
        let (bl, br) = (self.blip_l.clone(), self.blip_r.clone()); 
        *self = Self::new(); 
        self.blip_l = bl; self.blip_r = br; 
        self.blip_l.clear(); self.blip_r.clear();
    }

    pub fn read_status(&self) -> u8 {
        let mut res = self.status;
        if self.busy > 0 { res |= 0x80; }
        res
    }
    pub fn read(&self, _p: u8) -> u8 { self.read_status() }

    pub fn step(&mut self, m68k_cycles: u32) {
        let mclks = (m68k_cycles * 7) as i32;
        if self.busy > 0 { self.busy -= mclks; }
        
        // Timer Logic (Master Clock / 72 or 1152)
        for _ in 0..mclks {
            if (self.registers[0][0x27] & 0x01) != 0 {
                self.timer_a -= 1;
                if self.timer_a <= 0 {
                    let n = ((self.registers[0][0x24] as u32) << 2) | (self.registers[0][0x25] as u32 & 0x03);
                    self.timer_a = (1024 - n as i32) * 72;
                    if (self.registers[0][0x27] & 0x04) != 0 { self.status |= 0x01; }
                }
            }
            if (self.registers[0][0x27] & 0x02) != 0 {
                self.timer_b -= 1;
                if self.timer_b <= 0 {
                    self.timer_b = (256 - self.registers[0][0x26] as i32) * 1152;
                    if (self.registers[0][0x27] & 0x08) != 0 { self.status |= 0x02; }
                }
            }
        }

        // FM clock is Master Clock / 144 (approx 53267 Hz)
        // One FM clock per 144 master clocks.
        // m68k_cycles is 7 master clocks.
        self.clock_accumulator += (m68k_cycles as f32 * 7.0) / 144.0;

        while self.clock_accumulator >= 1.0 {
            self.total_clocks += 1;
            self.clock_accumulator -= 1.0;
            self.env_counter = (self.env_counter + 1) & 0xFFF;
            let mut left = 0i32; let mut right = 0i32;
            for i in 0..6 {
                let out = if i == 5 && self.dac_en { (self.dac_val as i32 - 128) << 6 }
                else { self.channels[i].clock(&self.registers[if i < 3 { 0 } else { 1 }], i % 3, self.env_counter) as i32 };
                if self.channels[i].panning_l { left += out; }
                if self.channels[i].panning_r { right += out; }
            }
            let dl = left - self.last_left;
            if dl != 0 { self.blip_l.add_delta(self.total_clocks, dl); self.last_left = left; }
            let dr = right - self.last_right;
            if dr != 0 { self.blip_r.add_delta(self.total_clocks, dr); self.last_right = right; }
        }
    }

    pub fn write_address(&mut self, p: u8, v: u8) { self.address[(p&1) as usize] = v; }
    pub fn write_addr(&mut self, b: Bank, v: u8) { self.address[b as usize] = v; }
    pub fn write_data(&mut self, p: u8, v: u8) { let b = (p&1) as usize; self.write_data_bank(if b==0 { Bank::Bank0 } else { Bank::Bank1 }, v); }
    pub fn write_data_bank(&mut self, b: Bank, v: u8) {
        self.busy = 224 * 7;
        let bank_idx = b as usize; let a = self.address[bank_idx];
        self.registers[bank_idx][a as usize] = v;
        match (b, a) {
            (Bank::Bank0, 0x28) => {
                let c = match v & 7 { 0..=2 => v&7, 4..=6 => (v&7)-1, _ => 7 } as usize;
                if c < 6 { for i in 0..4 { self.channels[c].operators[i].set_key_on((v & (0x10 << i)) != 0); } }
            }
            (Bank::Bank0, 0x2A) => self.dac_val = v,
            (Bank::Bank0, 0x2B) => self.dac_en = (v & 0x80) != 0,
            (_, 0xB4..=0xB6) => { let c = (a-0xB4) as usize + bank_idx*3; if c < 6 { self.channels[c].panning_l = (v&0x80)!=0; self.channels[c].panning_r = (v&0x40)!=0; } }
            _ => {}
        }
    }

    pub fn generate_sample(&mut self) -> (i16, i16) {
        let mut l = [0i16; 1]; let mut r = [0i16; 1];
        if self.blip_l.read_samples(&mut l) > 0 {
            self.blip_r.read_samples(&mut r);
            self.total_clocks = 0;
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
    let f11 = (f >> 10) & 1; let f10 = (f >> 9) & 1; let f9 = (f >> 8) & 1; let f8 = (f >> 7) & 1;
    let bit0 = (f11 & (f10 | f9 | f8)) | ((1 - f11) & f10 & f9 & f8);
    ((b << 2) as u32 | (f11 << 1) | bit0) as u8
}
