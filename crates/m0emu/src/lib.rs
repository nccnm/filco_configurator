//! Cortex-M0 (ARMv6-M Thumb) interpreter in pure Rust.
//!
//! It runs code the way the analysis repository ran it in Unicorn: memory is a set of mapped regions (reads and
//! writes outside them stop the run), a [`Hooks`] implementation sees every data read inside the peripheral
//! regions before it happens (and may change the value) and every instruction address in a watch list before it
//! executes (and may redirect the PC). Interrupts are not raised; the caller runs handlers as plain calls.
//!
//! Anything the model does not cover stops the run with an [`Stop`] instead of guessing: an instruction that is
//! not ARMv6-M, `svc`, `bkpt`, `udf`, a branch to an ARM-state address, a special register it does not hold.
#![forbid(unsafe_code)]

use std::fmt;

use thumb::decode::{AluOp, Insn, MemOp, Op, decode};

/// Why a run stopped before reaching its end address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stop {
    pub pc: u32,
    pub reason: String,
}

impl fmt::Display for Stop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at pc={:#x}", self.reason, self.pc)
    }
}

impl std::error::Error for Stop {}

/// A mapped memory region.
#[derive(Debug, Clone)]
struct Region {
    base: u32,
    data: Vec<u8>,
    hooked: bool,
}

/// The address space.
#[derive(Debug, Clone, Default)]
pub struct Memory {
    regions: Vec<Region>,
}

impl Memory {
    /// Maps `size` zero bytes at `base`. `hooked` regions report data reads to [`Hooks::on_read`].
    pub fn map(&mut self, base: u32, size: u32, hooked: bool) {
        self.regions.push(Region {
            base,
            data: vec![0; size as usize],
            hooked,
        });
    }

    fn find(&self, addr: u32, len: u32) -> Option<(usize, usize)> {
        self.regions.iter().enumerate().find_map(|(i, r)| {
            let off = addr.wrapping_sub(r.base) as usize;
            (addr >= r.base && off + len as usize <= r.data.len()).then_some((i, off))
        })
    }

    /// Whether the address is in a hooked region.
    pub fn is_hooked(&self, addr: u32) -> bool {
        self.find(addr, 1).is_some_and(|(i, _)| self.regions[i].hooked)
    }

    /// Reads bytes; `None` when not mapped.
    pub fn read(&self, addr: u32, len: u32) -> Option<&[u8]> {
        let (i, off) = self.find(addr, len)?;
        Some(&self.regions[i].data[off..off + len as usize])
    }

    /// Writes bytes; `false` when not mapped.
    pub fn write(&mut self, addr: u32, bytes: &[u8]) -> bool {
        let Ok(len) = u32::try_from(bytes.len()) else {
            return false;
        };
        match self.find(addr, len) {
            Some((i, off)) => {
                self.regions[i].data[off..off + bytes.len()].copy_from_slice(bytes);
                true
            }
            None => false,
        }
    }

    /// Little-endian word.
    pub fn read_u32(&self, addr: u32) -> Option<u32> {
        self.read(addr, 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// One byte.
    pub fn read_u8(&self, addr: u32) -> Option<u8> {
        self.read(addr, 1).map(|b| b[0])
    }
}

/// Registers and flags. N, Z, C, V are the four APSR flags of the architecture, hence the bools.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cpu {
    /// r0-r15; r13 is the (main) stack pointer, r15 the address of the next instruction.
    pub r: [u32; 16],
    pub n: bool,
    pub z: bool,
    pub c: bool,
    pub v: bool,
    pub primask: bool,
    pub control: u32,
    pub psp: u32,
}

impl Cpu {
    fn apsr(&self) -> u32 {
        u32::from(self.n) << 31 | u32::from(self.z) << 30 | u32::from(self.c) << 29 | u32::from(self.v) << 28
    }

    fn nz(&mut self, v: u32) {
        self.n = v >> 31 == 1;
        self.z = v == 0;
    }
}

/// Callbacks from a run.
pub trait Hooks {
    /// Before a data read of `size` bytes at `addr` in a hooked region. May write memory to change what is read.
    fn on_read(&mut self, cpu: &Cpu, mem: &mut Memory, addr: u32, size: u32);
    /// Before the instruction at `pc` executes, for addresses in [`Emu::watch`]. Return true when the hook changed
    /// `cpu.r[15]` (the instruction is then skipped).
    fn on_code(&mut self, cpu: &mut Cpu, mem: &mut Memory, pc: u32) -> bool;
}

/// The machine: CPU, memory, a decode cache for the first 64 KB (flash) and the watched code addresses.
#[derive(Debug, Clone)]
pub struct Emu {
    pub cpu: Cpu,
    pub mem: Memory,
    watch: Vec<u32>,
    cache: Vec<Option<Insn>>,
    /// Instructions executed so far.
    pub steps: u64,
}

const CACHE_LIMIT: u32 = 0x1_0000;

fn add_with_carry(x: u32, y: u32, carry: bool) -> (u32, bool, bool) {
    let wide = u64::from(x) + u64::from(y) + u64::from(carry);
    #[allow(clippy::cast_possible_truncation)]
    let res = wide as u32;
    let overflow = ((x ^ res) & (y ^ res)) >> 31 == 1;
    (res, wide >> 32 == 1, overflow)
}

impl Emu {
    pub fn new(mem: Memory) -> Self {
        Self {
            cpu: Cpu::default(),
            mem,
            watch: Vec::new(),
            cache: vec![None; (CACHE_LIMIT / 2) as usize],
            steps: 0,
        }
    }

    /// Adds a code address whose execution calls [`Hooks::on_code`].
    pub fn watch(&mut self, addr: u32) {
        self.watch.push(addr & !1);
    }

    /// Writes memory and drops cached decodes of what was overwritten.
    pub fn write(&mut self, addr: u32, bytes: &[u8]) -> bool {
        self.invalidate(addr, bytes.len());
        self.mem.write(addr, bytes)
    }

    fn invalidate(&mut self, addr: u32, len: usize) {
        if addr < CACHE_LIMIT {
            let first = (addr / 2) as usize;
            let last = ((addr as usize + len + 2) / 2).min(self.cache.len());
            // a 32-bit instruction starting one halfword earlier also covers the first byte
            for e in &mut self.cache[first.saturating_sub(1)..last] {
                *e = None;
            }
        }
    }

    fn fetch(&mut self, pc: u32) -> Result<Insn, Stop> {
        let stop = |reason: &str| Stop {
            pc,
            reason: reason.into(),
        };
        if pc < CACHE_LIMIT
            && let Some(i) = self.cache[(pc / 2) as usize]
        {
            return Ok(i);
        }
        let hw = |m: &Memory, a: u32| m.read(a, 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
        let hw1 = hw(&self.mem, pc).ok_or_else(|| stop("fetch from unmapped memory"))?;
        let hw2 = if thumb::decode::is_wide(hw1) {
            hw(&self.mem, pc + 2)
        } else {
            None
        };
        let insn =
            decode(hw1, hw2).ok_or_else(|| stop(&format!("not an ARMv6-M instruction ({hw1:04x})")))?;
        if pc < CACHE_LIMIT {
            self.cache[(pc / 2) as usize] = Some(insn);
        }
        Ok(insn)
    }

    /// Runs from `start` until the PC equals `until` (checked before each instruction) or `count` instructions
    /// have run. Returns whether `until` was reached.
    pub fn run(&mut self, hooks: &mut impl Hooks, start: u32, until: u32, count: u64) -> Result<bool, Stop> {
        self.cpu.r[15] = start & !1;
        for _ in 0..count {
            let pc = self.cpu.r[15];
            if pc == until {
                return Ok(true);
            }
            if self.watch.contains(&pc) && hooks.on_code(&mut self.cpu, &mut self.mem, pc) {
                self.steps += 1;
                continue;
            }
            let insn = self.fetch(pc)?;
            self.exec(hooks, pc, insn)?;
            self.steps += 1;
        }
        Ok(self.cpu.r[15] == until)
    }

    fn load(&mut self, hooks: &mut impl Hooks, pc: u32, addr: u32, size: u32) -> Result<u32, Stop> {
        if self.mem.is_hooked(addr) {
            hooks.on_read(&self.cpu, &mut self.mem, addr, size);
        }
        let b = self.mem.read(addr, size).ok_or_else(|| Stop {
            pc,
            reason: format!("read of unmapped {addr:#x}"),
        })?;
        Ok(match size {
            1 => u32::from(b[0]),
            2 => u32::from(u16::from_le_bytes([b[0], b[1]])),
            _ => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        })
    }

    fn store(&mut self, pc: u32, addr: u32, size: u32, value: u32) -> Result<(), Stop> {
        let bytes = value.to_le_bytes();
        if self.write(addr, &bytes[..size as usize]) {
            Ok(())
        } else {
            Err(Stop {
                pc,
                reason: format!("write to unmapped {addr:#x}"),
            })
        }
    }

    fn reg(&self, r: u8, pc: u32) -> u32 {
        if r == 15 {
            pc.wrapping_add(4)
        } else {
            self.cpu.r[usize::from(r)]
        }
    }

    fn branch_to(&mut self, pc: u32, target: u32) -> Result<(), Stop> {
        if target & 1 == 0 {
            return Err(Stop {
                pc,
                reason: format!("branch to ARM state ({target:#x})"),
            });
        }
        self.cpu.r[15] = target & !1;
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn exec(&mut self, hooks: &mut impl Hooks, pc: u32, insn: Insn) -> Result<(), Stop> {
        let next = pc.wrapping_add(u32::from(insn.size));
        self.cpu.r[15] = next;
        let rr = |x: u8| usize::from(x);
        match insn.op {
            Op::LslImm { rd, rm, imm } => {
                let x = self.cpu.r[rr(rm)];
                let res = if imm == 0 {
                    x
                } else {
                    self.cpu.c = (x >> (32 - u32::from(imm))) & 1 == 1;
                    x << imm
                };
                self.cpu.r[rr(rd)] = res;
                self.cpu.nz(res);
            }
            Op::LsrImm { rd, rm, imm } => {
                let x = self.cpu.r[rr(rm)];
                let s = u32::from(imm);
                self.cpu.c = (x >> (s - 1)) & 1 == 1;
                let res = x.checked_shr(s).unwrap_or(0);
                self.cpu.r[rr(rd)] = res;
                self.cpu.nz(res);
            }
            Op::AsrImm { rd, rm, imm } => {
                let x = self.cpu.r[rr(rm)];
                let s = u32::from(imm).min(32);
                self.cpu.c = (x.cast_signed() >> (s - 1).min(31)) & 1 == 1;
                let res = (x.cast_signed() >> s.min(31)).cast_unsigned();
                self.cpu.r[rr(rd)] = res;
                self.cpu.nz(res);
            }
            Op::AddReg { rd, rn, rm } => self.add_flags(rd, self.cpu.r[rr(rn)], self.cpu.r[rr(rm)], false),
            Op::SubReg { rd, rn, rm } => self.add_flags(rd, self.cpu.r[rr(rn)], !self.cpu.r[rr(rm)], true),
            Op::AddImm3 { rd, rn, imm } => self.add_flags(rd, self.cpu.r[rr(rn)], u32::from(imm), false),
            Op::SubImm3 { rd, rn, imm } => self.add_flags(rd, self.cpu.r[rr(rn)], !u32::from(imm), true),
            Op::AddImm8 { rdn, imm } => self.add_flags(rdn, self.cpu.r[rr(rdn)], u32::from(imm), false),
            Op::SubImm8 { rdn, imm } => self.add_flags(rdn, self.cpu.r[rr(rdn)], !u32::from(imm), true),
            Op::MovImm { rd, imm } => {
                self.cpu.r[rr(rd)] = u32::from(imm);
                self.cpu.nz(u32::from(imm));
            }
            Op::CmpImm { rn, imm } => self.compare(self.cpu.r[rr(rn)], !u32::from(imm), true),
            Op::Alu { op, rdn, rm } => self.alu(op, rdn, rm),
            Op::AddHi { rdn, rm } => {
                let v = self.reg(rdn, pc).wrapping_add(self.reg(rm, pc));
                if rdn == 15 {
                    self.cpu.r[15] = v & !1;
                } else {
                    self.cpu.r[rr(rdn)] = v;
                }
            }
            Op::CmpHi { rn, rm } => self.compare(self.reg(rn, pc), !self.reg(rm, pc), true),
            Op::MovHi { rd, rm } => {
                let v = self.reg(rm, pc);
                if rd == 15 {
                    self.cpu.r[15] = v & !1;
                } else {
                    self.cpu.r[rr(rd)] = v;
                }
            }
            Op::Bx { rm } => self.branch_to(pc, self.reg(rm, pc))?,
            Op::Blx { rm } => {
                let t = self.reg(rm, pc);
                self.cpu.r[14] = next | 1;
                self.branch_to(pc, t)?;
            }
            Op::LdrLit { rt, imm } => {
                let a = (pc.wrapping_add(4) & !3).wrapping_add(u32::from(imm));
                self.cpu.r[rr(rt)] = self.load(hooks, pc, a, 4)?;
            }
            Op::MemReg { op, rt, rn, rm } => {
                let a = self.cpu.r[rr(rn)].wrapping_add(self.cpu.r[rr(rm)]);
                self.mem_op(hooks, pc, op, rt, a)?;
            }
            Op::MemImm { op, rt, rn, imm } => {
                let a = self.cpu.r[rr(rn)].wrapping_add(u32::from(imm));
                self.mem_op(hooks, pc, op, rt, a)?;
            }
            Op::StrSp { rt, imm } => {
                let a = self.cpu.r[13].wrapping_add(u32::from(imm));
                self.store(pc, a, 4, self.cpu.r[rr(rt)])?;
            }
            Op::LdrSp { rt, imm } => {
                let a = self.cpu.r[13].wrapping_add(u32::from(imm));
                self.cpu.r[rr(rt)] = self.load(hooks, pc, a, 4)?;
            }
            Op::Adr { rd, imm } => {
                self.cpu.r[rr(rd)] = (pc.wrapping_add(4) & !3).wrapping_add(u32::from(imm));
            }
            Op::AddRdSp { rd, imm } => self.cpu.r[rr(rd)] = self.cpu.r[13].wrapping_add(u32::from(imm)),
            Op::AddSp { imm } => self.cpu.r[13] = self.cpu.r[13].wrapping_add(u32::from(imm)),
            Op::SubSp { imm } => self.cpu.r[13] = self.cpu.r[13].wrapping_sub(u32::from(imm)),
            Op::Sxth { rd, rm } => {
                #[allow(clippy::cast_possible_truncation)]
                let v = self.cpu.r[rr(rm)] as u16;
                self.cpu.r[rr(rd)] = i32::from(v.cast_signed()).cast_unsigned();
            }
            Op::Sxtb { rd, rm } => {
                #[allow(clippy::cast_possible_truncation)]
                let v = self.cpu.r[rr(rm)] as u8;
                self.cpu.r[rr(rd)] = i32::from(v.cast_signed()).cast_unsigned();
            }
            Op::Uxth { rd, rm } => self.cpu.r[rr(rd)] = self.cpu.r[rr(rm)] & 0xFFFF,
            Op::Uxtb { rd, rm } => self.cpu.r[rr(rd)] = self.cpu.r[rr(rm)] & 0xFF,
            Op::Rev { rd, rm } => self.cpu.r[rr(rd)] = self.cpu.r[rr(rm)].swap_bytes(),
            Op::Rev16 { rd, rm } => {
                let x = self.cpu.r[rr(rm)];
                self.cpu.r[rr(rd)] = ((x & 0x00FF_00FF) << 8) | ((x >> 8) & 0x00FF_00FF);
            }
            Op::Revsh { rd, rm } => {
                #[allow(clippy::cast_possible_truncation)]
                let v = (self.cpu.r[rr(rm)] as u16).swap_bytes();
                self.cpu.r[rr(rd)] = i32::from(v.cast_signed()).cast_unsigned();
            }
            Op::Push { regs } => {
                let list: Vec<usize> = (0..15).filter(|i| regs >> i & 1 == 1).collect();
                let n = u32::try_from(list.len()).unwrap_or(0);
                let mut a = self.cpu.r[13].wrapping_sub(4 * n);
                self.cpu.r[13] = a;
                for i in list {
                    self.store(pc, a, 4, self.cpu.r[i])?;
                    a = a.wrapping_add(4);
                }
            }
            Op::Pop { regs } => {
                let mut a = self.cpu.r[13];
                let mut new_pc = None;
                for i in 0..16usize {
                    if regs >> i & 1 == 1 {
                        let v = self.load(hooks, pc, a, 4)?;
                        if i == 15 {
                            new_pc = Some(v);
                        } else {
                            self.cpu.r[i] = v;
                        }
                        a = a.wrapping_add(4);
                    }
                }
                self.cpu.r[13] = a;
                if let Some(t) = new_pc {
                    self.branch_to(pc, t)?;
                }
            }
            Op::Stm { rn, regs } => {
                let mut a = self.cpu.r[rr(rn)];
                for i in 0..8usize {
                    if regs >> i & 1 == 1 {
                        self.store(pc, a, 4, self.cpu.r[i])?;
                        a = a.wrapping_add(4);
                    }
                }
                self.cpu.r[rr(rn)] = a;
            }
            Op::Ldm { rn, regs } => {
                let mut a = self.cpu.r[rr(rn)];
                let base = rr(rn);
                for i in 0..8usize {
                    if regs >> i & 1 == 1 {
                        self.cpu.r[i] = self.load(hooks, pc, a, 4)?;
                        a = a.wrapping_add(4);
                    }
                }
                if regs >> base & 1 == 0 {
                    self.cpu.r[base] = a;
                }
            }
            Op::Cps { disable } => self.cpu.primask = disable,
            Op::Hint { .. } | Op::Barrier => {}
            Op::BCond { cond, off } => {
                if cond.holds([self.cpu.n, self.cpu.z, self.cpu.c, self.cpu.v]) {
                    self.cpu.r[15] = pc.wrapping_add(4).wrapping_add_signed(off);
                }
            }
            Op::B { off } => self.cpu.r[15] = pc.wrapping_add(4).wrapping_add_signed(off),
            Op::Bl { off } => {
                self.cpu.r[14] = next | 1;
                self.cpu.r[15] = pc.wrapping_add(4).wrapping_add_signed(off);
            }
            Op::Mrs { rd, sysm } => {
                self.cpu.r[rr(rd)] = match sysm {
                    0..=7 => self.cpu.apsr(),
                    8 => self.cpu.r[13],
                    9 => self.cpu.psp,
                    16 => u32::from(self.cpu.primask),
                    20 => self.cpu.control,
                    _ => {
                        return Err(Stop {
                            pc,
                            reason: format!("mrs of special register {sysm}"),
                        });
                    }
                };
            }
            Op::Msr { sysm, rn } => {
                let v = self.cpu.r[rr(rn)];
                match sysm {
                    0..=3 => {
                        self.cpu.n = v >> 31 & 1 == 1;
                        self.cpu.z = v >> 30 & 1 == 1;
                        self.cpu.c = v >> 29 & 1 == 1;
                        self.cpu.v = v >> 28 & 1 == 1;
                    }
                    8 => self.cpu.r[13] = v & !3,
                    9 => self.cpu.psp = v & !3,
                    16 => self.cpu.primask = v & 1 == 1,
                    20 => {
                        if v & 2 != 0 {
                            return Err(Stop {
                                pc,
                                reason: "switch to the process stack is not modelled".into(),
                            });
                        }
                        self.cpu.control = v & 3;
                    }
                    _ => {
                        return Err(Stop {
                            pc,
                            reason: format!("msr to special register {sysm}"),
                        });
                    }
                }
            }
            Op::Svc { .. } | Op::Bkpt { .. } | Op::Udf { .. } => {
                return Err(Stop {
                    pc,
                    reason: format!("{:?} is not modelled", insn.op),
                });
            }
        }
        Ok(())
    }

    fn add_flags(&mut self, rd: u8, x: u32, y: u32, carry: bool) {
        let (res, c, v) = add_with_carry(x, y, carry);
        self.cpu.r[usize::from(rd)] = res;
        self.cpu.nz(res);
        self.cpu.c = c;
        self.cpu.v = v;
    }

    fn compare(&mut self, x: u32, y: u32, carry: bool) {
        let (res, c, v) = add_with_carry(x, y, carry);
        self.cpu.nz(res);
        self.cpu.c = c;
        self.cpu.v = v;
    }

    // Operand names as in the ARMv6-M pseudocode.
    #[allow(clippy::many_single_char_names)]
    fn alu(&mut self, op: AluOp, rdn: u8, rm: u8) {
        let (d, m) = (usize::from(rdn), usize::from(rm));
        let (x, y) = (self.cpu.r[d], self.cpu.r[m]);
        let res = match op {
            AluOp::And | AluOp::Tst => x & y,
            AluOp::Eor => x ^ y,
            AluOp::Orr => x | y,
            AluOp::Bic => x & !y,
            AluOp::Mvn => !y,
            AluOp::Mul => x.wrapping_mul(y),
            AluOp::Lsl | AluOp::Lsr | AluOp::Asr | AluOp::Ror => self.shift(op, x, y & 0xFF),
            AluOp::Adc | AluOp::Sbc | AluOp::Rsb | AluOp::Cmp | AluOp::Cmn => {
                let (a, b, cin) = match op {
                    AluOp::Adc => (x, y, self.cpu.c),
                    AluOp::Sbc => (x, !y, self.cpu.c),
                    AluOp::Rsb => (!y, 0, true),
                    AluOp::Cmp => (x, !y, true),
                    _ => (x, y, false),
                };
                let (res, c, v) = add_with_carry(a, b, cin);
                self.cpu.c = c;
                self.cpu.v = v;
                res
            }
        };
        self.cpu.nz(res);
        if !matches!(op, AluOp::Tst | AluOp::Cmp | AluOp::Cmn) {
            self.cpu.r[d] = res;
        }
    }

    fn shift(&mut self, op: AluOp, x: u32, n: u32) -> u32 {
        if n == 0 {
            return x;
        }
        let bit = |v: u32, i: u32| (v >> i) & 1 == 1;
        match op {
            AluOp::Lsl => {
                self.cpu.c = n <= 32 && bit(x, 32 - n);
                if n < 32 { x << n } else { 0 }
            }
            AluOp::Lsr => {
                self.cpu.c = n <= 32 && bit(x, n - 1);
                if n < 32 { x >> n } else { 0 }
            }
            AluOp::Asr => {
                let s = n.min(32);
                self.cpu.c = bit(x, (s - 1).min(31));
                (x.cast_signed() >> s.min(31)).cast_unsigned()
            }
            _ => {
                let res = x.rotate_right(n & 31);
                self.cpu.c = bit(res, 31);
                res
            }
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn mem_op(&mut self, hooks: &mut impl Hooks, pc: u32, op: MemOp, rt: u8, a: u32) -> Result<(), Stop> {
        let t = usize::from(rt);
        if op.is_load() {
            let v = self.load(hooks, pc, a, op.size())?;
            self.cpu.r[t] = match op {
                MemOp::Ldrsb => i32::from((v as u8).cast_signed()).cast_unsigned(),
                MemOp::Ldrsh => i32::from((v as u16).cast_signed()).cast_unsigned(),
                _ => v,
            };
            Ok(())
        } else {
            self.store(pc, a, op.size(), self.cpu.r[t])
        }
    }
}

#[cfg(test)]
mod tests;
