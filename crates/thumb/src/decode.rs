//! Decoder for the ARMv6-M Thumb instruction set (ARMv6-M Architecture Reference Manual, section A5).
//!
//! [`decode`] takes the first halfword and, for a 32-bit instruction, the second one. Anything that is not an
//! ARMv6-M instruction decodes to `None`; callers stop there instead of guessing.

/// Condition of a conditional branch, in encoding order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cond {
    Eq,
    Ne,
    Hs,
    Lo,
    Mi,
    Pl,
    Vs,
    Vc,
    Hi,
    Ls,
    Ge,
    Lt,
    Gt,
    Le,
}

impl Cond {
    /// The condition with encoding `n` (0-13).
    pub fn from_bits(n: u16) -> Option<Self> {
        Some(match n {
            0 => Self::Eq,
            1 => Self::Ne,
            2 => Self::Hs,
            3 => Self::Lo,
            4 => Self::Mi,
            5 => Self::Pl,
            6 => Self::Vs,
            7 => Self::Vc,
            8 => Self::Hi,
            9 => Self::Ls,
            10 => Self::Ge,
            11 => Self::Lt,
            12 => Self::Gt,
            13 => Self::Le,
            _ => return None,
        })
    }

    /// Encoding of the condition (0-13).
    pub fn bits(self) -> u16 {
        self as u16
    }

    /// Whether the condition holds for the flags `[N, Z, C, V]`.
    pub fn holds(self, [n, z, c, v]: [bool; 4]) -> bool {
        match self {
            Self::Eq => z,
            Self::Ne => !z,
            Self::Hs => c,
            Self::Lo => !c,
            Self::Mi => n,
            Self::Pl => !n,
            Self::Vs => v,
            Self::Vc => !v,
            Self::Hi => c && !z,
            Self::Ls => !c || z,
            Self::Ge => n == v,
            Self::Lt => n != v,
            Self::Gt => !z && n == v,
            Self::Le => z || n != v,
        }
    }
}

/// Data-processing operations of the `010000` group, in encoding order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AluOp {
    And,
    Eor,
    Lsl,
    Lsr,
    Asr,
    Adc,
    Sbc,
    Ror,
    Tst,
    Rsb,
    Cmp,
    Cmn,
    Orr,
    Mul,
    Bic,
    Mvn,
}

impl AluOp {
    const ALL: [Self; 16] = [
        Self::And,
        Self::Eor,
        Self::Lsl,
        Self::Lsr,
        Self::Asr,
        Self::Adc,
        Self::Sbc,
        Self::Ror,
        Self::Tst,
        Self::Rsb,
        Self::Cmp,
        Self::Cmn,
        Self::Orr,
        Self::Mul,
        Self::Bic,
        Self::Mvn,
    ];

    /// Encoding of the operation (bits 9:6).
    pub fn bits(self) -> u16 {
        self as u16
    }
}

/// Width and direction of a load or store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemOp {
    Str,
    Strh,
    Strb,
    Ldrsb,
    Ldr,
    Ldrh,
    Ldrb,
    Ldrsh,
}

impl MemOp {
    /// Bytes moved.
    pub fn size(self) -> u32 {
        match self {
            Self::Str | Self::Ldr => 4,
            Self::Strh | Self::Ldrh | Self::Ldrsh => 2,
            Self::Strb | Self::Ldrb | Self::Ldrsb => 1,
        }
    }

    /// Whether it reads memory.
    pub fn is_load(self) -> bool {
        matches!(
            self,
            Self::Ldrsb | Self::Ldr | Self::Ldrh | Self::Ldrb | Self::Ldrsh
        )
    }
}

/// One decoded instruction. Registers are 0-15; offsets and immediates are already scaled to bytes. Branch
/// offsets are relative to the address of the instruction plus 4 (the value of PC when it executes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// `lsls rd, rm, #imm` (imm 0 is `movs rd, rm`).
    LslImm {
        rd: u8,
        rm: u8,
        imm: u8,
    },
    /// `lsrs rd, rm, #imm` (imm 1-32).
    LsrImm {
        rd: u8,
        rm: u8,
        imm: u8,
    },
    /// `asrs rd, rm, #imm` (imm 1-32).
    AsrImm {
        rd: u8,
        rm: u8,
        imm: u8,
    },
    AddReg {
        rd: u8,
        rn: u8,
        rm: u8,
    },
    SubReg {
        rd: u8,
        rn: u8,
        rm: u8,
    },
    AddImm3 {
        rd: u8,
        rn: u8,
        imm: u8,
    },
    SubImm3 {
        rd: u8,
        rn: u8,
        imm: u8,
    },
    MovImm {
        rd: u8,
        imm: u8,
    },
    CmpImm {
        rn: u8,
        imm: u8,
    },
    AddImm8 {
        rdn: u8,
        imm: u8,
    },
    SubImm8 {
        rdn: u8,
        imm: u8,
    },
    /// Data processing on two low registers; `Rsb` is `rsbs rd, rm, #0`, `Mul` is `muls rdn, rm, rdn`.
    Alu {
        op: AluOp,
        rdn: u8,
        rm: u8,
    },
    /// `add rdn, rm` with any registers, flags unchanged.
    AddHi {
        rdn: u8,
        rm: u8,
    },
    /// `cmp rn, rm` with a high register.
    CmpHi {
        rn: u8,
        rm: u8,
    },
    /// `mov rd, rm`, flags unchanged.
    MovHi {
        rd: u8,
        rm: u8,
    },
    Bx {
        rm: u8,
    },
    Blx {
        rm: u8,
    },
    /// `ldr rt, [pc, #imm]`: address = align(PC, 4) + imm.
    LdrLit {
        rt: u8,
        imm: u16,
    },
    MemReg {
        op: MemOp,
        rt: u8,
        rn: u8,
        rm: u8,
    },
    MemImm {
        op: MemOp,
        rt: u8,
        rn: u8,
        imm: u8,
    },
    StrSp {
        rt: u8,
        imm: u16,
    },
    LdrSp {
        rt: u8,
        imm: u16,
    },
    /// `adr rd, label`: rd = align(PC, 4) + imm.
    Adr {
        rd: u8,
        imm: u16,
    },
    /// `add rd, sp, #imm`.
    AddRdSp {
        rd: u8,
        imm: u16,
    },
    AddSp {
        imm: u16,
    },
    SubSp {
        imm: u16,
    },
    Sxth {
        rd: u8,
        rm: u8,
    },
    Sxtb {
        rd: u8,
        rm: u8,
    },
    Uxth {
        rd: u8,
        rm: u8,
    },
    Uxtb {
        rd: u8,
        rm: u8,
    },
    Rev {
        rd: u8,
        rm: u8,
    },
    Rev16 {
        rd: u8,
        rm: u8,
    },
    Revsh {
        rd: u8,
        rm: u8,
    },
    /// Registers r0-r7 in the low byte, `lr` as bit 14.
    Push {
        regs: u16,
    },
    /// Registers r0-r7 in the low byte, `pc` as bit 15.
    Pop {
        regs: u16,
    },
    /// `cpsid i` (true) or `cpsie i` (false).
    Cps {
        disable: bool,
    },
    Bkpt {
        imm: u8,
    },
    /// NOP, YIELD, WFE, WFI, SEV: no effect on a single-core model.
    Hint {
        op: u8,
    },
    Stm {
        rn: u8,
        regs: u8,
    },
    Ldm {
        rn: u8,
        regs: u8,
    },
    BCond {
        cond: Cond,
        off: i32,
    },
    B {
        off: i32,
    },
    Bl {
        off: i32,
    },
    Svc {
        imm: u8,
    },
    Udf {
        imm: u16,
    },
    Msr {
        sysm: u8,
        rn: u8,
    },
    Mrs {
        rd: u8,
        sysm: u8,
    },
    /// DSB, DMB, ISB: no effect on a single-core model.
    Barrier,
}

/// A decoded instruction and its size in bytes (2 or 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Insn {
    pub op: Op,
    pub size: u8,
}

/// Whether the halfword starts a 32-bit instruction.
pub fn is_wide(hw1: u16) -> bool {
    hw1 >> 11 >= 0b11101
}

#[allow(clippy::cast_possible_truncation)]
fn r(v: u16, shift: u32) -> u8 {
    ((v >> shift) & 7) as u8
}

fn sign_extend(v: u32, bits: u32) -> i32 {
    let s = 32 - bits;
    (v << s).cast_signed() >> s
}

/// Decodes one instruction. `hw2` is the following halfword, needed only when [`is_wide`] says so.
#[allow(clippy::too_many_lines, clippy::cast_possible_truncation)]
pub fn decode(hw1: u16, hw2: Option<u16>) -> Option<Insn> {
    if is_wide(hw1) {
        return decode_wide(hw1, hw2?).map(|op| Insn { op, size: 4 });
    }
    let imm5 = ((hw1 >> 6) & 0x1F) as u8;
    let imm8 = (hw1 & 0xFF) as u8;
    let op = match hw1 >> 11 {
        0b00000 => Op::LslImm {
            rd: r(hw1, 0),
            rm: r(hw1, 3),
            imm: imm5,
        },
        0b00001 => Op::LsrImm {
            rd: r(hw1, 0),
            rm: r(hw1, 3),
            imm: if imm5 == 0 { 32 } else { imm5 },
        },
        0b00010 => Op::AsrImm {
            rd: r(hw1, 0),
            rm: r(hw1, 3),
            imm: if imm5 == 0 { 32 } else { imm5 },
        },
        0b00011 => {
            let (rd, rn, x) = (r(hw1, 0), r(hw1, 3), r(hw1, 6));
            match (hw1 >> 9) & 3 {
                0 => Op::AddReg { rd, rn, rm: x },
                1 => Op::SubReg { rd, rn, rm: x },
                2 => Op::AddImm3 { rd, rn, imm: x },
                _ => Op::SubImm3 { rd, rn, imm: x },
            }
        }
        0b00100 => Op::MovImm {
            rd: r(hw1, 8),
            imm: imm8,
        },
        0b00101 => Op::CmpImm {
            rn: r(hw1, 8),
            imm: imm8,
        },
        0b00110 => Op::AddImm8 {
            rdn: r(hw1, 8),
            imm: imm8,
        },
        0b00111 => Op::SubImm8 {
            rdn: r(hw1, 8),
            imm: imm8,
        },
        0b01000 => {
            if hw1 & 0x0400 == 0 {
                Op::Alu {
                    op: AluOp::ALL[usize::from((hw1 >> 6) & 0xF)],
                    rdn: r(hw1, 0),
                    rm: r(hw1, 3),
                }
            } else {
                let rm = ((hw1 >> 3) & 0xF) as u8;
                let rdn = (r(hw1, 0)) | (((hw1 >> 7) & 1) as u8) << 3;
                match (hw1 >> 8) & 3 {
                    0 => Op::AddHi { rdn, rm },
                    1 => {
                        if rdn < 8 && rm < 8 {
                            return None; // UNPREDICTABLE: two low registers use the T1 encoding
                        }
                        Op::CmpHi { rn: rdn, rm }
                    }
                    2 => Op::MovHi { rd: rdn, rm },
                    _ => {
                        if hw1 & 7 != 0 {
                            return None;
                        }
                        if hw1 & 0x80 == 0 {
                            Op::Bx { rm }
                        } else {
                            Op::Blx { rm }
                        }
                    }
                }
            }
        }
        0b01001 => Op::LdrLit {
            rt: r(hw1, 8),
            imm: u16::from(imm8) * 4,
        },
        0b01010 | 0b01011 => {
            let op = [
                MemOp::Str,
                MemOp::Strh,
                MemOp::Strb,
                MemOp::Ldrsb,
                MemOp::Ldr,
                MemOp::Ldrh,
                MemOp::Ldrb,
                MemOp::Ldrsh,
            ][usize::from((hw1 >> 9) & 7)];
            Op::MemReg {
                op,
                rt: r(hw1, 0),
                rn: r(hw1, 3),
                rm: r(hw1, 6),
            }
        }
        0b01100 => Op::MemImm {
            op: MemOp::Str,
            rt: r(hw1, 0),
            rn: r(hw1, 3),
            imm: imm5 * 4,
        },
        0b01101 => Op::MemImm {
            op: MemOp::Ldr,
            rt: r(hw1, 0),
            rn: r(hw1, 3),
            imm: imm5 * 4,
        },
        0b01110 => Op::MemImm {
            op: MemOp::Strb,
            rt: r(hw1, 0),
            rn: r(hw1, 3),
            imm: imm5,
        },
        0b01111 => Op::MemImm {
            op: MemOp::Ldrb,
            rt: r(hw1, 0),
            rn: r(hw1, 3),
            imm: imm5,
        },
        0b10000 => Op::MemImm {
            op: MemOp::Strh,
            rt: r(hw1, 0),
            rn: r(hw1, 3),
            imm: imm5 * 2,
        },
        0b10001 => Op::MemImm {
            op: MemOp::Ldrh,
            rt: r(hw1, 0),
            rn: r(hw1, 3),
            imm: imm5 * 2,
        },
        0b10010 => Op::StrSp {
            rt: r(hw1, 8),
            imm: u16::from(imm8) * 4,
        },
        0b10011 => Op::LdrSp {
            rt: r(hw1, 8),
            imm: u16::from(imm8) * 4,
        },
        0b10100 => Op::Adr {
            rd: r(hw1, 8),
            imm: u16::from(imm8) * 4,
        },
        0b10101 => Op::AddRdSp {
            rd: r(hw1, 8),
            imm: u16::from(imm8) * 4,
        },
        0b10110 | 0b10111 => decode_misc(hw1)?,
        0b11000 => Op::Stm {
            rn: r(hw1, 8),
            regs: imm8,
        },
        0b11001 => Op::Ldm {
            rn: r(hw1, 8),
            regs: imm8,
        },
        0b11010 | 0b11011 => match (hw1 >> 8) & 0xF {
            0b1110 => Op::Udf { imm: u16::from(imm8) },
            0b1111 => Op::Svc { imm: imm8 },
            c => Op::BCond {
                cond: Cond::from_bits(c)?,
                off: sign_extend(u32::from(imm8) << 1, 9),
            },
        },
        0b11100 => Op::B {
            off: sign_extend(u32::from(hw1 & 0x7FF) << 1, 12),
        },
        _ => return None,
    };
    Some(Insn { op, size: 2 })
}

#[allow(clippy::cast_possible_truncation)]
fn decode_misc(hw1: u16) -> Option<Op> {
    let (rd, rm) = (r(hw1, 0), r(hw1, 3));
    let low = hw1 & 0xFF;
    Some(match (hw1 >> 8) & 0xF {
        0b0000 => {
            let imm = (hw1 & 0x7F) * 4;
            if hw1 & 0x80 == 0 {
                Op::AddSp { imm }
            } else {
                Op::SubSp { imm }
            }
        }
        0b0010 => match (hw1 >> 6) & 3 {
            0 => Op::Sxth { rd, rm },
            1 => Op::Sxtb { rd, rm },
            2 => Op::Uxth { rd, rm },
            _ => Op::Uxtb { rd, rm },
        },
        0b0100 | 0b0101 => Op::Push {
            regs: low | ((hw1 >> 8) & 1) << 14,
        },
        0b0110 => {
            if hw1 & 0xFFEF == 0xB662 {
                Op::Cps {
                    disable: hw1 & 0x10 != 0,
                }
            } else {
                return None;
            }
        }
        0b1010 => match (hw1 >> 6) & 3 {
            0 => Op::Rev { rd, rm },
            1 => Op::Rev16 { rd, rm },
            3 => Op::Revsh { rd, rm },
            _ => return None,
        },
        0b1100 | 0b1101 => Op::Pop {
            regs: low | ((hw1 >> 8) & 1) << 15,
        },
        0b1110 => Op::Bkpt { imm: low as u8 },
        0b1111 => {
            if low & 0xF != 0 || low >> 4 > 4 {
                return None;
            }
            Op::Hint { op: (low >> 4) as u8 }
        }
        _ => return None,
    })
}

#[allow(clippy::cast_possible_truncation)]
fn decode_wide(hw1: u16, hw2: u16) -> Option<Op> {
    if hw1 >> 11 == 0b11110 && hw2 & 0xD000 == 0xD000 {
        let s = u32::from((hw1 >> 10) & 1);
        let j1 = u32::from((hw2 >> 13) & 1);
        let j2 = u32::from((hw2 >> 11) & 1);
        let i1 = 1 - (j1 ^ s);
        let i2 = 1 - (j2 ^ s);
        let imm = (s << 24)
            | (i1 << 23)
            | (i2 << 22)
            | (u32::from(hw1 & 0x3FF) << 12)
            | (u32::from(hw2 & 0x7FF) << 1);
        return Some(Op::Bl {
            off: sign_extend(imm, 25),
        });
    }
    if hw1 & 0xFFF0 == 0xF380 && hw2 & 0xFF00 == 0x8800 {
        return Some(Op::Msr {
            sysm: (hw2 & 0xFF) as u8,
            rn: (hw1 & 0xF) as u8,
        });
    }
    if hw1 == 0xF3EF && hw2 & 0xF000 == 0x8000 {
        return Some(Op::Mrs {
            rd: ((hw2 >> 8) & 0xF) as u8,
            sysm: (hw2 & 0xFF) as u8,
        });
    }
    if hw1 == 0xF3BF && matches!(hw2 & 0xFFF0, 0x8F40..=0x8F60) {
        return Some(Op::Barrier);
    }
    if hw1 & 0xFFF0 == 0xF7F0 && hw2 & 0xF000 == 0xA000 {
        return Some(Op::Udf {
            imm: ((hw1 & 0xF) << 12) | (hw2 & 0xFFF),
        });
    }
    None
}

/// Decodes the instruction at `addr` in `mem` (little-endian halfwords); `None` past the end or when it is not
/// an ARMv6-M instruction.
pub fn decode_at(mem: &[u8], addr: usize) -> Option<Insn> {
    let hw = |a: usize| Some(u16::from_le_bytes([*mem.get(a)?, *mem.get(a + 1)?]));
    let hw1 = hw(addr)?;
    decode(hw1, if is_wide(hw1) { hw(addr + 2) } else { None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_a_few_known_encodings() {
        assert_eq!(
            decode(0xB5F0, None).unwrap().op,
            Op::Push { regs: 0xF0 | 1 << 14 }
        );
        assert_eq!(decode(0xBDF0, None).unwrap().op, Op::Pop { regs: 0xF0 | 1 << 15 });
        assert_eq!(decode(0x4770, None).unwrap().op, Op::Bx { rm: 14 });
        assert_eq!(decode(0x4604, None).unwrap().op, Op::MovHi { rd: 4, rm: 0 });
        assert_eq!(
            decode(0xD0FE, None).unwrap().op,
            Op::BCond {
                cond: Cond::Eq,
                off: -4
            }
        );
        assert_eq!(decode(0xE7FE, None).unwrap().op, Op::B { off: -4 });
        assert_eq!(decode(0x4A01, None).unwrap().op, Op::LdrLit { rt: 2, imm: 4 });
        assert_eq!(decode(0xB282, None).unwrap().op, Op::Uxth { rd: 2, rm: 0 });
        assert_eq!(decode(0xBF00, None).unwrap().op, Op::Hint { op: 0 });
        assert_eq!(decode(0xB672, None).unwrap().op, Op::Cps { disable: true });
        // bl from 0x7cb6 to 0x6984 in the stock image
        assert_eq!(
            decode(0xF7FE, Some(0xFE65)).unwrap().op,
            Op::Bl { off: 0x6984 - 0x7CBA }
        );
        assert_eq!(
            decode(0xF3EF, Some(0x8010)).unwrap().op,
            Op::Mrs { rd: 0, sysm: 0x10 }
        );
        assert_eq!(
            decode(0xF380, Some(0x8810)).unwrap().op,
            Op::Msr { sysm: 0x10, rn: 0 }
        );
        assert!(decode(0xF3BF, Some(0x8F4F)).is_some());
    }

    #[test]
    fn refuses_what_a_cortex_m0_does_not_run() {
        assert!(decode(0xB100, None).is_none()); // cbz (ARMv7-M)
        assert!(decode(0xBF08, None).is_none()); // it eq (ARMv7-M)
        assert!(decode(0xE800, None).is_none()); // 32-bit prefix
        assert!(decode(0xF000, Some(0x8000)).is_none()); // Thumb-2 data processing
        assert!(decode(0x4501, None).is_none()); // cmp hi form with two low registers
    }
}
