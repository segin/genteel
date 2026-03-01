# Yamaha YM2612 / OPN2 (Genesis / Mega Drive) — emulator implementer notes (debugger/cycle-accurate leaning)

## 1) What you're emulating and why variants matter

* **YM2612 (OPN2)**: discrete NMOS chip in earlier Genesis revisions; has several quirks (notably **DAC "ladder effect" distortion** and **odd read-port mirroring behavior**).
* **YM3438 (OPN2C)**: CMOS variant used in later consoles / integrated ASICs; behaves slightly differently (e.g., ladder effect absent; status port mirroring differs; timing differences are real).

For a debugging emulator, you generally want a selectable "hardware profile" at minimum: **(a) discrete YM2612** vs **(b) YM3438 behavior**.

---

## 2) Clocking, sample cadence, and "internal cycles"

### 2.1 External clocks (Genesis integration)

* Genesis drives YM2612 with the **same clock as the 68000** (~7.67 MHz NTSC, slightly lower PAL).
* YM2612 **internally divides by 6**, giving an internal cycle rate ~1.28 MHz NTSC.

If your global scheduler is Genesis master clock (MCLK), one convenient relationship is:

* **1 YM2612 internal cycle = 6 × 68K cycles** (or per jsgroth, "1 internal cycle per 42 MCLK ticks" given the usual MCLK basis).

### 2.2 "Sample rate" the chip naturally operates at

YM2612 generates a full "mixed" sample once every **24 internal cycles**, i.e. about **53267 Hz** on NTSC when you treat it as "compute everything, then emit one sample".

Hardware detail that matters for cycle accuracy:

* Real YM2612 **cycles through channels**, roughly **1 channel per 4 internal cycles**, and **multiplexes** through its DAC; many emulators instead "mix all channels" once per 24 cycles (audibly fine, but not always timing-identical for quirks).

**Debugger-grade recommendation**

* Keep an internal sub-step counter (0..23) and schedule events (timers tick, LFO tick, operator updates, DAC slot behavior) at the right sub-positions. Even if you still output audio at 48 kHz, you can resample from the chip's ~53 kHz cadence.

---

## 3) Bus interface and port mapping (what the CPU can actually do)

### 3.1 Ports (Z80 map)

YM2612 appears as 4 byte ports at **$4000–$4003** in the Z80 address space (mirrored across $4000–$5FFF).

Conceptually documented as:

* $4000 addr (group 1), $4001 data (group 1)
* $4002 addr (group 2), $4003 data (group 2)

But the useful hardware truth:

* There is effectively **one data port** (writes via $4001 or $4003) and the chip remembers whether the last address write was to $4000 (group 1) or $4002 (group 2).

This matters because some code/demo content writes data only via $4001 and relies on "last-selected group" behavior.

### 3.2 Read port ($4000) and mirroring differences

The read port returns only:

* Bit 7: **BUSY**
* Bit 1: Timer B overflow
* Bit 0: Timer A overflow

**Mirroring quirk (big compatibility landmine):**

* On **YM3438** systems, reading $4001–$4003 mirrors $4000.
* On **discrete YM2612** systems, reads from $4001–$4003 behave differently (one observed behavior is "last-read status then decays toward 0 over time"), and specific games break if you emulate the wrong profile.

### 3.3 Register decode structure

Register address ranges:

* $20–$2F: "global"
* $30–$9F: per-operator
* $A0–$BF: per-channel

Channel index is low 2 bits (0–2) and group selects whether that maps to channels 1–3 or 4–6. Addresses with low bits == 3 are invalid.

**Operator index bits are bit-swapped (critical):**
For $30–$9F, bits 2–3 encode operator, but in this order:

* 00 = op1
* 01 = op3
* 10 = op2
* 11 = op4

---

## 4) Write timing, BUSY behavior, and "how long until a write takes effect"

### 4.1 BUSY is real and timing-sensitive

Software often writes a register, then spins until BUSY clears before writing again.

There isn't a single universally-perfect "BUSY = N cycles" across all revisions; behavior varies and can depend on where the chip is in its internal cycle when you write.

A pragmatic baseline some emulators use: keep BUSY high for **~32 internal cycles** after a data write.

### 4.2 YM3438 application-manual "wait cycles" (hard numbers)

The YM3438 manual provides explicit wait-cycle guidance in master-clock cycles (φM):

* After **address write** ($21–$B6): **17 cycles**
* After **data write**:

  * $21–$9E: **83 cycles**
  * $A0–$B6: **47 cycles**

Even if you don't perfectly match BUSY on all silicon, these numbers are extremely useful for:

* implementing a "YM3438-like" timing mode
* sanity-checking that your driver doesn't accept impossible write rates

---

## 5) FM synthesis core: structure, math, and the "gotchas" that affect real music

### 5.1 High-level structure

* 6 channels
* 4 operators per channel
* 8 algorithms (operator routing topologies)
* Operator 1 has optional **feedback** (self-mod)

### 5.2 Phase generator (frequency) — the parts you must get right

Each operator has a phase generator clocked at the "sample cadence" (~53267 Hz if you update once per 24 internal cycles).

Inputs:

* F-number (11-bit), Block (3-bit)
* Detune (3-bit)
* Multiple (4-bit)
* LFO FM sensitivity (per-channel, PMS)

**Two correctness-critical quirks:**

1. **F-num high+block writes latch only when F-num low is written** (don't apply $A4-$A6 immediately). Some games rely on this.
2. **Channel 3 special mode** can assign separate F-num/block per operator (register mapping is non-intuitive). Enabled via $27 bits 6/7.

Key-on behavior:

* Keying on an operator resets its phase counter to 0. Keying off does not affect phase.
  Key-on register $28:
* low 3 bits select channel (with gaps)
* bits 4–7 select which operators key on/off

### 5.3 Envelope generator (ADSR) — log attenuation, not "linear volume"

Each operator's envelope outputs a **10-bit attenuation** value (log2 scale, 4.6 fixed-point), roughly up to ~96 dB.

Key points for correctness:

* Attack decreases attenuation *exponentially*; Decay/Sustain/Release increase it linearly.
* Total Level (TL) is base attenuation added on top (doesn't change the ADSR state machine, it shifts the output).
* Key scaling (KSL/RS) affects envelope rates via key code derived from frequency.

### 5.4 SSG-EG mode (vestigial but real; some content/tests rely on it)

YM2612 retains SSG-EG behavior (controlled in $90–$9F range) with distinctive "triangle/saw/hold/invert" envelope shapes and special interactions like:

* magic threshold at 0x200 (512) for SSG behavior transitions
* output inversion logic with hold/alternate/attack bits
* in some shapes, phase counter resets at specific wrap points

---

## 6) Operator output math and algorithm evaluation (where many emulators get "almost right")

### 6.1 Log-sine table + exponentiation (hardware-style)

YM2612 computes a **log2-sine** from phase via a quarter-wave lookup table and combines it with envelope attenuation (log domain addition), then exponentiates back to a linear sample.

Output is **signed 14-bit** per operator (conceptually).

### 6.2 Phase modulation (PM) is literally "add modulator bits into phase"

Modulator output bits are added into the carrier's phase before sine lookup; the phase counter itself is not altered ("phase modulation," not true frequency modulation).

### 6.3 Algorithm routing gotchas: evaluation order and internal pipeline delays

Two chip-specific behaviors that can audibly matter:

* Operator evaluation order is **1 → 3 → 2 → 4** (not 1→2→3→4).
* Due to internal pipelining, some modulation paths effectively use the **previous sample's output** rather than the just-computed one for certain operator adjacency cases.

If you ignore these, certain instruments sound subtly wrong (often described as "too clean" or "wrong bite").

---

## 7) LFO (vibrato/tremolo) and timers

### 7.1 Timers: what they tick at, and how software uses them (polling)

On Genesis, the YM2612 timer interrupt line is **not connected**, so software must **poll** the overflow bits in the read port.

Per jsgroth's measured behavior:

* Timer A ticks **once per sample** (~53267 Hz NTSC scale).
* Timer B ticks **once per 16 samples** (~3329 Hz).

Timer overflow behavior:

* counter increments, overflows to 0, reloads interval, sets overflow flag if enabled
* flags persist until cleared via $27 bits 4/5

### 7.2 CSM mode

CSM is enabled via $27 with bit7 set and bit6 clear; it auto-triggers Channel 3 keying when Timer A overflows (mostly used by test ROMs).

### 7.3 LFO basics

* Controlled by $22 (enable + frequency select). Disabling resets the LFO counter.
* Vibrato (PM) depth: PMS bits in $B4–$B6
* Tremolo (AM) depth: AMS bits in $B4–$B6, and AM enable per operator in $60–$6F bit 7

The LFO divider table in jsgroth's writeup includes an off-by-one correction validated on hardware (worth following if you're chasing exactness).

---

## 8) Channel 6 DAC mode (PCM) and mixing details

### 8.1 DAC registers and semantics

DAC uses:

* $2A: 8-bit unsigned PCM value
* $2B bit7: enable DAC mode (replaces Channel 6 FM output)
* Channel 6 panning still applies (in $B6 group 2)

PCM interpretation:

* unsigned 0..255 converted to signed by subtracting 128
* then scaled into the FM output domain (jsgroth shifts to match 14-bit scale)

**Timing note (cycle-accuracy angle):**

* In a "sub-step" model, DAC output changes are best applied aligned to the chip's internal channel/DAC slot timing (the DAC is multiplexed in hardware).

---

## 9) Digital-to-analog quirks: 9-bit quantization and the "ladder effect"

These matter because composers *used the real distortion*.

### 9.1 9-bit quantization in multi-carrier algorithms

Even though operator samples are effectively 14-bit, the chip's summing for multi-carrier algorithms uses a **9-bit accumulator**, and (importantly) you should quantize **carrier outputs** before summing for algorithms with multiple carriers.

### 9.2 Ladder effect (discrete YM2612)

The YM2612 DAC introduces crossover distortion ("ladder effect"): a non-linear jump around -1 ↔ 0 that raises the effective noise floor and changes timbre, very noticeable in some games.

A widely-used approach (based on Nuked-OPN2 behavior, as summarized by jsgroth) models:

* a bias on non-negative samples
* "silence slots" that output ±1 depending on sign, and panning/muting influences the pattern, producing extra noise when a channel is not routed equally.

**YM3438 note:** ladder effect is generally treated as absent on YM3438-equipped consoles.

---

## 10) Practical "cycle-accurate" architecture (recommended)

If you want this chip to behave as a debug target (not just "sounds right"):

1. **Run YM core at internal-cycle resolution** (master/6).
2. Track a **0..23 sub-cycle phase** and schedule:

   * operator phase/envelope clocks (even if you batch-update once per 24, keep the timing hooks)
   * timer ticks (Timer A each sample; Timer B each 16 samples)
   * LFO divider/count
   * DAC multiplex / silence slot behavior (if you emulate ladder effect precisely)
3. Implement **hardware profile switches**:

   * YM2612 vs YM3438 read-port mirroring/decay
   * ladder effect on/off
   * BUSY timing model (simple fixed vs address-dependent / measured)

---

## 11) Register map quick reference (addresses; details are in sections above)

From the YM3438 application manual's register map (OPN2C; largely matches OPN2 functional layout):

* $21 test, $22 LFO, $24–$27 timers/mode, $28 key on/off
* $2A DAC data, $2B DAC select/enable
* $30–$3E DT/MUL (operator)
* $40–$4E TL (operator)
* $50–$5E KS/AR (operator)
* $60–$6E AM enable / DR (operator)
* $70–$7E SR (operator)
* $80–$8E SL/RR (operator)
* $90–$9E SSG-EG (operator)
* $A0–$A6 F-num / block (channel)
* $A8–$AE CH3 per-operator F-num/block
* $B0–$B2 FB/algorithm
* $B4–$B6 L/R + AMS/PMS

---

## References

- [Emulating the YM2612: Part 1 - Interface](https://jsgroth.dev/blog/posts/emulating-ym2612-part-1/)
- [Emulating the YM2612: Part 2 - Phase](https://jsgroth.dev/blog/posts/emulating-ym2612-part-2/)
- [Emulating the YM2612: Part 3 - Envelopes](https://jsgroth.dev/blog/posts/emulating-ym2612-part-3/)
- [Emulating the YM2612: Part 4 - Digital Output](https://jsgroth.dev/blog/posts/emulating-ym2612-part-4/)
- [Emulating the YM2612: Part 5 - Analog Output](https://jsgroth.dev/blog/posts/emulating-ym2612-part-5/)
- [Emulating the YM2612: Part 6 - LFO](https://jsgroth.dev/blog/posts/emulating-ym2612-part-6/)
- [Emulating the YM2612: Part 7 - SSG-EG](https://jsgroth.dev/blog/posts/emulating-ym2612-part-7/)
- [SpritesMind: YM2612 Documentation](https://gendev.spritesmind.net/forum/viewtopic.php?start=855&t=386)
