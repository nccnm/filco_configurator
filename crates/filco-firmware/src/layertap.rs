//! Tap-hold set per layer in the layer pages, for the V72 firmware.
//!
//! A layer slot of type 8 makes its key a tap-hold key in that layer: `[08, tap, hold, term]`, where `tap` is
//! the key code sent on a tap, `hold` the key code sent while held (a modifier or any key) or `F0 + n` to turn on
//! layer n (0-3) while held, and `term` the decision time in 10 ms steps (0 = 200 ms). Stock firmware reads type
//! 8 as "use the default keymap", so the same pages still work as plain keys on stock.
//!
//! With Fn held a key goes the stock way, so Fn + key keeps the Fn layer (Fn + ; = Home). The slot is read from
//! the page of the layer the key is pressed in, so each layer has its own tap-hold keys, any number of them.
//!
//! Two state machines run side by side, as in the global tap-hold of `taphold.rs`:
//!
//! - key hold (`hold` < 0xF0), decided like kanata's tap-hold-press: another key going down, or the term
//!   passing, makes it a hold;
//! - layer hold (`hold` >= 0xF0), decided by time only, with the keys pressed meanwhile held back and replayed.
//!
//! Each machine remembers only its key and the layer it went down in; tap, hold and term are read again from
//! the slot in flash. A tap-hold key that finds its machine busy is a plain key (its default code, the stock
//! way, on press and release alike, so nothing sticks).
//!
//! The hooks are the ones of `taphold.rs` (the two calls to 0x6984, the main-loop call, vector 31), every hook
//! leaves with a tail jump, and the 48 bytes of state are the same top of the stack.

use crate::Error;
use crate::taphold::{
    BASE, DISPATCH, EVENT_CALLS, EVENT_FN, Entries, ISR_VECTOR, LAYER_RAM, LOOP_CALL, LOOP_STEP,
    STACK_TOP_NEW, STACK_TOP_OLD, STOCK_ISR, VARS, VARS_SIZE,
};
use crate::v72::Firmware;

/// Slot type of a tap-hold key.
pub const SLOT_TYPE: u8 = 0x08;
/// `hold` values from this one up turn on layer `hold & 3`.
pub const HOLD_LAYER: u8 = 0xF0;
/// Term in 10 ms steps when the slot says 0.
pub const DEFAULT_TERM: u8 = 20;
/// Layer pages in flash: layer n at `PAGES + 0x240 * n`.
pub const PAGES: u32 = 0xF000;
/// The code must end below the info block command (`info.rs`).
const REGION_END: u32 = 0xE500;
const SP_LITERAL: u32 = 0xBEA8;
const FN_FLAGS: u32 = 0x2000_0078;
const FN_MASK: u32 = 0x0410_2000;
const TAP_MS: u32 = 20;

// State block offsets (48 bytes at VARS, zero after reset).
const NOW: u32 = 0;
const C_ST: u32 = 4;
const C_KEY: u32 = 5;
const C_LAY: u32 = 6;
const S_ST: u32 = 7;
const C_T0: u32 = 8;
const S_T0: u32 = 10;
const S_KEY: u32 = 12;
const SBASE: u32 = 13;
const HLAYER: u32 = 14;
const RESTORE: u32 = 15;
const RIDX: u32 = 16;
const NBUF: u32 = 17;
const BUF: u32 = 18;
/// Held-back events while a layer hold is undecided.
const BUF_N: u32 = 14;
const BITMAP: u32 = 32;
const _: () = assert!(BUF + BUF_N <= BITMAP && BITMAP + 16 == VARS_SIZE);

/// Key states: C_ST 0 idle, 1 undecided, 2 held, 3 tap sent (released 20 ms later). S_ST the same, plus 5:
/// released while undecided, a tap the next loop sends before it replays the held-back keys.
///
/// Routing of releases into the layer of their press (pressed while a layer hold was on): as in `taphold.rs`,
/// with this block's offsets. `p` prefixes the labels; uses r0-r3 and r7, keeps r4 (key), r5 (event), r6.
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

/// Milliseconds since the 16-bit start time at `t0` into r0 (uses r1).
fn elapsed(t0: u32) -> String {
    format!(
        "
    ldr r0, [r6, #{NOW}]
    ldrh r1, [r6, #{t0}]
    subs r0, r0, r1
    lsls r0, r0, #16
    lsrs r0, r0, #16
"
    )
}

/// The assembly source.
#[allow(clippy::too_many_lines)]
pub fn source() -> String {
    let route = route("rt_");
    let el_c = elapsed(C_T0);
    let el_s = elapsed(S_T0);
    let lit_isr = STOCK_ISR | 1;
    let lit_event = EVENT_FN | 1;
    let lit_step = LOOP_STEP | 1;
    let bitmap4 = BITMAP + 4;
    let bitmap8 = BITMAP + 8;
    let bitmap12 = BITMAP + 12;
    format!(
        "
on_event:
    push {{r4, r5, r6, r7, lr}}
    mov r4, r0
    mov r5, r1
    ldr r6, lit_vars
    bl press_decides
    ldrb r0, [r6, #{S_ST}]
    cmp r0, #1
    beq ev_wait
    cmp r0, #5
    bne ev_handle
ev_wait:
    ldrb r1, [r6, #{S_KEY}]
    cmp r1, r4
    bne ev_hold_back
    cmp r0, #1
    beq ev_handle
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
    bne ev_handle
    movs r0, #5
    strb r0, [r6, #{S_ST}]
ev_handle:
    bl handle
    cmp r0, #0
    bne ev_pass
    pop {{r4, r5, r6, r7, pc}}
ev_pass:
    mov r0, r4
    mov r1, r5
    pop {{r4, r5, r6, r7}}
    pop {{r2}}
    mov lr, r2
    ldr r2, lit_event
    bx r2

@ Another key going down while a key hold is undecided makes it a hold (r4 key, r5 event).
press_decides:
    cmp r5, #0
    bne pd_done
    ldrb r0, [r6, #{C_ST}]
    cmp r0, #1
    bne pd_done
    ldrb r0, [r6, #{C_KEY}]
    cmp r0, r4
    beq pd_done
    b c_to_hold
pd_done:
    bx lr

@ One key event (r4 key, r5 event, r6 state). r0 = 1: pass it to 0x6984 (already routed), 0: done here.
handle:
    push {{r7, lr}}
    cmp r5, #0
    beq h_down
    ldrb r0, [r6, #{C_KEY}]
    cmp r0, r4
    bne h_up_s
    ldrb r0, [r6, #{C_ST}]
    cmp r0, #1
    beq h_c_tap
    cmp r0, #2
    beq h_c_release
h_up_s:
    ldrb r0, [r6, #{S_KEY}]
    cmp r0, r4
    bne h_route
    ldrb r0, [r6, #{S_ST}]
    cmp r0, #1
    beq h_s_undecided
    cmp r0, #2
    beq h_s_release
    b h_route
h_c_tap:
    movs r0, #3
    strb r0, [r6, #{C_ST}]
    ldr r0, [r6, #{NOW}]
    strh r0, [r6, #{C_T0}]
    bl c_slot
    ldrb r0, [r0, #1]
    movs r1, #0
    bl {DISPATCH:#x}
    b h_done
h_c_release:
    movs r0, #0
    strb r0, [r6, #{C_ST}]
    bl c_slot
    ldrb r0, [r0, #2]
    movs r1, #2
    bl {DISPATCH:#x}
    b h_done
h_s_undecided:
    movs r0, #5
    strb r0, [r6, #{S_ST}]
    b h_done
h_s_release:
    movs r0, #0
    strb r0, [r6, #{S_ST}]
    strb r0, [r6, #{RESTORE}]
    ldrb r0, [r6, #{SBASE}]
    ldr r1, lit_layer
    strb r0, [r1]
    b h_done

h_down:
    ldr r0, lit_fnflags
    ldr r0, [r0]
    ldr r1, lit_fnmask
    tst r0, r1
    bne h_route
    ldrb r1, [r6, #{RESTORE}]
    cmp r1, #0
    beq h_layer_now
    subs r1, #1
    b h_layer_have
h_layer_now:
    ldr r1, lit_layer
    ldrb r1, [r1]
h_layer_have:
    mov r7, r1
    mov r0, r4
    bl slot_addr
    ldrb r1, [r0]
    cmp r1, #{SLOT_TYPE}
    bne h_route
    ldrb r1, [r0, #2]
    cmp r1, #{HOLD_LAYER}
    bhs h_layer_key
    ldrb r2, [r6, #{C_ST}]
    cmp r2, #0
    beq h_c_start
    cmp r2, #3
    bne h_route
    bl c_tap_up
h_c_start:
    strb r4, [r6, #{C_KEY}]
    strb r7, [r6, #{C_LAY}]
    movs r0, #1
    strb r0, [r6, #{C_ST}]
    ldr r0, [r6, #{NOW}]
    strh r0, [r6, #{C_T0}]
    b h_done

h_layer_key:
    ldrb r2, [r6, #{S_ST}]
    cmp r2, #0
    beq h_s_check
    cmp r2, #3
    bne h_route
h_s_check:
    movs r3, #3
    ands r1, r3
    ldrb r3, [r6, #{HLAYER}]
    cmp r3, r1
    beq h_s_start
    ldr r0, [r6, #{BITMAP}]
    ldr r1, [r6, #{bitmap4}]
    orrs r0, r1
    ldr r1, [r6, #{bitmap8}]
    orrs r0, r1
    ldr r1, [r6, #{bitmap12}]
    orrs r0, r1
    bne h_route
h_s_start:
    ldrb r2, [r6, #{S_ST}]
    cmp r2, #3
    bne h_s_go
    bl s_tap_up
h_s_go:
    strb r4, [r6, #{S_KEY}]
    strb r7, [r6, #{SBASE}]
    movs r0, #1
    strb r0, [r6, #{S_ST}]
    @ Keys still held back behind an earlier hold (this key came out of its replay) now wait for this one:
    @ move BUF[RIDX..NBUF] to the front. Outside a replay RIDX and NBUF are 0.
    ldrb r0, [r6, #{RIDX}]
    ldrb r1, [r6, #{NBUF}]
    subs r1, r1, r0
    strb r1, [r6, #{NBUF}]
    mov r2, r6
    adds r2, #{BUF}
    adds r0, r2, r0
h_s_move:
    cmp r1, #0
    beq h_s_moved
    ldrb r3, [r0]
    strb r3, [r2]
    adds r0, #1
    adds r2, #1
    subs r1, #1
    b h_s_move
h_s_moved:
    movs r0, #0
    strb r0, [r6, #{RIDX}]
    ldr r0, [r6, #{NOW}]
    strh r0, [r6, #{S_T0}]
    b h_done

h_route:
{route}
    movs r0, #1
    pop {{r7, pc}}
h_done:
    movs r0, #0
    pop {{r7, pc}}

@ r0 = flash address of the slot of key r0 in layer r1 (uses r0-r2).
slot_addr:
    movs r2, #9
    muls r1, r2
    lsls r1, r1, #6
    movs r2, #7
    ands r2, r0
    lsls r2, r2, #6
    adds r1, r1, r2
    lsrs r0, r0, #3
    lsls r0, r0, #2
    adds r0, r0, r1
    ldr r1, lit_pages
    adds r0, r0, r1
    bx lr

c_slot:
    ldrb r0, [r6, #{C_KEY}]
    ldrb r1, [r6, #{C_LAY}]
    b slot_addr

s_slot:
    ldrb r0, [r6, #{S_KEY}]
    ldrb r1, [r6, #{SBASE}]
    b slot_addr

@ r0 = term in ms of the slot at r0 (uses r1).
term_ms:
    ldrb r0, [r0, #3]
    cmp r0, #0
    bne tm_scale
    movs r0, #{DEFAULT_TERM}
tm_scale:
    lsls r1, r0, #3
    lsls r0, r0, #1
    adds r0, r0, r1
    bx lr

c_to_hold:
    push {{lr}}
    movs r0, #2
    strb r0, [r6, #{C_ST}]
    bl c_slot
    ldrb r0, [r0, #2]
    movs r1, #0
    bl {DISPATCH:#x}
    pop {{pc}}

c_tap_up:
    push {{lr}}
    movs r0, #0
    strb r0, [r6, #{C_ST}]
    bl c_slot
    ldrb r0, [r0, #1]
    movs r1, #2
    bl {DISPATCH:#x}
    pop {{pc}}

s_tap_up:
    push {{lr}}
    movs r0, #0
    strb r0, [r6, #{S_ST}]
    bl s_slot
    ldrb r0, [r0, #1]
    movs r1, #2
    bl {DISPATCH:#x}
    pop {{pc}}

on_loop:
    push {{r4, r5, r6, r7, lr}}
    ldr r6, lit_vars
    bl lp_restore
    ldrb r0, [r6, #{C_ST}]
    cmp r0, #1
    beq lp_c_pending
    cmp r0, #3
    bne lp_s
{el_c}
    cmp r0, #{TAP_MS}
    blo lp_s
    bl c_tap_up
    b lp_s
lp_c_pending:
    bl c_slot
    bl term_ms
    mov r7, r0
{el_c}
    cmp r0, r7
    blo lp_s
    bl c_to_hold
lp_s:
    ldrb r0, [r6, #{S_ST}]
    cmp r0, #1
    beq lp_s_pending
    cmp r0, #5
    beq lp_s_tap
    cmp r0, #3
    bne lp_done
{el_s}
    cmp r0, #{TAP_MS}
    blo lp_done
    bl s_tap_up
    b lp_done
lp_s_pending:
    bl s_slot
    mov r7, r0
    bl term_ms
    mov r5, r0
{el_s}
    cmp r0, r5
    blo lp_done
    ldrb r1, [r7, #2]
    movs r2, #3
    ands r1, r2
    strb r1, [r6, #{HLAYER}]
    ldr r2, lit_layer
    strb r1, [r2]
    movs r0, #2
    strb r0, [r6, #{S_ST}]
    b lp_replay
lp_s_tap:
    movs r0, #3
    strb r0, [r6, #{S_ST}]
    ldr r0, [r6, #{NOW}]
    strh r0, [r6, #{S_T0}]
    bl s_slot
    ldrb r0, [r0, #1]
    movs r1, #0
    bl {DISPATCH:#x}
lp_replay:
    movs r0, #0
    strb r0, [r6, #{RIDX}]
lp_r_next:
    ldrb r0, [r6, #{S_ST}]
    cmp r0, #1
    beq lp_r_wait
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
    bl press_decides
    bl handle
    cmp r0, #0
    beq lp_r_next
    mov r0, r4
    mov r1, r5
    bl {EVENT_FN:#x}
    b lp_r_next
lp_r_done:
    movs r0, #0
    strb r0, [r6, #{NBUF}]
    strb r0, [r6, #{RIDX}]
lp_r_wait:
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
lit_fnflags: .word {FN_FLAGS:#x}
lit_fnmask:  .word {FN_MASK:#x}
lit_layer:   .word {LAYER_RAM:#x}
lit_pages:   .word {PAGES:#x}
lit_isr:     .word {lit_isr:#x}
lit_event:   .word {lit_event:#x}
lit_step:    .word {lit_step:#x}
"
    )
}

/// Assembles the source at [`BASE`].
pub fn assemble(src: &str) -> Result<(Vec<u8>, Entries), Error> {
    let p = thumb::assemble(src, BASE).map_err(|e| Error::Asm(e.to_string()))?;
    let s = |n: &str| p.symbol(n).map_err(|e| Error::Asm(e.to_string()));
    crate::check_m0(&p.code[..(s("lit_vars")? - BASE) as usize], BASE)?;
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

/// Patches `fw` in place and logs every byte.
pub fn apply(fw: &mut Firmware) -> Result<Entries, Error> {
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
    let (code, entry) = assemble(&source())?;
    if u(BASE) + code.len() > u(REGION_END) || fw.b[u(BASE)..u(REGION_END)].iter().any(|&x| x != 0) {
        return Err(Error::Structure("code region 0xd200-0xe4ff is not free".into()));
    }
    fw.put_bytes(
        u(BASE),
        &code,
        &format!("layer tap-hold code ({} bytes)", code.len()),
    );
    for a in EVENT_CALLS {
        let why = format!(
            "0x7c72: key events go through layer tap-hold ({:#x})",
            entry.on_event
        );
        fw.put_bytes(u(a), &thumb::bl(a, entry.on_event), &why);
    }
    let why = format!(
        "main loop step goes through layer tap-hold ({:#x})",
        entry.on_loop
    );
    fw.put_bytes(u(LOOP_CALL), &thumb::bl(LOOP_CALL, entry.on_loop), &why);
    let why = format!(
        "vector 31: 1 ms timer counts for layer tap-hold first ({:#x})",
        entry.on_tick
    );
    fw.put_bytes(u(ISR_VECTOR), &(entry.on_tick | 1).to_le_bytes(), &why);
    for (addr, what) in [
        (0, "vector 0: initial SP"),
        (SP_LITERAL, "__user_initial_stackheap: stack top"),
    ] {
        let why = format!(
            "{what} {STACK_TOP_OLD:#x} -> {STACK_TOP_NEW:#x} (layer tap-hold state, {VARS_SIZE} bytes)"
        );
        fw.put_bytes(u(addr), &STACK_TOP_NEW.to_le_bytes(), &why);
    }
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembles_as_cortex_m0_code_inside_its_region() {
        let (code, e) = assemble(&source()).unwrap();
        assert!(u(BASE) + code.len() <= u(REGION_END), "{} bytes", code.len());
        assert!(e.on_event >= BASE && e.on_loop > e.on_event && e.on_tick > e.on_loop);
    }
}
