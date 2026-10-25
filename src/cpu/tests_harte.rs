//! TomHarte / SingleStepTests 68000 harness.
//!
//! Ignored by default (needs external JSON data). Run e.g.:
//!   HARTE_DIR=/path/to/68000/v1 cargo test --release --bin genteel \
//!       cpu::tests_harte::harte_run -- --ignored --nocapture
//!
//! Optional env: HARTE_ONLY=DIVU (substring filter), HARTE_SAMPLES=8.
//!
//! Compares D0-D7, A0-A6, PC, the full SR, USP, SSP and touched memory after
//! one instruction. The prefetch queue itself is not modelled or compared; the
//! data's final PC is the address of the next instruction, so it is comparable.
//!
//! Test data: clone https://github.com/SingleStepTests/680x0 and gunzip the
//! files in `68000/v1/*.json.gz`; point HARTE_DIR at the resulting directory.

use super::Cpu;
use crate::memory::MemoryInterface;
use std::collections::HashMap;
use std::fmt;

#[derive(Default)]
struct FlatMem {
    ram: HashMap<u32, u8>,
}

impl fmt::Debug for FlatMem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FlatMem")
    }
}

impl FlatMem {
    fn rb(&self, a: u32) -> u8 {
        *self.ram.get(&(a & 0x00FF_FFFF)).unwrap_or(&0)
    }
    fn wb(&mut self, a: u32, v: u8) {
        self.ram.insert(a & 0x00FF_FFFF, v);
    }
}

impl MemoryInterface for FlatMem {
    fn read_byte(&mut self, a: u32) -> u8 {
        self.rb(a)
    }
    fn write_byte(&mut self, a: u32, v: u8) {
        self.wb(a, v);
    }
    fn read_word(&mut self, a: u32) -> u16 {
        ((self.rb(a) as u16) << 8) | (self.rb(a.wrapping_add(1)) as u16)
    }
    fn write_word(&mut self, a: u32, v: u16) {
        self.wb(a, (v >> 8) as u8);
        self.wb(a.wrapping_add(1), v as u8);
    }
    fn read_long(&mut self, a: u32) -> u32 {
        ((self.read_word(a) as u32) << 16) | (self.read_word(a.wrapping_add(2)) as u32)
    }
    fn write_long(&mut self, a: u32, v: u32) {
        self.write_word(a, (v >> 16) as u16);
        self.write_word(a.wrapping_add(2), v as u16);
    }
}

fn u32f(v: &serde_json::Value, k: &str) -> u32 {
    v.get(k).and_then(|x| x.as_u64()).unwrap_or(0) as u32
}

/// Disassemble a ROM range with the built-in disassembler.
/// DISASM_ROM=<path> DISASM_START=<hex> DISASM_END=<hex>.
#[test]
#[ignore]
fn disasm_range() {
    let rom = std::fs::read(std::env::var("DISASM_ROM").expect("DISASM_ROM")).unwrap();
    let start = usize::from_str_radix(&std::env::var("DISASM_START").unwrap(), 16).unwrap();
    let end = usize::from_str_radix(&std::env::var("DISASM_END").unwrap(), 16).unwrap();
    let mut pc = start;
    while pc < end && pc + 1 < rom.len() {
        let opcode = ((rom[pc] as u16) << 8) | (rom[pc + 1] as u16);
        let instr = crate::cpu::decode(opcode);
        let len = (instr.length_words() as usize).clamp(1, 5);
        let mut words = [0u16; 5];
        for (i, w) in words.iter_mut().enumerate().take(len) {
            if pc + i * 2 + 1 < rom.len() {
                *w = ((rom[pc + i * 2] as u16) << 8) | (rom[pc + i * 2 + 1] as u16);
            }
        }
        println!(
            "{:06X}  {}",
            pc,
            instr.format_with_words(pc as u32, &words, false)
        );
        pc += len * 2;
    }
}

#[test]
#[ignore]
fn harte_run() {
    let dir = std::env::var("HARTE_DIR").expect("set HARTE_DIR to the 68000/v1 json directory");
    let only = std::env::var("HARTE_ONLY").ok().map(|s| s.to_uppercase());
    let max_report: usize = std::env::var("HARTE_SAMPLES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(6);

    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
        .collect();
    files.sort();

    let mut dummy = FlatMem::default();
    let mut cpu = Cpu::new(&mut dummy);

    let mut grand_pass = 0usize;
    let mut grand_fail = 0usize;

    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if let Some(o) = &only {
            if !name.to_uppercase().contains(o) {
                continue;
            }
        }
        let data: serde_json::Value = match serde_json::from_reader(std::io::BufReader::new(
            std::fs::File::open(path).unwrap(),
        )) {
            Ok(d) => d,
            Err(e) => {
                println!("{name}: parse error {e}");
                continue;
            }
        };
        let cases = data.as_array().cloned().unwrap_or_default();
        let mut pass = 0usize;
        let mut fail = 0usize;
        let mut arith_fail = 0usize;
        let mut exc_fail = 0usize;
        let mut samples: Vec<String> = Vec::new();

        for case in &cases {
            let init = &case["initial"];
            let fin = &case["final"];

            let mut mem = FlatMem::default();
            if let Some(ram) = init["ram"].as_array() {
                for pair in ram {
                    if let Some(p) = pair.as_array() {
                        mem.wb(p[0].as_u64().unwrap() as u32, p[1].as_u64().unwrap() as u8);
                    }
                }
            }
            let pc = u32f(init, "pc");
            if let Some(pf) = init["prefetch"].as_array() {
                mem.write_word(pc, pf[0].as_u64().unwrap_or(0) as u16);
                mem.write_word(
                    pc.wrapping_add(2),
                    pf.get(1).and_then(|x| x.as_u64()).unwrap_or(0) as u16,
                );
            }

            for i in 0..8 {
                cpu.d[i] = u32f(init, &format!("d{i}"));
            }
            for i in 0..7 {
                cpu.a[i] = u32f(init, &format!("a{i}"));
            }
            let sr = u32f(init, "sr") as u16;
            cpu.sr = sr;
            cpu.usp = u32f(init, "usp");
            cpu.ssp = u32f(init, "ssp");
            cpu.a[7] = if sr & 0x2000 != 0 { cpu.ssp } else { cpu.usp };
            cpu.pc = pc;
            cpu.halted = false;
            cpu.pending_exception = false;
            cpu.pending_interrupt = 0;
            cpu.interrupt_pending_mask = 0;
            cpu.invalidate_cache_entry(pc);

            cpu.step_instruction(&mut mem);

            // Was this test an exception case? (SP pushed / entered supervisor)
            let exc = u32f(fin, "ssp") != u32f(init, "ssp")
                || (u32f(fin, "sr") as u16 & 0x2000) != (sr & 0x2000);

            let mut diffs: Vec<String> = Vec::new();
            for i in 0..8 {
                let e = u32f(fin, &format!("d{i}"));
                if cpu.d[i] != e {
                    diffs.push(format!("d{i}={:08X}!={:08X}", cpu.d[i], e));
                }
            }
            for i in 0..7 {
                let e = u32f(fin, &format!("a{i}"));
                if cpu.a[i] != e {
                    diffs.push(format!("a{i}={:08X}!={:08X}", cpu.a[i], e));
                }
            }
            let esr = u32f(fin, "sr") as u16;
            if cpu.sr != esr {
                diffs.push(format!("sr={:04X}!={esr:04X}", cpu.sr));
            }
            let epc = u32f(fin, "pc");
            if cpu.pc != epc {
                diffs.push(format!("pc={:08X}!={epc:08X}", cpu.pc));
            }
            let (usp, ssp) = if cpu.sr & 0x2000 != 0 {
                (cpu.usp, cpu.a[7])
            } else {
                (cpu.a[7], cpu.ssp)
            };
            let (eusp, essp) = (u32f(fin, "usp"), u32f(fin, "ssp"));
            if usp != eusp {
                diffs.push(format!("usp={usp:08X}!={eusp:08X}"));
            }
            if ssp != essp {
                diffs.push(format!("ssp={ssp:08X}!={essp:08X}"));
            }
            if let Some(ram) = fin["ram"].as_array() {
                for pair in ram {
                    if let Some(p) = pair.as_array() {
                        let a = p[0].as_u64().unwrap() as u32;
                        let e = p[1].as_u64().unwrap() as u8;
                        if mem.rb(a) != e {
                            diffs.push(format!("m{a:06X}={:02X}!={e:02X}", mem.rb(a)));
                        }
                    }
                }
            }

            if diffs.is_empty() {
                pass += 1;
            } else {
                fail += 1;
                if exc {
                    exc_fail += 1;
                } else {
                    arith_fail += 1;
                    // Only sample genuine (non-exception) arithmetic failures.
                    if samples.len() < max_report {
                        samples.push(format!(
                            "{}: {}",
                            case["name"].as_str().unwrap_or("?"),
                            diffs.join(" ")
                        ));
                    }
                }
            }
        }
        grand_pass += pass;
        grand_fail += fail;
        println!(
            "{name:14} {pass:5} pass {fail:5} fail  ({arith_fail} arith, {exc_fail} exc-only)"
        );
        for s in &samples {
            println!("      {s}");
        }
    }
    println!("TOTAL: {grand_pass} pass, {grand_fail} fail");
}
