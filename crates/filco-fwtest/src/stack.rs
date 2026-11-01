//! Worst-case stack depth of a V72 image by static analysis (port of `tools/stackdepth.py`).
//!
//! For each function the walk follows every path from the entry (both sides of conditional branches,
//! unconditional branches and tail jumps `ldr rX, =function ; bx rX`, the Keil switch helper 0xc218 with its
//! inline table) and tracks the stack pointer through push / pop / add sp / sub sp. A call adds the callee's own
//! worst case. Handlers add the 36-byte exception frame. Anything the walk cannot follow is a warning; a result
//! with warnings is not a proof.

use std::collections::{HashMap, HashSet};

use thumb::decode::{Op, decode_at};

/// Keil `__ARM_common_switch8`: index in r3, table right after the `bl`.
const SWITCH_HELPER: u32 = 0xC218;
const SWITCH_HELPER_STACK: i64 = 8;
/// 8 words stacked by hardware, plus alignment padding.
const EXC_FRAME: i64 = 36;
pub const MAIN: u32 = 0x1F02;
/// Vector index and name, in the order of the report.
pub const HANDLERS: [(usize, &str); 8] = [
    (15, "SysTick"),
    (16, "IRQ0"),
    (17, "IRQ1 USB"),
    (26, "IRQ10 (0x40018000)"),
    (31, "IRQ15 timer 1 ms"),
    (32, "IRQ16 CT16B1 LED PWM"),
    (45, "IRQ29"),
    (47, "IRQ31"),
];
const STACK_LIMIT: u32 = 0x2000_0890;

struct Analyzer<'a> {
    img: &'a [u8],
    memo: HashMap<u32, i64>,
    active: HashSet<u32>,
    warnings: Vec<String>,
}

fn word(img: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([img[at], img[at + 1], img[at + 2], img[at + 3]])
}

impl Analyzer<'_> {
    #[allow(clippy::too_many_lines)]
    fn depth(&mut self, entry: u32) -> i64 {
        if let Some(&d) = self.memo.get(&entry) {
            return d;
        }
        if self.active.contains(&entry) {
            self.warnings.push(format!("recursion through {entry:#x}"));
            return 0;
        }
        self.active.insert(entry);
        let mut best = 0i64;
        let mut seen: HashMap<u32, i64> = HashMap::new();
        let mut work = vec![(entry, 0i64)];
        while let Some((mut addr, mut sp)) = work.pop() {
            let mut lit: HashMap<u8, u32> = HashMap::new();
            loop {
                if seen.get(&addr).is_some_and(|&s| s >= sp) {
                    break;
                }
                seen.insert(addr, sp);
                let Some(insn) = decode_at(self.img, addr as usize) else {
                    self.warnings
                        .push(format!("{entry:#x}: cannot decode at {addr:#x}"));
                    break;
                };
                let next = addr + u32::from(insn.size);
                let regs = |r: u16| i64::from(r.count_ones());
                match insn.op {
                    Op::Push { regs: r } => sp += 4 * regs(r),
                    Op::Pop { regs: r } => {
                        sp -= 4 * regs(r);
                        if r & 0x8000 != 0 {
                            break;
                        }
                    }
                    Op::SubSp { imm } => sp += i64::from(imm),
                    Op::AddSp { imm } => sp -= i64::from(imm),
                    Op::MovHi { rd: 13, .. } | Op::AddHi { rdn: 13, .. } => {
                        self.warnings
                            .push(format!("{entry:#x}: sp written at {addr:#x}: {:?}", insn.op));
                    }
                    _ => {}
                }
                best = best.max(sp);
                match insn.op {
                    Op::Bl { off } => {
                        let t = addr.wrapping_add(4).wrapping_add_signed(off);
                        if t == SWITCH_HELPER {
                            // table at the return address: count n, then n + 1 byte offsets (last = default)
                            let tbl = next as usize;
                            let n = usize::from(self.img[tbl]);
                            for k in 0..=n {
                                let target = tbl + 2 * usize::from(self.img[tbl + 1 + k]);
                                work.push((u32::try_from(target).unwrap_or(0), sp));
                            }
                            best = best.max(sp + SWITCH_HELPER_STACK);
                            break;
                        }
                        let d = self.depth(t);
                        best = best.max(sp + d);
                    }
                    Op::B { off } => {
                        addr = addr.wrapping_add(4).wrapping_add_signed(off);
                        continue;
                    }
                    Op::BCond { off, .. } => work.push((addr.wrapping_add(4).wrapping_add_signed(off), sp)),
                    Op::Bx { rm } => {
                        if let Some(&t) = lit.get(&rm)
                            && t & 1 == 1
                            && (t as usize) < self.img.len()
                        {
                            addr = t & !1; // tail jump: same frame
                            lit.clear();
                            continue;
                        }
                        if rm != 14 {
                            self.warnings
                                .push(format!("{entry:#x}: indirect branch at {addr:#x}: bx r{rm}"));
                        }
                        break;
                    }
                    Op::LdrLit { rt, imm } => {
                        let pool = ((addr + 4) & !3) + u32::from(imm);
                        lit.insert(rt, word(self.img, pool as usize));
                    }
                    Op::Blx { .. } | Op::Udf { .. } | Op::Bkpt { .. } | Op::MovHi { rd: 15, .. } => {
                        self.warnings
                            .push(format!("{entry:#x}: unhandled {:?} at {addr:#x}", insn.op));
                        break;
                    }
                    Op::Svc { .. } => self
                        .warnings
                        .push(format!("{entry:#x}: svc at {addr:#x} (handler not added)")),
                    _ => {}
                }
                addr = next;
            }
        }
        self.active.remove(&entry);
        self.memo.insert(entry, best);
        best
    }
}

/// Depths by entry point name ("main" and the handlers, handlers with the exception frame), in report order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackReport {
    pub depths: Vec<(String, i64)>,
    /// Bytes between the initial SP and the stack limit.
    pub stack_size: i64,
    pub warnings: Vec<String>,
}

/// Analyses an image.
pub fn analyze(img: &[u8]) -> StackReport {
    let mut a = Analyzer {
        img,
        memo: HashMap::new(),
        active: HashSet::new(),
        warnings: Vec::new(),
    };
    let mut depths = vec![("main".to_string(), a.depth(MAIN))];
    for (vec, name) in HANDLERS {
        let h = word(img, 4 * vec) & !1;
        depths.push((name.to_string(), EXC_FRAME + a.depth(h)));
    }
    let stack_size = i64::from(word(img, 0)) - i64::from(STACK_LIMIT);
    let mut warnings = a.warnings;
    warnings.sort();
    warnings.dedup();
    StackReport {
        depths,
        stack_size,
        warnings,
    }
}
