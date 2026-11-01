//! Tap-hold keys for the V72 firmware (port of `taphold.py`; the design, the hooks and their proofs are in that
//! file and in `FN_ANALYSIS.md` sections 10-12 of the analysis repository). Two kinds:
//!
//! - key hold, at most one key (CapsLock: tap Esc, hold LCtrl), decided like kanata's tap-hold-press;
//! - layer hold, at most two keys sharing one state machine (left Space: tap Space, hold layer 3), decided by
//!   time only, with the keys pressed meanwhile held back and replayed.
//!
//! The code goes to 0xd200 (free flash, 0xd124-0xe7ff), is entered from the two calls to 0x6984 at 0x7cb6 and
//! 0x7cc0, from the main-loop call at 0x2090 and from vector 31, and leaves every hook with a tail jump so the
//! stack is never deeper than in stock. Its 48 bytes of state are taken from the top of the stack.

use crate::Error;
use crate::v72::{Firmware, NAME_KEYMAP};

pub const BASE: u32 = 0xD200;
pub const REGION_END: u32 = 0xE800;
pub const STACK_TOP_OLD: u32 = 0x2000_0A90;
pub const VARS_SIZE: u32 = 48;
pub const VARS: u32 = STACK_TOP_OLD - VARS_SIZE;
pub const STACK_TOP_NEW: u32 = VARS;
// State block offsets
const C_ST: u32 = 0;
const S_ST: u32 = 1;
const NBUF: u32 = 2;
const SBASE: u32 = 3;
const NOW: u32 = 4;
const C_T0: u32 = 8;
const S_T0: u32 = 12;
const RESTORE: u32 = 16;
const RIDX: u32 = 17;
const S_WHICH: u32 = 18;
const HLAYER: u32 = 19;
const BUF: u32 = 20;
const BITMAP: u32 = 32;
pub const MAX_LAYER_KEYS: usize = 2;
/// Held-back events while the layer key is undecided.
const BUF_N: u32 = 12;
const _: () = assert!(BITMAP + 16 <= VARS_SIZE && BUF + BUF_N <= BITMAP);
/// 128 x (code, position), copied from flash at boot and edited by DIP modes.
pub const KEYMAP_RAM: u32 = 0x2000_05D6;
pub const LAYER_RAM: u32 = 0x2000_00F8;
/// Fn, Fn2, Fn3 held (codes 0xa4-0xa6).
const FN_FLAGS: u32 = 0x2000_0078;
const FN_MASK: u32 = 0x0410_2000;
pub const EVENT_FN: u32 = 0x6984;
pub const DISPATCH: u32 = 0x64C6;
pub const LOOP_STEP: u32 = 0x7E98;
pub const STOCK_ISR: u32 = 0x583C;
pub const EVENT_CALLS: [u32; 2] = [0x7CB6, 0x7CC0];
pub const LOOP_CALL: u32 = 0x2090;
pub const ISR_VECTOR: u32 = 31 * 4;
const SP_LITERAL: u32 = 0xBEA8;
const TAP_MS: u32 = 20;
/// Key index that never occurs (feature not configured).
const NO_KEY: u8 = 0xFF;

/// A key-hold key with its stock code: (key, expected code, tap, hold, term_ms 50-255).
pub type KeySpec = (u8, u8, u8, u8, u16);
/// A layer-hold key with its stock code: (key, expected code, tap, layer 0-3, term_ms 50-2000).
pub type LayerSpec = (u8, u8, u8, u8, u16);

fn route(p: &str) -> String {
    format!(
        "
    ldrb r7, [r6, #{S_ST}]
    ldrb r3, [r6, #{RESTORE}]
    cmp r3, #0
    beq {p}cur
    subs r3, #1
    b {p}have
{p}cur:
    ldr r3, lit_layer
    ldrb r3, [r3]
{p}have:
    mov r0, r4
    lsrs r0, r0, #3
    mov r1, r6
    adds r1, #{BITMAP}
    adds r1, r1, r0
    mov r0, r4
    movs r2, #7
    ands r0, r2
    movs r2, #1
    lsls r2, r0
    ldrb r0, [r1]
    cmp r5, #0
    bne {p}up
    bics r0, r2
    cmp r7, #2
    bne {p}store
    orrs r0, r2
    b {p}store
{p}up:
    tst r0, r2
    beq {p}up_plain
    bics r0, r2
    strb r0, [r1]
    ldrb r0, [r6, #{HLAYER}]
    b {p}set
{p}up_plain:
    cmp r7, #2
    bne {p}store
    ldrb r0, [r6, #{SBASE}]
    b {p}set
{p}store:
    strb r0, [r1]
    mov r0, r3
{p}set:
    ldr r1, lit_layer
    strb r0, [r1]
    movs r2, #0
    cmp r0, r3
    beq {p}keep
    adds r2, r3, #1
{p}keep:
    strb r2, [r6, #{RESTORE}]
"
    )
}

/// The assembly source. `caps` / `layers` as in [`apply`]; unused features get key 0xff and stay idle.
#[allow(clippy::too_many_lines)]
pub fn source(caps: Option<KeySpec>, layers: &[LayerSpec]) -> Result<String, Error> {
    let bad = |m: String| Err(Error::Config(m));
    let (ck, cexp, ctap, chold, cterm) = caps.unwrap_or((NO_KEY, 0, 0, 0, 200));
    if layers.len() > MAX_LAYER_KEYS {
        return bad(format!("at most {MAX_LAYER_KEYS} layer hold keys"));
    }
    let keys: Vec<u8> = std::iter::once(ck)
        .chain(layers.iter().map(|s| s.0))
        .filter(|&k| k != NO_KEY)
        .collect();
    let mut uniq = keys.clone();
    uniq.sort_unstable();
    uniq.dedup();
    if uniq.len() != keys.len() {
        return bad("tap-hold keys must differ".into());
    }
    let mut l: Vec<LayerSpec> = layers.to_vec();
    while l.len() < MAX_LAYER_KEYS {
        l.push((NO_KEY, 0, 0, 0, 300));
    }
    for (i, s) in l.iter().enumerate() {
        if s.3 > 3 {
            return bad("hold layer must be 0-3 (Diatec layer 1-4)".into());
        }
        if !(50..=2000).contains(&s.4) {
            return bad(format!("layer hold {i}: term_ms must be 50-2000"));
        }
    }
    if !(50..=255).contains(&cterm) {
        return bad("key hold term_ms must be 50-255".into());
    }
    let (sk0, sexp0, stap0, slayer0, sterm0) = l[0];
    let (sk1, sexp1, stap1, slayer1, sterm1) = l[1];
    let route_ev = route("ev_");
    let route_lp = route("lp_");
    let ckeymap = KEYMAP_RAM + 2 * u32::from(ck & 0x7F);
    let lit_isr = STOCK_ISR | 1;
    let lit_event = EVENT_FN | 1;
    let lit_step = LOOP_STEP | 1;
    let bitmap4 = BITMAP + 4;
    let bitmap8 = BITMAP + 8;
    let bitmap12 = BITMAP + 12;
    Ok(format!(
        "
on_event:
    push {{r4, r5, r6, r7, lr}}
    mov r4, r0
    mov r5, r1
    ldr r6, lit_vars
    cmp r4, #{ck}
    bne ev_notcaps
    b c_key
ev_notcaps:
    ldrb r0, [r6, #{C_ST}]
    cmp r0, #1
    bne ev_k2
    cmp r5, #0
    bne ev_k2
    bl c_to_hold
ev_k2:
    movs r7, #0
    cmp r4, #{sk0}
    beq ev_layer_key
    movs r7, #1
    cmp r4, #{sk1}
    bne ev_other
ev_layer_key:
    b s_key
ev_other:
    ldrb r0, [r6, #{S_ST}]
    cmp r0, #1
    beq ev_hold_back
    cmp r0, #5
    bne ev_route
ev_hold_back:
    ldrb r1, [r6, #{NBUF}]
    cmp r1, #{BUF_N}
    bhs ev_full
    mov r0, r4
    cmp r5, #0
    beq ev_hb_store
    adds r0, #0x80
ev_hb_store:
    mov r2, r6
    adds r2, #{BUF}
    strb r0, [r2, r1]
    adds r1, #1
    strb r1, [r6, #{NBUF}]
    pop {{r4, r5, r6, r7, pc}}
ev_full:
    ldrb r0, [r6, #{S_ST}]
    cmp r0, #1
    bne ev_route
    movs r0, #5
    strb r0, [r6, #{S_ST}]
ev_route:
{route_ev}
pass:
    mov r0, r4
    mov r1, r5
    pop {{r4, r5, r6, r7}}
    pop {{r2}}
    mov lr, r2
    ldr r2, lit_event
    bx r2

c_key:
    cmp r5, #0
    bne c_up
    ldrb r0, [r6, #{C_ST}]
    cmp r0, #3
    bne c_down1
    movs r0, #{ctap}
    movs r1, #2
    bl {DISPATCH:#x}
    movs r0, #0
    strb r0, [r6, #{C_ST}]
c_down1:
    ldr r0, lit_ckeymap
    ldrb r0, [r0]
    cmp r0, #{cexp}
    bne ev_route
    ldr r0, lit_fnflags
    ldr r0, [r0]
    ldr r1, lit_fnmask
    tst r0, r1
    beq c_pending
    movs r0, #4
    strb r0, [r6, #{C_ST}]
    b ev_route
c_pending:
    movs r0, #1
    strb r0, [r6, #{C_ST}]
    ldr r0, [r6, #{NOW}]
    str r0, [r6, #{C_T0}]
    pop {{r4, r5, r6, r7, pc}}
c_up:
    ldrb r0, [r6, #{C_ST}]
    cmp r0, #4
    beq c_up_pass
    cmp r0, #2
    beq c_up_hold
    cmp r0, #1
    bne ev_route
    movs r0, #3
    strb r0, [r6, #{C_ST}]
    ldr r0, [r6, #{NOW}]
    str r0, [r6, #{C_T0}]
    movs r0, #{ctap}
    movs r1, #0
    bl {DISPATCH:#x}
    pop {{r4, r5, r6, r7, pc}}
c_up_hold:
    movs r0, #0
    strb r0, [r6, #{C_ST}]
    movs r0, #{chold}
    movs r1, #2
    bl {DISPATCH:#x}
    pop {{r4, r5, r6, r7, pc}}
c_up_pass:
    movs r0, #0
    strb r0, [r6, #{C_ST}]
    b ev_route

c_to_hold:
    push {{lr}}
    movs r0, #2
    ldr r1, lit_vars
    strb r0, [r1, #{C_ST}]
    movs r0, #{chold}
    movs r1, #0
    bl {DISPATCH:#x}
    pop {{pc}}

s_key:
    ldrb r0, [r6, #{S_ST}]
    cmp r0, #0
    beq s_mine
    cmp r0, #3
    beq s_mine
    ldrb r1, [r6, #{S_WHICH}]
    cmp r1, r7
    beq s_mine
    b ev_other
s_mine:
    cmp r5, #0
    bne s_up
    cmp r0, #3
    bne s_down1
    mov r5, r7
    ldrb r7, [r6, #{S_WHICH}]
    bl s_params
    mov r7, r5
    movs r5, #0
    movs r1, #2
    bl {DISPATCH:#x}
    movs r0, #0
    strb r0, [r6, #{S_ST}]
s_down1:
    strb r7, [r6, #{S_WHICH}]
    bl s_params
    ldr r0, lit_keymap
    lsls r3, r4, #1
    ldrb r0, [r0, r3]
    cmp r0, r2
    bne s_route
    ldr r0, lit_fnflags
    ldr r0, [r0]
    ldr r2, lit_fnmask
    tst r0, r2
    bne s_pass
    ldrb r0, [r6, #{HLAYER}]
    cmp r0, r1
    beq s_pending
    ldr r0, [r6, #{BITMAP}]
    ldr r2, [r6, #{bitmap4}]
    orrs r0, r2
    ldr r2, [r6, #{bitmap8}]
    orrs r0, r2
    ldr r2, [r6, #{bitmap12}]
    orrs r0, r2
    beq s_pending
s_pass:
    movs r0, #4
    strb r0, [r6, #{S_ST}]
    b ev_route
s_pending:
    movs r0, #1
    strb r0, [r6, #{S_ST}]
    movs r0, #0
    strb r0, [r6, #{NBUF}]
    ldr r0, [r6, #{NOW}]
    str r0, [r6, #{S_T0}]
    pop {{r4, r5, r6, r7, pc}}
s_up:
    ldrb r0, [r6, #{S_ST}]
    cmp r0, #4
    beq s_up_pass
    cmp r0, #2
    beq s_up_layer
    cmp r0, #1
    bne s_route
    movs r0, #5
    strb r0, [r6, #{S_ST}]
    pop {{r4, r5, r6, r7, pc}}
s_up_layer:
    movs r0, #0
    strb r0, [r6, #{S_ST}]
    strb r0, [r6, #{RESTORE}]
    ldrb r0, [r6, #{SBASE}]
    ldr r1, lit_layer
    strb r0, [r1]
    pop {{r4, r5, r6, r7, pc}}
s_up_pass:
    movs r0, #0
    strb r0, [r6, #{S_ST}]
    b ev_route
s_route:
    b ev_route

s_params:
    cmp r7, #0
    bne s_params1
    movs r0, #{stap0}
    movs r1, #{slayer0}
    movs r2, #{sexp0}
    ldr r3, lit_sterm0
    bx lr
s_params1:
    movs r0, #{stap1}
    movs r1, #{slayer1}
    movs r2, #{sexp1}
    ldr r3, lit_sterm1
    bx lr

on_loop:
    push {{r4, r5, r6, r7, lr}}
    ldr r6, lit_vars
    bl lp_restore
    ldrb r0, [r6, #{C_ST}]
    cmp r0, #1
    beq lp_c_pending
    cmp r0, #3
    bne lp_space
    ldr r0, [r6, #{NOW}]
    ldr r1, [r6, #{C_T0}]
    subs r0, r0, r1
    cmp r0, #{TAP_MS}
    blo lp_space
    movs r0, #0
    strb r0, [r6, #{C_ST}]
    movs r0, #{ctap}
    movs r1, #2
    bl {DISPATCH:#x}
    b lp_space
lp_c_pending:
    ldr r0, [r6, #{NOW}]
    ldr r1, [r6, #{C_T0}]
    subs r0, r0, r1
    cmp r0, #{cterm}
    blo lp_space
    bl c_to_hold
lp_space:
    ldrb r7, [r6, #{S_WHICH}]
    bl s_params
    ldrb r2, [r6, #{S_ST}]
    cmp r2, #1
    beq lp_s_pending
    cmp r2, #5
    beq lp_s_tap
    cmp r2, #3
    bne lp_done
    ldr r1, [r6, #{NOW}]
    ldr r2, [r6, #{S_T0}]
    subs r1, r1, r2
    cmp r1, #{TAP_MS}
    blo lp_done
    movs r1, #0
    strb r1, [r6, #{S_ST}]
    movs r1, #2
    bl {DISPATCH:#x}
    b lp_done
lp_s_pending:
    ldr r0, [r6, #{NOW}]
    ldr r2, [r6, #{S_T0}]
    subs r0, r0, r2
    cmp r0, r3
    blo lp_done
    strb r1, [r6, #{HLAYER}]
    ldr r2, lit_layer
    ldrb r0, [r2]
    strb r0, [r6, #{SBASE}]
    strb r1, [r2]
    movs r0, #2
    strb r0, [r6, #{S_ST}]
    b lp_replay
lp_s_tap:
    movs r1, #3
    strb r1, [r6, #{S_ST}]
    ldr r1, [r6, #{NOW}]
    str r1, [r6, #{S_T0}]
    movs r1, #0
    bl {DISPATCH:#x}
lp_replay:
    movs r0, #0
    strb r0, [r6, #{RIDX}]
lp_r_next:
    ldrb r7, [r6, #{RIDX}]
    ldrb r0, [r6, #{NBUF}]
    cmp r7, r0
    bhs lp_r_done
    adds r0, r7, #1
    strb r0, [r6, #{RIDX}]
    mov r1, r6
    adds r1, #{BUF}
    ldrb r0, [r1, r7]
    movs r5, #0
    cmp r0, #0x80
    blo lp_r_key
    subs r0, #0x80
    movs r5, #2
lp_r_key:
    mov r4, r0
{route_lp}
    mov r0, r4
    mov r1, r5
    bl {EVENT_FN:#x}
    b lp_r_next
lp_r_done:
    movs r0, #0
    strb r0, [r6, #{NBUF}]
    bl lp_restore
lp_done:
    pop {{r4, r5, r6, r7}}
    pop {{r2}}
    mov lr, r2
    ldr r2, lit_step
    bx r2

lp_restore:
    ldrb r0, [r6, #{RESTORE}]
    cmp r0, #0
    beq lp_restore_done
    subs r0, #1
    ldr r1, lit_layer
    strb r0, [r1]
    movs r0, #0
    strb r0, [r6, #{RESTORE}]
lp_restore_done:
    bx lr

on_tick:
    ldr r0, lit_vars
    ldr r1, [r0, #{NOW}]
    adds r1, r1, #1
    str r1, [r0, #{NOW}]
    ldr r0, lit_isr
    bx r0

    .align 2
lit_vars:    .word {VARS:#x}
lit_ckeymap: .word {ckeymap:#x}
lit_keymap:  .word {KEYMAP_RAM:#x}
lit_fnflags: .word {FN_FLAGS:#x}
lit_fnmask:  .word {FN_MASK:#x}
lit_layer:   .word {LAYER_RAM:#x}
lit_sterm0:  .word {sterm0}
lit_sterm1:  .word {sterm1}
lit_isr:     .word {lit_isr:#x}
lit_event:   .word {lit_event:#x}
lit_step:    .word {lit_step:#x}
"
    ))
}

/// Entry points of the assembled code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entries {
    pub on_event: u32,
    pub on_loop: u32,
    pub on_tick: u32,
}

/// Assembles the source at [`BASE`].
pub fn assemble(src: &str) -> Result<(Vec<u8>, Entries), Error> {
    let p = thumb::assemble(src, BASE).map_err(|e| Error::Asm(e.to_string()))?;
    crate::check_m0(
        &p.code[..(p.symbol("lit_vars").map_err(|e| Error::Asm(e.to_string()))? - BASE) as usize],
        BASE,
    )?;
    let s = |n: &str| p.symbol(n).map_err(|e| Error::Asm(e.to_string()));
    Ok((
        p.code.clone(),
        Entries {
            on_event: s("on_event")?,
            on_loop: s("on_loop")?,
            on_tick: s("on_tick")?,
        },
    ))
}

fn u(a: u32) -> usize {
    a as usize
}

/// Patches `fw` in place and logs every byte. Each spec carries the key's stock code, checked here.
pub fn apply(fw: &mut Firmware, caps: Option<KeySpec>, layers: &[LayerSpec]) -> Result<Entries, Error> {
    let specs = caps
        .iter()
        .map(|s| (s.0, s.1))
        .chain(layers.iter().map(|s| (s.0, s.1)));
    for (k, exp) in specs {
        let stock = fw.b[usize::from(k) * 2 + NAME_KEYMAP];
        if stock != exp {
            let n = fw.names.get(&k).cloned().unwrap_or_default();
            return Err(Error::Config(format!(
                "{n}: stock code is {stock:#x}, expected {exp:#x}"
            )));
        }
    }
    let mut checks: Vec<(u32, Vec<u8>, &str)> = vec![
        (
            LOOP_CALL,
            thumb::bl(LOOP_CALL, LOOP_STEP).to_vec(),
            "main loop calls 0x7e98",
        ),
        (
            ISR_VECTOR,
            (STOCK_ISR | 1).to_le_bytes().to_vec(),
            "vector 31 = 0x583c",
        ),
        (0, STACK_TOP_OLD.to_le_bytes().to_vec(), "initial SP"),
        (
            SP_LITERAL,
            STACK_TOP_OLD.to_le_bytes().to_vec(),
            "stack top literal",
        ),
    ];
    for a in EVENT_CALLS {
        checks.push((a, thumb::bl(a, EVENT_FN).to_vec(), "0x7c72 calls 0x6984"));
    }
    for (addr, want, what) in &checks {
        if fw.b[u(*addr)..u(*addr) + want.len()] != want[..] {
            return Err(Error::Structure(format!(
                "stock structure not found at {addr:#x} ({what})"
            )));
        }
    }
    let (code, entry) = assemble(&source(caps, layers)?)?;
    if u(BASE) + code.len() > u(REGION_END) || fw.b[u(BASE)..u(REGION_END)].iter().any(|&x| x != 0) {
        return Err(Error::Structure("code region 0xd200-0xe7ff is not free".into()));
    }
    fw.put_bytes(u(BASE), &code, &format!("tap-hold code ({} bytes)", code.len()));
    for a in EVENT_CALLS {
        let why = format!("0x7c72: key events go through tap-hold ({:#x})", entry.on_event);
        fw.put_bytes(u(a), &thumb::bl(a, entry.on_event), &why);
    }
    let why = format!("main loop step goes through tap-hold ({:#x})", entry.on_loop);
    fw.put_bytes(u(LOOP_CALL), &thumb::bl(LOOP_CALL, entry.on_loop), &why);
    let why = format!(
        "vector 31: 1 ms timer counts for tap-hold first ({:#x})",
        entry.on_tick
    );
    fw.put_bytes(u(ISR_VECTOR), &(entry.on_tick | 1).to_le_bytes(), &why);
    for (addr, what) in [
        (0, "vector 0: initial SP"),
        (SP_LITERAL, "__user_initial_stackheap: stack top"),
    ] {
        let why =
            format!("{what} {STACK_TOP_OLD:#x} -> {STACK_TOP_NEW:#x} (tap-hold state, {VARS_SIZE} bytes)");
        fw.put_bytes(u(addr), &STACK_TOP_NEW.to_le_bytes(), &why);
    }
    Ok(entry)
}
