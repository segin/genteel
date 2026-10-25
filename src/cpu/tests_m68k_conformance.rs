//! 68000 conformance tests: address-error aborts and group 0 frames,
//! exception stacking, illegal encodings, SR/CCR masking, trace, halting,
//! TAS bus behaviour, divide/BCD/CHK semantics and bus-accurate timing.

use crate::cpu::ops::arithmetic::{divs_cycles, divu_cycles, muls_cycles, mulu_cycles};
use crate::cpu::test_utils::{create_cpu, write_op};
use crate::cpu::{flags, Cpu};
use crate::memory::{Memory, MemoryInterface};
use proptest::prelude::*;

const HANDLER: u32 = 0x4000;

/// Point every exception vector at HANDLER.
fn install_vectors(memory: &mut Memory) {
    for v in 2..64u32 {
        memory.write_long(v * 4, HANDLER);
    }
}

fn setup(program: &[u16]) -> (Cpu, Memory) {
    let (cpu, mut memory) = create_cpu();
    install_vectors(&mut memory);
    write_op(&mut memory, program);
    (cpu, memory)
}

/// Contents of a group 0 frame at SP.
struct Group0Frame {
    status: u16,
    addr: u32,
    ir: u16,
    sr: u16,
    pc: u32,
}

fn group0_frame(cpu: &Cpu, memory: &mut Memory) -> Group0Frame {
    let sp = cpu.a[7];
    Group0Frame {
        status: memory.read_word(sp),
        addr: memory.read_long(sp + 2),
        ir: memory.read_word(sp + 6),
        sr: memory.read_word(sp + 8),
        pc: memory.read_long(sp + 10),
    }
}

/* ---------------------------------------------------------------------- */
/* Address errors                                                           */
/* ---------------------------------------------------------------------- */

#[test]
fn odd_read_aborts_move_and_stacks_group0_frame() {
    /* MOVE.W (A0)+,D0 with A0 odd */
    let (mut cpu, mut memory) = setup(&[0x3018]);
    cpu.a[0] = 0x2001;
    cpu.d[0] = 0x1234_5678;
    let cycles = cpu.step_instruction(&mut memory);

    assert_eq!(cpu.pc, HANDLER);
    assert_eq!(cpu.d[0], 0x1234_5678, "destination untouched");
    assert_eq!(cpu.a[0], 0x2003, "the postincrement had already happened");
    assert_eq!(cpu.a[7], 0x8000 - 14, "7-word frame");
    assert_eq!(cycles, 50);
    let f = group0_frame(&cpu, &mut memory);
    assert_eq!(
        f.status,
        (0x3018 & 0xFFE0) | 0x10 | 0x05,
        "IR bits, read, supervisor data"
    );
    assert_eq!(f.addr, 0x2001);
    assert_eq!(f.ir, 0x3018);
    assert_eq!(f.sr, 0x2000);
    assert_eq!(f.pc, 0x1000);
    assert!(!cpu.halted);
}

#[test]
fn odd_write_after_flags_set_keeps_flags_and_reports_write() {
    /* MOVE.W D1,(A0) with A0 odd: flags are set before the write faults */
    let (mut cpu, mut memory) = setup(&[0x3081]);
    cpu.a[0] = 0x3001;
    cpu.d[1] = 0x8000;
    cpu.step_instruction(&mut memory);
    let f = group0_frame(&cpu, &mut memory);
    assert_eq!(f.status & 0x1F, 0x05, "write cycle, supervisor data");
    assert_eq!(f.sr & 0x0F, flags::NEGATIVE);
    assert_eq!(memory.read_word(0x3000), 0);
}

#[test]
fn read_modify_write_odd_address_is_one_address_error_not_a_halt() {
    /* ADD.W D0,(A0) */
    let (mut cpu, mut memory) = setup(&[0xD150]);
    cpu.a[0] = 0x2001;
    cpu.step_instruction(&mut memory);
    assert!(!cpu.halted);
    assert_eq!(cpu.pc, HANDLER);
    assert_eq!(cpu.a[7], 0x8000 - 14);
}

#[test]
fn move_to_sr_odd_source_enters_handler_in_supervisor_state() {
    /* MOVE (A0),SR */
    let (mut cpu, mut memory) = setup(&[0x46D0]);
    cpu.a[0] = 0x2001;
    cpu.sr = 0x2700;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, HANDLER);
    assert_eq!(cpu.sr & 0x2700, 0x2700, "S and the IPL mask survive");
}

#[test]
fn jsr_to_odd_target_faults_before_pushing_the_return_address() {
    /* JSR (A0) */
    let (mut cpu, mut memory) = setup(&[0x4E90]);
    cpu.a[0] = 0x3001;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, HANDLER);
    assert_eq!(cpu.a[7], 0x8000 - 14, "only the exception frame was pushed");
    let f = group0_frame(&cpu, &mut memory);
    assert_eq!(f.addr, 0x3001);
    assert_eq!(
        f.status & 0x1F,
        0x1E,
        "read, program fetch, supervisor program"
    );
    assert_eq!(f.pc, 0x3001 - 4);
}

#[test]
fn branch_to_odd_target_takes_address_error() {
    /* BRA.S *+3 */
    let (mut cpu, mut memory) = setup(&[0x6001]);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, HANDLER);
    let f = group0_frame(&cpu, &mut memory);
    assert_eq!(f.addr, 0x1003);
}

#[test]
fn odd_stack_pop_takes_address_error() {
    /* UNLK A0 with A0 odd */
    let (mut cpu, mut memory) = setup(&[0x4E58]);
    cpu.a[0] = 0x5001;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, HANDLER);
    assert_eq!(group0_frame(&cpu, &mut memory).addr, 0x5001);
}

#[test]
fn fault_while_stacking_a_group0_frame_halts() {
    /* MOVE.W (A0),D0 with A0 odd and an odd supervisor stack */
    let (mut cpu, mut memory) = setup(&[0x3010]);
    cpu.a[0] = 0x2001;
    cpu.a[7] = 0x8001;
    cpu.step_instruction(&mut memory);
    assert!(cpu.halted);
}

/* ---------------------------------------------------------------------- */
/* Group 1/2 exceptions                                                     */
/* ---------------------------------------------------------------------- */

#[test]
fn line_a_stacks_the_faulting_opcode_address() {
    let (mut cpu, mut memory) = setup(&[0xA123]);
    let cycles = cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, HANDLER);
    assert_eq!(memory.read_long(cpu.a[7] + 2), 0x1000);
    assert_eq!(cycles, 34);
}

#[test]
fn privilege_violation_stacks_the_opcode_address() {
    /* MOVE #$2700,SR in user mode */
    let (mut cpu, mut memory) = setup(&[0x46FC, 0x2700]);
    cpu.sr = 0;
    cpu.ssp = 0x9000;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, HANDLER);
    assert_eq!(cpu.a[7], 0x9000 - 6);
    assert_eq!(memory.read_long(cpu.a[7] + 2), 0x1000);
    assert_eq!(memory.read_word(cpu.a[7]), 0);
}

#[test]
fn disallowed_addressing_modes_are_illegal_instructions() {
    for program in [
        &[0x4EFCu16, 0x1234][..], // JMP #imm
        &[0x43D8],                // LEA (A0)+,A1
        &[0x49C0],                // EXTB.L (68020)
        &[0xD17C, 0x0001],        // ADD.W D0,#imm
        &[0x01FC, 0x0001],        // BSET D0,#imm
        &[0x4248],                // CLR.W A0
        &[0x0048, 0x0001],        // ORI.W #1,A0
        &[0xC180],                // EXG with opmode 10000
        &[0x4A48],                // TST.W A0
        &[0x4A7A, 0x0000],        // TST.W d16(PC)
        &[0x1008],                // MOVE.B A0,D0
        &[0xD008],                // ADD.B A0,D0
        &[0xE8C0],                // bit-field space
        &[0xE0C0],                // memory shift of D0
        &[0x4898, 0x0001],        // MOVEM.W regs,(A0)+
        &[0x4CA0, 0x0001],        // MOVEM.W -(A0),regs
    ] {
        let (mut cpu, mut memory) = setup(program);
        cpu.a[0] = 0x2000;
        cpu.step_instruction(&mut memory);
        assert_eq!(cpu.pc, HANDLER, "{:04X} must be illegal", program[0]);
        assert_eq!(memory.read_long(cpu.a[7] + 2), 0x1000);
        /* Stacked SR */
        assert_eq!(memory.read_word(cpu.a[7]), 0x2000);
    }
}

#[test]
fn btst_register_with_immediate_operand_is_legal() {
    /* BTST D1,#$80 */
    let (mut cpu, mut memory) = setup(&[0x033C, 0x0080]);
    cpu.d[1] = 7;
    let cycles = cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x1004);
    assert!(!cpu.get_flag(flags::ZERO));
    assert_eq!(cycles, 10);
}

/* ---------------------------------------------------------------------- */
/* SR / CCR                                                                 */
/* ---------------------------------------------------------------------- */

#[test]
fn sr_keeps_only_implemented_bits() {
    /* MOVE #$FFFF,SR ; MOVE SR,D0 */
    let (mut cpu, mut memory) = setup(&[0x46FC, 0xFFFF, 0x40C0]);
    cpu.step_instruction(&mut memory);
    /* T is now set: the next instruction traces, which clears T again */
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFFFF, 0xA71F);
}

#[test]
fn ccr_writes_keep_only_bits_0_to_4() {
    /* ORI #$FF,CCR */
    let (mut cpu, mut memory) = setup(&[0x003C, 0x00FF]);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.sr & 0xFF, 0x1F);
}

proptest! {
    #[test]
    fn move_to_sr_masks_any_value(v in any::<u16>()) {
        let (mut cpu, mut memory) = setup(&[0x46FC, v]);
        cpu.step_instruction(&mut memory);
        prop_assert_eq!(cpu.sr, v & 0xA71F);
    }

    #[test]
    fn move_to_ccr_masks_any_value(v in any::<u16>(), sr_hi in 0u16..8) {
        let (mut cpu, mut memory) = setup(&[0x44FC, v]);
        cpu.sr = 0x2000 | (sr_hi << 8);
        cpu.step_instruction(&mut memory);
        prop_assert_eq!(cpu.sr, 0x2000 | (sr_hi << 8) | (v & 0x1F));
    }

    /* An odd word read never changes the destination register. */
    #[test]
    fn odd_source_never_writes_destination(addr in any::<u32>(), d in any::<u32>()) {
        let (mut cpu, mut memory) = setup(&[0x3010]);
        cpu.a[0] = (addr & 0xFFFF) | 1;
        cpu.d[0] = d;
        cpu.step_instruction(&mut memory);
        prop_assert_eq!(cpu.d[0], d);
        prop_assert_eq!(cpu.pc, HANDLER);
    }
}

/* ---------------------------------------------------------------------- */
/* Trace, STOP, halt                                                        */
/* ---------------------------------------------------------------------- */

#[test]
fn trace_bit_takes_vector_9_after_the_instruction() {
    let (mut cpu, mut memory) = setup(&[0x4E71, 0x4E71]);
    memory.write_long(9 * 4, 0x5000);
    cpu.sr = flags::TRACE | flags::SUPERVISOR;
    let cycles = cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x5000);
    assert_eq!(memory.read_long(cpu.a[7] + 2), 0x1002);
    assert_eq!(memory.read_word(cpu.a[7]), flags::TRACE | flags::SUPERVISOR);
    assert_eq!(cpu.sr & flags::TRACE, 0);
    assert_eq!(cycles, 4 + 34);
}

#[test]
fn stop_and_halt_advance_the_cycle_counter() {
    /* STOP #$2700 */
    let (mut cpu, mut memory) = setup(&[0x4E72, 0x2700]);
    cpu.step_instruction(&mut memory);
    assert!(cpu.stopped);
    let before = cpu.cycles;
    assert_eq!(cpu.step_instruction(&mut memory), 4);
    assert_eq!(cpu.cycles, before + 4);
}

/* ---------------------------------------------------------------------- */
/* Decode table, bank switching and save states                             */
/* ---------------------------------------------------------------------- */

#[test]
fn changed_rom_contents_execute_without_stale_decodes() {
    /* Simulates a mapper bank switch: same PC, new code. */
    let (mut cpu, mut memory) = setup(&[0x7001]); // MOVEQ #1,D0
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 1);
    memory.write_word(0x1000, 0x7402); // MOVEQ #2,D2
    cpu.pc = 0x1000;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[2], 2);
}

#[test]
fn save_state_round_trip_keeps_executing() {
    let (cpu, mut memory) = setup(&[0x7005, 0x5280]); // MOVEQ #5,D0 ; ADDQ.L #1,D0
    let json = serde_json::to_string(&cpu).unwrap();
    let mut restored: Cpu = serde_json::from_str(&json).unwrap();
    restored.step_instruction(&mut memory);
    restored.step_instruction(&mut memory);
    assert_eq!(restored.d[0], 6);
}

/* ---------------------------------------------------------------------- */
/* Instruction semantics                                                    */
/* ---------------------------------------------------------------------- */

/// A bus that drops the write half of TAS, like the Mega Drive's.
#[derive(Debug)]
struct NoTasBus<'a>(&'a mut Memory);

impl MemoryInterface for NoTasBus<'_> {
    fn read_byte(&mut self, a: u32) -> u8 {
        self.0.read_byte(a)
    }
    fn write_byte(&mut self, a: u32, v: u8) {
        self.0.write_byte(a, v)
    }
    fn read_word(&mut self, a: u32) -> u16 {
        self.0.read_word(a)
    }
    fn write_word(&mut self, a: u32, v: u16) {
        self.0.write_word(a, v)
    }
    fn read_long(&mut self, a: u32) -> u32 {
        self.0.read_long(a)
    }
    fn write_long(&mut self, a: u32, v: u32) {
        self.0.write_long(a, v)
    }
    fn tas_write_back(&self) -> bool {
        false
    }
}

#[test]
fn tas_writes_back_on_a_plain_bus() {
    let (mut cpu, mut memory) = setup(&[0x4AD0]); // TAS (A0)
    cpu.a[0] = 0x2000;
    memory.write_byte(0x2000, 0x00);
    let cycles = cpu.step_instruction(&mut memory);
    assert_eq!(memory.read_byte(0x2000), 0x80);
    assert!(cpu.get_flag(flags::ZERO));
    assert_eq!(cycles, 14);
}

#[test]
fn tas_on_a_bus_without_write_back_only_tests() {
    let (mut cpu, mut memory) = setup(&[0x4AD0]);
    cpu.a[0] = 0x2000;
    memory.write_byte(0x2000, 0x81);
    let cycles = cpu.step_instruction(&mut NoTasBus(&mut memory));
    assert_eq!(memory.read_byte(0x2000), 0x81, "memory unchanged");
    assert!(
        cpu.get_flag(flags::NEGATIVE),
        "flags from the original byte"
    );
    assert!(!cpu.get_flag(flags::ZERO));
    assert_eq!(cycles, 14, "the bus cycle still takes its time");
}

#[test]
fn tas_on_a_register_always_sets_bit_7() {
    let (mut cpu, mut memory) = setup(&[0x4AC0]); // TAS D0
    let cycles = cpu.step_instruction(&mut NoTasBus(&mut memory));
    assert_eq!(cpu.d[0] & 0xFF, 0x80);
    assert_eq!(cycles, 4);
}

#[test]
fn divu_overflow_sets_v_clears_c_and_keeps_register_and_nz() {
    /* DIVU #1,D0 with D0 = $00010000 */
    let (mut cpu, mut memory) = setup(&[0x80FC, 0x0001]);
    cpu.d[0] = 0x0001_0000;
    cpu.sr = 0x2000 | flags::ZERO | flags::CARRY;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0x0001_0000);
    assert_eq!(cpu.sr & 0x0F, flags::OVERFLOW | flags::ZERO);
}

#[test]
fn divide_by_zero_traps_and_clears_carry() {
    /* DIVS D1,D0 with D1 = 0 */
    let (mut cpu, mut memory) = setup(&[0x81C1]);
    cpu.sr = 0x2000 | flags::CARRY;
    let cycles = cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, HANDLER);
    assert_eq!(memory.read_long(cpu.a[7] + 2), 0x1002);
    assert_eq!(memory.read_word(cpu.a[7]) & flags::CARRY, 0);
    assert_eq!(cycles, 38);
}

#[test]
fn bcd_byte_ops_on_the_stack_move_a7_by_two() {
    /* ABCD -(A7),-(A7) */
    let (mut cpu, mut memory) = setup(&[0xCF0F]);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.a[7], 0x8000 - 4);
}

#[test]
fn abcd_sets_n_and_v_from_the_decimal_adjust() {
    /* ABCD D1,D0: $41 + $39 = $80 (uncorrected $7A) */
    let (mut cpu, mut memory) = setup(&[0xC101]);
    cpu.d[0] = 0x41;
    cpu.d[1] = 0x39;
    cpu.sr = 0x2000 | flags::ZERO;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x80);
    assert!(cpu.get_flag(flags::NEGATIVE));
    assert!(cpu.get_flag(flags::OVERFLOW));
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::CARRY));
}

#[test]
fn link_a7_pushes_the_decremented_stack_pointer() {
    /* LINK A7,#-8 */
    let (mut cpu, mut memory) = setup(&[0x4E57, 0xFFF8]);
    cpu.step_instruction(&mut memory);
    assert_eq!(memory.read_long(0x7FFC), 0x7FFC);
    assert_eq!(cpu.a[7], 0x7FF4);
}

#[test]
fn chk_in_range_clears_v_and_c() {
    /* CHK D1,D0 */
    let (mut cpu, mut memory) = setup(&[0x4181]);
    cpu.d[0] = 5;
    cpu.d[1] = 10;
    cpu.sr = 0x2000 | flags::OVERFLOW | flags::CARRY;
    let cycles = cpu.step_instruction(&mut memory);
    assert_eq!(cpu.pc, 0x1002);
    assert_eq!(cpu.sr & (flags::OVERFLOW | flags::CARRY), 0);
    assert_eq!(cycles, 10);
}

#[test]
fn chk_trap_timing_depends_on_which_bound_failed() {
    /* CHK D1,D0: above the bound -> 38, negative -> 40 */
    let (mut cpu, mut memory) = setup(&[0x4181]);
    cpu.d[0] = 20;
    cpu.d[1] = 10;
    assert_eq!(cpu.step_instruction(&mut memory), 38);
    assert!(!cpu.get_flag(flags::NEGATIVE));
    let (mut cpu, mut memory) = setup(&[0x4181]);
    cpu.d[0] = 0xFFFF;
    cpu.d[1] = 10;
    assert_eq!(cpu.step_instruction(&mut memory), 40);
    assert!(cpu.get_flag(flags::NEGATIVE));
}

/// Records every data/program read address.
#[derive(Debug)]
struct ReadLog<'a>(&'a mut Memory, Vec<u32>);

impl MemoryInterface for ReadLog<'_> {
    fn read_byte(&mut self, a: u32) -> u8 {
        self.1.push(a);
        self.0.read_byte(a)
    }
    fn write_byte(&mut self, a: u32, v: u8) {
        self.0.write_byte(a, v)
    }
    fn read_word(&mut self, a: u32) -> u16 {
        self.1.push(a);
        self.0.read_word(a)
    }
    fn write_word(&mut self, a: u32, v: u16) {
        self.0.write_word(a, v)
    }
    fn read_long(&mut self, a: u32) -> u32 {
        self.1.push(a);
        self.0.read_long(a)
    }
    fn write_long(&mut self, a: u32, v: u32) {
        self.0.write_long(a, v)
    }
}

#[test]
fn movem_to_registers_reads_one_extra_word() {
    /* MOVEM.W (A0),D0 */
    let (mut cpu, mut memory) = setup(&[0x4C90, 0x0001]);
    cpu.a[0] = 0x2000;
    memory.write_word(0x2000, 0x8001);
    let mut bus = ReadLog(&mut memory, Vec::new());
    cpu.step_instruction(&mut bus);
    assert!(bus.1.contains(&0x2000));
    assert!(
        bus.1.contains(&0x2002),
        "trailing read after the last register"
    );
    assert_eq!(cpu.d[0], 0xFFFF_8001, "word loads sign-extend");
}

#[test]
fn movem_predecrement_stores_the_initial_address_register() {
    /* MOVEM.L A0,-(A0) */
    let (mut cpu, mut memory) = setup(&[0x48E0, 0x0080]);
    cpu.a[0] = 0x2000;
    cpu.step_instruction(&mut memory);
    assert_eq!(memory.read_long(0x1FFC), 0x2000);
    assert_eq!(cpu.a[0], 0x1FFC);
}

/// 64 KiB RAM mirrored over the whole 32-bit space.
#[derive(Debug)]
struct Mirror(Memory);

impl MemoryInterface for Mirror {
    fn read_byte(&mut self, a: u32) -> u8 {
        self.0.read_byte(a & 0xFFFF)
    }
    fn write_byte(&mut self, a: u32, v: u8) {
        self.0.write_byte(a & 0xFFFF, v)
    }
    fn read_word(&mut self, a: u32) -> u16 {
        self.0.read_word(a & 0xFFFE)
    }
    fn write_word(&mut self, a: u32, v: u16) {
        self.0.write_word(a & 0xFFFE, v)
    }
    fn read_long(&mut self, a: u32) -> u32 {
        ((self.read_word(a) as u32) << 16) | self.read_word(a.wrapping_add(2)) as u32
    }
    fn write_long(&mut self, a: u32, v: u32) {
        self.write_word(a, (v >> 16) as u16);
        self.write_word(a.wrapping_add(2), v as u16);
    }
}

#[test]
fn address_arithmetic_wraps_instead_of_overflowing() {
    let mut mem = Mirror(Memory::new(0x10000));
    let mut cpu = Cpu::new(&mut mem);
    /* BSR.S *+4 at $FFFFFFFE: return address and target wrap */
    mem.write_word(0xFFFE, 0x6102);
    cpu.pc = 0xFFFF_FFFE;
    cpu.a[7] = 0x8000;
    cpu.step_instruction(&mut mem);
    assert_eq!(cpu.pc, 0x0000_0002);
    assert_eq!(mem.read_long(0x7FFC), 0x0000_0000);

    /* MOVEP.W D0,(d16,A0) with A0 = $7FFFFFF0, d16 = $7FFF */
    mem.write_word(0x1000, 0x0188);
    mem.write_word(0x1002, 0x7FFF);
    cpu.pc = 0x1000;
    cpu.a[0] = 0x7FFF_FFF0;
    cpu.d[0] = 0xABCD;
    cpu.step_instruction(&mut mem);
    assert_eq!(mem.read_byte(0x7FEF), 0xAB);
}

/* ---------------------------------------------------------------------- */
/* Timing                                                                   */
/* ---------------------------------------------------------------------- */

#[test]
fn instruction_timing_matches_the_hardware() {
    let cases: &[(&[u16], u32, &str)] = &[
        (&[0x3080], 8, "MOVE.W D0,(A0)"),
        (&[0x3100], 8, "MOVE.W D0,-(A0)"),
        (&[0x2080], 12, "MOVE.L D0,(A0)"),
        (&[0xD150], 12, "ADD.W D0,(A0)"),
        (&[0xD090], 14, "ADD.L (A0),D0"),
        (&[0x0680, 0, 1], 16, "ADDI.L #1,D0"),
        (&[0x0650, 1], 16, "ADDI.W #1,(A0)"),
        (&[0x5290], 20, "ADDQ.L #1,(A0)"),
        (&[0x4480], 6, "NEG.L D0"),
        (&[0x4250], 12, "CLR.W (A0)"),
        (&[0x4690], 20, "NOT.L (A0)"),
        (&[0x0100], 6, "BTST D0,D0"),
        (&[0x0110], 8, "BTST D0,(A0)"),
        (&[0x08C0, 1], 10, "BSET #1,D0"),
        (&[0x08C0, 20], 12, "BSET #20,D0"),
        (&[0x0380], 8, "BCLR D1,D0 (bit 0)"),
        (&[0x08D0, 1], 16, "BSET #1,(A0)"),
        (&[0xC0FC, 0x0000], 42, "MULU #0,D0"),
        (&[0xC0FC, 0xFFFF], 74, "MULU #$FFFF,D0"),
        (&[0xC1FC, 0x5555], 42 + 32, "MULS #$5555,D0"),
        (&[0xE380], 10, "ASL.L #1,D0"),
        (&[0xE088], 24, "LSR.L #8,D0"),
        (&[0xE1D0], 12, "ASL.W (A0)"),
        (&[0x6700, 0x0010], 12, "BEQ.W not taken"),
        (&[0x6600, 0x0010], 10, "BNE.W taken"),
        (&[0x56C0], 6, "SNE D0 (true)"),
        (&[0x4EE8, 0x0010], 10, "JMP d16(A0)"),
        (&[0x4EB9, 0x0000, 0x3000], 20, "JSR abs.L"),
        (&[0x4850], 12, "PEA (A0)"),
        (&[0x4C90, 0x0001], 16, "MOVEM.W (A0),D0"),
        (&[0x4890, 0x0001], 12, "MOVEM.W D0,(A0)"),
        (&[0x4810], 12, "NBCD (A0)"),
        (&[0x41BC, 0x000A], 42, "CHK #10,D0 (trap)"),
        (&[0x80FC, 0x0000], 42, "DIVU #0,D0 (zero divide)"),
    ];
    for (program, expected, name) in cases {
        let (mut cpu, mut memory) = setup(program);
        cpu.a[0] = 0x2000;
        cpu.d[0] = 20;
        cpu.d[1] = 1;
        let cycles = cpu.step_instruction(&mut memory);
        assert_eq!(cycles, *expected, "{name}");
    }
}

#[test]
fn multiply_cycles_follow_operand_bits() {
    assert_eq!(mulu_cycles(0), 38);
    assert_eq!(mulu_cycles(0xFFFF), 70);
    assert_eq!(muls_cycles(0), 38);
    assert_eq!(muls_cycles(0x5555), 70);
    assert_eq!(muls_cycles(0xFFFF), 40);
}

#[test]
fn divide_cycles_follow_the_microcode() {
    assert_eq!(divu_cycles(0x0001_0000, 1), 10, "overflow exits early");
    assert_eq!(divu_cycles(0, 1), 136);
    assert!(divu_cycles(0x0000_FFFF, 0xFFFF) <= 140);
    assert_eq!(divs_cycles(0x7FFF_FFFF, 1), 16, "overflow exits early");
    assert_eq!(divs_cycles(-0x7FFF_FFFF, 1), 18);
    assert!(divs_cycles(100, 7) >= 120 && divs_cycles(100, 7) <= 158);
}
