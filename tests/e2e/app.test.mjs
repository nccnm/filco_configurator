// End-to-end tests: the real app (e2e-sim build: simulated keyboard behind the real IPC, UI and write flow),
// driven through WebDriver like a user. Run: node --test tests/e2e/app.test.mjs  (needs the e2e-sim build).
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { after, before, describe, it } from "node:test";
import { byTest, click, openApp, ROOT, sim, waitIdle, waitState } from "./driver.mjs";

const OUT = path.join(ROOT, "tests", "e2e", "out");

async function select(browser, layer, key) {
  await click(browser, `layer-${layer}`);
  await click(browser, `key-${key}`);
  return (await byTest(browser, "slot-hex")).getText();
}

async function saveAndOpenDiff(browser) {
  await click(browser, "save");
  const dlg = await byTest(browser, "diff-dialog");
  await dlg.waitForDisplayed({ timeout: 10000 });
  await waitIdle(browser);
  return dlg;
}

async function report(browser) {
  const dlg = await byTest(browser, "report-dialog");
  try {
    await dlg.waitForDisplayed({ timeout: 20000 });
  } catch (e) {
    fs.mkdirSync(OUT, { recursive: true });
    await browser.saveScreenshot(path.join(OUT, `no-report-${Date.now()}.png`));
    throw e;
  }
  const outcomes = await browser.execute(() =>
    [...document.querySelectorAll("[data-outcome]")].map((e) => e.getAttribute("data-outcome")),
  );
  const summary = await (await byTest(browser, "report-summary")).getText();
  return { outcomes, summary };
}

const attr = (browser, sel, name) =>
  browser.execute((s, n) => document.querySelector(s)?.getAttribute(n) ?? null, sel, name);

/** Waits for the error banner with this code; says what the inspector held if it does not come. */
async function waitError(browser, code) {
  const banner = () => attr(browser, '[data-testid="error-banner"]', "data-code");
  try {
    await browser.waitUntil(async () => (await banner()) === code, { timeout: 3000, interval: 25 });
  } catch {
    const raw = await (await byTest(browser, "raw-input")).getValue().catch(() => "?");
    const hex = await (await byTest(browser, "slot-hex")).getText();
    assert.fail(`no ${code} banner (got ${await banner()}); raw input "${raw}", slot ${hex}`);
  }
}

/** Waits until the key with this id is the selected one. */
async function waitSelected(browser, key) {
  const el = await byTest(browser, `key-${key}`);
  await el.waitUntil(async () => (await el.getAttribute("aria-pressed")) === "true", {
    timeout: 5000,
    timeoutMsg: `${key} was not selected`,
  });
}

describe("keyboard layer editor (simulated keyboard)", () => {
  let app;
  before(async () => {
    app = await openApp();
    await app.browser.execute(() => localStorage.setItem("lang", "en"));
    await app.browser.refresh();
    await waitState(app.browser, "ready");
  });
  after(async () => app?.close());

  it("shows the 4 layers read from the keyboard", async () => {
    const { browser } = app;
    assert.equal(await (await byTest(browser, "device-name")).getText(), "Majestouch Xacro M10SP US");
    assert.equal((await browser.$$('[data-testid^="layer-"]')).length, 4);
    assert.equal((await browser.$$('[data-testid^="key-"]')).length, 72);
    // The user's configuration: M10 is Fn on layer 1, P sends Win+Shift+S on layer 4.
    assert.equal(await select(browser, 0, "M10"), "02 00 a4 00");
    assert.equal(await select(browser, 3, "P"), "02 0a 16 00");
  });

  it("switches layers with the number keys and moves with the arrows", async () => {
    const { browser } = app;
    await click(browser, "key-Q");
    await browser.keys("3");
    assert.equal(await attr(browser, '[data-testid="layer-2"]', "aria-selected"), "true");
    await browser.keys("ArrowRight");
    assert.equal(await attr(browser, '[data-testid="key-W"]', "aria-pressed"), "true");
    await browser.keys("ArrowDown");
    assert.equal(await attr(browser, '[data-testid="key-S"]', "aria-pressed"), "true");
    await browser.keys("1");
  });

  it("leaves room for focus rings inside the inspector's scroll areas", async () => {
    const { browser } = app;
    await click(browser, "key-Q");
    await (await byTest(browser, "key-search")).waitForDisplayed({ timeout: 5000 });
    // The ring is a 2px outline 2px away from the element: 4px that a scroll area (overflow other than
    // visible) clips. Lists the focusable elements whose ring would be cut.
    const clipped = await browser.execute(() => {
      const ring = 4;
      const out = [];
      for (const el of document.querySelectorAll(".inspector button, .inspector input")) {
        const r = el.getBoundingClientRect();
        if (r.width === 0) continue;
        for (let p = el.parentElement; p; p = p.parentElement) {
          const s = getComputedStyle(p);
          if (s.overflowX === "visible" && s.overflowY === "visible") continue;
          const c = p.getBoundingClientRect();
          const box = { left: c.left + p.clientLeft, right: c.left + p.clientLeft + p.clientWidth };
          if (r.left - ring < box.left - 0.5 || r.right + ring > box.right + 0.5) {
            out.push(`${el.dataset.testid ?? el.textContent?.trim()} in .${p.className.split(" ")[0]}`);
          }
          break;
        }
      }
      return out;
    });
    assert.deepEqual(clipped, []);
    await browser.keys("Escape");
  });

  it("opens the tab of what the selected key holds and scrolls it into the middle", async () => {
    const { browser } = app;
    // The open tab, the marked button(s) in the panel, and how far the first sits from the panel's middle.
    const shown = () =>
      browser.execute(() => {
        const panel = document.querySelector(".inspector .panel");
        const marked = [...panel.querySelectorAll(".current")];
        const p = panel.getBoundingClientRect();
        const r = marked[0]?.getBoundingClientRect();
        return {
          tab: document.querySelector('.inspector [role="tab"][aria-selected="true"]')?.dataset.testid,
          marked: marked.map((e) => e.dataset.testid ?? e.textContent.trim()),
          off: r ? Math.round(r.top + r.height / 2 - (p.top + panel.clientHeight / 2)) : null,
          scrolled: panel.scrollTop,
          ctrl: document.querySelector(".inspector .chip[aria-pressed='true']")?.textContent.trim() ?? null,
        };
      });
    // Waits until `done` holds (the panel scrolls after the next render), then returns what is shown, so a
    // timeout still fails with the assertions below.
    const until = async (done) => {
      let s;
      await browser
        .waitUntil(async () => done((s = await shown())), { timeout: 5000, interval: 25 })
        .catch(() => {});
      return s;
    };
    const centred = (s) => s.off !== null && Math.abs(s.off) <= 2;

    // Layer 1, M2 = Home: far down the Key tab, so the panel scrolls until Home is in the middle.
    await select(browser, 0, "M2");
    let s = await until((x) => x.tab === "tab-key" && centred(x));
    assert.equal(s.tab, "tab-key");
    assert.deepEqual(s.marked, ["usage-4a"]);
    assert.ok(Math.abs(s.off) <= 2, `Home is in the middle (off by ${s.off}px)`);

    // Layer 3, W = Ctrl+Right: the Ctrl chip is on, so the arrow is marked.
    await select(browser, 2, "W");
    s = await until((x) => x.ctrl === "Ctrl" && centred(x));
    assert.equal(s.tab, "tab-key");
    assert.equal(s.ctrl, "Ctrl");
    assert.deepEqual(s.marked, ["usage-4f"]);
    assert.ok(Math.abs(s.off) <= 2, `Right is in the middle (off by ${s.off}px)`);

    // Layer 3, P = scroll up; R = left click: the Mouse tab.
    await click(browser, "key-P");
    s = await until((x) => x.tab === "tab-mouse" && x.marked.length > 0);
    assert.equal(s.tab, "tab-mouse");
    assert.equal(s.marked.length, 1, `one mouse action marked: ${s.marked}`);
    await click(browser, "key-R");
    s = await until((x) => x.tab === "tab-mouse" && x.marked.length > 0 && x.marked[0] !== "");
    assert.equal(s.tab, "tab-mouse");
    assert.equal(s.marked.length, 1, `one mouse action marked: ${s.marked}`);

    // An empty slot: the Key tab from the top, nothing marked.
    await click(browser, "key-Q");
    s = await until((x) => x.tab === "tab-key" && x.marked.length === 0 && x.scrolled === 0);
    assert.equal(s.tab, "tab-key");
    assert.deepEqual(s.marked, []);
    assert.equal(s.scrolled, 0);
    await browser.keys("Escape");
    await click(browser, "layer-0");
  });

  it("edits, undoes, redoes and discards", async () => {
    const { browser } = app;
    const before = await select(browser, 2, "M1");
    await click(browser, "usage-68"); // F13
    await waitIdle(browser);
    await waitSelected(browser, "M6");
    await click(browser, "key-M1");
    assert.equal(await (await byTest(browser, "slot-hex")).getText(), "02 00 68 00");
    assert.match(await (await byTest(browser, "save")).getText(), /Save 1 changes/);
    await browser.keys(["Control", "z"]);
    await waitIdle(browser);
    assert.equal(await (await byTest(browser, "slot-hex")).getText(), before);
    await browser.keys(["Control", "y"]);
    await waitIdle(browser);
    assert.equal(await (await byTest(browser, "slot-hex")).getText(), "02 00 68 00");
    await click(browser, "revert");
    await waitIdle(browser);
    assert.equal(await (await byTest(browser, "slot-hex")).getText(), before);
  });

  it("selects the next key after a mapping: along the row, down a row, back to the top", async () => {
    const { browser } = app;
    await select(browser, 2, "Q");
    await click(browser, "usage-68");
    await waitSelected(browser, "W");
    // The last key of a row goes on to the first key of the next row; the last row to the top row.
    await click(browser, "key-Bksp");
    await click(browser, "usage-69");
    await waitSelected(browser, "Tab");
    await click(browser, "key-RCtrl");
    await click(browser, "usage-6a");
    await waitSelected(browser, "Esc");
    assert.match(await (await byTest(browser, "save")).getText(), /Save 3 changes/);
    // Resetting a key to its default is not a mapping: the key stays selected.
    await click(browser, "key-Q");
    await click(browser, "clear-slot");
    await waitIdle(browser);
    assert.equal(await attr(browser, '[data-testid="key-Q"]', "aria-pressed"), "true");
    await click(browser, "revert");
    await waitIdle(browser);
    await click(browser, "layer-0");
  });

  it("rejects bytes the editor does not write (macro)", async () => {
    const { browser } = app;
    await select(browser, 0, "Q");
    await click(browser, "tab-advanced");
    const input = await byTest(browser, "raw-input");
    await input.setValue("06 01 00 00");
    await click(browser, "raw-assign");
    await waitIdle(browser);
    await waitError(browser, "edit_invalid");
    // A refused edit keeps the key selected.
    assert.equal(await attr(browser, '[data-testid="key-Q"]', "aria-pressed"), "true");
    await click(browser, "tab-key");
  });

  it("cancelling the diff writes nothing", async () => {
    const { browser } = app;
    await select(browser, 2, "M2");
    await click(browser, "usage-69");
    await waitIdle(browser);
    await saveAndOpenDiff(browser);
    // The safe button has the focus: Enter cancels.
    await browser.keys("Enter");
    await waitIdle(browser);
    assert.equal(await byTest(browser, "diff-dialog").then((d) => d.isExisting()), false);
    assert.match(await (await byTest(browser, "save")).getText(), /Save 1 changes/);
    await click(browser, "revert");
    await waitIdle(browser);
  });

  it("writes through the diff, reads back, and the keyboard keeps it", async () => {
    const { browser } = app;
    await select(browser, 2, "M1");
    await click(browser, "usage-68");
    await waitIdle(browser);
    await select(browser, 1, "M9");
    await click(browser, "usage-69"); // F14 on layer 2
    await waitIdle(browser);
    const dlg = await saveAndOpenDiff(browser);
    const rows = await dlg.$$("tbody tr");
    assert.equal(rows.length, 2);
    await click(browser, "diff-write");
    const r = await report(browser);
    assert.deepEqual(r.outcomes, ["written", "written"]);
    await click(browser, "report-close");
    await waitIdle(browser);
    assert.match(await (await byTest(browser, "save")).getText(), /No changes/);
    // Read again from the keyboard: the new slots are there.
    await click(browser, "read-again");
    await waitState(browser, "ready");
    assert.equal(await select(browser, 2, "M1"), "02 00 68 00");
    assert.equal(await select(browser, 1, "M9"), "02 00 69 00");
  });

  it("a failed write puts every layer back and keeps the edits", async () => {
    const { browser } = app;
    await select(browser, 2, "M3");
    await click(browser, "usage-6a");
    await waitIdle(browser);
    await select(browser, 3, "M3");
    await click(browser, "usage-6b");
    await waitIdle(browser);
    await saveAndOpenDiff(browser);
    // Fail the second layer of this write at its finish packet.
    await sim(
      browser,
      `fault_next:${JSON.stringify({ cmd: 17, nth: 1, stage: "finish", kind: "wrong_ack" })}`,
    );
    await click(browser, "diff-write");
    const r = await report(browser);
    assert.deepEqual(r.outcomes, ["restored", "restored"]);
    assert.match(r.summary, /put back exactly/);
    await click(browser, "report-close");
    await waitIdle(browser);
    assert.match(await (await byTest(browser, "save")).getText(), /Save 2 changes/);
    await click(browser, "revert");
    await waitIdle(browser);
  });

  it("blocks saving while FILCO Assist runs", async () => {
    const { browser } = app;
    await select(browser, 0, "M4");
    await click(browser, "usage-6c");
    await waitIdle(browser);
    await sim(browser, "assist_on");
    await (await byTest(browser, "assist-banner")).waitForDisplayed();
    assert.equal(await (await byTest(browser, "save")).isEnabled(), false);
    await browser.keys(["Control", "s"]);
    assert.equal(await byTest(browser, "diff-dialog").then((d) => d.isExisting()), false);
    await sim(browser, "assist_off");
    await browser.waitUntil(async () => (await byTest(browser, "save")).isEnabled(), { timeout: 5000 });
    await click(browser, "revert");
    await waitIdle(browser);
  });

  it("unplugging shows the no-keyboard screen and plugging in reads again", async () => {
    const { browser } = app;
    await sim(browser, "unplug");
    await waitState(browser, "no_device");
    await sim(browser, "replug");
    await waitState(browser, "ready");
  });

  it("an interrupted write is reported after reconnecting and can be restored", async () => {
    const { browser } = app;
    await sim(browser, "reset");
    await click(browser, "read-again");
    await waitState(browser, "ready");
    await select(browser, 0, "M5");
    await click(browser, "usage-6d");
    await waitIdle(browser);
    await select(browser, 1, "M5");
    await click(browser, "usage-6d");
    await waitIdle(browser);
    await saveAndOpenDiff(browser);
    await sim(
      browser,
      `fault_next:${JSON.stringify({ cmd: 17, nth: 1, stage: "data", n: 3, kind: "disconnect" })}`,
    );
    await click(browser, "diff-write");
    const r = await report(browser);
    assert.deepEqual(r.outcomes, ["written", "unknown"]);
    await click(browser, "report-close");
    await sim(browser, "replug");
    await waitState(browser, "ready");
    const banner = await byTest(browser, "journal-banner");
    await banner.waitForDisplayed({ timeout: 5000 });
    assert.match(await banner.getText(), /Layer 1: the new content/);
    await click(browser, "journal-restore"); // load the backup into the editor
    await waitIdle(browser);
    assert.equal(await byTest(browser, "journal-banner").then((b) => b.isExisting()), false);
    await saveAndOpenDiff(browser);
    await click(browser, "diff-write");
    assert.deepEqual((await report(browser)).outcomes, ["written"]);
    await click(browser, "report-close");
    await waitIdle(browser);
  });
});

describe("tap-hold per layer (keyboard runs a build with layer_tap_hold)", () => {
  let app;
  before(async () => {
    app = await openApp();
    await app.browser.execute(() => localStorage.setItem("lang", "en"));
    await app.browser.refresh();
    await waitState(app.browser, "ready");
    await sim(app.browser, "info_layer_tap");
    await sim(app.browser, "unplug");
    await sim(app.browser, "replug");
    await waitState(app.browser, "ready");
  });
  after(async () => app?.close());

  const hex = async (browser) => (await byTest(browser, "slot-hex")).getText();

  it("sets tap, hold and time on one key, and the key stays selected", async () => {
    const { browser } = app;
    await select(browser, 0, "Caps");
    await click(browser, "tab-tap_hold");
    assert.equal(await (await byTest(browser, "tap-hold-unsupported")).isExisting(), false);
    await click(browser, "th-tap");
    await click(browser, "usage-29"); // Esc; the grid then fills the hold
    await click(browser, "usage-e0"); // LCtrl
    await waitIdle(browser);
    assert.equal(await hex(browser), "08 29 e0 14");
    assert.equal(await attr(browser, '[data-testid="key-Caps"]', "aria-pressed"), "true");
    assert.match(await attr(browser, '[data-testid="key-Caps"]', "class"), /taphold/);
    assert.match(await (await byTest(browser, "slot-long")).getText(), /tap Escape, hold Left Control/);
    const term = await byTest(browser, "th-term");
    await term.setValue("300");
    await browser.keys("Tab");
    await waitIdle(browser);
    assert.equal(await hex(browser), "08 29 e0 1e");
    // Reopening the key opens the Tap-hold tab with its values.
    await click(browser, "key-Q");
    await click(browser, "key-Caps");
    assert.equal(await attr(browser, '[data-testid="tab-tap_hold"]', "aria-selected"), "true");
    assert.equal(await (await byTest(browser, "th-term")).getValue(), "300");
  });

  it("the same key holds into a layer on another layer; that layer shows it bold", async () => {
    const { browser } = app;
    await select(browser, 1, "Caps");
    await click(browser, "tab-tap_hold");
    assert.equal(await (await byTest(browser, "th-layer-1")).isEnabled(), false); // the layer shown
    await click(browser, "th-tap");
    await click(browser, "usage-39"); // Caps Lock
    await click(browser, "th-layer-2");
    await waitIdle(browser);
    assert.equal(await hex(browser), "08 39 f2 14");
    assert.doesNotMatch(await attr(browser, '[data-testid="key-Caps"]', "class"), /holds-here/);
    await click(browser, "layer-2");
    assert.match(await attr(browser, '[data-testid="key-Caps"]', "class"), /holds-here/);
    // Layer 1 still holds its own tap-hold on the key.
    assert.equal(await select(browser, 0, "Caps"), "08 29 e0 1e");
    assert.match(await (await byTest(browser, "save")).getText(), /Save 2 changes/);
  });

  it("the Firmware page shows tap-hold per layer as on", async () => {
    const { browser } = app;
    await click(browser, "fw-tab");
    await click(browser, "fwkey-Caps");
    assert.match(await (await byTest(browser, "fw-sec-taphold")).getText(), /per layer/);
    await click(browser, "fw-sec-taphold");
    assert.equal(await attr(browser, '[data-testid="fw-lt-on"]', "aria-pressed"), "true");
    await click(browser, "layer-0");
    await click(browser, "revert");
    await waitIdle(browser);
  });
});

describe("tap-hold on stock firmware: saving updates the firmware too", () => {
  let app;
  before(async () => {
    app = await openApp();
    await app.browser.execute(() => localStorage.setItem("lang", "en"));
    await app.browser.refresh();
    await waitState(app.browser, "ready");
  });
  after(async () => app?.close());

  it("sets a tap-hold, writes the layers, opens the updater and checks the keyboard after", async () => {
    const { browser } = app;
    await select(browser, 0, "Caps");
    await click(browser, "tab-tap_hold");
    assert.ok(await (await byTest(browser, "tap-hold-upgrade")).isDisplayed());
    await click(browser, "th-tap");
    await click(browser, "usage-29");
    await click(browser, "usage-e0");
    await waitIdle(browser);
    assert.equal(await (await byTest(browser, "slot-hex")).getText(), "08 29 e0 14");
    // Save: the firmware is built and tested first, so the diff can name its checksum.
    await click(browser, "save");
    const dlg = await byTest(browser, "diff-dialog");
    await dlg.waitForDisplayed({ timeout: 90000 });
    await waitIdle(browser);
    assert.match(await (await byTest(browser, "diff-upgrade")).getText(), /CheckSum : 0x[0-9A-F]{4}/);
    await click(browser, "diff-write");
    // The layers are written, then the updater is started (simulated: it flashes and unplugs the keyboard).
    await (await byTest(browser, "fw-flash-waiting")).waitForDisplayed({ timeout: 20000 });
    await waitIdle(browser);
    // Any refresh sees the keyboard gone (Windows reports it; here a simulator control refreshes), then back.
    await sim(browser, "assist_off");
    await sim(browser, "replug");
    await (await byTest(browser, "fw-flash-result")).waitForDisplayed({ timeout: 15000 });
    assert.match(await (await byTest(browser, "fw-flash-result")).getText(), /CheckSum 0x/);
    await click(browser, "fw-flash-close");
    await waitIdle(browser);
    // The keyboard now runs tap-hold per layer and holds the slot: nothing left to save.
    assert.match(await (await byTest(browser, "save")).getText(), /No changes/);
    await click(browser, "key-Caps");
    assert.equal(await (await byTest(browser, "slot-hex")).getText(), "08 29 e0 14");
    await click(browser, "tab-tap_hold");
    assert.equal(await (await byTest(browser, "tap-hold-upgrade")).isExisting(), false);
  });
});

describe("firmware mode (simulated keyboard, real build pipeline)", () => {
  let app;
  before(async () => {
    app = await openApp();
    await waitState(app.browser, "ready");
    // The keyboard runs the HJKL_CapsSpace2_Info build: reconnect so its info block is read.
    await sim(app.browser, "info_on");
    await sim(app.browser, "unplug");
    await sim(app.browser, "replug");
    await waitState(app.browser, "ready");
  });
  after(async () => app?.close());

  const text = async (browser, id) => (await byTest(browser, id)).getText();
  const attr = async (browser, id, name) => (await byTest(browser, id)).getAttribute(name);

  it("a tap-hold key of the firmware can be mapped like any other", async () => {
    const { browser } = app;
    await click(browser, "layer-0");
    await click(browser, "key-\\\\"); // "\\", the last key of its row; Caps below it is a tap-hold key
    await click(browser, "usage-68");
    await waitIdle(browser);
    await waitSelected(browser, "Caps");
    assert.match(await attr(browser, "key-Caps", "class"), /taphold/);
    assert.ok(await (await byTest(browser, "taphold-note")).isDisplayed());
    await click(browser, "revert");
    await waitIdle(browser);
  });

  it("shows the firmware on the keyboard and its Fn layer", async () => {
    const { browser } = app;
    await click(browser, "fw-tab");
    assert.match(await text(browser, "fw-line"), /HJKL_CapsSpace2_Info.*0x1321/s);
    // Fn + H is Left (patched), Fn + S is mouse left (an icon), A has no Fn function.
    assert.match(await attr(browser, "fwkey-H", "aria-label"), /Fn \+ H: Left/);
    assert.match(await attr(browser, "fwkey-S", "aria-label"), /Fn \+ S: Mouse: move left/);
    assert.equal(await attr(browser, "fwkey-A", "aria-label"), "A");
  });

  it("refuses an edit the firmware cannot hold, accepts one it can, undoes it", async () => {
    const { browser } = app;
    await click(browser, "fwkey-Q");
    await click(browser, "fw-code-04");
    await waitIdle(browser);
    assert.equal(await attr(browser, "error-banner", "data-code"), "fw_edit_refused");
    await click(browser, "fwkey-O");
    await click(browser, "fw-code-4a");
    await waitIdle(browser);
    assert.match(await text(browser, "fw-tab"), /1/);
    assert.match(await attr(browser, "fwkey-O", "aria-label"), /Fn \+ O: Home/);
    await click(browser, "undo");
    await waitIdle(browser);
    assert.match(await attr(browser, "fwkey-O", "aria-label"), /Fn \+ O: Print Screen/);
    await click(browser, "redo");
    await waitIdle(browser);
  });

  it("builds, tests in the emulator, flashes with the (simulated) updater and checks", async () => {
    const { browser } = app;
    await click(browser, "fw-build");
    await (await byTest(browser, "fw-build-dialog")).waitForDisplayed({ timeout: 5000 });
    const name = await (await byTest(browser, "fw-build-name")).getValue();
    assert.equal(name, "HJKL_CapsSpace2_Info_2");
    // Records each step shown as running, and whether later steps were waiting at that moment.
    await browser.execute(() => {
      const w = /** @type {any} */ (window);
      w.__fwRunning = [];
      w.__fwWaiting = false;
      w.__fwObserver = new MutationObserver(() => {
        const rows = [...document.querySelectorAll('[data-testid="fw-build-steps"] li')];
        const run = rows.find((li) => li.dataset.state === "running");
        if (run && !w.__fwRunning.includes(run.dataset.step)) w.__fwRunning.push(run.dataset.step);
        if (rows.some((li) => li.dataset.state === "waiting")) w.__fwWaiting = true;
      });
      w.__fwObserver.observe(document.body, { subtree: true, childList: true, attributes: true });
    });
    await click(browser, "fw-build-start");
    await (await byTest(browser, "fw-build-passed")).waitForDisplayed({ timeout: 60000 });
    await waitIdle(browser);
    const progress = await browser.execute(() => {
      const w = /** @type {any} */ (window);
      w.__fwObserver.disconnect();
      return { running: w.__fwRunning, waiting: w.__fwWaiting };
    });
    assert.ok(progress.waiting, "steps not reached yet are shown as waiting");
    assert.ok(
      progress.running.length >= 3 && progress.running.includes("tap_hold"),
      `the running step moves as the tests stream: ${progress.running.join(", ")}`,
    );
    const steps = await browser.execute(() =>
      [...document.querySelectorAll('[data-testid="fw-build-steps"] li')].map((li) => li.dataset.step),
    );
    assert.deepEqual(steps, ["image", "keys", "fn", "layers", "tap_hold", "info", "stack", "updater"]);
    await click(browser, "fw-flash");
    const flash = await byTest(browser, "fw-flash-dialog");
    await flash.waitForDisplayed({ timeout: 10000 });
    await waitIdle(browser);
    await click(browser, "fw-flash-launch");
    await (await byTest(browser, "fw-flash-waiting")).waitForDisplayed({ timeout: 5000 });
    // The simulated updater flashed the build and unplugged the keyboard; it comes back after "Start".
    await sim(browser, "assist_off");
    await sim(browser, "replug");
    await (await byTest(browser, "fw-flash-result")).waitForDisplayed({ timeout: 15000 });
    assert.match(await text(browser, "fw-flash-result"), /HJKL_CapsSpace2_Info_2/);
    await click(browser, "fw-flash-close");
    await waitIdle(browser);
    assert.match(await text(browser, "fw-line"), /HJKL_CapsSpace2_Info_2/);
    assert.doesNotMatch(await text(browser, "fw-tab"), /\d/);
  });
});
