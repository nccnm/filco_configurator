// Pixel regression of the main screens against tests/e2e/baseline/*.png (made by this test on this machine, then
// reviewed by eye). These only catch unintended change; docs/screenshots/ is what is reviewed for correctness.
// Run: node --test tests/e2e/visual.test.mjs     Update baselines: $env:UPDATE_BASELINE=1 (then review the diff).
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { after, before, describe, it } from "node:test";
import pixelmatch from "pixelmatch";
import { PNG } from "pngjs";
import { byTest, click, openApp, ROOT, setTheme, waitIdle, waitState } from "./driver.mjs";

const BASE = path.join(ROOT, "tests", "e2e", "baseline");
const OUT = path.join(ROOT, "tests", "e2e", "out");
const UPDATE = !!process.env.UPDATE_BASELINE;
/** Share of pixels allowed to differ (anti-aliasing noise), and the per-pixel colour threshold. */
const MAX_DIFF = 0.001;

async function check(browser, name) {
  // Nothing that changes from run to run: hide the backup path (it has a timestamp) and the caret.
  await browser.execute(() => {
    if (!document.getElementById("visual-test")) {
      const s = document.createElement("style");
      s.id = "visual-test";
      s.textContent =
        "code.path { visibility: hidden !important; } * { caret-color: transparent !important; }";
      document.head.append(s);
    }
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
  });
  await new Promise((r) => setTimeout(r, 150));
  const png = Buffer.from(await browser.takeScreenshot(), "base64");
  const file = path.join(BASE, `${name}.png`);
  if (UPDATE || !fs.existsSync(file)) {
    fs.mkdirSync(BASE, { recursive: true });
    fs.writeFileSync(file, png);
    return;
  }
  const a = PNG.sync.read(fs.readFileSync(file));
  const b = PNG.sync.read(png);
  assert.equal(`${b.width}x${b.height}`, `${a.width}x${a.height}`, `${name}: size changed`);
  const diff = new PNG({ width: a.width, height: a.height });
  const n = pixelmatch(a.data, b.data, diff.data, a.width, a.height, { threshold: 0.1 });
  if (n / (a.width * a.height) > MAX_DIFF) {
    fs.mkdirSync(OUT, { recursive: true });
    fs.writeFileSync(path.join(OUT, `${name}-actual.png`), png);
    fs.writeFileSync(path.join(OUT, `${name}-diff.png`), PNG.sync.write(diff));
    assert.fail(`${name}: ${n} pixels differ (see tests/e2e/out/${name}-diff.png)`);
  }
}

async function vietnamese(browser) {
  await browser.execute(() => localStorage.setItem("lang", "vi"));
  await browser.refresh();
}

describe("visual baselines", () => {
  describe("no keyboard", () => {
    let app;
    before(async () => {
      app = await openApp({ KLE_E2E_UNPLUGGED: "1" });
      await vietnamese(app.browser);
    });
    after(async () => app?.close());

    for (const theme of ["light", "dark"]) {
      it(`no-device ${theme}`, async () => {
        await setTheme(app.browser, theme);
        await waitState(app.browser, "no_device");
        await check(app.browser, `no-device-${theme}`);
      });
    }
  });

  describe("simulated M10SP", () => {
    let app;
    before(async () => {
      app = await openApp();
      await vietnamese(app.browser);
      await waitState(app.browser, "ready");
    });
    after(async () => app?.close());

    for (const theme of ["light", "dark"]) {
      it(`ready ${theme}`, async () => {
        await setTheme(app.browser, theme);
        await check(app.browser, `ready-${theme}`);
      });
    }

    it("key selected, layer 4", async () => {
      const { browser } = app;
      await setTheme(browser, "light");
      await click(browser, "layer-3");
      await click(browser, "key-P");
      await check(browser, "key-selected-layer4-light");
    });

    it("edited keys and the diff dialog", async () => {
      const { browser } = app;
      await click(browser, "layer-2");
      await click(browser, "key-M1");
      await click(browser, "usage-68");
      await waitIdle(browser);
      await check(browser, "edited-light");
      await click(browser, "save");
      await (await byTest(browser, "diff-dialog")).waitForDisplayed({ timeout: 10000 });
      await waitIdle(browser);
      await check(browser, "diff-light");
      await click(browser, "diff-cancel");
      await waitIdle(browser);
    });
  });
});
