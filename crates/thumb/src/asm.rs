//! Assembler for the Thumb source of the firmware patches.
//!
//! Syntax (GNU / Keystone style, the subset the patch sources use): one statement per line, `@` starts a
//! comment, `label:` may precede a statement on the same line, registers `r0`-`r12`, `sp`, `lr`, `pc`,
//! immediates `#n` (decimal or `0x` hex), memory operands `[rn, #imm]`, `[rn, rm]` and `[rn]`, register lists
//! `{r4, r5, lr}`. Branch and `bl` targets are labels or absolute addresses. Directives: `.align n` (pads with
//! zero bytes) and `.word value`.
//!
//! Encoding choices follow Keystone (LLVM MC) for the same source, which is what the golden images were built
//! with: `mov` between registers is the flag-preserving `mov` (T1 high-register form) even for two low
//! registers; three-operand `adds` / `subs` with an immediate use the 3-bit immediate form when it fits and the
//! 8-bit form when the destination is the source and it does not; `ldr rX, label` is PC-relative. Only 16-bit
//! encodings are emitted, plus `bl`; out-of-range operands are errors.

use std::collections::BTreeMap;
use std::fmt;

use crate::decode::{AluOp, Cond};

/// Assembled code and the address of every label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub base: u32,
    pub code: Vec<u8>,
    pub symbols: BTreeMap<String, u32>,
}

impl Program {
    /// Address of a label.
    pub fn symbol(&self, name: &str) -> Result<u32, AsmError> {
        self.symbols.get(name).copied().ok_or_else(|| AsmError {
            line: 0,
            msg: format!("no label {name}"),
        })
    }
}

/// An error with the 1-based source line it comes from (0 when not tied to a line).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsmError {
    pub line: usize,
    pub msg: String,
}

impl fmt::Display for AsmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "{}", self.msg)
        } else {
            write!(f, "line {}: {}", self.line, self.msg)
        }
    }
}

impl std::error::Error for AsmError {}

/// Thumb `bl` (T1) at `src` to `dst`, as the two halfwords in memory order.
///
/// # Panics
///
/// If `dst` is odd or out of the +-16 MB range, which cannot happen for two addresses of a 64 KB image.
pub fn bl(src: u32, dst: u32) -> [u8; 4] {
    let off = i64::from(dst) - (i64::from(src) + 4);
    assert!(
        (-(1 << 24)..(1 << 24)).contains(&off) && off & 1 == 0,
        "bl out of range"
    );
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let imm = (off as u32) & 0x1FF_FFFF;
    let sign = u32::from(off < 0);
    let (i1, i2) = ((imm >> 23) & 1, (imm >> 22) & 1);
    let (imm10, imm11) = ((imm >> 12) & 0x3FF, (imm >> 1) & 0x7FF);
    let (j1, j2) = (!(i1 ^ sign) & 1, !(i2 ^ sign) & 1);
    let hi = 0xF000 | (sign << 10) | imm10;
    let lo = 0xD000 | (j1 << 13) | (j2 << 11) | imm11;
    let mut out = [0u8; 4];
    out[..2].copy_from_slice(&hi.to_le_bytes()[..2]);
    out[2..].copy_from_slice(&lo.to_le_bytes()[..2]);
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Operand {
    Reg(u8),
    Imm(i64),
    /// A bare word: a label, or an absolute address for branches.
    Sym(String),
    Mem {
        base: u8,
        index: Option<MemIndex>,
    },
    List(u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MemIndex {
    Imm(i64),
    Reg(u8),
}

#[derive(Debug, Clone)]
enum Stmt {
    Insn { mnemonic: String, ops: Vec<Operand> },
    Align(u32),
    Word(Operand),
}

#[derive(Debug, Clone)]
struct Line {
    no: usize,
    addr: u32,
    stmt: Stmt,
}

fn err<T>(line: usize, msg: impl Into<String>) -> Result<T, AsmError> {
    Err(AsmError {
        line,
        msg: msg.into(),
    })
}

fn parse_reg(s: &str) -> Option<u8> {
    let s = s.trim().to_ascii_lowercase();
    match s.as_str() {
        "sp" => Some(13),
        "lr" => Some(14),
        "pc" => Some(15),
        "ip" => Some(12),
        "fp" => Some(11),
        "sl" => Some(10),
        "sb" => Some(9),
        _ => s
            .strip_prefix('r')
            .and_then(|n| n.parse::<u8>().ok())
            .filter(|&n| n < 16),
    }
}

fn parse_num(s: &str) -> Option<i64> {
    let s = s.trim();
    let (neg, s) = s.strip_prefix('-').map_or((false, s), |rest| (true, rest));
    let v = if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        i64::from_str_radix(h, 16).ok()?
    } else {
        s.parse::<i64>().ok()?
    };
    Some(if neg { -v } else { v })
}

/// Splits at commas that are not inside `[]` or `{}`.
fn split_operands(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for ch in s.chars() {
        match ch {
            '[' | '{' => depth += 1,
            ']' | '}' => depth -= 1,
            _ => {}
        }
        if ch == ',' && depth == 0 {
            out.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(ch);
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

fn parse_operand(s: &str, line: usize) -> Result<Operand, AsmError> {
    if let Some(imm) = s.strip_prefix('#') {
        return parse_num(imm)
            .map(Operand::Imm)
            .map_or_else(|| err(line, format!("bad immediate {s}")), Ok);
    }
    if let Some(inner) = s.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
        let parts = split_operands(inner);
        let base = parts.first().and_then(|p| parse_reg(p));
        let Some(base) = base else {
            return err(line, format!("bad memory operand {s}"));
        };
        let index = match parts.get(1) {
            None => None,
            Some(p) => Some(if let Some(i) = p.strip_prefix('#') {
                MemIndex::Imm(parse_num(i).ok_or_else(|| AsmError {
                    line,
                    msg: format!("bad offset {p}"),
                })?)
            } else {
                MemIndex::Reg(parse_reg(p).ok_or_else(|| AsmError {
                    line,
                    msg: format!("bad index {p}"),
                })?)
            }),
        };
        if parts.len() > 2 {
            return err(line, format!("bad memory operand {s}"));
        }
        return Ok(Operand::Mem { base, index });
    }
    if let Some(inner) = s.strip_prefix('{').and_then(|x| x.strip_suffix('}')) {
        let mut regs = 0u16;
        for p in split_operands(inner) {
            let Some(r) = parse_reg(&p) else {
                return err(line, format!("bad register {p} in list"));
            };
            regs |= 1 << r;
        }
        return Ok(Operand::List(regs));
    }
    if let Some(r) = parse_reg(s) {
        return Ok(Operand::Reg(r));
    }
    if parse_num(s).is_some()
        || s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
    {
        return Ok(Operand::Sym(s.to_string()));
    }
    err(line, format!("bad operand {s}"))
}

fn is_label_name(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '.')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}

fn insn_size(mnemonic: &str) -> u32 {
    if mnemonic == "bl" { 4 } else { 2 }
}

/// Assembles `src` to run at `base` (which must be 4-byte aligned).
pub fn assemble(src: &str, base: u32) -> Result<Program, AsmError> {
    if !base.is_multiple_of(4) {
        return err(0, "base address must be 4-byte aligned");
    }
    let mut symbols = BTreeMap::new();
    let mut lines = Vec::new();
    let mut addr = base;
    for (i, raw) in src.lines().enumerate() {
        let no = i + 1;
        let mut text = raw.split('@').next().unwrap_or("").trim();
        while let Some(colon) = text.find(':') {
            let name = text[..colon].trim();
            if !is_label_name(name) {
                break;
            }
            if symbols.insert(name.to_string(), addr).is_some() {
                return err(no, format!("label {name} defined twice"));
            }
            text = text[colon + 1..].trim();
        }
        if text.is_empty() {
            continue;
        }
        let (head, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
        let head = head.to_ascii_lowercase();
        let stmt = match head.as_str() {
            ".align" => {
                let n = parse_num(rest).filter(|n| (0..=4).contains(n));
                let Some(n) = n else {
                    return err(no, ".align needs 0-4");
                };
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let a = 1u32 << n as u32;
                let start = addr;
                addr = addr.div_ceil(a) * a;
                lines.push(Line {
                    no,
                    addr: start,
                    stmt: Stmt::Align(addr - start),
                });
                continue;
            }
            ".word" => Stmt::Word(parse_operand(rest.trim(), no)?),
            m if m.starts_with('.') => return err(no, format!("unknown directive {m}")),
            _ => {
                let ops = split_operands(rest)
                    .iter()
                    .map(|o| parse_operand(o, no))
                    .collect::<Result<Vec<_>, _>>()?;
                Stmt::Insn { mnemonic: head, ops }
            }
        };
        let size = match &stmt {
            Stmt::Insn { mnemonic, .. } => insn_size(mnemonic),
            Stmt::Word(_) => 4,
            Stmt::Align(_) => 0,
        };
        lines.push(Line { no, addr, stmt });
        addr += size;
    }
    let mut code = Vec::with_capacity((addr - base) as usize);
    for line in &lines {
        match &line.stmt {
            Stmt::Align(n) => code.extend(std::iter::repeat_n(0u8, *n as usize)),
            Stmt::Word(v) => {
                let value = value_of(v, &symbols, line.no)?;
                code.extend_from_slice(&value.to_le_bytes());
            }
            Stmt::Insn { mnemonic, ops } => {
                let enc = Encoder {
                    line: line.no,
                    addr: line.addr,
                    symbols: &symbols,
                };
                match enc.encode(mnemonic, ops)? {
                    Encoded::Half(h) => code.extend_from_slice(&h.to_le_bytes()),
                    Encoded::Bl(b) => code.extend_from_slice(&b),
                }
            }
        }
    }
    debug_assert_eq!(code.len(), (addr - base) as usize);
    Ok(Program { base, code, symbols })
}

fn value_of(op: &Operand, symbols: &BTreeMap<String, u32>, line: usize) -> Result<u32, AsmError> {
    match op {
        Operand::Imm(v) => u32::try_from(*v).map_or_else(|_| err(line, "value out of range"), Ok),
        Operand::Sym(s) => {
            if let Some(v) = parse_num(s) {
                return u32::try_from(v).map_or_else(|_| err(line, "value out of range"), Ok);
            }
            symbols.get(s).copied().ok_or_else(|| AsmError {
                line,
                msg: format!("undefined label {s}"),
            })
        }
        _ => err(line, "bad value"),
    }
}

enum Encoded {
    Half(u16),
    Bl([u8; 4]),
}

struct Encoder<'a> {
    line: usize,
    addr: u32,
    symbols: &'a BTreeMap<String, u32>,
}

fn cond_of(s: &str) -> Option<Cond> {
    Some(match s {
        "eq" => Cond::Eq,
        "ne" => Cond::Ne,
        "hs" | "cs" => Cond::Hs,
        "lo" | "cc" => Cond::Lo,
        "mi" => Cond::Mi,
        "pl" => Cond::Pl,
        "vs" => Cond::Vs,
        "vc" => Cond::Vc,
        "hi" => Cond::Hi,
        "ls" => Cond::Ls,
        "ge" => Cond::Ge,
        "lt" => Cond::Lt,
        "gt" => Cond::Gt,
        "le" => Cond::Le,
        _ => return None,
    })
}

fn alu_of(m: &str) -> Option<AluOp> {
    Some(match m {
        "ands" => AluOp::And,
        "eors" => AluOp::Eor,
        "adcs" => AluOp::Adc,
        "sbcs" => AluOp::Sbc,
        "rors" => AluOp::Ror,
        "tst" => AluOp::Tst,
        "cmn" => AluOp::Cmn,
        "orrs" => AluOp::Orr,
        "muls" => AluOp::Mul,
        "bics" => AluOp::Bic,
        "mvns" => AluOp::Mvn,
        _ => return None,
    })
}

impl Encoder<'_> {
    fn fail<T>(&self, msg: impl Into<String>) -> Result<T, AsmError> {
        err(self.line, msg)
    }

    fn low(&self, r: u8) -> Result<u16, AsmError> {
        if r < 8 {
            Ok(u16::from(r))
        } else {
            self.fail(format!("r{r} is not a low register here"))
        }
    }

    fn imm(&self, v: i64, max: i64, scale: i64) -> Result<u16, AsmError> {
        if v < 0 || v > max * scale || v % scale != 0 {
            return self.fail(format!(
                "immediate {v} out of range (0-{}, multiple of {scale})",
                max * scale
            ));
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Ok((v / scale) as u16)
    }

    fn target(&self, op: &Operand) -> Result<u32, AsmError> {
        value_of(op, self.symbols, self.line)
    }

    fn branch_off(&self, op: &Operand, bits: u32) -> Result<u16, AsmError> {
        let off = i64::from(self.target(op)?) - (i64::from(self.addr) + 4);
        let lim = 1i64 << bits;
        if off & 1 != 0 || !(-lim..lim).contains(&off) {
            return self.fail(format!("branch target out of range ({off:+} bytes)"));
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Ok(((off >> 1) as u16) & ((1 << bits) - 1))
    }

    #[allow(clippy::too_many_lines)]
    fn encode(&self, m: &str, ops: &[Operand]) -> Result<Encoded, AsmError> {
        use Operand::{Imm, List, Mem, Reg, Sym};
        let h = |v: u16| Ok(Encoded::Half(v));
        match (m, ops) {
            ("bl", [t]) => {
                let dst = self.target(t)?;
                Ok(Encoded::Bl(bl(self.addr, dst)))
            }
            ("b", [t]) => h(0xE000 | self.branch_off(t, 11)?),
            (m, [t]) if m.len() == 3 && m.starts_with('b') && cond_of(&m[1..]).is_some() => {
                let c = cond_of(&m[1..]).map_or(0, Cond::bits);
                h(0xD000 | c << 8 | self.branch_off(t, 8)?)
            }
            ("bx", [Reg(rm)]) => h(0x4700 | u16::from(*rm) << 3),
            ("blx", [Reg(rm)]) => h(0x4780 | u16::from(*rm) << 3),
            ("nop", []) => h(0xBF00),
            ("push", [List(regs)]) => {
                if regs & !0x40FF != 0 {
                    return self.fail("push takes r0-r7 and lr");
                }
                h(0xB400 | (regs & 0xFF) | ((regs >> 14) & 1) << 8)
            }
            ("pop", [List(regs)]) => {
                if regs & !0x80FF != 0 {
                    return self.fail("pop takes r0-r7 and pc");
                }
                h(0xBC00 | (regs & 0xFF) | ((regs >> 15) & 1) << 8)
            }
            ("mov", [Reg(rd), Reg(rm)]) => {
                h(0x4600 | u16::from(rd & 8) << 4 | u16::from(*rm) << 3 | u16::from(rd & 7))
            }
            ("movs", [Reg(rd), Imm(v)]) => h(0x2000 | self.low(*rd)? << 8 | self.imm(*v, 255, 1)?),
            ("movs", [Reg(rd), Reg(rm)]) => h(self.low(*rm)? << 3 | self.low(*rd)?),
            ("cmp", [Reg(rn), Imm(v)]) => h(0x2800 | self.low(*rn)? << 8 | self.imm(*v, 255, 1)?),
            ("cmp", [Reg(rn), Reg(rm)]) => {
                if *rn < 8 && *rm < 8 {
                    h(0x4280 | u16::from(*rm) << 3 | u16::from(*rn))
                } else {
                    h(0x4500 | u16::from(rn & 8) << 4 | u16::from(*rm) << 3 | u16::from(rn & 7))
                }
            }
            ("adds" | "subs", [Reg(rdn), Imm(v)]) => {
                let opc = if m == "adds" { 0x3000 } else { 0x3800 };
                h(opc | self.low(*rdn)? << 8 | self.imm(*v, 255, 1)?)
            }
            ("adds" | "subs", [Reg(rd), Reg(rn), Imm(v)]) => {
                let add = m == "adds";
                if (0..8).contains(v) {
                    let opc = if add { 0x1C00 } else { 0x1E00 };
                    h(opc | self.imm(*v, 7, 1)? << 6 | self.low(*rn)? << 3 | self.low(*rd)?)
                } else if rd == rn {
                    let opc = if add { 0x3000 } else { 0x3800 };
                    h(opc | self.low(*rd)? << 8 | self.imm(*v, 255, 1)?)
                } else {
                    self.fail(format!("immediate {v} does not fit"))
                }
            }
            ("adds" | "subs", [Reg(rd), Reg(rn), Reg(rm)]) => {
                let opc = if m == "adds" { 0x1800 } else { 0x1A00 };
                h(opc | self.low(*rm)? << 6 | self.low(*rn)? << 3 | self.low(*rd)?)
            }
            ("add", [Reg(rdn), Reg(rm)]) => {
                h(0x4400 | u16::from(rdn & 8) << 4 | u16::from(*rm) << 3 | u16::from(rdn & 7))
            }
            ("add", [Reg(13), Imm(v)]) => h(0xB000 | self.imm(*v, 127, 4)?),
            ("sub", [Reg(13), Imm(v)]) => h(0xB080 | self.imm(*v, 127, 4)?),
            ("add", [Reg(rd), Reg(13), Imm(v)]) => h(0xA800 | self.low(*rd)? << 8 | self.imm(*v, 255, 4)?),
            ("lsls" | "lsrs" | "asrs", [Reg(rd), Reg(rm), Imm(v)]) => {
                let (opc, lo) = match m {
                    "lsls" => (0x0000, 0),
                    "lsrs" => (0x0800, 1),
                    _ => (0x1000, 1),
                };
                if *v < lo || *v > 31 + lo {
                    return self.fail(format!("shift {v} out of range"));
                }
                h(opc | (self.imm(*v, 32, 1)? & 0x1F) << 6 | self.low(*rm)? << 3 | self.low(*rd)?)
            }
            ("lsls" | "lsrs" | "asrs", [Reg(rdn), Reg(rm)]) => {
                let op = match m {
                    "lsls" => AluOp::Lsl,
                    "lsrs" => AluOp::Lsr,
                    _ => AluOp::Asr,
                };
                h(0x4000 | op.bits() << 6 | self.low(*rm)? << 3 | self.low(*rdn)?)
            }
            ("negs" | "rsbs", [Reg(rd), Reg(rm), ..]) => {
                h(0x4000 | AluOp::Rsb.bits() << 6 | self.low(*rm)? << 3 | self.low(*rd)?)
            }
            (m, [Reg(rdn), Reg(rm)]) if alu_of(m).is_some() => {
                let op = alu_of(m).map_or(0, AluOp::bits);
                h(0x4000 | op << 6 | self.low(*rm)? << 3 | self.low(*rdn)?)
            }
            ("uxth" | "uxtb" | "sxth" | "sxtb", [Reg(rd), Reg(rm)]) => {
                let opc = match m {
                    "sxth" => 0xB200,
                    "sxtb" => 0xB240,
                    "uxth" => 0xB280,
                    _ => 0xB2C0,
                };
                h(opc | self.low(*rm)? << 3 | self.low(*rd)?)
            }
            ("ldr", [Reg(rt), t @ Sym(_)]) => {
                let target = i64::from(self.target(t)?);
                let pc = i64::from((self.addr + 4) & !3);
                let off = target - pc;
                if !(0..=1020).contains(&off) || off % 4 != 0 {
                    return self.fail(format!("literal out of reach ({off:+} bytes from PC)"));
                }
                h(0x4800 | self.low(*rt)? << 8 | self.imm(off, 255, 4)?)
            }
            ("ldr" | "str" | "ldrb" | "strb" | "ldrh" | "strh", [Reg(rt), Mem { base, index }]) => {
                self.mem(m, *rt, *base, index.as_ref())
            }
            (
                "ldrsb" | "ldrsh",
                [
                    Reg(rt),
                    Mem {
                        base,
                        index: Some(MemIndex::Reg(rm)),
                    },
                ],
            ) => {
                let opc = if m == "ldrsb" { 0x5600 } else { 0x5E00 };
                h(opc | self.low(*rm)? << 6 | self.low(*base)? << 3 | self.low(*rt)?)
            }
            _ => self.fail(format!("unsupported instruction: {m} {ops:?}")),
        }
    }

    fn mem(&self, m: &str, rt: u8, base: u8, index: Option<&MemIndex>) -> Result<Encoded, AsmError> {
        let h = |v: u16| Ok(Encoded::Half(v));
        match index {
            Some(MemIndex::Reg(rm)) => {
                let opc = match m {
                    "str" => 0x5000,
                    "strh" => 0x5200,
                    "strb" => 0x5400,
                    "ldr" => 0x5800,
                    "ldrh" => 0x5A00,
                    _ => 0x5C00,
                };
                h(opc | self.low(*rm)? << 6 | self.low(base)? << 3 | self.low(rt)?)
            }
            None | Some(MemIndex::Imm(_)) => {
                let v = match index {
                    Some(MemIndex::Imm(v)) => *v,
                    _ => 0,
                };
                if base == 13 && (m == "ldr" || m == "str") {
                    let opc = if m == "str" { 0x9000 } else { 0x9800 };
                    return h(opc | self.low(rt)? << 8 | self.imm(v, 255, 4)?);
                }
                if base == 15 && m == "ldr" {
                    return h(0x4800 | self.low(rt)? << 8 | self.imm(v, 255, 4)?);
                }
                let (opc, scale) = match m {
                    "str" => (0x6000, 4),
                    "ldr" => (0x6800, 4),
                    "strb" => (0x7000, 1),
                    "ldrb" => (0x7800, 1),
                    "strh" => (0x8000, 2),
                    _ => (0x8800, 2),
                };
                h(opc | self.imm(v, 31, scale)? << 6 | self.low(base)? << 3 | self.low(rt)?)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::{Op, decode_at};

    #[test]
    fn bl_matches_the_stock_call_at_0x7cb6() {
        assert_eq!(bl(0x7CB6, 0x6984), [0xFE, 0xF7, 0x65, 0xFE]);
    }

    #[test]
    fn labels_literals_and_alignment() {
        let p = assemble(
            "start:\n  ldr r0, lit  @ comment\n  bx r0\n  movs r1, #1\n  .align 2\nlit: .word 0x583d\n",
            0xD200,
        )
        .unwrap();
        assert_eq!(p.symbols["lit"], 0xD208);
        assert_eq!(&p.code[..2], &0x4801u16.to_le_bytes());
        assert_eq!(&p.code[6..8], &[0, 0]);
        assert_eq!(&p.code[8..], &0x583Du32.to_le_bytes());
    }

    #[test]
    fn encodings_round_trip_through_the_decoder() {
        let src = "mov r4, r0\nmov lr, r2\nadds r1, r1, #1\nadds r1, #32\nadds r1, r1, r0\nsubs r0, #0x80\n\
                   lsls r2, r0\nlsrs r0, r0, #3\nldrb r0, [r6, r7]\nstrb r0, [r6, #19]\nldr r0, [r6, #4]\n\
                   uxth r1, r0\ncmp r7, r0\npush {r4, r5, r6, r7, lr}\npop {r4, r5, r6, r7, pc}\nbics r0, r2\n";
        let p = assemble(src, 0).unwrap();
        let ops: Vec<Op> = (0..p.code.len())
            .step_by(2)
            .map(|a| decode_at(&p.code, a).unwrap().op)
            .collect();
        assert_eq!(ops[0], Op::MovHi { rd: 4, rm: 0 });
        assert_eq!(ops[1], Op::MovHi { rd: 14, rm: 2 });
        assert_eq!(ops[2], Op::AddImm3 { rd: 1, rn: 1, imm: 1 });
        assert_eq!(ops[3], Op::AddImm8 { rdn: 1, imm: 32 });
        assert_eq!(ops[4], Op::AddReg { rd: 1, rn: 1, rm: 0 });
        assert_eq!(ops[5], Op::SubImm8 { rdn: 0, imm: 0x80 });
        assert_eq!(
            ops[6],
            Op::Alu {
                op: AluOp::Lsl,
                rdn: 2,
                rm: 0
            }
        );
        assert_eq!(ops[7], Op::LsrImm { rd: 0, rm: 0, imm: 3 });
        assert_eq!(ops[13], Op::Push { regs: 0xF0 | 1 << 14 });
        assert_eq!(ops[14], Op::Pop { regs: 0xF0 | 1 << 15 });
    }

    #[test]
    fn out_of_range_is_an_error_not_a_wide_encoding() {
        let far = format!("beq far\n{}far:\n", "nop\n".repeat(200));
        assert!(assemble(&far, 0).is_err());
        assert!(assemble("movs r0, #256\n", 0).is_err());
        assert!(assemble("ldrb r0, [r6, #32]\n", 0).is_err());
        assert!(assemble("movs r8, #1\n", 0).is_err());
        assert!(assemble("x:\nx:\n", 0).is_err());
    }
}
