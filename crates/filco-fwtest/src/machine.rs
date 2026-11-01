//! The real V72 firmware code running in [`m0emu`], with a modelled key matrix (port of `emu.py`).
//!
//! Boots from the reset vector with peripherals stubbed: reads return what was last written, and a register read
//! more than 50 times alternates all-zero and all-one (so polling loops end); the busy delay 0xb090 is skipped.
//! The column port 0x40048000 returns the pressed keys of the row being scanned (the scan 0x7db2 keeps the row
//! in r4; columns active low in bits 3..10). One [`V72::step`] is one main-loop step followed by one call of the
//! 1 ms timer handler, so time-outs expire as on the device. Every call of the key-code dispatcher 0x64c6 is
//! logged. Nothing is sent to a device.

use std::collections::{BTreeSet, HashMap};

use m0emu::{Cpu, Emu, Hooks, Memory, Stop};

/// Key-code dispatcher: r0 = HID code, r1 = event (0 down, 2 up).
pub const DISPATCH: u32 = 0x64C6;
/// main(): `bl` to the main-loop step, redirected by the tap-hold patch.
pub const MAIN_STEP_CALL: u32 = 0x2090;
const GPIO_COLS: u32 = 0x4004_8000;
const DELAY: u32 = 0xB090;
const TIMER_VECTOR: usize = 31;
/// Sentinel return address (erased flash).
const RET: u32 = 0xFFF0;
pub const LAYER: u32 = 0x2000_00F8;
pub const DOWN: u8 = 0;
pub const UP: u8 = 2;
/// Main-loop steps after each matrix change (debounce needs several scans).
pub const TICKS: u32 = 40;
const BOOT_STEPS: u64 = 3_000_000;
const CALL_STEPS: u64 = 500_000;

/// Hook state: the key matrix, peripheral read counts and the dispatcher log.
#[derive(Debug, Default, Clone)]
pub struct Env {
    pub pressed: BTreeSet<u8>,
    mmio_reads: HashMap<u32, u32>,
    pub calls: Vec<(u8, u8)>,
}

impl Hooks for Env {
    fn on_read(&mut self, cpu: &Cpu, mem: &mut Memory, addr: u32, size: u32) {
        if addr == GPIO_COLS {
            let row = cpu.r[4] & 0xFF;
            let cols: u32 = self
                .pressed
                .iter()
                .filter(|&&k| u32::from(k >> 3) == row)
                .map(|k| 1 << (k & 7))
                .sum();
            mem.write(addr, &(!(cols << 3)).to_le_bytes());
            return;
        }
        let n = self.mmio_reads.entry(addr).or_insert(0);
        *n += 1;
        if *n > 50 {
            let fill = if *n & 1 == 1 { 0xFF } else { 0 };
            mem.write(addr, &vec![fill; size as usize]);
        }
    }

    fn on_code(&mut self, cpu: &mut Cpu, _: &mut Memory, pc: u32) -> bool {
        if pc == DISPATCH {
            #[allow(clippy::cast_possible_truncation)]
            self.calls.push((cpu.r[0] as u8, cpu.r[1] as u8));
            false
        } else {
            cpu.r[15] = cpu.r[14] & !1; // DELAY: return at once
            true
        }
    }
}

/// Target of the Thumb `bl` at `addr`.
pub fn bl_target(img: &[u8], addr: u32) -> Option<u32> {
    let insn = thumb::decode::decode_at(img, addr as usize)?;
    match insn.op {
        thumb::Op::Bl { off } => Some(addr.wrapping_add(4).wrapping_add_signed(off)),
        _ => None,
    }
}

fn word(img: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([img[at], img[at + 1], img[at + 2], img[at + 3]])
}

/// A booted V72 image.
#[derive(Debug, Clone)]
pub struct V72 {
    pub emu: Emu,
    pub env: Env,
    main_step: u32,
    timer_isr: u32,
    sp: u32,
}

/// Layer slots (`[type, modifier mask, code, 0]`) placed in the layer pages before boot, by (layer 0-3, key).
pub type Slots<'a> = &'a [((u8, u8), [u8; 4])];

impl V72 {
    /// Boots `img` with `slots` written into the layer pages 0xf000 + 0x240 * layer, then runs 20 steps.
    pub fn new(img: &[u8], slots: Slots<'_>) -> Result<Self, Stop> {
        let mut img = img.to_vec();
        for &((layer, k), slot) in slots {
            let off =
                0xF000 + 0x240 * usize::from(layer) + 0x40 * usize::from(k & 7) + 4 * usize::from(k >> 3);
            img[off..off + 4].copy_from_slice(&slot);
        }
        let main_step = bl_target(&img, MAIN_STEP_CALL).ok_or_else(|| Stop {
            pc: 0,
            reason: "no bl at 0x2090".into(),
        })?;
        let timer_isr = word(&img, 4 * TIMER_VECTOR) & !1;
        let mut mem = Memory::default();
        mem.map(0, 0x1_0000, false);
        mem.map(0x2000_0000, 0x1_0000, false);
        for base in [0x4000_0000, 0x5000_0000, 0xE000_0000] {
            mem.map(base, 0x10_0000, true);
        }
        mem.write(0, &img);
        let mut emu = Emu::new(mem);
        emu.watch(DISPATCH);
        emu.watch(DELAY);
        let (sp, reset) = (word(&img, 0), word(&img, 4));
        emu.cpu.r[13] = sp;
        let mut env = Env::default();
        emu.run(&mut env, reset | 1, 0xFFFF_FFFF, BOOT_STEPS)
            .map_err(|e| Stop {
                pc: e.pc,
                reason: format!("boot stopped: {}", e.reason),
            })?;
        let mut m = Self {
            emu,
            env,
            main_step,
            timer_isr,
            sp,
        };
        for _ in 0..20 {
            m.step()?;
        }
        Ok(m)
    }

    /// Calls the function at `addr` with a fresh stack below the initial SP and returns when it returns.
    pub fn call(&mut self, addr: u32) -> Result<(), Stop> {
        self.emu.cpu.r[13] = self.sp - 0x100;
        self.emu.cpu.r[14] = RET | 1;
        if self.emu.run(&mut self.env, addr | 1, RET, CALL_STEPS)? {
            Ok(())
        } else {
            Err(Stop {
                pc: self.emu.cpu.r[15],
                reason: format!("call {addr:#x} did not return"),
            })
        }
    }

    /// One main-loop step followed by one 1 ms timer tick.
    pub fn step(&mut self) -> Result<(), Stop> {
        self.call(self.main_step)?;
        self.call(self.timer_isr)
    }

    pub fn idle(&mut self, steps: u32) -> Result<(), Stop> {
        for _ in 0..steps {
            self.step()?;
        }
        Ok(())
    }

    /// Changes one key without running the firmware.
    pub fn press(&mut self, k: u8, down: bool) {
        if down {
            self.env.pressed.insert(k);
        } else {
            self.env.pressed.remove(&k);
        }
    }

    /// Changes one key and runs [`TICKS`] steps.
    pub fn set_key(&mut self, k: u8, down: bool) -> Result<(), Stop> {
        self.press(k, down);
        self.idle(TICKS)
    }

    /// Holds `holds` in order, taps `key`, releases `holds` in reverse; returns the dispatcher calls.
    pub fn chord(&mut self, holds: &[u8], key: u8) -> Result<Vec<(u8, u8)>, Stop> {
        self.env.calls.clear();
        for &h in holds {
            self.set_key(h, true)?;
        }
        self.set_key(key, true)?;
        self.set_key(key, false)?;
        for &h in holds.iter().rev() {
            self.set_key(h, false)?;
        }
        self.idle(TICKS)?;
        Ok(self.env.calls.clone())
    }

    pub fn ram_u8(&self, addr: u32) -> u8 {
        self.emu.mem.read_u8(addr).unwrap_or(0)
    }

    pub fn ram_u32(&self, addr: u32) -> u32 {
        self.emu.mem.read_u32(addr).unwrap_or(0)
    }

    /// Active layer 0-3.
    pub fn layer(&self) -> u8 {
        self.ram_u8(LAYER)
    }
}

/// First key code sent down that is not the Fn code.
pub fn first_down(calls: &[(u8, u8)]) -> Option<u8> {
    calls
        .iter()
        .find(|&&(c, ev)| ev == DOWN && c != filco_firmware::v72::FN_CODE)
        .map(|&(c, _)| c)
}
