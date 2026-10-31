<script lang="ts">
  import type { Action, Catalog, KeyView, ModelView, SlotView } from "../bindings/ipc";
  import type { Key } from "../lib/i18n";

  interface Props {
    model: ModelView;
    keyView: KeyView | null;
    slot: SlotView | null;
    base: SlotView | null;
    layer: number;
    catalog: Catalog;
    disabled: boolean;
    t: (key: Key, vars?: Record<string, string | number>) => string;
    onassign: (layers: number[], action: Action) => void;
  }
  let { model, keyView, slot, base, layer, catalog, disabled, t, onassign }: Props = $props();

  type Tab = "key" | "modifier" | "media" | "mouse" | "system" | "advanced";
  const TABS: [Tab, Key][] = [
    ["key", "tab_key"],
    ["modifier", "tab_modifier"],
    ["media", "tab_media"],
    ["mouse", "tab_mouse"],
    ["system", "tab_system"],
    ["advanced", "tab_advanced"],
  ];
  let tab = $state<Tab>("key");
  let query = $state("");
  let mods = $state(0);
  let extraLayers = $state<number[]>([]);
  let scrollAmount = $state(1);
  let moveStep = $state(8);
  let raw = $state("");

  const MOD_BITS: [number, string][] = [
    [0x01, "Ctrl"],
    [0x02, "Shift"],
    [0x04, "Alt"],
    [0x08, "Win"],
    [0x10, "RCtrl"],
    [0x20, "RShift"],
    [0x40, "RAlt"],
    [0x80, "RWin"],
  ];
  const BUTTONS: [number, Key][] = [
    [0x01, "button_left"],
    [0x02, "button_right"],
    [0x04, "button_middle"],
    [0x08, "button_back"],
    [0x10, "button_forward"],
  ];
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

  // When another key is selected, show its current bytes in the Advanced tab and reset the extra layers.
  $effect(() => {
    raw = slot?.hex ?? "";
    extraLayers = [];
  });

  const targets = $derived([layer, ...extraLayers.filter((l) => l !== layer)].sort());

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return catalog.keys.filter(
      (u) => !q || u.legend.toLowerCase().includes(q) || u.name.toLowerCase().includes(q),
    );
  });

  function hex(n: number, digits = 2): string {
    return n.toString(16).padStart(digits, "0");
  }

  function slotBytes(a: Action): string | null {
    switch (a.type) {
      case "key":
        return `02 ${hex(a.mods)} ${hex(a.usage)} 00`;
      case "consumer":
        return `03 ${hex(a.usage & 0xff)} ${hex(a.usage >> 8)} 00`;
      case "system":
        return `04 ${hex(a.usage)} 00 00`;
      default:
        return null;
    }
  }

  const keyAction = (usage: number): Action => ({ type: "key", mods, usage });
  const consumerAction = (usage: number): Action => ({ type: "consumer", usage });
  const systemAction = (usage: number): Action => ({ type: "system", usage });

  function current(a: Action): boolean {
    const b = slotBytes(a);
    return b !== null && b === slot?.hex;
  }

  function assign(a: Action) {
    if (!keyView || disabled) return;
    onassign(targets, a);
  }

  function toggleLayer(l: number) {
    extraLayers = extraLayers.includes(l) ? extraLayers.filter((x) => x !== l) : [...extraLayers, l];
  }

  const shortcuts = $derived<[string[], Key][]>([
    [["1", "-", `${model.layers.length}`], "sc_layer"],
    [["←", "↑", "↓", "→"], "sc_move"],
    [["Enter"], "sc_select"],
    [["Esc"], "sc_deselect"],
    [["Delete"], "sc_clear"],
    [["Ctrl", "+", "Z"], "sc_undo"],
    [["Ctrl", "+", "Y"], "sc_redo"],
    [["Ctrl", "+", "S"], "sc_save"],
  ]);

  const rawValid = $derived(/^\s*([0-9a-f]{2}\s*){4}$/i.test(raw));
</script>

<section class="inspector" data-testid="inspector">
  {#if !keyView || !slot}
    <header class="info" data-testid="no-key">
      <h2 class="muted">{t("no_key_title")}</h2>
      <span class="what muted">{t("no_key_selected")}</span>
    </header>
    <div class="tabs" role="tablist" aria-disabled="true">
      {#each TABS as [id, label] (id)}
        <button role="tab" class="tab" aria-selected="false" disabled>{t(label)}</button>
      {/each}
      <span class="later muted small">{t("macro_later")}</span>
    </div>
    <div class="panel">
      <h3 class="small muted">{t("shortcuts")}</h3>
      <dl class="shortcuts">
        {#each shortcuts as [keys, label] (label)}
          <div>
            <dt>
              {#each keys as k, i (i)}{#if k === "-" || k === "+"}<span class="sep">{k}</span>{:else}<kbd
                    >{k}</kbd
                  >{/if}{/each}
            </dt>
            <dd>{t(label)}</dd>
          </div>
        {/each}
      </dl>
    </div>
  {:else}
    <header class="info">
      <h2>{t("key_info", { key: keyView.legend, index: keyView.index })}</h2>
      <span class="bytes"><code data-testid="slot-hex">{slot.hex}</code></span>
      <span class="what" data-testid="slot-long">{slot.long}</span>
      {#if slot.changed && base}
        <span class="edited"
          >{t("edited_from", {
            old: base.kind === "empty" ? t("slot_empty_short") : base.long,
          })}</span
        >
      {/if}
      <span class="apply">
        <span class="muted small">{t("apply_to")}</span>
        {#each model.layers as l, i (i)}
          <button
            class="layer-chip l{i}"
            aria-pressed={targets.includes(i)}
            disabled={i === layer}
            onclick={() => toggleLayer(i)}
            title={t("layer", { n: l.label })}
            ><span class="led" style:background={l.led} style:border-color={l.led === "#ffffff" ? "" : l.led}
            ></span>{l.label}</button
          >
        {/each}
        <button
          onclick={() => assign({ type: "empty" })}
          disabled={disabled || slot.kind === "empty"}
          data-testid="clear-slot">{t("clear_slot")}</button
        >
      </span>
    </header>
    {#if keyView.unverified}
      <p class="note warn small">{t("unverified_key")}</p>
    {/if}
    {#if keyView.note}
      <p class="note small muted">{keyView.note}</p>
    {/if}
    {#if !slot.editable}
      <p class="note warn small">{t("not_editable", { kind: slot.kind })}</p>
    {/if}

    <div class="tabs" role="tablist">
      {#each TABS as [id, label] (id)}
        <button
          role="tab"
          class="tab"
          aria-selected={tab === id}
          class:active={tab === id}
          onclick={() => (tab = id)}
          data-testid={`tab-${id}`}>{t(label)}</button
        >
      {/each}
      <span class="later muted small">{t("macro_later")}</span>
    </div>

    <div class="panel" role="tabpanel">
      {#if tab === "key"}
        <div class="row">
          <input
            type="search"
            placeholder={t("search")}
            bind:value={query}
            aria-label={t("search")}
            data-testid="key-search"
          />
          <span class="muted small">{t("modifiers")}</span>
          {#each MOD_BITS.slice(0, 4) as [bit, name] (bit)}
            <button class="chip" aria-pressed={(mods & bit) !== 0} onclick={() => (mods ^= bit)}
              >{name}</button
            >
          {/each}
        </div>
        <div class="groups">
          {#each GROUPS as g (g)}
            {@const items = filtered.filter((u) => u.group === g)}
            {#if items.length}
              <div class="group">
                <h3 class="small muted">{t(`group_${g}`)}</h3>
                <div class="grid">
                  {#each items as u (u.code)}
                    {@const a = keyAction(u.code)}
                    <button
                      class="cap"
                      class:current={current(a)}
                      title={`${u.name} (0x${hex(u.code)})`}
                      {disabled}
                      onclick={() => assign(a)}
                      data-testid={`usage-${hex(u.code)}`}>{u.legend}</button
                    >
                  {/each}
                </div>
              </div>
            {/if}
          {/each}
        </div>
      {:else if tab === "modifier"}
        <p class="small muted">{t("modifier_only")}</p>
        <div class="row">
          {#each MOD_BITS as [bit, name] (bit)}
            <button class="chip" aria-pressed={(mods & bit) !== 0} onclick={() => (mods ^= bit)}
              >{name}</button
            >
          {/each}
          <button
            class="primary"
            disabled={disabled || mods === 0}
            onclick={() => assign({ type: "key", mods, usage: 0 })}>{t("assign_modifiers")}</button
          >
        </div>
      {:else if tab === "media"}
        {#each [true, false] as fa (fa)}
          <h3 class="small muted">{t(fa ? "media_filco" : "media_more")}</h3>
          <div class="grid">
            {#each catalog.consumer.filter((u) => u.filco_assist === fa) as u (u.code)}
              {@const a = consumerAction(u.code)}
              <button
                class="cap wide"
                class:current={current(a)}
                title={`${u.name} (0x${hex(u.code, 4)})`}
                {disabled}
                onclick={() => assign(a)}>{u.legend}</button
              >
            {/each}
          </div>
        {/each}
      {:else if tab === "mouse"}
        <div class="mouse">
          <h3 class="small muted">{t("mouse_click")}</h3>
          <div class="grid">
            {#each BUTTONS as [bit, label] (bit)}
              <button
                class="cap wide"
                {disabled}
                onclick={() => assign({ type: "mouse_click", buttons: bit })}>{t(label)}</button
              >
            {/each}
          </div>
          <h3 class="small muted">{t("mouse_scroll")}</h3>
          <div class="row">
            <label class="small"
              >{t("amount")} <input type="number" min="1" max="127" bind:value={scrollAmount} /></label
            >
            <button
              class="cap wide"
              {disabled}
              onclick={() => assign({ type: "mouse_scroll", amount: Math.abs(scrollAmount) })}
              >{t("scroll_up")}</button
            >
            <button
              class="cap wide"
              {disabled}
              onclick={() => assign({ type: "mouse_scroll", amount: -Math.abs(scrollAmount) })}
              >{t("scroll_down")}</button
            >
          </div>
          <h3 class="small muted">{t("mouse_move")}</h3>
          <div class="row">
            <label class="small"
              >{t("amount")} <input type="number" min="1" max="127" bind:value={moveStep} /></label
            >
            <button
              class="cap"
              {disabled}
              onclick={() => assign({ type: "mouse_move", dx: -moveStep, dy: 0 })}>←</button
            >
            <button
              class="cap"
              {disabled}
              onclick={() => assign({ type: "mouse_move", dx: 0, dy: -moveStep })}>↑</button
            >
            <button class="cap" {disabled} onclick={() => assign({ type: "mouse_move", dx: 0, dy: moveStep })}
              >↓</button
            >
            <button class="cap" {disabled} onclick={() => assign({ type: "mouse_move", dx: moveStep, dy: 0 })}
              >→</button
            >
          </div>
        </div>
      {:else if tab === "system"}
        <p class="small warn-text">{t("system_note")}</p>
        <div class="grid">
          {#each catalog.system as u (u.code)}
            {@const a = systemAction(u.code)}
            <button
              class="cap wide"
              class:current={current(a)}
              title={u.name}
              {disabled}
              onclick={() => assign(a)}>{u.legend}</button
            >
          {/each}
        </div>
      {:else}
        <div class="row">
          <label class="small" for="raw">{t("raw_bytes")}</label>
          <input
            id="raw"
            type="text"
            class="mono"
            bind:value={raw}
            spellcheck="false"
            data-testid="raw-input"
          />
          <button
            class="primary"
            disabled={disabled || !rawValid}
            onclick={() => assign({ type: "raw", hex: raw })}
            data-testid="raw-assign">{t("assign")}</button
          >
        </div>
        <p class="small muted">{t("raw_hint")}</p>
      {/if}
    </div>
  {/if}
</section>

<style>
  .inspector {
    border-top: 1px solid var(--line);
    padding: 12px 20px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
  }
  .tab:disabled {
    opacity: 0.45;
  }
  .shortcuts {
    margin: 2px 0 0;
    display: grid;
    /* Two columns read top to bottom: picking keys on the left, editing and saving on the right. */
    grid-template-columns: repeat(2, minmax(0, 480px));
    grid-template-rows: repeat(4, auto);
    grid-auto-flow: column;
    gap: 8px 40px;
  }
  .shortcuts div {
    display: grid;
    grid-template-columns: 150px 1fr;
    align-items: center;
    gap: 10px;
  }
  .shortcuts dt {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .shortcuts dd {
    margin: 0;
    font-size: 14px;
  }
  kbd {
    min-width: 30px;
    height: 28px;
    padding: 0 7px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--line);
    border-radius: 5px;
    box-shadow: inset 0 -2px 0 var(--soft);
    font: 500 13px var(--font-key);
    color: var(--ink);
  }
  .sep {
    color: var(--line);
    font-size: 13px;
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
  .edited {
    font: 500 13px var(--font-key);
    color: var(--acc);
  }
  .apply {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .layer-chip {
    padding: 3px 8px;
  }
  .layer-chip:disabled {
    opacity: 1;
    border-color: var(--acc);
    box-shadow: inset 0 0 0 1px var(--acc);
    font-weight: 600;
  }
  .note {
    margin: 0;
  }
  .warn,
  .warn-text {
    color: var(--warn);
    margin: 0;
  }
  .tabs {
    display: flex;
    align-items: flex-end;
    gap: 2px;
    border-bottom: 1px solid var(--line);
  }
  .tab {
    border: 1px solid transparent;
    border-bottom: none;
    border-radius: 6px 6px 0 0;
    padding: 5px 13px;
    margin-bottom: -1px;
    background: transparent;
  }
  .tab.active {
    border-color: var(--line);
    background: var(--paper);
    font-weight: 600;
    box-shadow: inset 0 2px 0 var(--acc);
  }
  .later {
    margin-left: auto;
    padding-bottom: 5px;
  }
  .panel {
    overflow-y: auto;
    min-height: 0;
    flex: 1;
    padding: 4px 0 18px;
    /* A row cut by the bottom edge fades out instead of ending in a hard slice; at the end of the scroll the
       padding keeps the last row clear of the fade. */
    mask-image: linear-gradient(to bottom, #000 calc(100% - 18px), transparent);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-bottom: 8px;
  }
  .row input[type="search"] {
    width: 220px;
    margin-right: 10px;
  }
  .row input[type="number"] {
    width: 64px;
    margin-left: 4px;
  }
  #raw {
    width: 140px;
  }
  .groups {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 22px;
  }
  .group h3,
  .mouse h3,
  .panel > h3 {
    font: 500 12.5px var(--font-key);
    margin: 4px 0 4px;
  }
  .grid {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-bottom: 6px;
  }
  .cap {
    min-width: 34px;
    height: 30px;
    justify-content: center;
    padding: 0 7px;
    font-size: 13px;
  }
  .cap.wide {
    min-width: 64px;
  }
  .cap.current {
    border-color: var(--acc);
    box-shadow: inset 0 0 0 1px var(--acc);
    font-weight: 700;
  }
  .chip {
    padding: 3px 9px;
  }
</style>
