// Performance budgets of PLAN.md section 8, measured on the e2e-sim release build. Prints a JSON report and
// writes it to tests/e2e/out/perf.json. Usage: node tests/e2e/perf.mjs
import { execFileSync, spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { APP, click, openApp, ROOT, waitIdle, waitState } from "./driver.mjs";

const OUT = path.join(ROOT, "tests", "e2e", "out");
fs.mkdirSync(OUT, { recursive: true });

const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];
const p95 = (xs) => [...xs].sort((a, b) => a - b)[Math.min(xs.length - 1, Math.ceil(xs.length * 0.95) - 1)];
const round = (x) => Math.round(x * 10) / 10;

/**
 * Startup without WebDriver in between: the app is started directly and reports, through the e2e_startup command,
 * when `main` began and when the page reached its marks. Median of `runs` cold-ish starts.
 */
async function startup(runs = 10) {
  const rows = [];
  for (let i = 0; i < runs; i++) {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "kle-start-"));
    const file = path.join(dir, "startup.json");
    const t0 = Date.now();
    const child = spawn(APP, [], {
      env: { ...process.env, KLE_E2E_DIR: dir, KLE_E2E_UNPLUGGED: "1", KLE_E2E_STARTUP_FILE: file },
      stdio: "ignore",
    });
    const timeout = setTimeout(() => child.kill(), 20000);
    await new Promise((r) => child.on("exit", r));
    clearTimeout(timeout);
    const m = JSON.parse(fs.readFileSync(file, "utf8"));
    rows.push({
      spawn_to_main: m.main_epoch_ms - t0,
      webview: m.page_origin_epoch_ms - m.main_epoch_ms,
      app: m.ready_ms,
      first_frame: m.page_origin_epoch_ms + m.ready_ms - t0,
      interactive: m.page_origin_epoch_ms + m.interactive_ms - t0,
    });
    fs.rmSync(dir, { recursive: true, force: true });
  }
  const med = (k) => round(median(rows.map((r) => r[k])));
  return {
    first_frame_ms: med("first_frame"),
    interactive_ms: med("interactive"),
    spawn_to_main_ms: med("spawn_to_main"),
    tauri_webview_until_page_ms: med("webview"),
    app_part_ms: med("app"),
    runs,
  };
}

/** Time of a full read of 4 layers with a given pause per report, from plugging in to the layers on screen. */
async function readTime(paceMs) {
  const { browser, close } = await openApp({ KLE_E2E_UNPLUGGED: "1", KLE_E2E_PACE_MS: String(paceMs) });
  try {
    await waitState(browser, "no_device");
    const ms = await browser.execute(async () => {
      const t = performance.now();
      await window.__kleSim("replug");
      await new Promise((r) => requestAnimationFrame(() => r(undefined)));
      return performance.now() - t;
    });
    await waitState(browser, "ready");
    return round(ms);
  } finally {
    await close();
  }
}

function processMemory(rootPid) {
  const script = `
    $all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId, Name, PrivatePageCount, CommandLine
    $ids = [System.Collections.Generic.HashSet[int]]::new(); [void]$ids.Add(${rootPid})
    do { $n = $ids.Count; foreach ($p in $all) { if ($ids.Contains([int]$p.ParentProcessId)) { [void]$ids.Add([int]$p.ProcessId) } } } while ($ids.Count -ne $n)
    $all | Where-Object { $ids.Contains([int]$_.ProcessId) } | ForEach-Object {
      $t = if ($_.CommandLine -match '--type=([a-z-]+)') { $matches[1] } else { 'main' }
      "$($_.Name)|$t|$($_.PrivatePageCount)" }`;
  const out = execFileSync("pwsh", ["-NoProfile", "-Command", script], { encoding: "utf8" });
  let app = 0;
  let total = 0;
  const processes = [];
  for (const line of out.trim().split(/\r?\n/)) {
    const [name, type, bytes] = line.split("|");
    const b = Number(bytes);
    // conhost.exe appears only because the test harness gives the app console handles; not part of the app.
    if (name === "conhost.exe") continue;
    total += b;
    if (name === "keyboard-layer-editor.exe") app += b;
    processes.push(`${name} ${type}: ${round(b / 2 ** 20)} MB`);
  }
  return { rust_process_mb: round(app / 2 ** 20), total_mb: round(total / 2 ** 20), processes };
}

function appPid() {
  const out = execFileSync(
    "pwsh",
    [
      "-NoProfile",
      "-Command",
      "(Get-Process keyboard-layer-editor | Sort-Object StartTime | Select-Object -Last 1).Id",
    ],
    { encoding: "utf8" },
  );
  return Number(out.trim());
}

async function interaction() {
  const { browser, close } = await openApp();
  try {
    await waitState(browser, "ready");
    // Click on a key: time until the DOM shows it selected and the layout is done (the work that must fit in one
    // frame), and separately until the next animation frame (what the eye sees). 200 times over different keys.
    const measure = (selector, attr) =>
      browser.execute(
        async (sel, a) => {
          const els = [...document.querySelectorAll(sel)];
          const work = [];
          const frame = [];
          for (let i = 0; i < 200; i++) {
            const el = els[(i * 7) % els.length];
            const t = performance.now();
            el.dispatchEvent(new MouseEvent("click", { bubbles: true }));
            for (let n = 0; el.getAttribute(a) !== "true" && n < 10000; n++) await Promise.resolve();
            if (el.getAttribute(a) !== "true") throw new Error("no update");
            document.body.getBoundingClientRect(); // force style and layout
            work.push(performance.now() - t);
            await new Promise((r) => requestAnimationFrame(() => r(undefined)));
            frame.push(performance.now() - t);
          }
          return { work, frame };
        },
        selector,
        attr,
      );
    const select = await measure('[data-testid^="key-"]', "aria-pressed");
    const layers = await measure('[data-testid^="layer-"]', "aria-selected");
    // Assign an action and wait until the key shows it (IPC to Rust, new snapshot, redraw): 100 times.
    await click(browser, "layer-2");
    await click(browser, "key-M1");
    const assign = await browser.execute(async () => {
      const hex = () => document.querySelector('[data-testid="slot-hex"]')?.textContent;
      const app = document.querySelector('[data-testid="app"]');
      const times = [];
      for (let i = 0; i < 100; i++) {
        const code = i % 2 ? "68" : "69";
        const button = document.querySelector(`[data-testid="usage-${code}"]`);
        const t = performance.now();
        button.dispatchEvent(new MouseEvent("click", { bubbles: true }));
        while (hex() !== `02 00 ${code} 00` || app.getAttribute("data-busy") !== "") {
          await new Promise((r) => requestAnimationFrame(() => r(undefined)));
        }
        times.push(performance.now() - t);
      }
      return times;
    });
    await waitIdle(browser);
    const mem = processMemory(appPid());
    return {
      click_select_p95_ms: round(p95(select.work)),
      click_select_next_frame_p95_ms: round(p95(select.frame)),
      layer_switch_p95_ms: round(p95(layers.work)),
      layer_switch_next_frame_p95_ms: round(p95(layers.frame)),
      assign_redraw_p95_ms: round(p95(assign)),
      assign_redraw_median_ms: round(median(assign)),
      ...mem,
    };
  } finally {
    await close();
  }
}

const report = {
  startup: await startup(),
  interaction: await interaction(),
  read_4_layers_100ms_ms: await readTime(100),
  read_4_layers_15ms_ms: await readTime(15),
};
fs.writeFileSync(path.join(OUT, "perf.json"), JSON.stringify(report, null, 2) + "\n");
console.log(JSON.stringify(report, null, 2));
