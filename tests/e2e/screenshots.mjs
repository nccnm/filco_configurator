// Captures every screen and state of the app (simulated keyboard) in light and dark mode into docs/screenshots/,
// for review by eye. Usage: node tests/e2e/screenshots.mjs [light|dark ...]
import fs from "node:fs";
import path from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import { click, openApp, ROOT, setTheme, sim, waitIdle, waitState } from "./driver.mjs";

const OUT = path.join(ROOT, "docs", "screenshots");
fs.mkdirSync(OUT, { recursive: true });

/** Screens are reviewed in Vietnamese (the user's language); one English screenshot is taken at the end. */
async function vietnamese(browser) {
  await browser.execute(() => localStorage.setItem("lang", "vi"));
  await browser.refresh();
}

async function shot(browser, theme, name) {
  await sleep(120); // let transitions and fonts settle
  const file = path.join(OUT, `${name}-${theme}.png`);
  await browser.saveScreenshot(file);
  console.log("saved", path.relative(ROOT, file));
}

async function run(theme) {
  // 1. No keyboard.
  {
    const { browser, close } = await openApp({ KLE_E2E_UNPLUGGED: "1" });
    await setTheme(browser, theme);
    await vietnamese(browser);
    await waitState(browser, "no_device");
    await shot(browser, theme, "01-no-device");
    // 2. Reading (slow simulator so the progress is visible).
    await close();
  }
  {
    const { browser, close } = await openApp({ KLE_E2E_PACE_MS: "40", KLE_E2E_UNPLUGGED: "1" });
    await setTheme(browser, theme);
    await vietnamese(browser);
    await waitState(browser, "no_device");
    await browser.execute(() => {
      void window.__kleSim("replug");
    });
    await browser.waitUntil(
      () =>
        browser.execute(() => {
          const v = document.querySelector('[role="progressbar"]')?.getAttribute("aria-valuenow");
          return v !== null && v !== undefined && Number(v) >= 30;
        }),
      { timeout: 10000, interval: 10 },
    );
    await shot(browser, theme, "02-reading");
    await close();
  }
  const { browser, close } = await openApp();
  try {
    await setTheme(browser, theme);
    await vietnamese(browser);
    await waitState(browser, "ready");
    await shot(browser, theme, "03-ready-layer1");
    for (const l of [2, 3]) {
      await click(browser, `layer-${l}`);
      await shot(browser, theme, `04-layer${l + 1}`);
    }
    // Select a key and walk the action tabs.
    await click(browser, "layer-2");
    await click(browser, "key-P");
    await shot(browser, theme, "05-key-selected");
    for (const tab of ["modifier", "media", "mouse", "system", "advanced"]) {
      await click(browser, `tab-${tab}`);
      await shot(browser, theme, `06-tab-${tab}`);
    }
    // Edit: M1 on layer 3 = F13, P = Mute.
    await click(browser, "tab-key");
    await click(browser, "key-M1");
    await click(browser, "usage-68");
    await waitIdle(browser);
    await click(browser, "key-P");
    await click(browser, "tab-media");
    await (await browser.$("button=Mute")).click();
    await waitIdle(browser);
    await shot(browser, theme, "07-edited");
    // Diff dialog.
    await click(browser, "save");
    await (await browser.$('[data-testid="diff-dialog"]')).waitForDisplayed({ timeout: 10000 });
    await waitIdle(browser);
    await shot(browser, theme, "08-diff");
    // Writing (slow it down so the progress shows), then the report.
    await click(browser, "diff-write");
    await sleep(150);
    await shot(browser, theme, "09-writing");
    await (await browser.$('[data-testid="report-dialog"]')).waitForDisplayed({ timeout: 20000 });
    await shot(browser, theme, "10-written");
    await click(browser, "report-close");
    await waitIdle(browser);

    // A failing write: the second layer's finish is not acknowledged, both layers are put back.
    await click(browser, "key-M2");
    await click(browser, "tab-key");
    await click(browser, "usage-69");
    await waitIdle(browser);
    await click(browser, "layer-3");
    await click(browser, "key-M2");
    await click(browser, "usage-6a");
    await waitIdle(browser);
    await click(browser, "save");
    await (await browser.$('[data-testid="diff-dialog"]')).waitForDisplayed({ timeout: 10000 });
    await waitIdle(browser);
    await sim(
      browser,
      `fault_next:${JSON.stringify({ cmd: 17, nth: 1, stage: "finish", kind: "wrong_ack" })}`,
    );
    await click(browser, "diff-write");
    await (await browser.$('[data-testid="report-dialog"]')).waitForDisplayed({ timeout: 20000 });
    await shot(browser, theme, "11-write-failed");
    await click(browser, "report-close");
    await waitIdle(browser);

    // FILCO Assist running.
    await sim(browser, "assist_on");
    await waitIdle(browser);
    await shot(browser, theme, "12-assist-running");
    await sim(browser, "assist_off");
    await waitIdle(browser);

    // Backups.
    await click(browser, "more-menu");
    await shot(browser, theme, "13-menu");
    await click(browser, "menu-restore");
    await (await browser.$('[data-testid="backups-dialog"]')).waitForDisplayed({ timeout: 5000 });
    await shot(browser, theme, "14-backups");
    await browser.keys("Escape");

    // English.
    await click(browser, "more-menu");
    await click(browser, "lang-en");
    await waitIdle(browser);
    await click(browser, "key-SpaceL");
    await shot(browser, theme, "15-english");
    await click(browser, "more-menu");
    await click(browser, "lang-vi");
    await waitIdle(browser);

    // Unplugged while open.
    await sim(browser, "unplug");
    await waitIdle(browser);
    await shot(browser, theme, "16-unplugged");
  } finally {
    await close();
  }
}

const themes = process.argv.slice(2);
for (const theme of themes.length ? themes : ["light", "dark"]) await run(theme);
