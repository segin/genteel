# YM2612 (OPN2) — Bus/Port/Timing State Machines (Debugger/Cycle-Accurate Spec)

## Scope
Defines the externally-visible port behavior and timing-facing internal state machines:
- Address/data port selection and mirroring profiles (discrete YM2612 vs YM3438-style)
- BUSY flag generation and write-accept rules
- Register write commit scheduling (address->data pairing, group selection)
- Timer A/B and CSM behavior
- LFO tick scheduling and divider table

This is intended to be driven by an emulator-wide scheduler that can tick the YM core once per "internal YM cycle".

---

## 0) Timebases and derived tick functions

### 0.1 External clocks
- YM master clock = Genesis 68000 clock (NTSC ~7.67 MHz, PAL slightly lower)
- Internal YM cycle frequency = master_clock / 6
- One "sample period" = 24 internal YM cycles (~53267 Hz NTSC equivalent)

Define:
- tick_internal(): called once per internal YM cycle
- tick_sample(): called once every 24 internal cycles (at sample boundary)

Optionally define a finer per-sample subcycle counter:
- sub = internal_cycle_counter % 24

---

## 1) Address/Data Port Front-End

### 1.1 Z80-visible ports
- Address ports: $4000 (select group 1), $4002 (select group 2)
- Data port: mapped to BOTH $4001 and $4003 (one physical data port)

State:
- selected_group ∈ {G1, G2}  // last written address port
- latched_addr[G1], latched_addr[G2] : u8  // last address written per group (optional)
- pending_addr_write : { none | (group, addr) }  // if you want to model commit timing

Actions:
- write($4000, addr):
    selected_group = G1
    latched_addr[G1] = addr
    addr_write_timestamp = now
- write($4002, addr):
    selected_group = G2
    latched_addr[G2] = addr
    addr_write_timestamp = now
- write($4001 or $4003, data):
    target_group = selected_group
    accept as "data write" to (target_group, latched_addr[target_group], data) if allowed by BUSY policy

### 1.2 Read port
- read($4000) returns status bits:
    bit7 BUSY
    bit1 timerB_overflow
    bit0 timerA_overflow
  Other bits undefined (implementation choice; usually 0)

### 1.3 Read mirroring profiles
Provide at least two hardware profiles:

PROFILE_YM3438_STYLE:
- read($4001/$4002/$4003) == read($4000)

PROFILE_DISCRETE_YM2612_STYLE:
- reads at $4001/$4002/$4003 are "undefined"; emulate common observed behavior:
    returns last value read from $4000, but decays toward 0 over time

State for DISCRETE behavior:
- last_status_read : u8
- last_status_read_time : time
- decay_constant : time  // e.g., around "quarter second worth of cycles" tuned to match recordings

Implementation sketch:
- read($4000): compute new_status, set last_status_read=new_status, last_status_read_time=now, return new_status
- read($4001/$4002/$4003): return decay(last_status_read, now - last_status_read_time)

---

## 2) BUSY Flag + Write Acceptance

### 2.1 Simplified but robust BUSY model (recommended baseline)
State:
- busy_until : time  // earliest time when BUSY becomes 0

Rules:
- BUSY is 1 iff now < busy_until
- On each accepted DATA write (not address write), set:
    busy_until = max(busy_until, now + busy_hold_time)

Where busy_hold_time is hardware-profile dependent:
- Common practical value: 32 internal YM cycles (baseline)
- Optionally vary by register class or internal subcycle phase

### 2.2 Documentation-derived wait-cycle gating (optional "strict timing" mode)
If you want a more spec-driven write-spacing rule set (especially YM3438-like):
- Require minimum spacing between writes:
    - address->data: >= 17 YM cycles
    - data->data (regs $30-$9E): >= 83 YM cycles
    - data->data (regs $A0-$B6): >= 47 YM cycles
- Implement by stalling/ignoring writes that violate spacing OR by keeping BUSY asserted until spacing is met.

State:
- last_addr_write_time
- last_data_write_time
- last_data_write_reg_class

---

## 3) Register File Write Commit

### 3.1 Core register file
State:
- regs[G1][0x00..0xFF], regs[G2][0x00..0xFF] // not all used; keep for debugging
- plus decoded channel/operator state derived from these regs

### 3.2 Deferred-apply register semantics (must emulate)
Some register writes do not take effect immediately in the analog sense; they update latched fields and are applied on specific follow-up writes or at sample boundaries.

Minimum required deferred behavior:
- Channel frequency high/block ($A4-$A6) does not take effect until paired low write ($A0-$A2) occurs.
  (Applies per group/channels; channel 3 special mode alters mapping.)

State:
- pending_fnum_high_block[group][chan] : {valid, value}
- pending_fnum_low[group][chan] : {valid, value}

On write to $A4-$A6:
- store into pending_fnum_high_block
- do NOT update phase increment yet

On write to $A0-$A2:
- store low; then atomically apply:
    fnum = combine(high3, low8)
    block = high_block_bits
    update phase increment inputs for channel/operator mapping

Timer A interval writes are NOT latched in the same "apply on low write" manner; changing either half
updates interval bits immediately but only affects counter on next reload (overflow or LOAD edge).

---

## 4) Timers A/B + Overflow Flags + CSM

### 4.1 Registers
- $24: Timer A interval high 8 bits
- $25: Timer A interval low 2 bits (bits0-1)
- $26: Timer B interval 8 bits
- $27: Timer control:
    bit0 LOAD_A (enable/load)
    bit1 LOAD_B
    bit2 ENABLE_A_OVERFLOW_FLAG
    bit3 ENABLE_B_OVERFLOW_FLAG
    bit4 CLR_A_FLAG (write 1 clears)
    bit5 CLR_B_FLAG
    bits6-7 mode (CH3 special freq modes and CSM)

### 4.2 Tick rates
- Timer A ticks once per SAMPLE (i.e., once per 24 internal cycles)
- Timer B ticks once every 16 samples (internal divider)

State per timer:
- interval (A: 10-bit, B: 8-bit)
- counter  (A: 10-bit, B: 8-bit)
- overflow_flag (sticky until cleared)
- overflow_flag_enabled (bit2/bit3)
- load_bit (bit0/bit1)
- (B only) divider_16 (0..15)

Rules:
- Timers only tick when corresponding LOAD bit is 1.
- On LOAD transition 0->1: reload counter = interval immediately (optionally with 1-sample delay, see below).
- Each tick: counter = (counter + 1) mod 2^N; if wraps to 0:
    counter = interval (reload)
    overflow_flag |= overflow_flag_enabled

Flag clearing:
- Writing 1 to $27 bit4 clears A overflow_flag
- Writing 1 to $27 bit5 clears B overflow_flag

Optional timing refinement:
- 1-sample delay on LOAD bit changes taking effect (due to internal latch point).
  Provide as hardware-profile switch.

### 4.3 CSM
- Enabled when ($27 & 0xC0) == 0x80 (bit7=1, bit6=0)
- When Timer A overflows and LOAD_A=1:
    auto key on then key off all 4 operators of Channel 3 (in a way that does not disrupt already-on operators)
- This is mainly relevant for test ROMs; implement deterministically at the Timer A overflow event.

---

## 5) LFO State Machine

### 5.1 Registers
- $22:
    bits0-2 LFO frequency select (0..7)
    bit3 LFO enable

### 5.2 Tick cadence
- LFO counter is 7-bit (0..127)
- LFO "divider" counts samples; when it reaches divider_table[freq], increment counter and reset divider.

Divider table (samples per LFO-counter increment):
    [108, 77, 71, 67, 62, 44, 8, 5]

State:
- lfo_enabled
- lfo_freq_sel
- lfo_div : u16
- lfo_counter : u8 (7-bit)

Rules:
- If LFO enable bit transitions 1->0: lfo_counter=0 and hold at 0 until re-enabled.
- On each sample tick (not each internal cycle):
    if lfo_enabled:
        lfo_div += 1
        if lfo_div >= DIV[lfo_freq_sel]:
            lfo_div = 0
            lfo_counter = (lfo_counter + 1) & 0x7F

The LFO counter value is then used by:
- Vibrato (phase increment adjustment) via PMS per channel
- Tremolo (envelope attenuation adjustment) via AMS per channel and AM-enable per operator

---

## 6) Power-on reset defaults (high impact)
At minimum:
- Panning bits (regs $B4-$B6 bits7/6) should default to BOTH enabled (L=1, R=1) for all channels.
- Timers disabled; overflow flags clear; BUSY low; LFO disabled and counter=0.
- DAC disabled.

(Exact defaults can be hardware-profile-specific; expose in debugger UI.)

---

# YM2612 (OPN2) — FM Core Micro-Ops and State Machines (Per-Sample, With Pipeline Delays)

## Scope
Defines:
- Operator state machines (phase + envelope + SSG-EG)
- Per-sample evaluation order (operators 1->3->2->4)
- Algorithm routing and the known "delayed modulator output" edges (previous-sample modulation)
- Feedback computation for operator 1 (uses last two outputs)
- DAC mode (Channel 6 replace)
- 9-bit DAC quantization (carrier quantization, not channel-only)
- Ladder effect (discrete YM2612-style analog artifact model)
- Optional "multiplexed DAC slots" model vs "mixed channels" model

This spec assumes the bus/port/timers/LFO states from the companion document.

---

## 0) Core data structures

### 0.1 Indexing
- 6 channels: ch = 0..5
- 4 operators per channel: op = 0..3 (op1, op2, op3, op4)
- Evaluation order (hardware): [op1, op3, op2, op4]  => [0,2,1,3]

### 0.2 Operator state
For each operator:
- phase_counter : u32 (sufficient width; output is top 10 bits)
- phase_increment : u32 (computed from fnum/block/mul/detune + LFO vibrato)
- env_phase : {ATTACK, DECAY, SUSTAIN, RELEASE}
- env_level : u16 (10-bit attenuation 0..0x3FF, log2 4.6 scale)
- total_level : u16 (TL contribution, log-domain)
- rates: AR, DR, SR, RR; plus RS (rate scaling) and KSL (key scaling)
- ssg_enabled : bool
- ssg_attack, ssg_hold, ssg_alternate : bool
- ssg_invert_flag : bool (internal; toggles at attenuation threshold logic)
- key_on : bool

Operator history:
- last_output : i16 (signed 14-bit)
- last_output2 : i16 (signed 14-bit)  // for op1 feedback averaging

### 0.3 Channel state
For each channel:
- algorithm : u3 (0..7)
- feedback : u3 (0..7)
- panning_L, panning_R : bool
- ams : u2 (AM depth)
- pms : u3 (PM depth)
- fnum, block : current base frequency (or per-operator if CH3 special mode)
- ch3_special_mode : enum {OFF, MODE1, MODE2, MODE3/CSM}  // from $27 bits6-7
- dac_enable (channel 6 only): bool
- dac_sample_u8 (channel 6 only): u8 (from $2A)

Audio mixing:
- out_L, out_R accumulators (prefer i32)
- per-channel_output_14bit (debugging tap)
- per-carrier_output_14bit (debugging tap)

---

## 1) Sample boundary scheduler (24 internal cycles)

The YM core produces one logical "sample output" per 24 internal cycles.

Recommended implementation:
- tick_internal(): increments internal_cycle; if internal_cycle % 24 == 0 => tick_sample()

Optional "multiplex model":
- In hardware, the chip cycles channels at ~1 channel per 4 internal cycles and multiplexes DAC.
- For debug-level accuracy of ladder effect and panning noise, you may model 4 "DAC slots" per channel.

---

## 2) Per-sample update ordering (tick_sample)

At each sample tick:
1) timers_tick() (Timer A always; Timer B via /16 divider)
2) lfo_tick() (if enabled; updates 7-bit counter)
3) for each channel ch:
     channel_clock(ch)  // computes next channel output sample (digital)
4) output_stage()  // apply DAC mode override, panning, quantization, ladder, filtering
5) commit histories (operator last_output buffers)

Notes:
- Many internal effects depend only on sample cadence; but register write commits may occur mid-sample if you
  implement finer internal-cycle latching.

---

## 3) Phase generator (per operator, per sample)

### 3.1 Base phase increment
Inputs:
- fnum (11-bit), block (3-bit), detune (3-bit), multiple (4-bit), pms + LFO counter

Compute phase_increment such that:
- phase_counter advances by phase_increment each sample tick (or per-operator clock)
- phase_output = top 10 bits of phase_counter (0..1023) representing [0, 2π)

### 3.2 F-num high/block latch rule (must emulate)
For normal channels:
- writes to FNUM_HIGH/BLOCK regs do not apply until the paired FNUM_LOW write occurs.
- On FNUM_LOW write: atomically apply both most-recent high and low.

For Channel 3 special mode:
- fnum/block can be per-operator (register mapping differs); treat each op's base freq separately.

### 3.3 Key-on phase reset
On key_on transition 0->1 for an operator:
- phase_counter = 0
On key off: phase_counter unchanged.

---

## 4) Envelope generator (EG) core (per operator, per sample)

### 4.1 EG output and meaning
- env_level is attenuation in log2 scale (0=full volume, 0x3FF ~ silence)
- TL is added in log domain (additional attenuation)

### 4.2 ADSR transitions
- Key-on triggers ATTACK (subject to AR=0 special behavior).
- When env_level reaches 0: transition to DECAY.
- DECAY increases attenuation until reaching sustain level (SL), then SUSTAIN.
- Key-off triggers RELEASE.
- RELEASE increases attenuation toward 0x3FF; clamp at 0x3FF.

### 4.3 Rate scaling and key scaling
- Compute key_code from block and fnum (and detune interactions) and use RS/KSL to adjust rates.

---

## 5) SSG-EG mode (operator, per sample)

SSG-EG is enabled via $90-$9F per operator and modifies how env_level is interpreted and updated once it crosses 0x200.

Key behaviors:
- SSG logic does nothing until internal attenuation >= 0x200.
- HOLD:
    when attenuation reaches 0x200, on the NEXT sample it jumps to 0x3FF and stays there (if not inverted).
- ALTERNATE:
    toggles an internal inversion flag each time internal attenuation hits 0x200.
    Output is inverted when (ssg_attack XOR ssg_invert_flag) is true.
- ATTACK bit:
    acts as XOR against inversion flag, producing globally inverted output shapes.

Output inversion transform:
- if invert_output:
      output_level = (0x200 - internal_level) & 0x3FF
  else output_level = internal_level

Phase-generator reset coupling:
- When both ALTERNATE and HOLD are clear, reaching 0x200 resets the phase counter to 0.
- If HOLD or ALTERNATE is set, do NOT reset phase counter at 0x200.

---

## 6) Operator output computation (log-sine + exp), per operator clock

Given:
- phase_output_10bit (after applying phase modulation)
- env_output_level_10bit (after SSG inversion logic) + TL + AM tremolo adjustment

Compute signed 14-bit operator output:
1) log_sine = quarter-wave lookup using phase (with mirror/sign handling)
2) combined_atten = log_sine + (env_level << alignment_shift)  // log-domain add
3) exp = base2_exponentiation lookup
4) apply sign bit

---

## 7) Phase modulation (PM) and evaluation ordering

### 7.1 PM uses bits 1-10 of modulator sums
- Sum modulator outputs (signed) where an operator has multiple modulators.
- Use bits 1-10 of the sum as phase offset to the carrier's phase (typically implemented as >>1 scaling).

### 7.2 Operator evaluation order and pipeline delays (must emulate)
Hardware evaluates operators in order: 1 -> 3 -> 2 -> 4.

This creates two delay mechanisms:
A) Order-based delay:
   If operator 2 modulates operator 3, operator 3 cannot see operator 2's new output this sample.
B) Pipeline delay:
   Even if A and B are consecutive in evaluation order, A's new output may not be available when B
   reads modulators, so B uses A's previous-sample output.

Implement by storing both:
- op.out_prev (last sample)
- op.out_curr (this sample, once computed)

And, per algorithm, choosing whether a modulation edge uses prev or curr.

### 7.3 Delayed modulation edges by algorithm (hard list)
For these edges, the *modulated operator must use the modulator's previous-sample output*:

- Algorithm 0: op2 -> op3
- Algorithm 1: op1 -> op3 AND op2 -> op3
- Algorithm 2: op2 -> op3
- Algorithm 3: op2 -> op4
- Algorithm 5: op1 -> op3

Algorithms 4, 6, 7: no delays required beyond normal eval ordering.

Implementation rule:
- For each modulation input edge (mod -> dst), consult delayed_edge_table[algorithm][mod,dst]
  to select mod_value = mod.out_prev instead of mod.out_curr.

---

## 8) Operator 1 feedback (FB)

Operator 1 can self-modulate via feedback instead of having explicit modulators.

Feedback value when feedback_level != 0:
    fb_val = (op1.last_output + op1.last_output2) >> (10 - feedback_level)

Notes:
- Uses average of last two outputs (sum then extra >>1 baked into shift).
- Be careful whether you already applied the ">>1 uses bits 1-10" scaling globally; if you do,
  shift is (9 - feedback_level) instead.

Apply feedback as op1's phase_offset input.

Update history each sample:
- op1.last_output2 = op1.last_output
- op1.last_output  = op1.out_curr

---

## 9) Channel algorithm evaluation (carrier/mod routing)

For each algorithm 0..7, define which operators are carriers and which are modulators.
Evaluation uses:
- operator order [1,3,2,4]
- delayed modulation edges table (§7.3)
- feedback injection (§8)

Algorithm engine per channel:
1) Determine modulation inputs for each operator:
   - op1 uses feedback (if any), else 0
   - others use sums of designated modulators (some edges delayed)
2) Compute operators in eval order, producing out_curr
3) Determine carriers per algorithm and compute channel_out as sum(carriers)

Clamp:
- Sum of carriers should clamp to signed 14-bit (not overflow wrap).

---

## 10) DAC Mode (Channel 6 override)

Registers:
- $2B bit7 enables DAC
- $2A provides unsigned 8-bit sample

When dac_enable==1:
- Channel 6 FM digital output is computed as usual internally, but final output is replaced at output stage:
    dac_signed = (dac_u8 - 128) << 5  // scale to 14-bit domain
- Then apply panning bits for channel 6.

---

## 11) 9-bit DAC quantization (digital-to-analog boundary)

YM2612 DAC is 9-bit: truncates lowest 5 bits.

If you mix channels in emulator space:
- Quantize BEFORE mixing, not after.
- For multi-carrier algorithms (4..7), quantize carrier outputs (or use 9-bit accumulator), not only final sum.

Reference implementation approach:
- Convert each carrier 14-bit sample to 9-bit via arithmetic shift right 5:
    carrier9 = clamp_i16(carrier14 >> 5, -256, +255)
- Sum carriers in 9-bit domain (or sum masked carrier14 with mask = ~0x1F and clamp)
- Expand back to 14-bit domain by <<5 if needed by later ladder model.

---

## 12) Ladder effect + DAC slot model (discrete YM2612 profile)

This models crossover distortion and "silence slot" behavior.

### 12.1 Simple ladder offset
- Add 1 to all non-negative 9-bit samples at DAC output (emulates larger -1↔0 gap).
(If you are still in 14-bit domain, do it after >>5 and re-expand.)

### 12.2 4-slot-per-channel multiplex noise model (more accurate)
Hardware outputs in groups of 4 slots per channel:
- slot0: sample value
- slot1..3: intended silence, but actually output tends toward -1 or 0 (emulated as -1 or +1 after ladder offset),
  pulled by sign bit.

Emulation in 9-bit domain:
- sample_slot_out = ladder(sample9)
- silence_slot_out = (sample9 < 0) ? -1 : +1   // after ladder modeling
- If channel is muted via panning, the sample slot becomes a 4th "silence" slot.

If not explicitly simulating slots:
- Common approximation per channel per output side:
    if muted: output fixed (+4 or -4) based on sign
    else: add +4 to non-negative samples and -3 to negative samples
(This matches the aggregate effect of the 4-slot model in many emulators.)

### 12.3 Panning defaults (important)
- All channels should power-on with L=1 and R=1; some games rely on this.

---

## 13) Output stage (per sample)

For each channel ch:
1) channel_out_14bit = compute algorithm output (or DAC override for ch6 if enabled)
2) Apply quantization/ladder model (profile dependent)
3) Apply panning:
    if panning_L: mix into out_L else apply "muted-slot noise" if ladder-slot model
    if panning_R: mix into out_R else apply "muted-slot noise" if ladder-slot model

Final:
- out_L/out_R are mixed at the YM sample rate (~53 kHz).
- Resample to host output (e.g., 48 kHz) with a suitable resampler.
- Optionally apply Genesis low-pass filter externally (not part of YM2612 core).

---

## References

- [Emulating the YM2612: Part 1 - Interface](https://jsgroth.dev/blog/posts/emulating-ym2612-part-1/)
- [SpritesMind: YM2612 Documentation](https://gendev.spritesmind.net/forum/viewtopic.php?start=855&t=386)
