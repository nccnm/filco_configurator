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

  it("edits, undoes, redoes and discards", async () => {
    const { browser } = app;
    const before = await select(browser, 2, "M1");
    await click(browser, "usage-68"); // F13
    await waitIdle(browser);
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

  it("rejects bytes the editor does not write (macro)", async () => {
    const { browser } = app;
    await select(browser, 0, "Q");
    await click(browser, "tab-advanced");
    const input = await byTest(browser, "raw-input");
    await input.setValue("06 01 00 00");
    await click(browser, "raw-assign");
    await waitIdle(browser);
    assert.equal(await attr(browser, '[data-testid="error-banner"]', "data-code"), "edit_invalid");
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
