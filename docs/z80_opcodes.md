# Z80 Opcode Reference

This document provides a complete reference for the Z80 instruction set as implemented in `genteel`.

## Table of Contents

1. [Opcode Encoding](#opcode-encoding)
2. [Prefixes](#prefixes)
3. [Main Opcodes (No Prefix)](#main-opcodes-no-prefix)
4. [CB Prefix (Bit Operations)](#cb-prefix-bit-operations)
5. [ED Prefix (Extended)](#ed-prefix-extended)
6. [DD Prefix (IX Index)](#dd-prefix-ix-index)
7. [FD Prefix (IY Index)](#fd-prefix-iy-index)
8. [DD CB / FD CB Prefix (Indexed Bit Operations)](#dd-cb--fd-cb-prefix-indexed-bit-operations)
9. [Flags](#flags)
10. [Condition Codes](#condition-codes)

---

## Opcode Encoding

Z80 opcodes follow a structured encoding pattern. The main opcode byte can be decoded as:

```
Bit:  7  6  5  4  3  2  1  0
      [  x  ] [   y   ] [ z ]
              [ p ] [q]
```

Where:
- `x` = bits 7-6 (0-3)
- `y` = bits 5-3 (0-7)
- `z` = bits 2-0 (0-7)
- `p` = bits 5-4 (y >> 1, 0-3)
- `q` = bit 3 (y & 1, 0-1)

### Register Encoding (r)

| Index | 8-bit Reg | 16-bit Pair (rp) | 16-bit Pair 2 (rp2) |
|-------|-----------|------------------|---------------------|
| 0     | B         | BC               | BC                  |
| 1     | C         | DE               | DE                  |
| 2     | D         | HL               | HL                  |
| 3     | E         | SP               | AF                  |
| 4     | H         | -                | -                   |
| 5     | L         | -                | -                   |
| 6     | (HL)      | -                | -                   |
| 7     | A         | -                | -                   |

---

## Prefixes

The Z80 uses prefix bytes to extend the instruction set:

| Prefix | Hex  | Purpose |
|--------|------|---------|
| None   | -    | Main instruction set |
| CB     | 0xCB | Bit manipulation, rotates, shifts |
| ED     | 0xED | Extended instructions (block ops, I/O, etc.) |
| DD     | 0xDD | Use IX instead of HL |
| FD     | 0xFD | Use IY instead of HL |
| DD CB  | 0xDD 0xCB | Bit ops on (IX+d) |
| FD CB  | 0xFD 0xCB | Bit ops on (IY+d) |

### Prefix Stacking Rules

- DD/FD can prefix CB to form DD CB / FD CB
- DD/FD before ED is ignored (ED takes precedence)
- Multiple DD or FD: only the last one applies
- DD followed by FD: FD wins (and vice versa)

---

## Main Opcodes (No Prefix)

### x=0 Instructions

| Opcode | Hex  | Mnemonic | Description | Cycles |
|--------|------|----------|-------------|--------|
| 00     | 0x00 | NOP | No operation | 4 |
| 01     | 0x01 | LD BC, nn | Load 16-bit immediate to BC | 10 |
| 02     | 0x02 | LD (BC), A | Store A at address BC | 7 |
| 03     | 0x03 | INC BC | Increment BC | 6 |
| 04     | 0x04 | INC B | Increment B | 4 |
| 05     | 0x05 | DEC B | Decrement B | 4 |
| 06     | 0x06 | LD B, n | Load 8-bit immediate to B | 7 |
| 07     | 0x07 | RLCA | Rotate A left circular | 4 |
| 08     | 0x08 | EX AF, AF' | Exchange AF with AF' | 4 |
| 09     | 0x09 | ADD HL, BC | Add BC to HL | 11 |
| 0A     | 0x0A | LD A, (BC) | Load A from address BC | 7 |
| 0B     | 0x0B | DEC BC | Decrement BC | 6 |
| 0C     | 0x0C | INC C | Increment C | 4 |
| 0D     | 0x0D | DEC C | Decrement C | 4 |
| 0E     | 0x0E | LD C, n | Load 8-bit immediate to C | 7 |
| 0F     | 0x0F | RRCA | Rotate A right circular | 4 |
| 10     | 0x10 | DJNZ d | Decrement B, jump if not zero | 13/8 |
| 11     | 0x11 | LD DE, nn | Load 16-bit immediate to DE | 10 |
| 12     | 0x12 | LD (DE), A | Store A at address DE | 7 |
| 13     | 0x13 | INC DE | Increment DE | 6 |
| 14     | 0x14 | INC D | Increment D | 4 |
| 15     | 0x15 | DEC D | Decrement D | 4 |
| 16     | 0x16 | LD D, n | Load 8-bit immediate to D | 7 |
| 17     | 0x17 | RLA | Rotate A left through carry | 4 |
| 18     | 0x18 | JR d | Relative jump | 12 |
| 19     | 0x19 | ADD HL, DE | Add DE to HL | 11 |
| 1A     | 0x1A | LD A, (DE) | Load A from address DE | 7 |
| 1B     | 0x1B | DEC DE | Decrement DE | 6 |
| 1C     | 0x1C | INC E | Increment E | 4 |
| 1D     | 0x1D | DEC E | Decrement E | 4 |
| 1E     | 0x1E | LD E, n | Load 8-bit immediate to E | 7 |
| 1F     | 0x1F | RRA | Rotate A right through carry | 4 |
| 20     | 0x20 | JR NZ, d | Jump if not zero | 12/7 |
| 21     | 0x21 | LD HL, nn | Load 16-bit immediate to HL | 10 |
| 22     | 0x22 | LD (nn), HL | Store HL at address nn | 16 |
| 23     | 0x23 | INC HL | Increment HL | 6 |
| 24     | 0x24 | INC H | Increment H | 4 |
| 25     | 0x25 | DEC H | Decrement H | 4 |
| 26     | 0x26 | LD H, n | Load 8-bit immediate to H | 7 |
| 27     | 0x27 | DAA | Decimal adjust A | 4 |
| 28     | 0x28 | JR Z, d | Jump if zero | 12/7 |
| 29     | 0x29 | ADD HL, HL | Add HL to HL | 11 |
| 2A     | 0x2A | LD HL, (nn) | Load HL from address nn | 16 |
| 2B     | 0x2B | DEC HL | Decrement HL | 6 |
| 2C     | 0x2C | INC L | Increment L | 4 |
| 2D     | 0x2D | DEC L | Decrement L | 4 |
| 2E     | 0x2E | LD L, n | Load 8-bit immediate to L | 7 |
| 2F     | 0x2F | CPL | Complement A | 4 |
| 30     | 0x30 | JR NC, d | Jump if no carry | 12/7 |
| 31     | 0x31 | LD SP, nn | Load 16-bit immediate to SP | 10 |
| 32     | 0x32 | LD (nn), A | Store A at address nn | 13 |
| 33     | 0x33 | INC SP | Increment SP | 6 |
| 34     | 0x34 | INC (HL) | Increment memory at HL | 11 |
| 35     | 0x35 | DEC (HL) | Decrement memory at HL | 11 |
| 36     | 0x36 | LD (HL), n | Load immediate to memory at HL | 10 |
| 37     | 0x37 | SCF | Set carry flag | 4 |
| 38     | 0x38 | JR C, d | Jump if carry | 12/7 |
| 39     | 0x39 | ADD HL, SP | Add SP to HL | 11 |
| 3A     | 0x3A | LD A, (nn) | Load A from address nn | 13 |
| 3B     | 0x3B | DEC SP | Decrement SP | 6 |
| 3C     | 0x3C | INC A | Increment A | 4 |
| 3D     | 0x3D | DEC A | Decrement A | 4 |
| 3E     | 0x3E | LD A, n | Load 8-bit immediate to A | 7 |
| 3F     | 0x3F | CCF | Complement carry flag | 4 |

### x=1 Instructions (LD r, r' and HALT)

| Opcode Range | Mnemonic | Description | Cycles |
|--------------|----------|-------------|--------|
| 40-7F (except 76) | LD r, r' | Copy register to register | 4 (7 if (HL)) |
| 76 | HALT | Halt CPU | 4 |

**Full LD r, r' table:**

| | B | C | D | E | H | L | (HL) | A |
|---|---|---|---|---|---|---|------|---|
| **to B** | 40 | 41 | 42 | 43 | 44 | 45 | 46 | 47 |
| **to C** | 48 | 49 | 4A | 4B | 4C | 4D | 4E | 4F |
| **to D** | 50 | 51 | 52 | 53 | 54 | 55 | 56 | 57 |
| **to E** | 58 | 59 | 5A | 5B | 5C | 5D | 5E | 5F |
| **to H** | 60 | 61 | 62 | 63 | 64 | 65 | 66 | 67 |
| **to L** | 68 | 69 | 6A | 6B | 6C | 6D | 6E | 6F |
| **to (HL)** | 70 | 71 | 72 | 73 | 74 | 75 | HALT | 77 |
| **to A** | 78 | 79 | 7A | 7B | 7C | 7D | 7E | 7F |

### x=2 Instructions (ALU A, r)

| y | Operation | Flags Affected |
|---|-----------|----------------|
| 0 | ADD A, r | S, Z, H, P/V, N=0, C |
| 1 | ADC A, r | S, Z, H, P/V, N=0, C |
| 2 | SUB r | S, Z, H, P/V, N=1, C |
| 3 | SBC A, r | S, Z, H, P/V, N=1, C |
| 4 | AND r | S, Z, H=1, P, N=0, C=0 |
| 5 | XOR r | S, Z, H=0, P, N=0, C=0 |
| 6 | OR r | S, Z, H=0, P, N=0, C=0 |
| 7 | CP r | S, Z, H, P/V, N=1, C |

**Opcode = 0x80 + (y * 8) + z**

| | B | C | D | E | H | L | (HL) | A |
|---|---|---|---|---|---|---|------|---|
| **ADD** | 80 | 81 | 82 | 83 | 84 | 85 | 86 | 87 |
| **ADC** | 88 | 89 | 8A | 8B | 8C | 8D | 8E | 8F |
| **SUB** | 90 | 91 | 92 | 93 | 94 | 95 | 96 | 97 |
| **SBC** | 98 | 99 | 9A | 9B | 9C | 9D | 9E | 9F |
| **AND** | A0 | A1 | A2 | A3 | A4 | A5 | A6 | A7 |
| **XOR** | A8 | A9 | AA | AB | AC | AD | AE | AF |
| **OR** | B0 | B1 | B2 | B3 | B4 | B5 | B6 | B7 |
| **CP** | B8 | B9 | BA | BB | BC | BD | BE | BF |

### x=3 Instructions

| Opcode | Hex  | Mnemonic | Description | Cycles |
|--------|------|----------|-------------|--------|
| C0 | 0xC0 | RET NZ | Return if not zero | 11/5 |
| C1 | 0xC1 | POP BC | Pop BC from stack | 10 |
| C2 | 0xC2 | JP NZ, nn | Jump if not zero | 10 |
| C3 | 0xC3 | JP nn | Unconditional jump | 10 |
| C4 | 0xC4 | CALL NZ, nn | Call if not zero | 17/10 |
| C5 | 0xC5 | PUSH BC | Push BC to stack | 11 |
| C6 | 0xC6 | ADD A, n | Add immediate to A | 7 |
| C7 | 0xC7 | RST 00H | Restart at 0x0000 | 11 |
| C8 | 0xC8 | RET Z | Return if zero | 11/5 |
| C9 | 0xC9 | RET | Unconditional return | 10 |
| CA | 0xCA | JP Z, nn | Jump if zero | 10 |
| CB | 0xCB | *prefix* | CB prefix | - |
| CC | 0xCC | CALL Z, nn | Call if zero | 17/10 |
| CD | 0xCD | CALL nn | Unconditional call | 17 |
| CE | 0xCE | ADC A, n | Add with carry immediate | 7 |
| CF | 0xCF | RST 08H | Restart at 0x0008 | 11 |
| D0 | 0xD0 | RET NC | Return if no carry | 11/5 |
| D1 | 0xD1 | POP DE | Pop DE from stack | 10 |
| D2 | 0xD2 | JP NC, nn | Jump if no carry | 10 |
| D3 | 0xD3 | OUT (n), A | Output A to port n | 11 |
| D4 | 0xD4 | CALL NC, nn | Call if no carry | 17/10 |
| D5 | 0xD5 | PUSH DE | Push DE to stack | 11 |
| D6 | 0xD6 | SUB n | Subtract immediate from A | 7 |
| D7 | 0xD7 | RST 10H | Restart at 0x0010 | 11 |
| D8 | 0xD8 | RET C | Return if carry | 11/5 |
| D9 | 0xD9 | EXX | Exchange BC/DE/HL with BC'/DE'/HL' | 4 |
| DA | 0xDA | JP C, nn | Jump if carry | 10 |
| DB | 0xDB | IN A, (n) | Input from port n to A | 11 |
| DC | 0xDC | CALL C, nn | Call if carry | 17/10 |
| DD | 0xDD | *prefix* | DD prefix (IX) | - |
| DE | 0xDE | SBC A, n | Subtract with carry immediate | 7 |
| DF | 0xDF | RST 18H | Restart at 0x0018 | 11 |
| E0 | 0xE0 | RET PO | Return if parity odd | 11/5 |
| E1 | 0xE1 | POP HL | Pop HL from stack | 10 |
| E2 | 0xE2 | JP PO, nn | Jump if parity odd | 10 |
| E3 | 0xE3 | EX (SP), HL | Exchange top of stack with HL | 19 |
| E4 | 0xE4 | CALL PO, nn | Call if parity odd | 17/10 |
| E5 | 0xE5 | PUSH HL | Push HL to stack | 11 |
| E6 | 0xE6 | AND n | AND immediate with A | 7 |
| E7 | 0xE7 | RST 20H | Restart at 0x0020 | 11 |
| E8 | 0xE8 | RET PE | Return if parity even | 11/5 |
| E9 | 0xE9 | JP (HL) | Jump to address in HL | 4 |
| EA | 0xEA | JP PE, nn | Jump if parity even | 10 |
| EB | 0xEB | EX DE, HL | Exchange DE and HL | 4 |
| EC | 0xEC | CALL PE, nn | Call if parity even | 17/10 |
| ED | 0xED | *prefix* | ED prefix (extended) | - |
| EE | 0xEE | XOR n | XOR immediate with A | 7 |
| EF | 0xEF | RST 28H | Restart at 0x0028 | 11 |
| F0 | 0xF0 | RET P | Return if positive | 11/5 |
| F1 | 0xF1 | POP AF | Pop AF from stack | 10 |
| F2 | 0xF2 | JP P, nn | Jump if positive | 10 |
| F3 | 0xF3 | DI | Disable interrupts | 4 |
| F4 | 0xF4 | CALL P, nn | Call if positive | 17/10 |
| F5 | 0xF5 | PUSH AF | Push AF to stack | 11 |
| F6 | 0xF6 | OR n | OR immediate with A | 7 |
| F7 | 0xF7 | RST 30H | Restart at 0x0030 | 11 |
| F8 | 0xF8 | RET M | Return if minus | 11/5 |
| F9 | 0xF9 | LD SP, HL | Load SP from HL | 6 |
| FA | 0xFA | JP M, nn | Jump if minus | 10 |
| FB | 0xFB | EI | Enable interrupts | 4 |
| FC | 0xFC | CALL M, nn | Call if minus | 17/10 |
| FD | 0xFD | *prefix* | FD prefix (IY) | - |
| FE | 0xFE | CP n | Compare immediate with A | 7 |
| FF | 0xFF | RST 38H | Restart at 0x0038 | 11 |

---

## CB Prefix (Bit Operations)

After CB, the opcode is decoded as:

| x | Operation Type |
|---|----------------|
| 0 | Rotate/Shift |
| 1 | BIT (test bit) |
| 2 | RES (reset bit) |
| 3 | SET (set bit) |

### x=0: Rotate/Shift Operations

| y | Mnemonic | Operation | Carry |
|---|----------|-----------|-------|
| 0 | RLC r | Rotate left circular | bit 7 |
| 1 | RRC r | Rotate right circular | bit 0 |
| 2 | RL r | Rotate left through carry | bit 7 |
| 3 | RR r | Rotate right through carry | bit 0 |
| 4 | SLA r | Shift left arithmetic | bit 7 |
| 5 | SRA r | Shift right arithmetic | bit 0 |
| 6 | SLL r | Shift left logical (undoc.) | bit 7 |
| 7 | SRL r | Shift right logical | bit 0 |

**Opcode table (CB xx):**

| | B | C | D | E | H | L | (HL) | A |
|---|---|---|---|---|---|---|------|---|
| **RLC** | 00 | 01 | 02 | 03 | 04 | 05 | 06 | 07 |
| **RRC** | 08 | 09 | 0A | 0B | 0C | 0D | 0E | 0F |
| **RL** | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 |
| **RR** | 18 | 19 | 1A | 1B | 1C | 1D | 1E | 1F |
| **SLA** | 20 | 21 | 22 | 23 | 24 | 25 | 26 | 27 |
| **SRA** | 28 | 29 | 2A | 2B | 2C | 2D | 2E | 2F |
| **SLL** | 30 | 31 | 32 | 33 | 34 | 35 | 36 | 37 |
| **SRL** | 38 | 39 | 3A | 3B | 3C | 3D | 3E | 3F |

### x=1, 2, 3: Bit Operations

**BIT b, r** (test bit b of register r):
- Opcode = 0x40 + (b * 8) + r
- Range: 0x40-0x7F

**RES b, r** (reset bit b of register r):
- Opcode = 0x80 + (b * 8) + r
- Range: 0x80-0xBF

**SET b, r** (set bit b of register r):
- Opcode = 0xC0 + (b * 8) + r
- Range: 0xC0-0xFF

---

## ED Prefix (Extended)

### ED 4x-7x Instructions

| Opcode | Hex  | Mnemonic | Description | Cycles |
|--------|------|----------|-------------|--------|
| 40 | ED 40 | IN B, (C) | Input from port C to B | 12 |
| 41 | ED 41 | OUT (C), B | Output B to port C | 12 |
| 42 | ED 42 | SBC HL, BC | Subtract BC from HL with carry | 15 |
| 43 | ED 43 | LD (nn), BC | Store BC at address nn | 20 |
| 44 | ED 44 | NEG | Negate A | 8 |
| 45 | ED 45 | RETN | Return from NMI | 14 |
| 46 | ED 46 | IM 0 | Interrupt mode 0 | 8 |
| 47 | ED 47 | LD I, A | Load I from A | 9 |
| 48 | ED 48 | IN C, (C) | Input from port C to C | 12 |
| 49 | ED 49 | OUT (C), C | Output C to port C | 12 |
| 4A | ED 4A | ADC HL, BC | Add BC to HL with carry | 15 |
| 4B | ED 4B | LD BC, (nn) | Load BC from address nn | 20 |
| 4C | ED 4C | NEG | Negate A (undoc.) | 8 |
| 4D | ED 4D | RETI | Return from interrupt | 14 |
| 4E | ED 4E | IM 0 | Interrupt mode 0 (undoc.) | 8 |
| 4F | ED 4F | LD R, A | Load R from A | 9 |
| 50 | ED 50 | IN D, (C) | Input from port C to D | 12 |
| 51 | ED 51 | OUT (C), D | Output D to port C | 12 |
| 52 | ED 52 | SBC HL, DE | Subtract DE from HL with carry | 15 |
| 53 | ED 53 | LD (nn), DE | Store DE at address nn | 20 |
| 56 | ED 56 | IM 1 | Interrupt mode 1 | 8 |
| 57 | ED 57 | LD A, I | Load A from I | 9 |
| 58 | ED 58 | IN E, (C) | Input from port C to E | 12 |
| 59 | ED 59 | OUT (C), E | Output E to port C | 12 |
| 5A | ED 5A | ADC HL, DE | Add DE to HL with carry | 15 |
| 5B | ED 5B | LD DE, (nn) | Load DE from address nn | 20 |
| 5E | ED 5E | IM 2 | Interrupt mode 2 | 8 |
| 5F | ED 5F | LD A, R | Load A from R | 9 |
| 60 | ED 60 | IN H, (C) | Input from port C to H | 12 |
| 61 | ED 61 | OUT (C), H | Output H to port C | 12 |
| 62 | ED 62 | SBC HL, HL | Subtract HL from HL with carry | 15 |
| 63 | ED 63 | LD (nn), HL | Store HL at address nn | 20 |
| 67 | ED 67 | RRD | Rotate right decimal | 18 |
| 68 | ED 68 | IN L, (C) | Input from port C to L | 12 |
| 69 | ED 69 | OUT (C), L | Output L to port C | 12 |
| 6A | ED 6A | ADC HL, HL | Add HL to HL with carry | 15 |
| 6B | ED 6B | LD HL, (nn) | Load HL from address nn | 20 |
| 6F | ED 6F | RLD | Rotate left decimal | 18 |
| 70 | ED 70 | IN F, (C) | Input from port C (flags only) | 12 |
| 71 | ED 71 | OUT (C), 0 | Output 0 to port C | 12 |
| 72 | ED 72 | SBC HL, SP | Subtract SP from HL with carry | 15 |
| 73 | ED 73 | LD (nn), SP | Store SP at address nn | 20 |
| 78 | ED 78 | IN A, (C) | Input from port C to A | 12 |
| 79 | ED 79 | OUT (C), A | Output A to port C | 12 |
| 7A | ED 7A | ADC HL, SP | Add SP to HL with carry | 15 |
| 7B | ED 7B | LD SP, (nn) | Load SP from address nn | 20 |

### ED Ax-Bx Block Instructions

| Opcode | Hex  | Mnemonic | Description | Cycles |
|--------|------|----------|-------------|--------|
| A0 | ED A0 | LDI | Load and increment | 16 |
| A1 | ED A1 | CPI | Compare and increment | 16 |
| A2 | ED A2 | INI | Input and increment | 16 |
| A3 | ED A3 | OUTI | Output and increment | 16 |
| A8 | ED A8 | LDD | Load and decrement | 16 |
| A9 | ED A9 | CPD | Compare and decrement | 16 |
| AA | ED AA | IND | Input and decrement | 16 |
| AB | ED AB | OUTD | Output and decrement | 16 |
| B0 | ED B0 | LDIR | Load, increment, repeat | 21/16 |
| B1 | ED B1 | CPIR | Compare, increment, repeat | 21/16 |
| B2 | ED B2 | INIR | Input, increment, repeat | 21/16 |
| B3 | ED B3 | OTIR | Output, increment, repeat | 21/16 |
| B8 | ED B8 | LDDR | Load, decrement, repeat | 21/16 |
| B9 | ED B9 | CPDR | Compare, decrement, repeat | 21/16 |
| BA | ED BA | INDR | Input, decrement, repeat | 21/16 |
| BB | ED BB | OTDR | Output, decrement, repeat | 21/16 |

---

## DD Prefix (IX Index)

The DD prefix modifies instructions to use IX instead of HL:

| Original | With DD Prefix | Description |
|----------|----------------|-------------|
| LD HL, nn | LD IX, nn | Load 16-bit to IX |
| LD (nn), HL | LD (nn), IX | Store IX |
| LD HL, (nn) | LD IX, (nn) | Load IX from memory |
| INC HL | INC IX | Increment IX |
| DEC HL | DEC IX | Decrement IX |
| ADD HL, rr | ADD IX, rr | Add to IX |
| POP HL | POP IX | Pop IX |
| PUSH HL | PUSH IX | Push IX |
| EX (SP), HL | EX (SP), IX | Exchange with stack |
| JP (HL) | JP (IX) | Jump to IX |
| LD SP, HL | LD SP, IX | Load SP from IX |

### Indexed Addressing (IX+d)

| Opcode | Mnemonic | Description |
|--------|----------|-------------|
| DD 34 d | INC (IX+d) | Increment memory |
| DD 35 d | DEC (IX+d) | Decrement memory |
| DD 36 d n | LD (IX+d), n | Load immediate to memory |
| DD 46 d | LD B, (IX+d) | Load register from memory |
| DD 4E d | LD C, (IX+d) | Load register from memory |
| DD 56 d | LD D, (IX+d) | Load register from memory |
| DD 5E d | LD E, (IX+d) | Load register from memory |
| DD 66 d | LD H, (IX+d) | Load register from memory |
| DD 6E d | LD L, (IX+d) | Load register from memory |
| DD 7E d | LD A, (IX+d) | Load register from memory |
| DD 70 d | LD (IX+d), B | Store register to memory |
| DD 71 d | LD (IX+d), C | Store register to memory |
| DD 72 d | LD (IX+d), D | Store register to memory |
| DD 73 d | LD (IX+d), E | Store register to memory |
| DD 74 d | LD (IX+d), H | Store register to memory |
| DD 75 d | LD (IX+d), L | Store register to memory |
| DD 77 d | LD (IX+d), A | Store register to memory |

---

## FD Prefix (IY Index)

The FD prefix works identically to DD but uses IY instead of IX.

All instructions from the DD section apply with:
- IX → IY
- DD → FD

---

## DD CB / FD CB Prefix (Indexed Bit Operations)

Format: `DD CB d op` or `FD CB d op`

Where:
- `d` = signed displacement (-128 to +127)
- `op` = bit operation opcode

These perform CB-prefix operations on (IX+d) or (IY+d).

**Rotate/Shift on (IX+d):**
| Op | Mnemonic |
|----|----------|
| 06 | RLC (IX+d) |
| 0E | RRC (IX+d) |
| 16 | RL (IX+d) |
| 1E | RR (IX+d) |
| 26 | SLA (IX+d) |
| 2E | SRA (IX+d) |
| 36 | SLL (IX+d) |
| 3E | SRL (IX+d) |

**Bit operations on (IX+d):**
| Op Range | Mnemonic |
|----------|----------|
| 46, 4E, 56, 5E, 66, 6E, 76, 7E | BIT b, (IX+d) |
| 86, 8E, 96, 9E, A6, AE, B6, BE | RES b, (IX+d) |
| C6, CE, D6, DE, E6, EE, F6, FE | SET b, (IX+d) |

**Undocumented:** Using other `z` values (not 6) stores result to that register too.

---

## Flags

| Flag | Bit | Symbol | Description |
|------|-----|--------|-------------|
| Carry | 0 | C | Set if carry/borrow occurred |
| Add/Subtract | 1 | N | Set if last op was subtraction |
| Parity/Overflow | 2 | P/V | Parity (logic) or Overflow (arith) |
| (Unused) | 3 | X | Copy of bit 3 of result |
| Half Carry | 4 | H | Carry from bit 3 to bit 4 |
| (Unused) | 5 | Y | Copy of bit 5 of result |
| Zero | 6 | Z | Set if result is zero |
| Sign | 7 | S | Set if result is negative |

---

## Condition Codes

| cc | Condition | Flag Test |
|----|-----------|-----------|
| 0 | NZ | Z = 0 |
| 1 | Z | Z = 1 |
| 2 | NC | C = 0 |
| 3 | C | C = 1 |
| 4 | PO | P/V = 0 |
| 5 | PE | P/V = 1 |
| 6 | P | S = 0 |
| 7 | M | S = 1 |

---

*This document is part of the genteel emulator project.*
