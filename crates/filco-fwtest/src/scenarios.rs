//! The test scenarios of `build.py`, line for line: stock and new firmware side by side for every key plain and
//! with Fn, the Fn key, layer switching, the Fn layer through the M10 user slot, timed tap-hold scenarios, the
//! info command over HID and the static stack depth. Each scenario writes the report lines `build.py` writes and
//! returns whether everything passed.

use filco_firmware::Build;
use filco_firmware::v72::{BITMAPS, FN_CODE, code_name};
use std::cell::Cell;

use m0emu::Stop;

use crate::hid::HidHost;
use crate::machine::{DOWN, UP, V72, first_down};
use crate::stack;
use Act::{D, U};

const STOCK_FN: u8 = 73;
/// Keys 1-4; Fn + Alt + key selects layer 0-3.
const LAYER_KEYS: [u8; 4] = [8, 16, 24, 32];
const SLOT_FN: [u8; 4] = [0x02, 0x00, FN_CODE, 0x00];
/// Set by Fn + Alt + number to save the layer to flash (0x6822).
const LAYER_SAVE_FLAG: (u32, u32) = (0x2000_00DC, 0x10);
const MOUSE_BUTTONS: u32 = 0x2000_008F;
const MODIFIERS: u32 = 0x2000_02B8;
const LEFT: u8 = 0x50;
const LCTRL: u8 = 0xE0;

/// Report lines, each also handed to an optional callback as it is written (progress for a UI).
#[derive(Default)]
pub struct Out<'a> {
    pub lines: Vec<String>,
    on_line: Option<&'a mut dyn FnMut(&str)>,
}

impl std::fmt::Debug for Out<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Out")
            .field("lines", &self.lines)
            .field("streaming", &self.on_line.is_some())
            .finish()
    }
}

impl<'a> Out<'a> {
    /// Collects the lines and calls `on_line` with each one as it is written.
    pub fn streaming(on_line: &'a mut dyn FnMut(&str)) -> Self {
        Out {
            lines: Vec::new(),
            on_line: Some(on_line),
        }
    }

    fn push(&mut self, s: impl Into<String>) {
        let s = s.into();
        if let Some(f) = self.on_line.as_mut() {
            f(&s);
        }
        self.lines.push(s);
    }
}

/// What the tests need to know about the build.
struct Ctx<'a> {
    b: &'a Build,
}

fn name_of(c: Option<u8>) -> String {
    code_name(c)
}

impl Ctx<'_> {
    fn n(&self, k: u8) -> &str {
        self.b.names.get(&k).map_or("?", String::as_str)
    }

    fn key(&self, text: &str) -> u8 {
        if let Some(i) = text.strip_prefix('#') {
            return i.parse().unwrap_or(0);
        }
        let lower = text.to_ascii_lowercase();
        self.b
            .names
            .iter()
            .find(|(_, n)| n.to_ascii_lowercase() == lower)
            .map_or(0, |(k, _)| *k)
    }

    /// Whether key `k` has a Fn function in the new image.
    fn capable(&self, k: u8) -> Option<bool> {
        let bit = |bm: usize| (self.b.image[bm + usize::from(k >> 3)] >> (k & 7)) & 1 == 1;
        let (a, c) = (bit(BITMAPS[0]), bit(BITMAPS[1]));
        (a == c).then_some(a)
    }

    /// The code key `k` sends in the new image, pressed alone with the layer slots empty.
    fn plain(&self, k: u8) -> Result<Option<u8>, Stop> {
        Ok(first_down(&V72::new(&self.b.image, &[])?.chord(&[], k)?))
    }

    /// The key a scenario presses next to a tap-hold key: the first of `prefer` that is no tap-hold key, not the
    /// Fn key, not in `taken`, and sends an ordinary key code on its own, so a config that makes the usual key a
    /// tap-hold key, a modifier or Fn does not break the scenario. The first of `prefer` if none qualifies.
    fn helper(&self, prefer: &[&str], taken: &[u8]) -> Result<u8, Stop> {
        let busy: Vec<u8> = self.tap_hold_specs().iter().map(|s| s.0).collect();
        for name in prefer {
            let k = self.key(name);
            if k == self.b.fn_position || busy.contains(&k) || taken.contains(&k) {
                continue;
            }
            if self.plain(k)?.is_some_and(|c| (0x04..=0x65).contains(&c)) {
                return Ok(k);
            }
        }
        Ok(self.key(prefer[0]))
    }

    fn tap_hold_specs(&self) -> Vec<(u8, u8)> {
        let c = &self.b.config;
        c.tap_key
            .iter()
            .map(|s| (s.0, s.1))
            .chain(c.tap_layers.iter().map(|s| (s.0, s.1)))
            .collect()
    }
}

fn py_calls(calls: &[(u8, u8)]) -> String {
    let parts: Vec<String> = calls.iter().map(|(c, e)| format!("({c}, {e})")).collect();
    format!("[{}]", parts.join(", "))
}

fn fmt_events(got: &[(u8, u8)]) -> String {
    let parts: Vec<String> = got
        .iter()
        .map(|&(c, ev)| format!("{}{}", name_of(Some(c)), if ev == DOWN { "v" } else { "^" }))
        .collect();
    parts.join(" ")
}

fn ok(good: bool) -> &'static str {
    if good { "OK" } else { "FAIL" }
}

/// Every scenario passes only if each of its checks passes; `v` is evaluated before it is recorded.
fn and(all: &Cell<bool>, v: bool) {
    all.set(all.get() && v);
}

#[derive(Clone, Copy)]
enum Act {
    D,
    U,
}

/// Runs timed key actions on `e` and compares the dispatcher calls (Fn left out) with `want`.
fn timed(
    e: &mut V72,
    out: &mut Out,
    label: &str,
    actions: &[(Act, u8, u32)],
    want: &[(u8, u8)],
    extra: Option<(bool, &str)>,
) -> Result<bool, Stop> {
    e.env.calls.clear();
    for &(act, key, ms) in actions {
        e.press(key, matches!(act, D));
        e.idle(ms)?;
    }
    let got: Vec<(u8, u8)> = e
        .env
        .calls
        .iter()
        .copied()
        .filter(|&(c, _)| c != FN_CODE)
        .collect();
    let good = got == want && extra.is_none_or(|x| x.0);
    let note = extra.map_or(String::new(), |x| format!(" {}", x.1));
    out.push(format!("  {label:44} {:34} {}{note}", fmt_events(&got), ok(good)));
    Ok(good)
}

/// Runs every scenario on a build. Returns whether all passed.
#[allow(clippy::too_many_lines)]
pub fn run_all(b: &Build, out: &mut Out) -> Result<bool, Stop> {
    let cx = Ctx { b };
    let cfg = &b.config;
    let new_fn = b.fn_position;
    let all = Cell::new(true);
    let check = |out: &mut Out, label: &str, got: Option<u8>, want: Option<u8>| {
        let good = got == want;
        and(&all, good);
        if !good || label.starts_with('*') {
            out.push(format!(
                "  {:34} got {:10} expected {:10} {}",
                label.trim_start_matches('*'),
                name_of(got),
                name_of(want),
                ok(good)
            ));
        }
        good
    };

    let mut stock = V72::new(&b.stock, &[])?;
    let mut new = V72::new(&b.image, &[])?;
    out.push(format!(
        "Every key, plain and with Fn (stock Fn = {}, new Fn = {}):",
        cx.n(STOCK_FN),
        cx.n(new_fn)
    ));
    let specs = cx.tap_hold_specs();
    let mut tested = 0;
    for &k in b.names.keys() {
        if k == STOCK_FN || k == new_fn {
            continue;
        }
        let sp = first_down(&stock.chord(&[], k)?);
        let np = first_down(&new.chord(&[], k)?);
        // Fn + Left Shift and Fn + Alt arm a mode that outlasts the chord: the next key with a Fn function, even
        // pressed without Fn, ends it, and 1-4 then clear that layer's macros or select the layer (manual p. 10).
        // Which key comes next with a Fn function depends on the config, so these chords run on fresh machines
        // and the armed mode never reaches the next key of this walk.
        let arms = |c: Option<u8>| matches!(c, Some(0xE1 | 0xE2 | 0xE6));
        let (sf, nf) = if arms(sp) || arms(np) {
            (
                first_down(&V72::new(&b.stock, &[])?.chord(&[STOCK_FN], k)?),
                first_down(&V72::new(&b.image, &[])?.chord(&[new_fn], k)?),
            )
        } else {
            (
                first_down(&stock.chord(&[STOCK_FN], k)?),
                first_down(&new.chord(&[new_fn], k)?),
            )
        };
        let in_keymap = cfg.keymap.iter().find(|e| e.0 == k).map(|e| e.1);
        let in_fn = cfg.fn_layer.iter().find(|e| e.0 == k).map(|e| e.1);
        let plain = in_keymap.or(sp);
        let th = specs.iter().find(|s| s.0 == k);
        // A tap-hold key sends its tap in the chord (holds are tested below); with Fn held it passes through.
        let want_p = th.map_or(plain, |t| Some(t.1));
        let want_f = match in_fn {
            Some(Some(c)) => Some(c),
            Some(None) => plain,
            None if in_keymap.is_some() && cx.capable(k) != Some(true) => plain,
            None => sf,
        };
        let mark = if in_fn.is_some() || in_keymap.is_some() || th.is_some() {
            "*"
        } else {
            ""
        };
        check(out, &format!("{mark}{}", cx.n(k)), np, want_p);
        check(out, &format!("{mark}Fn + {}", cx.n(k)), nf, want_f);
        tested += 1;
    }
    out.push(format!(
        "  {tested} keys compared; lines above are the configured keys and any failure"
    ));

    out.push("Fn keys:");
    let calls = new.chord(&[], new_fn)?;
    let good = calls.contains(&(FN_CODE, DOWN)) && first_down(&calls).is_none();
    and(&all, good);
    out.push(format!(
        "  {} alone sends Fn and nothing else: {} {}",
        cx.n(new_fn),
        ok(good),
        py_calls(&calls)
    ));
    if new_fn != STOCK_FN {
        let want = cfg.keymap.iter().find(|e| e.0 == STOCK_FN).map(|e| e.1);
        let got = first_down(&new.chord(&[], STOCK_FN)?);
        check(out, &format!("*{} (old Fn position)", cx.n(STOCK_FN)), got, want);
        let got = first_down(&new.chord(&[new_fn], STOCK_FN)?);
        check(out, &format!("*Fn + {}", cx.n(STOCK_FN)), got, want);
    }

    out.push("Layer switching (Fn + Alt + 1..4), new firmware:");
    let mut alts = vec![cx.key("LAlt"), cx.key("#78")];
    alts.extend(
        cfg.keymap
            .iter()
            .filter(|e| e.1 == 0xE2 || e.1 == 0xE6)
            .map(|e| e.0),
    );
    let mut seen = Vec::new();
    alts.retain(|a| {
        let fresh = !seen.contains(a);
        seen.push(*a);
        fresh
    });
    for &alt in &alts {
        for layer in [1u8, 2, 3, 0] {
            new.chord(&[new_fn, alt], LAYER_KEYS[usize::from(layer)])?;
            let got = new.layer();
            and(&all, got == layer);
            out.push(format!(
                "  {} + {} + {}: layer {} {}",
                cx.n(new_fn),
                cx.n(alt),
                layer + 1,
                got + 1,
                ok(got == layer)
            ));
        }
    }
    out.push(
        "  (Fn + 1..4 without Alt is covered by the key-by-key comparison above, run before any Alt chord)",
    );

    out.push("Fn layer with the M10 user slot = Fn on all layers (state of the keyboard since 2026-10-31):");
    let m10 = cx.key("M10");
    let slots: Vec<((u8, u8), [u8; 4])> = (0..4).map(|l| ((l, m10), SLOT_FN)).collect();
    let mut dev = V72::new(&b.image, &slots)?;
    for &(k, c) in &cfg.fn_layer {
        let plain = match cfg.keymap.iter().find(|e| e.0 == k) {
            Some(e) => Some(e.1),
            None => first_down(&stock.chord(&[], k)?),
        };
        let got = first_down(&dev.chord(&[m10], k)?);
        check(out, &format!("*M10 + {}", cx.n(k)), got, c.or(plain));
    }
    if let Some(spec) = cfg.tap_key {
        and(&all, test_tap_hold(&cx, spec, out)?);
    }
    for &spec in &cfg.tap_layers {
        and(&all, test_layer_hold(&cx, spec, cfg.tap_key, out)?);
    }
    if cfg.tap_layers.len() == 2 {
        and(&all, test_two_layer_holds(&cx, out)?);
    }
    if cfg.layer_tap_hold {
        and(&all, test_layer_tap(&cx, out)?);
    }
    if b.info.is_some() {
        and(&all, test_info(&cx, out)?);
    }
    and(&all, test_stack(&b.stock, &b.image, out));
    Ok(all.get())
}

// Short names (d, u, n, h, e) follow build.py, whose scenarios these are line for line.
#[allow(clippy::too_many_lines, clippy::many_single_char_names)]
fn test_tap_hold(cx: &Ctx<'_>, (k, tap, hold, term): (u8, u8, u8, u16), out: &mut Out) -> Result<bool, Stop> {
    let term = u32::from(term);
    let other = cx.helper(&["C", "V", "X", "Z", "N", "M"], &[])?;
    let fnk = cx.b.fn_position;
    let oc = cx.plain(other)?.unwrap_or(0);
    let mut e = V72::new(&cx.b.image, &[])?;
    let all = Cell::new(true);
    let n = |k: u8| cx.n(k).to_string();
    let (d, u) = (DOWN, UP);
    out.push(format!(
        "Tap-hold {}: tap = {}, hold = {}, term {term} ms (v = down, ^ = up; Fn not listed):",
        n(k),
        name_of(Some(tap)),
        name_of(Some(hold))
    ));
    let run = |e: &mut V72, out: &mut Out, label: String, a: &[(Act, u8, u32)], w: &[(u8, u8)]| {
        let r = timed(e, out, &label, a, w, None);
        if let Ok(g) = r {
            and(&all, g);
        }
        r.map(|_| ())
    };
    run(
        &mut e,
        out,
        format!("{} tapped 40 ms", n(k)),
        &[(D, k, 40), (U, k, 60)],
        &[(tap, d), (tap, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} held {} ms", n(k), term - 20),
        &[(D, k, term - 20), (U, k, 60)],
        &[(tap, d), (tap, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} held {} ms alone", n(k), term + 30),
        &[(D, k, term + 30), (U, k, 60)],
        &[(hold, d), (hold, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} + {} within 40 ms", n(k), n(other)),
        &[(D, k, 40), (D, other, 40), (U, other, 40), (U, k, 60)],
        &[(hold, d), (oc, d), (oc, u), (hold, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} held {} ms, then {}", n(k), term + 30, n(other)),
        &[(D, k, term + 30), (D, other, 40), (U, other, 40), (U, k, 60)],
        &[(hold, d), (oc, d), (oc, u), (hold, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} down, {} tapped, {} up", n(other), n(k), n(other)),
        &[(D, other, 40), (D, k, 40), (U, k, 40), (U, other, 60)],
        &[(oc, d), (tap, d), (tap, u), (oc, u)],
    )?;
    run(
        &mut e,
        out,
        "two taps 40 ms apart".into(),
        &[(D, k, 30), (U, k, 40), (D, k, 30), (U, k, 60)],
        &[(tap, d), (tap, u), (tap, d), (tap, u)],
    )?;
    // The key's code in the built image, not in stock: when the config removes the key's Fn function, the stock
    // byte is a Fn entry reference and the image holds the plain code the key now sends with Fn held.
    let plain_code = cx.b.image[filco_firmware::v72::NAME_KEYMAP + 2 * usize::from(k)];
    run(
        &mut e,
        out,
        format!("{} + {} (passes through)", n(fnk), n(k)),
        &[(D, fnk, 40), (D, k, 40), (U, k, 40), (U, fnk, 60)],
        &[(plain_code, d), (plain_code, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} + {}, {} released first", n(fnk), n(k), n(fnk)),
        &[(D, fnk, 40), (D, k, 40), (U, fnk, 40), (U, k, 60)],
        &[(plain_code, d), (plain_code, u)],
    )?;
    let j = cx.helper(&["J", "K", "L", "I", "U", "M"], &[])?;
    let want_fj = first_down(&V72::new(&cx.b.image, &[])?.chord(&[fnk], j)?).unwrap_or(0);
    run(
        &mut e,
        out,
        format!("{} + {} + {}", n(k), n(fnk), n(j)),
        &[
            (D, k, 40),
            (D, fnk, 40),
            (D, j, 40),
            (U, j, 40),
            (U, fnk, 40),
            (U, k, 60),
        ],
        &[(hold, d), (want_fj, d), (want_fj, u), (hold, u)],
    )?;
    Ok(all.get())
}

/// Layer, save flag and mouse buttons back to rest after a scenario; prints a FAIL line otherwise.
fn clean_after(e: &V72, out: &mut Out) -> bool {
    let lay = e.layer();
    let flag = e.ram_u32(LAYER_SAVE_FLAG.0) & LAYER_SAVE_FLAG.1;
    let mouse = e.ram_u8(MOUSE_BUTTONS);
    let clean = lay == 0 && flag == 0 && mouse == 0;
    if !clean {
        out.push(format!(
            "    after: layer {}, save flag {}, mouse buttons {mouse}: FAIL",
            lay + 1,
            if flag != 0 { "True" } else { "False" }
        ));
    }
    clean
}

// Short names (d, u, n, h, e) follow build.py, whose scenarios these are line for line.
#[allow(clippy::too_many_lines, clippy::many_single_char_names)]
fn test_layer_hold(
    cx: &Ctx<'_>,
    (k, tap, layer, term): (u8, u8, u8, u16),
    key_hold: Option<(u8, u8, u8, u16)>,
    out: &mut Out,
) -> Result<bool, Stop> {
    let term = u32::from(term);
    let h = cx.helper(&["H", "J", "K", "L", "Y", "U"], &[])?;
    let bk = cx.helper(&["B", "N", "M", "V"], &[h])?;
    let dk = cx.helper(&["D", "F", "G", "S"], &[h, bk])?;
    let rk = cx.helper(&["R", "T", "E", "W"], &[h, bk, dk])?;
    let fnk = cx.b.fn_position;
    // With Fn held the key passes through: its code in the built image (see test_tap_hold).
    let plain_code = cx.b.image[filco_firmware::v72::NAME_KEYMAP + 2 * usize::from(k)];
    let slots = [
        ((layer, h), [2, 0, LEFT, 0]),
        ((layer, bk), [2, 1, LEFT, 0]),
        ((layer, dk), [2, 0, 0x2A, 0]),
        ((layer, rk), [1, 1, 1, 0]),
    ];
    let mut e = V72::new(&cx.b.image, &slots)?;
    let all = Cell::new(true);
    let (d, u) = (DOWN, UP);
    let (c_h, c_d) = (cx.plain(h)?.unwrap_or(0), cx.plain(dk)?.unwrap_or(0));
    let n = |k: u8| cx.n(k).to_string();
    out.push(format!(
        "Tap-hold {}: tap = {}, hold = layer {}, term {term} ms (test slots in layer {}: {} = Left, {} = Ctrl+Left, \
         {} = Backspace, {} = mouse left):",
        n(k),
        name_of(Some(tap)),
        layer + 1,
        layer + 1,
        n(h),
        n(bk),
        n(dk),
        n(rk)
    ));
    let run = |e: &mut V72, out: &mut Out, label: String, a: &[(Act, u8, u32)], w: &[(u8, u8)]| {
        let good = timed(e, out, &label, a, w, None)?;
        let clean = clean_after(e, out);
        and(&all, good && clean);
        Ok::<(), Stop>(())
    };
    run(
        &mut e,
        out,
        format!("{} tapped 40 ms", n(k)),
        &[(D, k, 40), (U, k, 60)],
        &[(tap, d), (tap, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} held {} ms", n(k), term - 30),
        &[(D, k, term - 30), (U, k, 60)],
        &[(tap, d), (tap, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} held {} ms alone", n(k), term + 30),
        &[(D, k, term + 30), (U, k, 60)],
        &[],
    )?;
    run(
        &mut e,
        out,
        format!("{} + {} tapped, all within {term} ms", n(k), n(h)),
        &[(D, k, 40), (D, h, 40), (U, h, 40), (U, k, 60)],
        &[(tap, d), (c_h, d), (c_h, u), (tap, u)],
    )?;
    run(
        &mut e,
        out,
        format!("fast roll: {}, {}, {} up, {} up", n(k), n(dk), n(k), n(dk)),
        &[(D, k, 30), (D, dk, 30), (U, k, 60), (U, dk, 60)],
        &[(tap, d), (c_d, d), (tap, u), (c_d, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} held, then {}", n(k), n(h)),
        &[(D, k, term + 30), (D, h, 40), (U, h, 40), (U, k, 60)],
        &[(LEFT, d), (LEFT, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} down during the wait, held past {term} ms", n(h)),
        &[(D, k, 50), (D, h, term), (U, h, 40), (U, k, 60)],
        &[(LEFT, d), (LEFT, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} held, {} down, {} up first, {} up", n(k), n(h), n(k), n(h)),
        &[(D, k, term + 30), (D, h, 40), (U, k, 40), (U, h, 60)],
        &[(LEFT, d), (LEFT, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} down, {} held, {} up, {} up", n(dk), n(k), n(dk), n(k)),
        &[(D, dk, 40), (D, k, term + 30), (U, dk, 40), (U, k, 60)],
        &[(c_d, d), (c_d, u)],
    )?;
    e.env.calls.clear();
    e.press(k, true);
    e.idle(term + 30)?;
    e.press(bk, true);
    e.idle(40)?;
    let modifiers = e.ram_u8(MODIFIERS);
    e.press(bk, false);
    e.idle(40)?;
    e.press(k, false);
    e.idle(60)?;
    let got: Vec<(u8, u8)> = e
        .env
        .calls
        .iter()
        .copied()
        .filter(|&(c, _)| c != FN_CODE)
        .collect();
    let good = got == [(LCTRL, d), (LEFT, d), (LCTRL, u), (LEFT, u)] && modifiers & 1 == 1;
    and(&all, good);
    out.push(format!(
        "  {:44} {:34} {}",
        format!("{} held, {} (slot Ctrl+Left)", n(k), n(bk)),
        if modifiers & 1 == 1 {
            "LCtrl+Left while held"
        } else {
            "no Ctrl"
        },
        ok(good)
    ));
    e.env.calls.clear();
    e.press(k, true);
    e.idle(term + 30)?;
    e.press(rk, true);
    e.idle(40)?;
    let btn = e.ram_u8(MOUSE_BUTTONS);
    e.press(k, false);
    e.idle(40)?;
    e.press(rk, false);
    e.idle(60)?;
    let good = btn == 1 && e.ram_u8(MOUSE_BUTTONS) == 0 && e.layer() == 0;
    and(&all, good);
    out.push(format!(
        "  {:44} {:34} {}",
        format!("{} held, {} (mouse left), {} up first", n(k), n(rk), n(k)),
        if good { "button down, then up" } else { "stuck" },
        ok(good)
    ));
    run(
        &mut e,
        out,
        format!("{} + {} (passes through)", n(fnk), n(k)),
        &[(D, fnk, 40), (D, k, 40), (U, k, 40), (U, fnk, 60)],
        &[(plain_code, d), (plain_code, u)],
    )?;
    if let Some((ck, _, chold, _)) = key_hold {
        run(
            &mut e,
            out,
            format!("{} held + {} tapped", n(ck), n(k)),
            &[(D, ck, 40), (D, k, 40), (U, k, 40), (U, ck, 60)],
            &[(chold, d), (tap, d), (tap, u), (chold, u)],
        )?;
    }
    Ok(all.get())
}

// Short names (d, u, n, h, e) follow build.py, whose scenarios these are line for line.
#[allow(clippy::too_many_lines, clippy::many_single_char_names)]
fn test_two_layer_holds(cx: &Ctx<'_>, out: &mut Out) -> Result<bool, Stop> {
    let (ka, tap_a, la, term_a) = cx.b.config.tap_layers[0];
    let (kb, tap_b, lb, term_b) = cx.b.config.tap_layers[1];
    let term = u32::from(term_a.max(term_b));
    let h = cx.helper(&["H", "J", "K", "L", "Y", "U"], &[])?;
    let o = cx.helper(&["O", "P", "I", "U"], &[h])?;
    let c_o = 0x12;
    let slots = [((la, h), [2, 0, LEFT, 0]), ((lb, o), [2, 7, c_o, 0])];
    let mut e = V72::new(&cx.b.image, &slots)?;
    let all = Cell::new(true);
    let (d, u) = (DOWN, UP);
    let n = |k: u8| cx.n(k).to_string();
    out.push(format!(
        "Two layer keys: {} = layer {}, {} = layer {} (test slots: {} = Left in layer {}, {} = Ctrl+Shift+Alt+O in \
         layer {}):",
        n(ka),
        la + 1,
        n(kb),
        lb + 1,
        n(h),
        la + 1,
        n(o),
        lb + 1
    ));
    e.env.calls.clear();
    e.press(kb, true);
    e.idle(term + 30)?;
    e.press(o, true);
    e.idle(40)?;
    let modifiers = e.ram_u8(MODIFIERS);
    e.press(o, false);
    e.idle(40)?;
    e.press(kb, false);
    e.idle(60)?;
    let got: Vec<(u8, u8)> = e
        .env
        .calls
        .iter()
        .copied()
        .filter(|&(c, _)| c != FN_CODE)
        .collect();
    let good = modifiers & 7 == 7
        && got.contains(&(c_o, d))
        && got.contains(&(c_o, u))
        && e.layer() == 0
        && e.ram_u8(MODIFIERS) == 0;
    and(&all, good);
    out.push(format!(
        "  {:44} {:34} {} (modifiers while held {modifiers:#04x})",
        format!("{} held, {}", n(kb), n(o)),
        fmt_events(&got),
        ok(good)
    ));
    let run = |e: &mut V72, out: &mut Out, label: String, a: &[(Act, u8, u32)], w: &[(u8, u8)]| {
        let good = timed(e, out, &label, a, w, None)?;
        let lay = e.layer();
        if lay != 0 {
            out.push(format!("    after: layer {}: FAIL", lay + 1));
        }
        and(&all, good && lay == 0);
        Ok::<(), Stop>(())
    };
    run(
        &mut e,
        out,
        format!("{} held, {} tapped", n(ka), n(kb)),
        &[(D, ka, term + 30), (D, kb, 40), (U, kb, 40), (U, ka, 60)],
        &[(tap_b, d), (tap_b, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} held, {} tapped", n(kb), n(ka)),
        &[(D, kb, term + 30), (D, ka, 40), (U, ka, 40), (U, kb, 60)],
        &[(tap_a, d), (tap_a, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} tapped, then {} tapped", n(ka), n(kb)),
        &[(D, ka, 40), (U, ka, 40), (D, kb, 40), (U, kb, 60)],
        &[(tap_a, d), (tap_a, u), (tap_b, d), (tap_b, u)],
    )?;
    run(
        &mut e,
        out,
        format!(
            "{} + {} held, {} up, {} held, {} up",
            n(ka),
            n(h),
            n(ka),
            n(kb),
            n(h)
        ),
        &[
            (D, ka, term + 30),
            (D, h, 40),
            (U, ka, 40),
            (D, kb, term + 30),
            (U, h, 40),
            (U, kb, 60),
        ],
        &[(LEFT, d), (tap_b, d), (LEFT, u), (tap_b, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} down, {} down, {} up, {} up", n(ka), n(kb), n(ka), n(kb)),
        &[(D, ka, 40), (D, kb, 40), (U, ka, 40), (U, kb, 60)],
        &[(tap_a, d), (tap_b, d), (tap_a, u), (tap_b, u)],
    )?;
    Ok(all.get())
}

/// Tap-hold set in layer slots (`layertap.rs`): the test slots below, then timed scenarios, then random rolls
/// that must leave no key, modifier or layer behind.
#[allow(clippy::too_many_lines, clippy::many_single_char_names)]
fn test_layer_tap(cx: &Ctx<'_>, out: &mut Out) -> Result<bool, Stop> {
    use filco_firmware::layertap::{HOLD_LAYER, SLOT_TYPE};
    let k = |n: &str| cx.key(n);
    let (caps, semi, space, c, h, j) = (
        k("CapsLock"),
        k("Semicolon"),
        k("Space#77"),
        k("C"),
        k("H"),
        k("J"),
    );
    let (fnk, lalt, one, two) = (cx.b.fn_position, k("LAlt"), k("1"), k("2"));
    let (esc, lctrl, rwin, lshift, sp) = (0x29, LCTRL, 0xE7, 0xE1, 0x2C);
    let th = |tap: u8, hold: u8, term10: u8| [SLOT_TYPE, tap, hold, term10];
    let slots = [
        ((0, caps), th(esc, lctrl, 20)),
        ((0, semi), th(0x33, rwin, 0)),
        ((0, space), th(sp, HOLD_LAYER | 2, 30)),
        ((1, j), th(0x0D, lshift, 15)),
        ((2, h), [2, 0, LEFT, 0]),
    ];
    out.push(
        "Tap-hold in layer slots (type 8): layer 1 CapsLock = Esc / LCtrl 200 ms, Semicolon = ; / RWin (term 0 = \
         200 ms), Space#77 = Space / layer 3 300 ms; layer 2 J = J / LShift 150 ms; layer 3 H = Left:"
            .to_string(),
    );
    let mut e = V72::new(&cx.b.image, &slots)?;
    let mut stock = V72::new(&cx.b.image, &[])?;
    let fn_caps = first_down(&stock.chord(&[fnk], caps)?).unwrap_or(0);
    let fn_semi = first_down(&stock.chord(&[fnk], semi)?).unwrap_or(0);
    let plain_semi = first_down(&stock.chord(&[], semi)?).unwrap_or(0);
    let plain_caps = first_down(&stock.chord(&[], caps)?).unwrap_or(0);
    let all = Cell::new(true);
    let (d, u) = (DOWN, UP);
    let n = |k: u8| cx.n(k).to_string();
    let run = |e: &mut V72, out: &mut Out, label: String, a: &[(Act, u8, u32)], w: &[(u8, u8)]| {
        let good = timed(e, out, &label, a, w, None)?;
        let clean = clean_after(e, out);
        and(&all, good && clean);
        Ok::<(), Stop>(())
    };
    run(
        &mut e,
        out,
        format!("{} tapped 40 ms", n(caps)),
        &[(D, caps, 40), (U, caps, 60)],
        &[(esc, d), (esc, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} held 180 ms", n(caps)),
        &[(D, caps, 180), (U, caps, 60)],
        &[(esc, d), (esc, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} held 230 ms alone", n(caps)),
        &[(D, caps, 230), (U, caps, 60)],
        &[(lctrl, d), (lctrl, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} + {} within 40 ms", n(caps), n(c)),
        &[(D, caps, 40), (D, c, 40), (U, c, 40), (U, caps, 60)],
        &[(lctrl, d), (0x06, d), (0x06, u), (lctrl, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} + {} (Fn layer, as without the slot)", n(fnk), n(caps)),
        &[(D, fnk, 40), (D, caps, 40), (U, caps, 40), (U, fnk, 60)],
        &[(fn_caps, d), (fn_caps, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} + {} (Fn layer, as without the slot)", n(fnk), n(semi)),
        &[(D, fnk, 40), (D, semi, 40), (U, semi, 40), (U, fnk, 60)],
        &[(fn_semi, d), (fn_semi, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} + {}, {} released first", n(fnk), n(semi), n(fnk)),
        &[(D, fnk, 40), (D, semi, 40), (U, fnk, 40), (U, semi, 60)],
        &[(fn_semi, d), (fn_semi, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} tapped 40 ms", n(semi)),
        &[(D, semi, 40), (U, semi, 60)],
        &[(0x33, d), (0x33, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} held 230 ms alone", n(semi)),
        &[(D, semi, 230), (U, semi, 60)],
        &[(rwin, d), (rwin, u)],
    )?;
    run(
        &mut e,
        out,
        format!(
            "{} held, {} tapped (second key hold: plain key)",
            n(caps),
            n(semi)
        ),
        &[(D, caps, 230), (D, semi, 40), (U, semi, 40), (U, caps, 60)],
        &[(lctrl, d), (plain_semi, d), (plain_semi, u), (lctrl, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} tapped 40 ms", n(space)),
        &[(D, space, 40), (U, space, 60)],
        &[(sp, d), (sp, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} held 330 ms, then {} (slot Left in layer 3)", n(space), n(h)),
        &[(D, space, 330), (D, h, 40), (U, h, 40), (U, space, 60)],
        &[(LEFT, d), (LEFT, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} + {} tapped, all within 300 ms", n(space), n(h)),
        &[(D, space, 40), (D, h, 40), (U, h, 40), (U, space, 60)],
        &[(sp, d), (0x0B, d), (0x0B, u), (sp, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} held 330 ms, {} (no slot in layer 3)", n(space), n(caps)),
        &[(D, space, 330), (D, caps, 40), (U, caps, 40), (U, space, 60)],
        &[(plain_caps, d), (plain_caps, u)],
    )?;
    run(
        &mut e,
        out,
        format!("{} held, {} tapped", n(caps), n(space)),
        &[(D, caps, 230), (D, space, 40), (U, space, 40), (U, caps, 60)],
        &[(lctrl, d), (sp, d), (sp, u), (lctrl, u)],
    )?;
    run(
        &mut e,
        out,
        format!(
            "{} down, {} held 330 ms, {} (layer 3 slot)",
            n(caps),
            n(space),
            n(h)
        ),
        &[
            (D, caps, 40),
            (D, space, 330),
            (D, h, 40),
            (U, h, 40),
            (U, space, 40),
            (U, caps, 60),
        ],
        &[(lctrl, d), (LEFT, d), (LEFT, u), (lctrl, u)],
    )?;
    // Layer 2: its own tap-hold keys only (the layer stays 2 until Fn + LAlt + 1 below).
    let run2 = |e: &mut V72, out: &mut Out, label: String, a: &[(Act, u8, u32)], w: &[(u8, u8)]| {
        let good = timed(e, out, &label, a, w, None)?;
        and(&all, good && e.layer() == 1 && e.ram_u8(MODIFIERS) == 0);
        Ok::<(), Stop>(())
    };
    e.chord(&[fnk, lalt], two)?;
    let l2 = e.layer() == 1;
    and(&all, l2);
    out.push(format!(
        "  {:44} {:34} {}",
        "Fn + LAlt + 2",
        format!("layer {}", e.layer() + 1),
        ok(l2)
    ));
    run2(
        &mut e,
        out,
        format!("layer 2: {} tapped (no slot here)", n(caps)),
        &[(D, caps, 40), (U, caps, 60)],
        &[(plain_caps, d), (plain_caps, u)],
    )?;
    run2(
        &mut e,
        out,
        format!("layer 2: {} tapped 40 ms", n(j)),
        &[(D, j, 40), (U, j, 60)],
        &[(0x0D, d), (0x0D, u)],
    )?;
    e.env.calls.clear();
    e.press(j, true);
    e.idle(180)?;
    let held = e.ram_u8(MODIFIERS);
    e.press(j, false);
    e.idle(60)?;
    let got: Vec<(u8, u8)> = e
        .env
        .calls
        .iter()
        .copied()
        .filter(|&(c, _)| c != FN_CODE)
        .collect();
    let good = got == [(lshift, d), (lshift, u)] && held & 2 == 2 && e.layer() == 1;
    and(&all, good);
    out.push(format!(
        "  {:44} {:34} {}",
        format!("layer 2: {} held 180 ms", n(j)),
        fmt_events(&got),
        ok(good)
    ));
    e.chord(&[fnk, lalt], one)?;
    let l1 = e.layer() == 0;
    and(&all, l1);
    out.push(format!(
        "  {:44} {:34} {}",
        "Fn + LAlt + 1",
        format!("layer {}", e.layer() + 1),
        ok(l1)
    ));

    // Random rolls over the tap-hold keys and a few others: nothing may stay down.
    let keys: [u8; 7] = [caps, semi, space, c, h, j, fnk];
    let mut seed: u32 = 0x1234_5678;
    let mut rnd = |m: u32| {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        (seed >> 16) % m
    };
    let mut bad = 0;
    let rolls = 200;
    for _ in 0..rolls {
        e.env.calls.clear();
        let mut down: Vec<u8> = Vec::new();
        for _ in 0..(2 + rnd(5)) {
            let key = keys[rnd(7) as usize];
            if down.contains(&key) {
                e.press(key, false);
                down.retain(|&x| x != key);
            } else {
                e.press(key, true);
                down.push(key);
            }
            e.idle(1 + rnd(400))?;
        }
        for key in down {
            e.press(key, false);
            e.idle(1 + rnd(60))?;
        }
        e.idle(400)?;
        let mut balance = std::collections::BTreeMap::<u8, i32>::new();
        for &(code, ev) in &e.env.calls {
            *balance.entry(code).or_default() += if ev == DOWN { 1 } else { -1 };
        }
        let left_on = balance.values().any(|&v| v != 0);
        if left_on || e.layer() != 0 || e.ram_u8(MODIFIERS) != 0 {
            bad += 1;
            if bad <= 3 {
                out.push(format!("    stuck after: {}", fmt_events(&e.env.calls)));
            }
        }
    }
    and(&all, bad == 0);
    out.push(format!(
        "  {:44} {:34} {}",
        format!("{rolls} random rolls, all keys up"),
        format!("{bad} left a key, modifier or layer on"),
        ok(bad == 0)
    ));
    Ok(all.get())
}

fn with_pages(img: &[u8], pages: &[Vec<u8>; 4]) -> Vec<u8> {
    let mut b = img.to_vec();
    for (layer, page) in pages.iter().enumerate() {
        let at = 0xF000 + 0x240 * layer;
        b[at..at + 512].copy_from_slice(page);
    }
    b
}

fn test_info(cx: &Ctx<'_>, out: &mut Out) -> Result<bool, Stop> {
    use filco_fwinfo::{BLOCK_ADDR, BLOCK_LEN, CMD_INFO};
    let all = Cell::new(true);
    let line = |out: &mut Out, label: &str, good: bool, note: &str| {
        and(&all, good);
        out.push(format!("  {label:58} {}{note}", ok(good)));
    };
    #[allow(clippy::cast_possible_truncation)]
    let pages: [Vec<u8>; 4] =
        std::array::from_fn(|layer| (0..512).map(|i| ((layer * 0x35 + i * 3) & 0xFF) as u8).collect());
    out.push(format!(
        "Info block over HID (command {CMD_INFO:#04x}, emulated host as fnmap.py reads a layer):"
    ));
    let mut host = HidHost::new(V72::new(&with_pages(&cx.b.image, &pages), &[])?);
    for layer in [0u8, 3] {
        let good = host.read(0x10, layer, 8)?.as_deref() == Some(&pages[usize::from(layer)][..]);
        line(
            out,
            &format!("layer {} read (0x10), ACK + 8 chunks + checksum", layer + 1),
            good,
            "",
        );
    }
    let data = host.read(CMD_INFO, 0, 8)?;
    line(out, "info command: ACK + 8 chunks + checksum", data.is_some(), "");
    let block = &cx.b.image[BLOCK_ADDR..BLOCK_ADDR + BLOCK_LEN];
    line(
        out,
        "info data equals the block in the image",
        data.as_deref() == Some(block),
        "",
    );
    let want = expected_info(cx);
    let got = data.as_deref().and_then(|d| filco_fwinfo::parse(d).ok());
    let same = got.as_ref().is_some_and(|g| info_matches(g, &want));
    let note = if same {
        String::new()
    } else {
        format!(" got {got:?}, expected {want:?}")
    };
    line(
        out,
        "block says what the config says (name, tap-hold, Fn layer, keymap)",
        same,
        &note,
    );
    let again = host.read(CMD_INFO, 0, 8)?;
    line(
        out,
        "info read twice in a row gives the same data",
        again == data,
        "",
    );
    line(
        out,
        "layer 2 read after the info read",
        host.read(0x10, 1, 8)?.as_deref() == Some(&pages[1][..]),
        "",
    );
    for cmd in [CMD_INFO + 1, 0x13] {
        let (r, _) = host.ack(cmd, 0, 0)?;
        let good = (r[4] == 0xFF) == (cmd != 0x13);
        line(
            out,
            &format!("command {cmd:#04x} answered as in stock"),
            good,
            &format!(" ([4] = {:#04x})", r[4]),
        );
    }
    let mut stock_host = HidHost::new(V72::new(&with_pages(&cx.b.stock, &pages), &[])?);
    let (_, acked) = stock_host.ack(CMD_INFO, 8, 0)?;
    line(
        out,
        &format!("stock V72 refuses command {CMD_INFO:#04x} (how a host tells there is no block)"),
        !acked,
        "",
    );
    let good = stock_host.read(0x10, 2, 8)?.as_deref() == Some(&pages[2][..]);
    line(out, "stock V72: layer read right after that refusal", good, "");
    Ok(all.get())
}

/// What the block must say, from the config.
fn expected_info(cx: &Ctx<'_>) -> filco_firmware::info::BlockContent {
    let c = &cx.b.config;
    filco_firmware::info::BlockContent {
        name: c.name.clone(),
        tap_key: c.tap_key,
        tap_layers: c.tap_layers.clone(),
        fn_layer: c.fn_layer.clone(),
        keymap: c.keymap.clone(),
        layer_tap_hold: c.layer_tap_hold,
    }
}

fn info_matches(got: &filco_fwinfo::Info, want: &filco_firmware::info::BlockContent) -> bool {
    let mut fl: Vec<(u8, Option<u8>)> = want.fn_layer.clone();
    fl.sort_by_key(|e| e.0);
    let mut km: Vec<(u8, u8)> = want.keymap.clone();
    km.sort_by_key(|e| e.0);
    let got_fl: Vec<(u8, Option<u8>)> = got.fn_layer.iter().map(|e| (e.key, e.code)).collect();
    let got_km: Vec<(u8, u8)> = got.keymap.iter().map(|e| (e.key, e.code)).collect();
    got.name == want.name
        && got.tap_key.map(|t| (t.key, t.tap, t.hold, t.term_ms)) == want.tap_key
        && got
            .tap_layers
            .iter()
            .map(|t| (t.key, t.tap, t.layer, t.term_ms))
            .collect::<Vec<_>>()
            == want.tap_layers
        && got_fl == fl
        && got_km == km
        && got.layer_tap_hold == want.layer_tap_hold.then_some(filco_firmware::layertap::SLOT_TYPE)
}

fn test_stack(stock: &[u8], new: &[u8], out: &mut Out) -> bool {
    let old = stack::analyze(stock);
    let neu = stack::analyze(new);
    let all = Cell::new(true);
    out.push(format!(
        "Stack depth, static worst case per entry point (stack {} -> {} bytes):",
        old.stack_size, neu.stack_size
    ));
    for ((name, o), (_, n)) in old.depths.iter().zip(&neu.depths) {
        let good = n <= o;
        and(&all, good);
        out.push(format!(
            "  {name:22} stock {o:>3}  new {n:>3}  {}",
            if good { "OK" } else { "FAIL (deeper than stock)" }
        ));
    }
    // 0x148: jump to the ROM bootloader, expected
    let extra: Vec<&String> = neu.warnings.iter().filter(|w| !w.starts_with("0x148:")).collect();
    for w in &extra {
        out.push(format!("  analysis warning: {w}"));
    }
    and(&all, extra.is_empty());
    out.push(format!(
        "  stack reserved by the patch: {} bytes",
        old.stack_size - neu.stack_size
    ));
    all.get()
}
