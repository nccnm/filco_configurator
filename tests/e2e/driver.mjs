// Starts tauri-driver (which starts msedgedriver and the e2e-sim build of the app) and opens a WebDriver session.
// Machine setup (not in the repo): tauri-driver from `cargo install tauri-driver --locked`, msedgedriver of the same
// major version as the installed WebView2 runtime, path in MSEDGEDRIVER or %LOCALAPPDATA%\kle-tools\edgedriver-*.
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import { fileURLToPath } from "node:url";
import { remote } from "webdriverio";

export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
export const APP =
  process.env.KLE_E2E_APP ?? path.join(ROOT, "target", "e2e", "release", "keyboard-layer-editor.exe");

function edgeDriver() {
  if (process.env.MSEDGEDRIVER) return process.env.MSEDGEDRIVER;
  const tools = path.join(process.env.LOCALAPPDATA ?? "", "kle-tools");
  const dirs = fs.existsSync(tools) ? fs.readdirSync(tools).filter((d) => d.startsWith("edgedriver-")) : [];
  dirs.sort();
  const last = dirs.at(-1);
  if (!last) throw new Error("msedgedriver not found: set MSEDGEDRIVER");
  return path.join(tools, last, "msedgedriver.exe");
}

let port = 4444;

/**
 * Opens the app. `env` is passed to the app (through tauri-driver), e.g. KLE_E2E_UNPLUGGED=1.
 * Each session gets its own empty data folder, so backups and journals never leak between tests.
 */
export async function openApp(env = {}) {
  const dataDir = fs.mkdtempSync(path.join(os.tmpdir(), "kle-e2e-"));
  const myPort = port++;
  const nativePort = myPort + 100;
  const driver = spawn(
    "tauri-driver",
    ["--port", String(myPort), "--native-port", String(nativePort), "--native-driver", edgeDriver()],
    { env: { ...process.env, KLE_E2E_DIR: dataDir, ...env }, stdio: ["ignore", "ignore", "pipe"] },
  );
  let stderr = "";
  driver.stderr.on("data", (d) => (stderr += d));
  for (let i = 0; i < 50; i++) {
    try {
      await fetch(`http://127.0.0.1:${myPort}/status`);
      break;
    } catch {
      await sleep(100);
    }
  }
  const spawned = performance.now();
  let browser;
  try {
    browser = await remote({
      hostname: "127.0.0.1",
      port: myPort,
      logLevel: "error",
      capabilities: { browserName: "wry", "tauri:options": { application: APP } },
    });
  } catch (e) {
    driver.kill();
    throw new Error(`could not start the app\n${stderr}`, { cause: e });
  }
  const close = async () => {
    try {
      await browser.deleteSession();
    } catch {
      /* already gone */
    }
    driver.kill();
    fs.rmSync(dataDir, { recursive: true, force: true });
  };
  return { browser, close, dataDir, spawned };
}

/** Waits until the app reports `state` (data-state of the root) and is not busy. */
export async function waitState(browser, state, timeout = 15000) {
  await browser.waitUntil(
    async () =>
      browser.execute((s) => {
        const el = document.querySelector('[data-testid="app"]');
        return !!el && el.getAttribute("data-state") === s && el.getAttribute("data-busy") === "";
      }, state),
    { timeout, interval: 25, timeoutMsg: `state ${state} not reached` },
  );
}

export async function waitIdle(browser, timeout = 15000) {
  await browser.waitUntil(
    async () =>
      browser.execute(() => document.querySelector('[data-testid="app"]')?.getAttribute("data-busy") === ""),
    { timeout, interval: 20, timeoutMsg: "app stayed busy" },
  );
}

export const byTest = (browser, id) => browser.$(`[data-testid="${id}"]`);

export async function click(browser, id) {
  const el = await byTest(browser, id);
  await el.waitForClickable({ timeout: 5000 });
  await el.click();
}

/** Simulator controls of the e2e-sim build. */
export async function sim(browser, op) {
  await browser.execute(async (o) => {
    await window.__kleSim(o);
  }, op);
}

export async function setTheme(browser, theme) {
  await browser.execute((t) => {
    localStorage.setItem("theme", t);
    if (t === "system") delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = t;
  }, theme);
}
