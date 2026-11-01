//! One test per group of ARMv6-M instructions, with results and flags worked out from the pseudocode of the
//! ARMv6-M Architecture Reference Manual (AddWithCarry, Shift_C, section A6.7).

use super::*;

struct NoHooks;

impl Hooks for NoHooks {
    fn on_read(&mut self, _: &Cpu, _: &mut Memory, _: u32, _: u32) {}
    fn on_code(&mut self, _: &mut Cpu, _: &mut Memory, _: u32) -> bool {
        false
    }
}

const END: u32 = 0x0800;

/// Runs halfwords placed at 0x100 until they fall through to END (padded with `b END` jumps not needed: the
/// code is followed by a branch to END).
fn run(code: &[u16], setup: impl FnOnce(&mut Cpu)) -> Emu {
    let mut mem = Memory::default();
    mem.map(0, 0x1_0000, false);
    mem.map(0x2000_0000, 0x1000, false);
    let mut bytes: Vec<u8> = code.iter().flat_map(|h| h.to_le_bytes()).collect();
    let here = 0x100 + u32::try_from(bytes.len()).unwrap();
    let off = (END - (here + 4)) >> 1;
    bytes.extend_from_slice(&(0xE000 | u16::try_from(off & 0x7FF).unwrap()).to_le_bytes()); // b END
    mem.write(0x100, &bytes);
    let mut e = Emu::new(mem);
    e.cpu.r[13] = 0x2000_0800;
    setup(&mut e.cpu);
    assert_eq!(e.run(&mut NoHooks, 0x101, END, 1000), Ok(true));
    e
}

fn asm(src: &str) -> Vec<u16> {
    let p = thumb::assemble(src, 0x100).unwrap();
    p.code
        .chunks(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect()
}

fn flags(e: &Emu) -> (bool, bool, bool, bool) {
    (e.cpu.n, e.cpu.z, e.cpu.c, e.cpu.v)
}

#[test]
fn add_and_subtract_set_flags_like_add_with_carry() {
    let e = run(&asm("adds r2, r0, r1\n"), |c| (c.r[0], c.r[1]) = (0x7FFF_FFFF, 1));
    assert_eq!((e.cpu.r[2], flags(&e)), (0x8000_0000, (true, false, false, true)));
    let e = run(&asm("adds r2, r0, r1\n"), |c| (c.r[0], c.r[1]) = (0xFFFF_FFFF, 1));
    assert_eq!((e.cpu.r[2], flags(&e)), (0, (false, true, true, false)));
    let e = run(&asm("subs r2, r0, r1\n"), |c| (c.r[0], c.r[1]) = (0, 1));
    assert_eq!(
        (e.cpu.r[2], flags(&e)),
        (0xFFFF_FFFF, (true, false, false, false))
    );
    let e = run(&asm("subs r2, r0, r1\n"), |c| (c.r[0], c.r[1]) = (0x8000_0000, 1));
    assert_eq!((e.cpu.r[2], flags(&e)), (0x7FFF_FFFF, (false, false, true, true)));
    let e = run(&asm("cmp r0, #5\n"), |c| c.r[0] = 5);
    assert_eq!(flags(&e), (false, true, true, false));
    let e = run(&asm("cmp r0, #6\n"), |c| c.r[0] = 5);
    assert_eq!(flags(&e), (true, false, false, false));
    // adcs (0x4148 = adcs r0, r1) and sbcs (0x4188 = sbcs r0, r1) use the carry in
    let e = run(&[0x4148], |c| (c.r[0], c.r[1], c.c) = (1, 2, true));
    assert_eq!(e.cpu.r[0], 4);
    let e = run(&[0x4188], |c| (c.r[0], c.r[1], c.c) = (5, 2, false));
    assert_eq!((e.cpu.r[0], e.cpu.c), (2, true));
    // rsbs r0, r1, #0 (0x4248) and cmn r0, r1 (0x42C8)
    let e = run(&[0x4248], |c| c.r[1] = 1);
    assert_eq!((e.cpu.r[0], e.cpu.c), (0xFFFF_FFFF, false));
    let e = run(&[0x42C8], |c| (c.r[0], c.r[1]) = (1, 0xFFFF_FFFF));
    assert_eq!(flags(&e), (false, true, true, false));
}

#[test]
fn logic_and_multiply_keep_carry_and_overflow() {
    let set = |c: &mut Cpu| {
        (c.r[0], c.r[2], c.c, c.v) = (0xF0F0, 0x00FF, true, true);
    };
    let e = run(&asm("ands r0, r2\n"), set);
    assert_eq!((e.cpu.r[0], flags(&e)), (0xF0, (false, false, true, true)));
    let e = run(&asm("bics r0, r2\n"), set);
    assert_eq!(e.cpu.r[0], 0xF000);
    let e = run(&asm("orrs r0, r2\n"), set);
    assert_eq!(e.cpu.r[0], 0xF0FF);
    let e = run(&asm("tst r0, r2\n"), |c| (c.r[0], c.r[2]) = (0xF00, 0xFF));
    assert_eq!((e.cpu.r[0], e.cpu.z), (0xF00, true));
    let e = run(&[0x4050, 0x43D1], set); // eors r0, r2 ; mvns r1, r2
    assert_eq!((e.cpu.r[0], e.cpu.r[1]), (0xF00F, 0xFFFF_FF00));
    let e = run(&asm("muls r0, r2\n"), |c| (c.r[0], c.r[2]) = (0x1_0000, 0x1_0000));
    assert_eq!((e.cpu.r[0], e.cpu.z), (0, true));
}

#[test]
fn shifts_follow_shift_c() {
    let e = run(&asm("lsls r0, r1, #1\n"), |c| c.r[1] = 0x8000_0001);
    assert_eq!((e.cpu.r[0], e.cpu.c), (2, true));
    let e = run(&asm("lsls r0, r1, #0\n"), |c| (c.r[1], c.c) = (0x8000_0000, true));
    assert_eq!((e.cpu.r[0], e.cpu.c, e.cpu.n), (0x8000_0000, true, true));
    let e = run(&[0x0808], |c| c.r[1] = 0x8000_0000); // lsrs r0, r1, #32
    assert_eq!((e.cpu.r[0], e.cpu.c, e.cpu.z), (0, true, true));
    let e = run(&asm("lsrs r0, r1, #4\n"), |c| c.r[1] = 0x18);
    assert_eq!((e.cpu.r[0], e.cpu.c), (1, true));
    let e = run(&[0x1008], |c| c.r[1] = 0x8000_0000); // asrs r0, r1, #32
    assert_eq!((e.cpu.r[0], e.cpu.c), (0xFFFF_FFFF, true));
    let e = run(&asm("lsls r0, r1\n"), |c| (c.r[0], c.r[1]) = (1, 32));
    assert_eq!((e.cpu.r[0], e.cpu.c), (0, true));
    let e = run(&asm("lsls r0, r1\n"), |c| (c.r[0], c.r[1]) = (1, 33));
    assert_eq!((e.cpu.r[0], e.cpu.c), (0, false));
    let e = run(&asm("lsls r0, r1\n"), |c| {
        (c.r[0], c.r[1], c.c) = (5, 0x100, true);
    });
    assert_eq!((e.cpu.r[0], e.cpu.c), (5, true)); // shift by the low byte only: 0
    let e = run(&[0x41C8], |c| (c.r[0], c.r[1]) = (0x1, 1)); // rors r0, r1
    assert_eq!((e.cpu.r[0], e.cpu.c), (0x8000_0000, true));
    let e = run(&[0x4108], |c| (c.r[0], c.r[1]) = (0x8000_0000, 40)); // asrs r0, r1
    assert_eq!((e.cpu.r[0], e.cpu.c), (0xFFFF_FFFF, true));
}

#[test]
fn loads_stores_and_extends() {
    let code = asm(
        "ldr r1, lit\nstr r1, [r0, #4]\nldrb r2, [r0, #4]\nldrh r3, [r0, #6]\nmovs r4, #4\nldr r5, [r0, r4]\n\
         strb r2, [r0, #0]\nb over\n.align 2\nlit: .word 0x8180fe7f\nover:\n",
    );
    let e = run(&code, |c| c.r[0] = 0x2000_0000);
    assert_eq!(e.cpu.r[1..=3], [0x8180_FE7F, 0x7F, 0x8180]);
    assert_eq!(e.cpu.r[5], 0x8180_FE7F);
    assert_eq!(e.mem.read_u8(0x2000_0000), Some(0x7F));
    // ldrsb r2, [r0, r1] (0x5642) and ldrsh r2, [r0, r1] (0x5E42)
    let mut m = run(&[0x5642, 0x5E43], |_| {}).mem; // ldrsb r2, [r0, r1] ; ldrsh r3, [r0, r1]
    m.write(0x2000_0000, &[0x80, 0xFF]);
    let mut x = Emu::new(m);
    x.cpu.r[0] = 0x2000_0000;
    assert_eq!(x.run(&mut NoHooks, 0x101, END, 10), Ok(true));
    assert_eq!((x.cpu.r[2], x.cpu.r[3]), (0xFFFF_FF80, 0xFFFF_FF80));
    let e = run(
        &asm("sxth r1, r0\nuxtb r2, r0\nuxth r3, r0\nsxtb r4, r0\n"),
        |c| c.r[0] = 0x1234_8081,
    );
    assert_eq!(e.cpu.r[1..=4], [0xFFFF_8081, 0x81, 0x8081, 0xFFFF_FF81]);
    // rev r1, r0 ; rev16 r2, r0 ; revsh r3, r0
    let e = run(&[0xBA01, 0xBA42, 0xBAC3], |c| c.r[0] = 0x1122_3380);
    assert_eq!(e.cpu.r[1..=3], [0x8033_2211, 0x2211_8033, 0xFFFF_8033]);
}

#[test]
fn stack_calls_and_multiple_transfers() {
    let code = asm(
        "push {r4, r5, lr}\nmovs r4, #7\nbl f\npop {r4, r5}\nb out\nf:\npush {lr}\nmovs r0, #9\npop {pc}\nout:\n",
    );
    let e = run(&code, |c| {
        c.r[4] = 1;
        c.r[14] = 0x51;
    });
    assert_eq!((e.cpu.r[0], e.cpu.r[4], e.cpu.r[13]), (9, 1, 0x2000_0800 - 4));
    // stmia r0!, {r1, r2} (0xC006) then ldmia r0, {r0, r3} after moving r0 back (0xC809 = ldmia r0!, {r0, r3})
    let e = run(&[0xC006, 0x3808, 0xC809], |c| {
        (c.r[0], c.r[1], c.r[2]) = (0x2000_0000, 0xAA, 0xBB);
    });
    assert_eq!((e.cpu.r[0], e.cpu.r[3]), (0xAA, 0xBB)); // base in the list: no writeback
}

#[test]
fn branches_take_the_right_side() {
    let src = "cmp r0, #3\nbgt big\nmovs r1, #1\nb out\nbig:\nmovs r1, #2\nout:\n";
    assert_eq!(run(&asm(src), |c| c.r[0] = 5).cpu.r[1], 2);
    assert_eq!(run(&asm(src), |c| c.r[0] = 3).cpu.r[1], 1);
    assert_eq!(run(&asm(src), |c| c.r[0] = 0xFFFF_FFFF).cpu.r[1], 1); // -1 is not greater
    let src = "cmp r0, #3\nbhi big\nmovs r1, #1\nb out\nbig:\nmovs r1, #2\nout:\n";
    assert_eq!(run(&asm(src), |c| c.r[0] = 0xFFFF_FFFF).cpu.r[1], 2); // unsigned
    // mov pc / add pc and reading pc as an operand (= address + 4)
    let e = run(&[0x4678], |_| {}); // mov r0, pc
    assert_eq!(e.cpu.r[0], 0x104);
}

#[test]
fn special_registers_and_interrupt_mask() {
    let e = run(&[0xB672, 0xF3EF, 0x8010], |_| {}); // cpsid i ; mrs r0, primask
    assert_eq!(e.cpu.r[0], 1);
    let e = run(&[0xF380, 0x8808, 0xB662], |c| c.r[0] = 0x2000_0400); // msr msp, r0 ; cpsie i
    assert_eq!((e.cpu.r[13], e.cpu.primask), (0x2000_0400, false));
}

#[test]
fn stops_instead_of_guessing() {
    let mut mem = Memory::default();
    mem.map(0, 0x1000, false);
    mem.write(0x100, &[0x00, 0xDF]); // svc 0
    let mut e = Emu::new(mem.clone());
    assert!(e.run(&mut NoHooks, 0x101, END, 10).is_err());
    mem.write(0x100, &[0x00, 0xB1]); // cbz (ARMv7-M)
    let mut e = Emu::new(mem.clone());
    assert!(e.run(&mut NoHooks, 0x101, END, 10).is_err());
    mem.write(0x100, &[0x00, 0x68]); // ldr r0, [r0] with r0 = 0x2000_0000: unmapped
    let mut e = Emu::new(mem.clone());
    e.cpu.r[0] = 0x2000_0000;
    assert!(e.run(&mut NoHooks, 0x101, END, 10).is_err());
    mem.write(0x100, &[0x00, 0x47]); // bx r0 with an even address
    let mut e = Emu::new(mem);
    e.cpu.r[0] = 0x200;
    assert!(e.run(&mut NoHooks, 0x101, END, 10).is_err());
}

#[test]
fn hooks_see_reads_and_can_redirect() {
    struct H {
        reads: Vec<u32>,
    }
    impl Hooks for H {
        fn on_read(&mut self, _: &Cpu, mem: &mut Memory, addr: u32, _: u32) {
            self.reads.push(addr);
            mem.write(addr, &0x55u32.to_le_bytes());
        }
        fn on_code(&mut self, cpu: &mut Cpu, _: &mut Memory, _: u32) -> bool {
            cpu.r[15] = cpu.r[14] & !1; // skip the function
            true
        }
    }
    let mut mem = Memory::default();
    mem.map(0, 0x1000, false);
    mem.map(0x4000_0000, 0x100, true);
    let p = thumb::assemble("ldr r1, [r0]\nbl 0x300\nb 0x800\n", 0x100).unwrap();
    mem.write(0x100, &p.code);
    let mut e = Emu::new(mem);
    e.watch(0x300);
    e.cpu.r[0] = 0x4000_0010;
    let mut h = H { reads: vec![] };
    assert_eq!(e.run(&mut h, 0x101, END, 100), Ok(true));
    assert_eq!((e.cpu.r[1], h.reads), (0x55, vec![0x4000_0010]));
}
