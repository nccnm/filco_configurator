<script lang="ts">
  import type { FwView } from "../bindings/ipc";
  import type { Key } from "../lib/i18n";
  import Modal from "./Modal.svelte";

  interface Props {
    fw: FwView;
    busy: boolean;
    t: (key: Key, vars?: Record<string, string | number>) => string;
    onbuild: (name: string) => void;
    onflash: () => void;
    onopenfolder: (dir: string) => void;
    onclose: () => void;
  }
  let { fw, busy, t, onbuild, onflash, onopenfolder, onclose }: Props = $props();

  // Seeded once from the suggestion; the user may change it.
  // svelte-ignore state_referenced_locally
  let name = $state(fw.suggested_name);
  let showReport = $state(false);
  const valid = $derived(/^[A-Za-z0-9_-]{1,32}$/.test(name));
  const b = $derived(fw.build);
  const tag = $derived(b?.tag != null ? b.tag.toString(16).toUpperCase().padStart(4, "0") : "");

  // Every step is listed from the start. While building, the report streams in: a step is running from its
  // section title until the next one appears ("image" is done as soon as it appears, the tests follow).
  const STEP_IDS = ["image", "keys", "fn", "layers", "tap_hold", "info", "stack", "updater"] as const;
  type RowState = "done" | "running" | "waiting" | "skipped";
  const rows = $derived.by(() => {
    if (!b) return [];
    const seen = STEP_IDS.map((id) => b.steps.find((s) => s.id === id));
    const last = seen.reduce((at, s, i) => (s ? i : at), -1);
    const current = last < 0 ? 0 : STEP_IDS[last] === "image" ? last + 1 : last;
    return STEP_IDS.map((id, i) => {
      const s = seen[i];
      const state: RowState = b.running
        ? i < current
          ? "done"
          : i === current
            ? "running"
            : "waiting"
        : s
          ? "done"
          : "skipped";
      return { id, ok: s?.ok ?? 0, fail: s?.fail ?? 0, state };
    });
  });
</script>

<Modal title={t("fw_build_title")} testid="fw-build-dialog" wide onclose={b?.running ? undefined : onclose}>
  {#if !b}
    <label class="field">
      <span>{t("fw_build_name")}</span>
      <input
        type="text"
        class="mono"
        bind:value={name}
        maxlength="32"
        spellcheck="false"
        data-testid="fw-build-name"
      />
      <small class="muted">{t("fw_build_name_hint")}</small>
    </label>
    <p class="small muted">{t("fw_build_info")}</p>
  {:else}
    <p class="name"><b>{b.name}</b></p>
    {#if b.running}
      <p class="small muted" data-testid="fw-build-running">{t("fw_build_running")}</p>
    {/if}
    {#if b.error}
      <p class="bad"><b>{t("fw_build_error")}</b> <span class="small">{b.error}</span></p>
    {/if}
    <ol class="steps" data-testid="fw-build-steps">
      {#each rows as s (s.id)}
        <li class:fail={s.fail > 0} class={s.state} data-step={s.id} data-state={s.state}>
          <span>{t(`fw_step_${s.id}` as Key)}</span>
          <span class="count">
            {#if s.fail > 0}{t("fw_step_fail", {
                n: s.fail,
              })}{:else if s.state === "running"}{#if s.ok > 0}{t("fw_step_ok", { n: s.ok })}{/if}<span
                class="spinner"
                aria-label={t("fw_step_running")}
              ></span>{:else if s.state === "waiting"}{t(
                "fw_step_waiting",
              )}{:else if s.state === "skipped"}{t("fw_step_skipped")}{:else if s.ok > 1}{t("fw_step_ok", {
                n: s.ok,
              })}{:else}OK{/if}
          </span>
        </li>
      {/each}
    </ol>
    {#if b.passed === true}
      <div class="result good" data-testid="fw-build-passed">
        <b>{t("fw_build_passed")}</b>
        <div class="tag">{t("fw_checksum")} <span class="mono">0x{tag}</span></div>
        <p class="small">{t("fw_checksum_note")}</p>
        {#if b.stale}<p class="small warn">{t("fw_stale")}</p>{/if}
      </div>
    {:else if b.passed === false && !b.error}
      <div class="result bad" data-testid="fw-build-failed">
        <b>{t("fw_build_failed")}</b>
        <p class="small">{t("fw_build_failed_text")}</p>
        <pre class="lines">{b.failures.join("\n")}</pre>
      </div>
    {/if}
    {#if showReport && b.report.length}
      <pre class="report" data-testid="fw-report">{b.report.join("\n")}</pre>
    {/if}
  {/if}
  {#snippet footer()}
    {#if !b}
      <button onclick={onclose}>{t("cancel")}</button>
      <button
        class="primary"
        disabled={!valid || busy}
        onclick={() => onbuild(name)}
        data-testid="fw-build-start">{t("fw_build_start")}</button
      >
    {:else if b.running}
      <button onclick={onclose}>{t("cancel")}</button>
    {:else}
      {#if b.report.length}
        <button class="ghost" onclick={() => (showReport = !showReport)}
          >{t(showReport ? "fw_hide_report" : "fw_show_report")}</button
        >
      {/if}
      {#if b.dir}
        <button onclick={() => onopenfolder(b.dir ?? "")}>{t("fw_open_folder")}</button>
      {/if}
      <span class="sp"></span>
      <button onclick={onclose}>{t("close")}</button>
      {#if b.passed && b.has_updater}
        <button class="primary" disabled={b.stale || busy} onclick={onflash} data-testid="fw-flash"
          >{t("fw_flash_button")}</button
        >
      {/if}
    {/if}
  {/snippet}
</Modal>

<style>
  .field {
    display: grid;
    gap: 4px;
    margin-bottom: 10px;
  }
  .field span {
    font: 500 13.5px var(--font-key);
  }
  .field input {
    width: 100%;
    height: 36px;
  }
  .name {
    margin: 0 0 6px;
    font: 600 15px var(--font-mono);
  }
  .steps {
    list-style: none;
    margin: 0 0 12px;
    padding: 0;
    display: grid;
    gap: 2px;
  }
  .steps li {
    display: flex;
    justify-content: space-between;
    padding: 4px 10px;
    border-radius: 5px;
    background: var(--soft);
    font: 500 13.5px var(--font-key);
  }
  .steps li .count {
    color: var(--good);
  }
  .steps li.fail .count {
    color: var(--bad);
    font-weight: 600;
  }
  .steps li .count {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
  .steps li.waiting,
  .steps li.skipped {
    opacity: 0.6;
  }
  .steps li.waiting .count,
  .steps li.skipped .count {
    color: inherit;
  }
  .steps li.running {
    font-weight: 600;
  }
  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid color-mix(in srgb, var(--c-fw) 25%, transparent);
    border-top-color: var(--c-fw);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation-duration: 2.4s;
    }
  }
  .result {
    padding: 10px 12px;
    border-radius: 7px;
    border-left: 3px solid;
  }
  .result.good {
    border-color: var(--good);
    background: color-mix(in srgb, var(--good) 8%, var(--paper));
  }
  .result.bad {
    border-color: var(--bad);
    background: var(--bad-bg);
  }
  .result p {
    margin: 4px 0 0;
  }
  .tag {
    margin-top: 6px;
    font: 600 22px var(--font-key);
  }
  .tag .mono {
    font-size: 22px;
  }
  .warn {
    color: var(--warn);
  }
  .bad {
    color: var(--bad);
  }
  .lines,
  .report {
    margin: 8px 0 0;
    padding: 8px;
    max-height: 220px;
    overflow: auto;
    font: 12px/1.45 var(--font-mono);
    background: var(--soft);
    border-radius: 5px;
    white-space: pre;
  }
  .sp {
    flex: 1;
  }
</style>
