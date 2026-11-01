<script lang="ts">
  import type {
    Catalog,
    FwCodeView,
    FwEditInput,
    FwKeyView,
    FwTapHoldView,
    FwView,
    KeyView,
  } from "../bindings/ipc";
  import { tick } from "svelte";
  import type { Key } from "../lib/i18n";
  import { centerCurrent } from "../lib/reveal";
  import FwIcon from "./FwIcon.svelte";

  interface Props {
    fw: FwView;
    keyView: KeyView | null;
    key: FwKeyView | null;
    catalog: Catalog;
    lang: "vi" | "en";
    disabled: boolean;
    t: (key: Key, vars?: Record<string, string | number>) => string;
    onedit: (edit: FwEditInput) => void;
  }
  let { fw, keyView, key, catalog, lang, disabled, t, onedit }: Props = $props();

  type Section = "taphold" | "fn" | "default";
  let section = $state<Section>("fn");
  /** Which tap-hold field the picker fills. */
  let field = $state<"tap" | "hold">("tap");
  let query = $state("");
  let term = $state(200);

  const LCTRL = 0xe0;
  const GROUPS = [
    "letter",
    "digit",
    "function",
    "navigation",
    "editing",
    "symbol",
    "lock",
    "keypad",
    "international",
    "modifier",
    "special",
  ] as const;

  // Another key: open the section that matters for it.
  let lastIndex = -1;
  $effect(() => {
    const i = key?.index ?? -1;
    if (i === lastIndex) return;
    lastIndex = i;
    section = key?.tap_hold ? "taphold" : "fn";
    field = "tap";
    term = key?.tap_hold?.term_ms ?? 200;
    query = "";
  });

  // Scroll the code the key holds into the middle of the panel: for another key, section or tap-hold field.
  // A derived index, so an edit (a new `key` object for the same key) does not scroll again.
  let panel = $state<HTMLElement>();
  const keyIndex = $derived(key?.index ?? -1);
  $effect(() => {
    void keyIndex;
    void section;
    void field;
    void tick().then(() => centerCurrent(panel));
  });
  $effect(() => {
    if (key?.tap_hold) term = key.tap_hold.term_ms;
  });

  const specials = $derived(lang === "en" ? catalog.fw_special_en : catalog.fw_special);
  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return catalog.keys.filter(
      (u) => !q || u.legend.toLowerCase().includes(q) || u.name.toLowerCase().includes(q),
    );
  });
  const showSpecials = $derived(
    section === "fn" &&
      specials.some((s) => !query.trim() || s.name.toLowerCase().includes(query.trim().toLowerCase())),
  );

  const th = $derived(key?.tap_hold ?? null);
  const kind = $derived(th ? th.hold.type : "none");
  const termMax = $derived(kind === "layer" ? 2000 : 255);

  function hex(n: number): string {
    return n.toString(16).padStart(2, "0");
  }

  function codeText(c: FwCodeView | null | undefined): string {
    return c ? c.legend || c.name : t("fw_removed");
  }

  function setTapHold(v: FwTapHoldView | null) {
    if (!key) return;
    onedit({ type: "set_tap_hold", key: key.index, tap_hold: v });
  }

  function setKind(k: "none" | "key" | "layer") {
    if (!key || k === kind) return;
    if (k === "none") return setTapHold(null);
    const tap = th?.tap ?? key.default.code;
    setTapHold(
      k === "key"
        ? { tap, hold: { type: "key", code: LCTRL }, term_ms: Math.min(term, 255) }
        : { tap, hold: { type: "layer", layer: 2 }, term_ms: th ? Math.max(term, 50) : 300 },
    );
  }

  function pick(code: number) {
    if (!key || disabled) return;
    if (section === "fn") onedit({ type: "set_fn", key: key.index, code });
    else if (section === "default") onedit({ type: "set_default", key: key.index, code });
    else if (th) {
      if (field === "tap") setTapHold({ ...th, tap: code });
      else setTapHold({ ...th, hold: { type: "key", code } });
    }
  }

  function current(code: number): boolean {
    if (!key) return false;
    if (section === "fn") return key.fn_code?.code === code;
    if (section === "default") return key.default.code === code;
    if (!th) return false;
    return field === "tap" ? th.tap === code : th.hold.type === "key" && th.hold.code === code;
  }

  function legendOf(code: number): string {
    const u = catalog.keys.find((k) => k.code === code);
    return u?.legend ?? `0x${hex(code)}`;
  }

  const lockedFn = $derived(key?.fn_locked ?? null);
  const lockedTh = $derived(key?.tap_hold_locked ?? null);
  const pickerOff = $derived(
    disabled ||
      (section === "fn" && !!lockedFn) ||
      (section === "taphold" && (!th || (field === "hold" && th.hold.type === "layer"))),
  );

  const summary = $derived.by(() => {
    if (!key) return "";
    if (th) {
      const tap = legendOf(th.tap);
      return th.hold.type === "layer"
        ? t("fw_th_summary_layer", { tap, layer: th.hold.layer + 1, ms: th.term_ms })
        : t("fw_th_summary_key", { tap, hold: legendOf(th.hold.code), ms: th.term_ms });
    }
    return `${t("fw_fn_of", { key: keyView?.legend ?? "" })} = ${key.fn_code ? key.fn_code.name : t("fw_none")}`;
  });
</script>

<section class="inspector" data-testid="fw-inspector">
  {#if !key || !keyView}
    <header class="info">
      <h2 class="muted">{t("no_key_title")}</h2>
      <span class="what muted">{t("fw_no_key")}</span>
    </header>
  {:else}
    <header class="info">
      <h2>{t("fw_key_title", { key: keyView.legend, index: keyView.index })}</h2>
      <span class="what" data-testid="fw-summary">{summary}</span>
      <span class="sp"></span>
      <button
        disabled={disabled || !key.changed}
        onclick={() => onedit({ type: "key_to_base", key: key.index })}
        data-testid="fw-to-base">{t("fw_to_base")}</button
      >
      <button
        disabled={disabled || !key.custom}
        onclick={() => onedit({ type: "key_to_stock", key: key.index })}
        data-testid="fw-to-stock">{t("fw_to_stock")}</button
      >
    </header>
    <div class="body">
      <div class="tabs" role="tablist" aria-orientation="vertical">
        {#each [["taphold", "fw_sec_taphold"], ["fn", "fw_sec_fn"], ["default", "fw_sec_default"]] as const as [id, label] (id)}
          {@const value =
            id === "taphold"
              ? th
                ? `${legendOf(th.tap)} / ${th.hold.type === "layer" ? `L${th.hold.layer + 1}` : legendOf(th.hold.code)}`
                : t("fw_none")
              : id === "fn"
                ? codeText(key.fn_code)
                : codeText(key.default)}
          <button
            role="tab"
            class="tab"
            aria-selected={section === id}
            class:active={section === id}
            onclick={() => (section = id)}
            data-testid={`fw-sec-${id}`}><span>{t(label)}</span><small>{value}</small></button
          >
        {/each}
      </div>

      <div class="panel" role="tabpanel" bind:this={panel}>
        {#if section === "taphold"}
          {#if lockedTh && !th}
            <p class="note warn small">{t(`fw_th_locked_${lockedTh}` as Key)}</p>
          {:else}
            <div class="row">
              <span class="label">{t("fw_th_kind")}</span>
              <span class="seg">
                {#each [["none", "fw_th_off"], ["key", "fw_th_key"], ["layer", "fw_th_layer"]] as const as [k, label] (k)}
                  <button
                    aria-pressed={kind === k}
                    {disabled}
                    onclick={() => setKind(k)}
                    data-testid={`fw-th-${k}`}>{t(label)}</button
                  >
                {/each}
              </span>
              <span class="quota small">{t("fw_th_quota", { k: fw.tap_key_used, l: fw.tap_layer_used })}</span
              >
            </div>
            {#if th}
              <div class="row">
                <span class="label">{t("fw_th_tap")}</span>
                <button class="chip" aria-pressed={field === "tap"} onclick={() => (field = "tap")}
                  >{legendOf(th.tap)}</button
                >
                <span class="label inline">{t("fw_th_hold")}</span>
                {#if th.hold.type === "layer"}
                  {#each [0, 1, 2, 3] as l (l)}
                    <button
                      class="chip"
                      aria-pressed={th.hold.type === "layer" && th.hold.layer === l}
                      {disabled}
                      onclick={() => setTapHold({ ...th, hold: { type: "layer", layer: l } })}
                      >L{l + 1}</button
                    >
                  {/each}
                {:else}
                  <button class="chip" aria-pressed={field === "hold"} onclick={() => (field = "hold")}
                    >{legendOf(th.hold.code)}</button
                  >
                  <span class="hint small">{t("fw_th_pick")}</span>
                {/if}
              </div>
              <div class="row">
                <span class="label">{t("fw_th_term")}</span>
                <input
                  class="slider"
                  type="range"
                  min="50"
                  max={termMax}
                  step={kind === "layer" ? 10 : 5}
                  bind:value={term}
                  onchange={() => setTapHold({ ...th, term_ms: term })}
                  {disabled}
                  aria-label={t("fw_th_term")}
                />
                <input
                  class="num"
                  type="number"
                  min="50"
                  max={termMax}
                  bind:value={term}
                  onchange={() => setTapHold({ ...th, term_ms: term })}
                  {disabled}
                  data-testid="fw-th-term"
                />
                <span>ms</span>
                <span class="hint small">{t(kind === "layer" ? "fw_th_term_layer" : "fw_th_term_key")}</span>
              </div>
            {/if}
          {/if}
        {:else if section === "fn"}
          <div class="row">
            <span class="label">{t("fw_fn_of", { key: keyView.legend })}</span>
            <span class="chip static">
              {#if key.fn_code?.icon}
                <svg viewBox="-10 -8 20 16" class="icon"><FwIcon name={key.fn_code.icon} /></svg>
              {/if}
              {key.fn_code ? key.fn_code.legend || key.fn_code.name : t("fw_none")}
            </span>
            <button
              disabled={disabled || !!lockedFn || !key.fn_code}
              onclick={() => onedit({ type: "set_fn", key: key.index, code: null })}
              data-testid="fw-fn-remove">{t("fw_fn_remove")}</button
            >
            <span class="quota small">{t("fw_fn_slots", { used: fw.fn_slots_used, total: fw.fn_slots })}</span
            >
          </div>
          {#if lockedFn}
            <p class="note warn small">{t(`fw_locked_${lockedFn}` as Key)}</p>
          {/if}
        {:else}
          <div class="row">
            <span class="label">{keyView.legend}</span>
            <span class="chip static">{codeText(key.default)}</span>
            <span class="hint small">{t("fw_default_note")}</span>
          </div>
        {/if}

        {#if !(section === "taphold" && (!th || (th.hold.type === "layer" && field === "hold")))}
          <div class="row">
            <input
              type="search"
              placeholder={t("search")}
              bind:value={query}
              aria-label={t("search")}
              data-testid="fw-search"
            />
          </div>
          {#if showSpecials}
            <h3 class="small muted">{t("fw_group_special")}</h3>
            <div class="grid">
              {#each specials as s (s.code)}
                <button
                  class="cap"
                  class:current={current(s.code)}
                  title={`${s.name} (0x${hex(s.code)})`}
                  disabled={pickerOff}
                  onclick={() => pick(s.code)}
                  data-testid={`fw-code-${hex(s.code)}`}
                  ><svg viewBox="-10 -8 20 16" class="icon big"><FwIcon name={s.icon ?? ""} /></svg></button
                >
              {/each}
            </div>
          {/if}
          {#each GROUPS as g (g)}
            {@const items = filtered.filter((u) => u.group === g)}
            {#if items.length}
              <h3 class="small muted">{t(`group_${g}`)}</h3>
              <div class="grid">
                {#each items as u (u.code)}
                  <button
                    class="cap"
                    class:current={current(u.code)}
                    title={`${u.name} (0x${hex(u.code)})`}
                    disabled={pickerOff}
                    onclick={() => pick(u.code)}
                    data-testid={`fw-code-${hex(u.code)}`}
                    >{#if u.shifted}<span class="shift">{u.shifted}</span>{/if}<span>{u.legend}</span></button
                  >
                {/each}
              </div>
            {/if}
          {/each}
        {/if}
      </div>
    </div>
  {/if}
</section>

<style>
  .inspector {
    --cap: 60px;
    border-top: 1px solid var(--line);
    padding: 12px 20px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
  }
  .info {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 14px;
  }
  .info h2 {
    font: 600 17px var(--font-key);
  }
  .what {
    font: 500 14.5px var(--font-key);
  }
  .sp {
    flex: 1;
  }
  .body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 196px minmax(0, 1fr);
    gap: 0 18px;
  }
  .tabs {
    display: flex;
    flex-direction: column;
    gap: 1px;
    /* 4px around the scroll area so a focus ring (2px outline, 2px offset) is not clipped; the negative margin
       keeps the tabs where they were. */
    padding: 4px 14px 4px 4px;
    margin: -2px 0 -2px -4px;
    border-right: 1px solid var(--line);
    overflow-y: auto;
    min-height: 0;
  }
  .tab {
    border: 1px solid transparent;
    border-radius: 7px;
    padding: 5px 12px;
    background: transparent;
    font-size: 15px;
    display: flex;
    justify-content: space-between;
    gap: 10px;
  }
  .tab small {
    font: 500 12.5px var(--font-key);
    opacity: 0.7;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tab.active {
    background: color-mix(in srgb, var(--acc) 16%, var(--paper));
    border-color: var(--acc);
    color: var(--acc);
    font-weight: 600;
  }
  .panel {
    overflow-y: auto;
    min-height: 0;
    /* Room on the left for a focus ring, which the scroll area would clip; the margin keeps the content in place. */
    padding: 4px 6px 18px 4px;
    margin-left: -4px;
    mask-image: linear-gradient(to bottom, #000 calc(100% - 18px), transparent);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-bottom: 10px;
  }
  .label {
    width: 84px;
    font: 500 13.5px var(--font-key);
    opacity: 0.75;
  }
  .label.inline {
    width: auto;
    margin-left: 14px;
  }
  .seg {
    display: inline-flex;
  }
  .seg button {
    border-radius: 0;
  }
  .seg button:first-child {
    border-radius: 6px 0 0 6px;
  }
  .seg button:last-child {
    border-radius: 0 6px 6px 0;
  }
  .seg button + button {
    margin-left: -1px;
  }
  .quota {
    margin-left: 6px;
  }
  .hint {
    opacity: 0.7;
  }
  .chip {
    height: 34px;
    min-width: 56px;
    padding: 0 12px;
    justify-content: center;
    font-size: 14px;
  }
  .chip.static {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: 1px solid var(--acc);
    box-shadow: inset 0 0 0 1px var(--acc);
    border-radius: 6px;
    color: var(--acc);
    font: 600 14px var(--font-key);
  }
  .icon {
    width: 24px;
    height: 20px;
    overflow: visible;
  }
  .icon.big {
    width: 40px;
    height: 32px;
  }
  .slider {
    width: 260px;
    accent-color: var(--acc);
  }
  .num {
    width: 76px;
    height: 32px;
  }
  .note {
    margin: 0 0 10px;
  }
  .warn {
    color: var(--warn);
  }
  input[type="search"] {
    width: 260px;
    height: 36px;
  }
  h3 {
    font: 500 12.5px var(--font-key);
    margin: 6px 0 6px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, var(--cap));
    justify-content: space-between;
    gap: 10px;
    margin-bottom: 10px;
  }
  .cap {
    width: var(--cap);
    height: var(--cap);
    flex-direction: column;
    justify-content: center;
    gap: 1px;
    padding: 0 4px;
    border-radius: 7px;
    font-size: 14px;
    line-height: 1.15;
    text-align: center;
    white-space: normal;
    overflow-wrap: anywhere;
  }
  .shift {
    font-size: 13px;
  }
  .cap:hover:not(:disabled) {
    background: var(--soft);
  }
  .cap.current {
    border-color: var(--acc);
    box-shadow: inset 0 0 0 1px var(--acc);
    font-weight: 700;
  }
  .cap.current :global(.ic) {
    color: var(--acc);
  }
</style>
