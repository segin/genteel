#![no_main]

//! Z80 ALU flag fuzzer.
//!
//! Runs one of ADD/ADC/SUB/SBC/AND/XOR/OR/CP A,B on arbitrary operands and
//! carry-in, then checks the flag rules that hold for every input.

use genteel::memory::{IoInterface, Memory};
use genteel::z80::{flags, Z80};
use libfuzzer_sys::fuzz_target;

/// I/O space with nothing attached: reads float high, writes are dropped.
#[derive(Debug)]
struct OpenBusIo;

impl IoInterface for OpenBusIo {
    fn read_port(&mut self, _port: u16) -> u8 {
        0xFF
    }
    fn write_port(&mut self, _port: u16, _value: u8) {}
}

/// ADD, ADC, SUB, SBC, AND, XOR, OR, CP with B as the source operand.
const OPCODES: [u8; 8] = [0x80, 0x88, 0x90, 0x98, 0xA0, 0xA8, 0xB0, 0xB8];
const CP: usize = 7;

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }

    let a_val = data[0];
    let b_val = data[1];
    let class = (data[2] % 8) as usize;
    let carry_in = (data[3] & 1) != 0;
    let opcode = OPCODES[class];

    let mut memory = Memory::new(0x10000);
    memory.data[0] = opcode;

    let mut cpu = Z80::new(memory, OpenBusIo);
    cpu.a = a_val;
    cpu.b = b_val;
    cpu.set_flag(flags::CARRY, carry_in);

    cpu.step();

    let result = cpu.a;
    if class != CP {
        assert_eq!(
            cpu.get_flag(flags::ZERO),
            result == 0,
            "Z mismatch for {opcode:02X}: A={result:02X}"
        );
        assert_eq!(
            cpu.get_flag(flags::SIGN),
            result & 0x80 != 0,
            "S mismatch for {opcode:02X}: A={result:02X}"
        );
    } else {
        assert_eq!(cpu.a, a_val, "CP must not modify A");
    }

    /* N is set by SUB, SBC and CP and cleared by everything else. */
    let expect_n = matches!(class, 2 | 3 | 7);
    assert_eq!(
        cpu.get_flag(flags::ADD_SUB),
        expect_n,
        "N mismatch for {opcode:02X}"
    );

    if class == 4 {
        assert!(cpu.get_flag(flags::HALF_CARRY), "AND must set H");
        assert!(!cpu.get_flag(flags::CARRY), "AND must clear C");
    }
    if class == 5 || class == 6 {
        assert!(!cpu.get_flag(flags::HALF_CARRY), "XOR/OR must clear H");
        assert!(!cpu.get_flag(flags::CARRY), "XOR/OR must clear C");
    }
});
