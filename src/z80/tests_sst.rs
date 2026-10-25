//! SingleStepTests Z80 harness.
//!
//! Ignored by default (needs external JSON data). Run e.g.:
//!   Z80TESTS_DIR=/path/to/z80/v1 cargo test --release --lib \
//!       z80::tests_sst::sst_run -- --ignored --nocapture
//!
//! Optional env: Z80TESTS_ONLY="dd cb" (file-name prefix filter),
//! Z80TESTS_SAMPLES=4 (mismatch lines printed per file).
//!
//! Each case runs exactly one instruction on a flat 64K memory with a port mock
//! fed from the case's `ports` list. All registers (including every F bit,
//! MEMPTR, R, IFF1/IFF2, IM, the EI shadow and Q) and the listed memory bytes
//! are compared; T-state mismatches are counted separately.
//!
//! Test data: sparse clone of https://github.com/SingleStepTests/z80 (`v1/`).

use super::Z80;
use crate::memory::{IoInterface, MemoryInterface};
use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug)]
struct FlatMem {
    ram: Vec<u8>,
}

impl MemoryInterface for FlatMem {
    fn read_byte(&mut self, a: u32) -> u8 {
        self.ram[(a & 0xFFFF) as usize]
    }
    fn write_byte(&mut self, a: u32, v: u8) {
        self.ram[(a & 0xFFFF) as usize] = v;
    }
    fn read_word(&mut self, a: u32) -> u16 {
        u16::from_le_bytes([self.read_byte(a), self.read_byte(a.wrapping_add(1))])
    }
    fn write_word(&mut self, a: u32, v: u16) {
        self.write_byte(a, v as u8);
        self.write_byte(a.wrapping_add(1), (v >> 8) as u8);
    }
    fn read_long(&mut self, a: u32) -> u32 {
        self.read_word(a) as u32 | ((self.read_word(a.wrapping_add(2)) as u32) << 16)
    }
    fn write_long(&mut self, a: u32, v: u32) {
        self.write_word(a, v as u16);
        self.write_word(a.wrapping_add(2), (v >> 16) as u16);
    }
}

#[derive(Debug, Default)]
struct PortMock {
    reads: VecDeque<(u16, u8)>,
    writes: Vec<(u16, u8)>,
    bad_reads: Vec<u16>,
}

impl IoInterface for PortMock {
    fn read_port(&mut self, port: u16) -> u8 {
        match self.reads.pop_front() {
            Some((p, v)) if p == port => v,
            other => {
                self.bad_reads.push(port);
                other.map(|(_, v)| v).unwrap_or(0xFF)
            }
        }
    }
    fn write_port(&mut self, port: u16, value: u8) {
        self.writes.push((port, value));
    }
}

fn num(v: &Value, k: &str) -> u64 {
    v.get(k).and_then(|x| x.as_u64()).unwrap_or(0)
}

type Cpu = Z80<FlatMem, PortMock>;

fn load(cpu: &mut Cpu, s: &Value) {
    cpu.pc = num(s, "pc") as u16;
    cpu.sp = num(s, "sp") as u16;
    cpu.a = num(s, "a") as u8;
    cpu.f = num(s, "f") as u8;
    cpu.b = num(s, "b") as u8;
    cpu.c = num(s, "c") as u8;
    cpu.d = num(s, "d") as u8;
    cpu.e = num(s, "e") as u8;
    cpu.h = num(s, "h") as u8;
    cpu.l = num(s, "l") as u8;
    cpu.i = num(s, "i") as u8;
    cpu.r = num(s, "r") as u8;
    cpu.memptr = num(s, "wz") as u16;
    cpu.ix = num(s, "ix") as u16;
    cpu.iy = num(s, "iy") as u16;
    let af_ = num(s, "af_");
    cpu.a_prime = (af_ >> 8) as u8;
    cpu.f_prime = af_ as u8;
    let bc_ = num(s, "bc_");
    cpu.b_prime = (bc_ >> 8) as u8;
    cpu.c_prime = bc_ as u8;
    let de_ = num(s, "de_");
    cpu.d_prime = (de_ >> 8) as u8;
    cpu.e_prime = de_ as u8;
    let hl_ = num(s, "hl_");
    cpu.h_prime = (hl_ >> 8) as u8;
    cpu.l_prime = hl_ as u8;
    cpu.im = num(s, "im") as u8;
    cpu.iff1 = num(s, "iff1") != 0;
    cpu.iff2 = num(s, "iff2") != 0;
    cpu.pending_ei = num(s, "ei") != 0;
    cpu.q = num(s, "q") as u8;
    cpu.halted = false;
    if let Some(ram) = s.get("ram").and_then(|r| r.as_array()) {
        for e in ram {
            let addr = e[0].as_u64().unwrap_or(0) as usize & 0xFFFF;
            cpu.memory.ram[addr] = e[1].as_u64().unwrap_or(0) as u8;
        }
    }
}

/// Compare final state; returns a list of (field, got, want).
fn compare(cpu: &mut Cpu, s: &Value) -> Vec<(&'static str, u64, u64)> {
    let mut out = Vec::new();
    let pair = |h: u8, l: u8| ((h as u64) << 8) | l as u64;
    let got: [(&'static str, u64); 26] = [
        ("pc", cpu.pc as u64),
        ("sp", cpu.sp as u64),
        ("a", cpu.a as u64),
        ("f", cpu.f as u64),
        ("b", cpu.b as u64),
        ("c", cpu.c as u64),
        ("d", cpu.d as u64),
        ("e", cpu.e as u64),
        ("h", cpu.h as u64),
        ("l", cpu.l as u64),
        ("i", cpu.i as u64),
        ("r", cpu.r as u64),
        ("wz", cpu.memptr as u64),
        ("ix", cpu.ix as u64),
        ("iy", cpu.iy as u64),
        ("af_", pair(cpu.a_prime, cpu.f_prime)),
        ("bc_", pair(cpu.b_prime, cpu.c_prime)),
        ("de_", pair(cpu.d_prime, cpu.e_prime)),
        ("hl_", pair(cpu.h_prime, cpu.l_prime)),
        ("im", cpu.im as u64),
        ("iff1", cpu.iff1 as u64),
        ("iff2", cpu.iff2 as u64),
        ("ei", cpu.pending_ei as u64),
        ("q", cpu.q as u64),
        ("ram", 0),
        ("ports", 0),
    ];
    for (k, g) in got.iter().take(24) {
        let w = num(s, k);
        if *g != w {
            out.push((*k, *g, w));
        }
    }
    if let Some(ram) = s.get("ram").and_then(|r| r.as_array()) {
        for e in ram {
            let addr = e[0].as_u64().unwrap_or(0) as u32 & 0xFFFF;
            let want = e[1].as_u64().unwrap_or(0);
            let g = cpu.memory.read_byte(addr) as u64;
            if g != want {
                out.push(("ram", ((addr as u64) << 8) | g, ((addr as u64) << 8) | want));
            }
        }
    }
    out
}

#[test]
#[ignore]
fn sst_run() {
    let dir = std::env::var("Z80TESTS_DIR").expect("set Z80TESTS_DIR to the z80 v1 directory");
    let only = std::env::var("Z80TESTS_ONLY")
        .ok()
        .map(|s| s.to_lowercase());
    let max_report: usize = std::env::var("Z80TESTS_SAMPLES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2);

    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("read Z80TESTS_DIR")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".json"))
        .filter(|n| only.as_ref().is_none_or(|o| n.starts_with(o.as_str())))
        .collect();
    names.sort();

    let mut grand_pass = 0usize;
    let mut grand_fail = 0usize;
    let mut grand_cyc = 0usize;
    let mut by_field: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut by_group: BTreeMap<String, (usize, usize)> = BTreeMap::new();

    for name in &names {
        let text = match std::fs::read_to_string(format!("{dir}/{name}")) {
            Ok(t) => t,
            Err(e) => {
                println!("{name}: read error {e}");
                continue;
            }
        };
        let cases: Vec<Value> = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                println!("{name}: parse error {e}");
                continue;
            }
        };
        let (mut pass, mut fail, mut cyc) = (0usize, 0usize, 0usize);
        let mut samples = Vec::new();
        for case in &cases {
            let mut cpu = Z80::new(
                FlatMem {
                    ram: vec![0; 0x10000],
                },
                PortMock::default(),
            );
            load(&mut cpu, &case["initial"]);
            let mut expected_writes = Vec::new();
            if let Some(ports) = case.get("ports").and_then(|p| p.as_array()) {
                for p in ports {
                    let port = p[0].as_u64().unwrap_or(0) as u16;
                    let val = p[1].as_u64().unwrap_or(0) as u8;
                    if p[2].as_str() == Some("r") {
                        cpu.io.reads.push_back((port, val));
                    } else {
                        expected_writes.push((port, val));
                    }
                }
            }
            let t = cpu.step();
            let mut diffs = compare(&mut cpu, &case["final"]);
            if cpu.io.writes != expected_writes || !cpu.io.bad_reads.is_empty() {
                diffs.push(("ports", 0, 0));
            }
            let want_t = case["cycles"].as_array().map(|c| c.len()).unwrap_or(0);
            if t as usize != want_t {
                cyc += 1;
                if samples.len() < max_report {
                    samples.push(format!(
                        "{}: T-states got {t} want {want_t}",
                        case["name"].as_str().unwrap_or("?")
                    ));
                }
            }
            if diffs.is_empty() {
                pass += 1;
            } else {
                fail += 1;
                let mut seen = Vec::new();
                for d in &diffs {
                    if !seen.contains(&d.0) {
                        seen.push(d.0);
                        *by_field.entry(d.0).or_default() += 1;
                    }
                }
                if samples.len() < max_report {
                    let desc: Vec<String> = diffs
                        .iter()
                        .map(|(k, g, w)| format!("{k} got {g:#X} want {w:#X}"))
                        .collect();
                    samples.push(format!(
                        "{}: {}",
                        case["name"].as_str().unwrap_or("?"),
                        desc.join(", ")
                    ));
                }
            }
        }
        let group = match name.split(' ').next() {
            Some(g @ ("cb" | "ed")) => g.to_string(),
            Some(g @ ("dd" | "fd")) if name.contains("cb __") => format!("{g} cb"),
            Some(g @ ("dd" | "fd")) => g.to_string(),
            _ => "base".to_string(),
        };
        let entry = by_group.entry(group).or_default();
        entry.0 += fail;
        entry.1 += cyc;
        if fail > 0 || cyc > 0 {
            println!("{name}: {pass} pass, {fail} fail, {cyc} T-state mismatches");
            for s in &samples {
                println!("      {s}");
            }
        }
        grand_pass += pass;
        grand_fail += fail;
        grand_cyc += cyc;
    }
    println!("FAILURES BY FIELD: {by_field:?}");
    println!("BY GROUP (state fails, T-state mismatches): {by_group:?}");
    println!("TOTAL: {grand_pass} pass, {grand_fail} fail, {grand_cyc} T-state mismatches");
    assert_eq!(grand_fail, 0, "state mismatches");
    assert_eq!(grand_cyc, 0, "T-state mismatches");
}
