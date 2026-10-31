<script lang="ts">
  import { onMount } from "svelte";
  import type { Action, BackupView, ErrorView } from "./bindings/ipc";
  import BackupsDialog from "./components/BackupsDialog.svelte";
  import DiffDialog from "./components/DiffDialog.svelte";
  import Inspector from "./components/Inspector.svelte";
  import Keyboard from "./components/Keyboard.svelte";
  import Menu from "./components/Menu.svelte";
  import Modal from "./components/Modal.svelte";
  import Progress from "./components/Progress.svelte";
  import ReportDialog from "./components/ReportDialog.svelte";
  import { hasKey, PRODUCT_NAME, systemLang } from "./lib/i18n";
  import { ipc, onDevicesChanged, onProgress } from "./lib/ipc";
  import { applyTheme, store, type Theme } from "./lib/store.svelte";

  const t = store.t;
  const snap = $derived(store.snap);
  const model = $derived(store.model);
  const phase = $derived(snap?.phase ?? "no_device");
  const ready = $derived(phase === "ready" && model !== null && (snap?.layers.length ?? 0) > 0);
  const slots = $derived(snap?.layers[store.layer] ?? []);
  const selKey = $derived(store.selected !== null ? (model?.keys[store.selected] ?? null) : null);
  const selSlot = $derived(store.selected !== null ? (slots[store.selected] ?? null) : null);
  const assistBlocked = $derived((snap?.assist.running.length ?? 0) > 0 || snap?.assist.probe_error != null);
  const nChanges = $derived(snap?.changes.length ?? 0);
  const linkedHere = $derived(snap?.assist.linked.filter((l) => l.layer === store.layer) ?? []);
  let backups = $state<BackupView[]>([]);
  let confirmRead = $state(false);

  // What the keyboard holds for the selected slot (for "edited, keyboard has ...").
  const baseSlot = $derived.by(() => {
    if (!selKey || !snap) return null;
    const c = snap.changes.find((c) => c.layer === store.layer && c.index === selKey.index);
    return c ? c.old : null;
  });

  function errorText(e: ErrorView): string {
    const key = `err_${e.code}`;
    return hasKey(key) ? t(key) : t("err_unknown");
  }

  function assign(layers: number[], action: Action) {
    if (!selKey) return;
    const index = selKey.index;
    void store.run("edit", () => ipc.assign(layers, index, action));
  }

  function save() {
    if (!ready || nChanges === 0 || assistBlocked || store.busy) return;
    void store.run("prepare", ipc.prepareWrite);
  }

  function readAgain() {
    if (nChanges > 0) confirmRead = true;
    else void store.run("read", ipc.read);
  }

  async function openBackups() {
    backups = await ipc.backups();
    store.dialog = "backups";
  }

  function menuAction(close: () => void, f: () => void) {
    close();
    f();
  }

  function setLang(lang: "vi" | "en") {
    localStorage.setItem("lang", lang);
    void store.run("edit", () => ipc.setLang(lang));
  }

  function inField(e: KeyboardEvent): boolean {
    const el = e.target as HTMLElement | null;
    return !!el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable);
  }

  function onkeydown(e: KeyboardEvent) {
    if (store.dialog || confirmRead || !ready) return;
    const ctrl = e.ctrlKey || e.metaKey;
    if (ctrl && e.key.toLowerCase() === "s") {
      e.preventDefault();
      save();
    } else if (ctrl && !e.shiftKey && e.key.toLowerCase() === "z" && !inField(e)) {
      e.preventDefault();
      void store.run("edit", ipc.undo);
    } else if (
      ctrl &&
      (e.key.toLowerCase() === "y" || (e.shiftKey && e.key.toLowerCase() === "z")) &&
      !inField(e)
    ) {
      e.preventDefault();
      void store.run("edit", ipc.redo);
    } else if (!ctrl && !e.altKey && /^[1-9]$/.test(e.key) && !inField(e)) {
      const n = Number(e.key) - 1;
      if (model && n < model.layers.length) store.layer = n;
    } else if (e.key === "Delete" && !inField(e) && selKey && selSlot?.kind !== "empty") {
      assign([store.layer], { type: "empty" });
    } else if (e.key === "Escape" && !inField(e)) {
      store.selected = null;
    }
  }

  onMount(() => {
    const theme = (localStorage.getItem("theme") as Theme | null) ?? "system";
    store.theme = theme;
    applyTheme(theme);
    const lang = (localStorage.getItem("lang") as "vi" | "en" | null) ?? systemLang(navigator.languages);
    let refreshTimer: ReturnType<typeof setTimeout> | undefined;
    const unlisten = [
      onProgress((p) => (store.progress = p)),
      onDevicesChanged(() => {
        clearTimeout(refreshTimer);
        refreshTimer = setTimeout(() => void store.run("refresh", ipc.refresh), 300);
      }),
    ];
    void (async () => {
      await store.run("edit", () => ipc.setLang(lang));
      performance.mark("ui-ready");
      store.catalog = await ipc.catalog();
      await store.run("refresh", ipc.refresh);
      performance.mark("ui-interactive");
      if (store.snap?.simulated) {
        // Simulator controls for the E2E tests (the command exists only in the e2e-sim build).
        Object.assign(window, { __kleSim: (op: string) => store.run("refresh", () => ipc.e2eSim(op)) });
      }
    })();
    const poll = setInterval(() => void store.poll(), 2000);
    const onfocus = () => void store.poll();
    window.addEventListener("focus", onfocus);
    return () => {
      clearInterval(poll);
      clearTimeout(refreshTimer);
      window.removeEventListener("focus", onfocus);
      for (const u of unlisten) void u.then((f) => f());
    };
  });
</script>

<svelte:window {onkeydown} />

<div
  class="app l{store.layer}"
  class:idle={!ready}
  data-state={phase}
  data-busy={store.busy ?? ""}
  data-testid="app"
>
  <header class="top">
    <div class="ident">
      <p class="product small">
        {#if snap?.device || model}<b>{PRODUCT_NAME}</b> ·&nbsp;{/if}{t("subtitle")}
      </p>
      <div class="device">
        <h1 data-testid="device-name">{snap?.device?.name ?? model?.name ?? PRODUCT_NAME}</h1>
        {#if snap?.device}
          <span class="status small"
            ><span class="conn on"></span>{t("connected")} · {t("rev")} 0x{snap.device.version
              .toString(16)
              .padStart(4, "0")
              .toUpperCase()}</span
          >
        {:else}
          <span class="status small muted"><span class="conn"></span>{t("not_connected")}</span>
        {/if}
        {#if snap?.simulated}
          <span class="sim small" data-testid="simulated">{t("simulated")}</span>
        {/if}
      </div>
    </div>
    <div class="actions">
      {#if snap?.device}
        <button onclick={readAgain} disabled={!!store.busy || assistBlocked} data-testid="read-again"
          >{t("read_again")}</button
        >
      {/if}
      <Menu label={t("more")} testid="more-menu">
        {#snippet children(close)}
          <button
            disabled={!ready || assistBlocked}
            onclick={() =>
              menuAction(
                close,
                () =>
                  void store
                    .run("backup", ipc.backupNow)
                    .then(() => (store.notice = store.snap?.error ? null : t("backup_saved"))),
              )}>{t("backup_now")}</button
          >
          <button
            disabled={!ready}
            onclick={() => menuAction(close, () => void openBackups())}
            data-testid="menu-restore">{t("restore_backup")}</button
          >
          <button onclick={() => menuAction(close, () => void ipc.openBackupFolder())}
            >{t("open_backup_folder")}</button
          >
          <hr />
          <button
            disabled={!ready}
            onclick={() => menuAction(close, () => void store.run("edit", ipc.importFile))}
            >{t("import_file")}</button
          >
          <button
            disabled={!ready}
            onclick={() => menuAction(close, () => void store.run("edit", ipc.exportFile))}
            >{t("export_file")}</button
          >
          {#if ready && model}
            <hr />
            <div class="label">{t("copy_layer_to", { layer: model.layers[store.layer]?.label ?? "" })}</div>
            <div class="row">
              {#each model.layers as l, i (i)}
                <button
                  disabled={i === store.layer}
                  onclick={() =>
                    menuAction(close, () => void store.run("edit", () => ipc.copyLayer(store.layer, i)))}
                  >{l.label}</button
                >
              {/each}
            </div>
          {/if}
          <hr />
          <div class="label">{t("theme")}</div>
          <div class="row">
            {#each [["system", "theme_system"], ["light", "theme_light"], ["dark", "theme_dark"]] as const as [id, key] (id)}
              <button aria-pressed={store.theme === id} onclick={() => store.setTheme(id)}>{t(key)}</button>
            {/each}
          </div>
          <div class="label">{t("language")}</div>
          <div class="row">
            <button aria-pressed={store.lang === "vi"} onclick={() => setLang("vi")} data-testid="lang-vi"
              >Tiếng Việt</button
            >
            <button aria-pressed={store.lang === "en"} onclick={() => setLang("en")} data-testid="lang-en"
              >English</button
            >
          </div>
        {/snippet}
      </Menu>
    </div>
  </header>

  {#if ready && model && snap}
    <nav class="layers" aria-label="Layers">
      <div class="tabs" role="tablist">
        {#each model.layers as l, i (i)}
          {@const n = snap.changes.filter((c) => c.layer === i).length}
          <button
            role="tab"
            class="layer-tab l{i}"
            aria-selected={store.layer === i}
            class:active={store.layer === i}
            onclick={() => (store.layer = i)}
            title={t("led", { name: l.led_name })}
            data-testid={`layer-${i}`}
            ><span class="led" style:background={l.led} style:border-color={l.led === "#ffffff" ? "" : l.led}
            ></span>{t("layer", { n: l.label })}{#if n > 0}<span class="count">{n}</span>{/if}</button
          >
        {/each}
      </div>
      <div class="edit">
        <button
          class="ghost"
          disabled={!snap.can_undo || !!store.busy}
          onclick={() => void store.run("edit", ipc.undo)}
          title="Ctrl+Z"
          data-testid="undo">{t("undo")}</button
        >
        <button
          class="ghost"
          disabled={!snap.can_redo || !!store.busy}
          onclick={() => void store.run("edit", ipc.redo)}
          title="Ctrl+Y"
          data-testid="redo">{t("redo")}</button
        >
        <button
          class="ghost"
          disabled={nChanges === 0 || !!store.busy}
          onclick={() => void store.run("edit", ipc.revert)}
          data-testid="revert">{t("revert")}</button
        >
        <button
          class="primary"
          disabled={nChanges === 0 || assistBlocked || !!store.busy}
          onclick={save}
          title="Ctrl+S"
          data-testid="save">{nChanges > 0 ? t("save_n", { n: nChanges }) : t("save_none")}</button
        >
      </div>
    </nav>
  {/if}

  <div class="banners">
    {#if snap && snap.assist.running.length > 0}
      <div class="banner warn" data-testid="assist-banner">
        <div><b>{t("assist_running_title")}</b> {t("assist_running_text")}</div>
        <button onclick={() => void store.poll()}>{t("check_again")}</button>
      </div>
    {:else if snap?.assist.probe_error}
      <div class="banner warn">
        <div>{t("assist_probe_failed", { detail: snap.assist.probe_error })}</div>
      </div>
    {/if}
    {#if snap?.journal}
      <div class="banner bad" data-testid="journal-banner">
        <div>
          <b>{t("journal_title")}</b>
          {t("journal_text", { started: snap.journal.started })}
          {#each snap.journal.layers as jl (jl.layer)}
            <span class="jl"
              >{t("layer", { n: model?.layers[jl.layer]?.label ?? jl.layer + 1 })}: {t(
                `journal_state_${jl.state}` as "journal_state_original",
              )}</span
            >
          {/each}
        </div>
        <button onclick={() => void store.run("edit", ipc.restoreFromJournal)} data-testid="journal-restore"
          >{t("journal_restore")}</button
        >
        <button class="ghost" onclick={() => void store.run("edit", ipc.dismissJournal)}
          >{t("journal_keep")}</button
        >
      </div>
    {/if}
    {#if snap?.error && phase !== "read_failed"}
      <div class="banner bad" data-testid="error-banner" data-code={snap.error.code}>
        <div>
          <b>{errorText(snap.error)}</b>
          <span class="detail small muted">{snap.error.detail}</span>
        </div>
      </div>
    {/if}
    {#if store.ipcError}
      <div class="banner bad">
        <div><b>{t("err_unknown")}</b> <span class="small">{store.ipcError}</span></div>
      </div>
    {/if}
    {#if store.notice}
      <div class="banner good">
        <div>{store.notice}</div>
        <button class="ghost" onclick={() => (store.notice = null)}>{t("close")}</button>
      </div>
    {/if}
    {#if ready && linkedHere.length > 0}
      <div class="banner warn small">
        {t("linked_warning", {
          layers: model?.layers[store.layer]?.label ?? "",
          profiles: linkedHere.map((l) => `"${l.profile}"`).join(", "),
        })}
      </div>
    {/if}
    {#if ready && store.progress && (store.busy === "read" || store.busy === "backup" || store.busy === "prepare" || store.busy === "refresh")}
      <div class="banner plain"><Progress progress={store.progress} {t} /></div>
    {/if}
  </div>

  <main class="stage">
    {#if ready && model}
      <div class="kb-wrap">
        <Keyboard
          {model}
          {slots}
          selected={store.selected}
          label={t("keyboard_label", { n: model.layers[store.layer]?.label ?? "" })}
          onselect={(i) => (store.selected = i)}
        />
      </div>
    {:else}
      <div class="screen" data-testid={`screen-${phase}`}>
        {#if (store.busy === "read" || store.busy === "refresh") && store.progress}
          <h2>{t("reading", { n: (store.progress?.layer ?? 0) + 1, total: model?.layers.length ?? 4 })}</h2>
          <div class="screen-progress"><Progress progress={store.progress} {t} /></div>
        {:else if phase === "no_device"}
          <svg class="plug" viewBox="0 0 140 76" aria-hidden="true">
            <path class="cable" d="M70 26V14c0-6 4-10 10-10h14" />
            <rect x="8" y="26" width="124" height="46" rx="7" />
            {#each [0, 1, 2] as r (r)}
              {#each Array.from({ length: 10 - r }, (_, i) => i) as c (c)}
                <rect class="k" x={16 + r * 5 + c * 11} y={33 + r * 12} width="8" height="8" rx="2" />
              {/each}
            {/each}
          </svg>
          <h2>{t("no_device_title")}</h2>
          <p>{t("no_device_text")}</p>
          <button
            onclick={() => void store.run("refresh", ipc.refresh)}
            disabled={!!store.busy}
            data-testid="search-again">{t("search_again")}</button
          >
        {:else if phase === "unsupported"}
          <h2>{t("unsupported_title")}</h2>
          <p>{t("unsupported_text", { name: snap?.devices[0]?.name ?? "" })}</p>
          <button onclick={() => void store.run("refresh", ipc.refresh)} disabled={!!store.busy}
            >{t("search_again")}</button
          >
        {:else if phase === "connected"}
          <h2>{assistBlocked ? t("assist_running_title") : t("connected_title")}</h2>
          <p>{assistBlocked ? t("assist_running_text") : t("connected_text")}</p>
          <div class="buttons">
            {#if assistBlocked}
              <button onclick={() => void store.poll()}>{t("check_again")}</button>
            {/if}
            <button
              class="primary"
              onclick={() => void store.run("read", ipc.read)}
              disabled={!!store.busy || assistBlocked}
              data-testid="read">{t("read")}</button
            >
          </div>
        {:else if phase === "read_failed"}
          <h2>{t("read_failed_title")}</h2>
          {#if snap?.error}
            <p>{errorText(snap.error)} <span class="small muted">{snap.error.detail}</span></p>
          {/if}
          <button
            class="primary"
            onclick={() => void store.run("read", ipc.read)}
            disabled={!!store.busy || assistBlocked}>{t("try_again")}</button
          >
        {/if}
      </div>
    {/if}
  </main>

  {#if ready && model && store.catalog}
    <Inspector
      {model}
      keyView={selKey}
      slot={selSlot}
      base={baseSlot}
      layer={store.layer}
      catalog={store.catalog}
      disabled={!!store.busy}
      {t}
      onassign={assign}
    />
  {/if}
</div>

{#if snap?.plan && model}
  <DiffDialog
    plan={snap.plan}
    {model}
    writing={store.busy === "write"}
    progress={store.progress}
    {t}
    oncancel={() => void store.run("edit", ipc.cancelWrite)}
    onwrite={() => void store.run("write", ipc.executeWrite)}
  />
{:else if snap?.report && model}
  <ReportDialog report={snap.report} {model} {t} onclose={() => void store.run("edit", ipc.dismissReport)} />
{:else if store.dialog === "backups"}
  <BackupsDialog
    {backups}
    {t}
    onload={(path) => {
      store.dialog = null;
      void store.run("edit", () => ipc.loadBackup(path));
    }}
    onopenfolder={() => void ipc.openBackupFolder()}
    onpickfolder={() => {
      store.dialog = null;
      void store.run("edit", ipc.loadBackupFolder);
    }}
    onclose={() => (store.dialog = null)}
  />
{/if}

{#if confirmRead}
  <Modal title={t("read_again")} testid="confirm-read" onclose={() => (confirmRead = false)}>
    <p>{t("confirm_discard", { n: nChanges })}</p>
    {#snippet footer()}
      <button onclick={() => (confirmRead = false)}>{t("cancel")}</button>
      <button
        class="primary"
        onclick={() => {
          confirmRead = false;
          void store.run("read", ipc.read);
        }}>{t("read_again")}</button
      >
    {/snippet}
  </Modal>
{/if}

<style>
  .app {
    height: 100%;
    display: grid;
    grid-template-rows: auto auto auto minmax(0, auto) minmax(0, 1fr);
    max-width: 1440px;
    margin: 0 auto;
  }
  .app.idle {
    grid-template-rows: auto auto auto minmax(0, 1fr);
  }
  .top {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    padding: 14px 20px 10px;
  }
  .product {
    margin: 0 0 2px;
    opacity: 0.62;
    font-size: 12.5px;
  }
  .product b {
    font-weight: 600;
  }
  .device {
    display: flex;
    align-items: baseline;
    gap: 14px;
    flex-wrap: wrap;
  }
  h1 {
    font: 600 24px/1.2 var(--font-key);
    letter-spacing: 0.01em;
  }
  .status {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-family: var(--font-key);
  }
  .conn {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--faint);
  }
  .conn.on {
    background: var(--good);
  }
  .sim {
    font: 600 12px var(--font-key);
    border: 1px dashed var(--warn);
    color: var(--warn);
    padding: 1px 7px;
    border-radius: 5px;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  .layers {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 0 20px 10px;
    border-bottom: 1px solid var(--line);
  }
  .tabs {
    display: flex;
    gap: 8px;
  }
  .layer-tab {
    padding: 5px 12px;
    font-size: 14px;
  }
  .layer-tab.active {
    border-color: var(--acc);
    box-shadow: inset 0 0 0 1px var(--acc);
    color: var(--acc);
    font-weight: 600;
  }
  .count {
    font: 600 11.5px/1 var(--font-key);
    background: var(--acc);
    color: var(--paper);
    border-radius: 8px;
    padding: 2px 6px;
  }
  .edit {
    display: flex;
    gap: 6px;
  }
  .banners {
    display: grid;
    gap: 6px;
    padding: 0 20px;
  }
  .banners:not(:empty) {
    padding-top: 10px;
  }
  .banner {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border-radius: 6px;
    font-size: 13.5px;
  }
  .banner > div {
    flex: 1;
  }
  .banner.warn {
    background: var(--warn-bg);
    border-left: 3px solid var(--warn);
  }
  .banner.bad {
    background: var(--bad-bg);
    border-left: 3px solid var(--bad);
  }
  .banner.good {
    border: 1px solid var(--good);
    border-left-width: 3px;
  }
  .banner.plain {
    padding: 4px 0;
  }
  .banner b {
    font-weight: 600;
    margin-right: 4px;
  }
  .detail {
    display: block;
    word-break: break-word;
  }
  .jl {
    display: inline-block;
    margin-right: 12px;
    font-weight: 500;
  }
  .stage {
    min-height: 0;
    display: flex;
  }
  .kb-wrap {
    flex: 1;
    padding: 14px 20px 12px;
    aspect-ratio: 1250 / 360;
    max-height: 46vh;
    margin: 0 auto;
    width: 100%;
  }
  .screen {
    margin: auto;
    padding: 48px 20px;
    max-width: 560px;
    text-align: center;
    display: grid;
    gap: 12px;
    justify-items: center;
  }
  .screen h2 {
    font: 600 20px var(--font-key);
  }
  .screen p {
    margin: 0;
  }
  .screen .buttons {
    display: flex;
    gap: 8px;
  }
  .screen-progress {
    width: 320px;
  }
  .plug {
    width: 150px;
    fill: none;
    stroke: var(--line);
    stroke-width: 1.3;
    margin-bottom: 6px;
  }
  .plug .k {
    stroke-width: 1;
  }
  .plug .cable {
    stroke-dasharray: 4 3;
  }
</style>
