<script lang="ts">
  import type {
    Action,
    Catalog,
    FirmwareEditView,
    KeyView,
    ModelView,
    SlotView,
    TapHoldView,
  } from "../bindings/ipc";
  import { tick, untrack } from "svelte";
  import type { Key } from "../lib/i18n";
  import { centerCurrent } from "../lib/reveal";

  interface Props {
    model: ModelView;
    keyView: KeyView | null;
    slot: SlotView | null;
    base: SlotView | null;
    /** Firmware tap-hold on this key, if any: the layer slot is then locked. */
    tapHold?: TapHoldView | null;
    /** Firmware Fn-layer change on this key, if any. */
    fnEdit?: FirmwareEditView | null;
    /** The firmware on the keyboard reads tap-hold keys from the layer slots (type 8); if not, saving updates it. */
    layerTapHold?: boolean;
    layer: number;
    catalog: Catalog;
    disabled: boolean;
    t: (key: Key, vars?: Record<string, string | number>) => string;
    onassign: (layers: number[], action: Action) => void;
  }
  let {
    model,
    keyView,
    slot,
    base,
    tapHold = null,
    fnEdit = null,
    layerTapHold = false,
    layer,
    catalog,
    disabled,
    t,
    onassign,
  }: Props = $props();

  type Tab = "key" | "modifier" | "tap_hold" | "media" | "mouse" | "system" | "advanced";
  const TABS: [Tab, Key][] = [
    ["key", "tab_key"],
    ["modifier", "tab_modifier"],
    ["tap_hold", "tab_tap_hold"],
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
  // Tap-hold tab: what a tap sends, what holding does (a key, or HOLD_LAYER + n for layer n), the decision time
  // in ms, and which of tap and hold the key grid fills.
  let thTap = $state(0);
  let thHold = $state(0);
  let thTerm = $state(200);
  let thField = $state<"tap" | "hold">("tap");
  const HOLD_LAYER = 0xf0;

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

  let panel = $state<HTMLElement>();
  const selection = $derived(keyView && slot ? `${layer}:${keyView.index}` : "");
  const slotHex = $derived(slot?.hex ?? "");

  // Another key selected: reset the extra layers. The Advanced tab shows the slot's bytes, again when they change;
  // a new snapshot with the same bytes leaves what is being typed alone.
  $effect(() => {
    void selection;
    extraLayers = [];
  });
  $effect(() => {
    void selection;
    raw = slotHex;
  });
  const signed = (b: number) => (b << 24) >> 24;

  /** The tab that shows what a slot holds; `b1`, `b2` are its second and third bytes. */
  function tabFor(s: SlotView, b1: number, b2: number): Tab {
    switch (s.kind) {
      case "empty":
        return "key";
      case "key":
        return b2 === 0 && b1 !== 0 ? "modifier" : "key";
      case "mouse":
        return "mouse";
      case "consumer":
        return "media";
      case "system":
        return "system";
      case "tap_hold":
        return "tap_hold";
      default:
        return "advanced";
    }
  }

  // Another key (or layer) selected: open the tab for what it holds, take its modifiers and mouse amounts so the
  // matching button is marked, and clear the search so that button is not filtered out. Focus stays on the
  // keyboard, where the arrows move between keys.
  $effect(() => {
    if (!selection) return;
    untrack(() => {
      if (!slot) return;
      const [, b1 = 0, b2 = 0, b3 = 0] = slot.hex
        .trim()
        .split(/\s+/)
        .map((x) => parseInt(x, 16));
      tab = tabFor(slot, b1, b2);
      query = "";
      mods = slot.kind === "key" ? b1 : 0;
      if (slot.kind === "mouse" && b1 === 3) scrollAmount = Math.abs(signed(b2)) || 1;
      if (slot.kind === "mouse" && b1 === 4)
        moveStep = Math.max(Math.abs(signed(b2)), Math.abs(signed(b3))) || 8;
      if (slot.kind === "tap_hold") {
        [thTap, thHold, thTerm, thField] = [b1, b2, (b3 || 20) * 10, "tap"];
      } else {
        // A new tap-hold starts from what the key sends now; holding is picked next.
        const printed = catalog.keys.find((u) => u.legend === keyView?.legend)?.code ?? 0;
        thTap = slot.kind === "key" && b2 !== 0 ? b2 : printed;
        [thHold, thTerm, thField] = [0, 200, thTap ? "hold" : "tap"];
      }
    });
  });

  // Then scroll the marked button into the middle of the panel; also when another tab is opened.
  $effect(() => {
    void selection;
    void tab;
    void tick().then(() => centerCurrent(panel));
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
      case "mouse_click":
        return `01 01 ${hex(a.buttons)} 00`;
      case "mouse_scroll":
        return `01 03 ${hex(a.amount & 0xff)} 00`;
      case "mouse_move":
        return `01 04 ${hex(a.dx & 0xff)} ${hex(a.dy & 0xff)}`;
      case "tap_hold":
        return `08 ${hex(a.tap)} ${hex(a.hold)} ${hex(a.term)}`;
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

  /** Sets the tap-hold fields; the slot is written once both tap and hold are set. */
  function setTapHold(tap: number, hold: number, termMs: number) {
    [thTap, thHold] = [tap, hold];
    thTerm = Math.min(2550, Math.max(10, Math.round(termMs / 10) * 10 || 200));
    if (tap && hold) assign({ type: "tap_hold", tap, hold, term: thTerm / 10 });
  }

  function pickTapHold(code: number) {
    if (thField === "tap") {
      setTapHold(code, thHold, thTerm);
      if (!thHold) thField = "hold";
    } else {
      setTapHold(thTap, code, thTerm);
    }
  }

  function legendOf(code: number): string {
    return catalog.keys.find((u) => u.code === code)?.legend ?? `0x${hex(code)}`;
  }

  const thOff = $derived(disabled);

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

{#snippet keyGrid(isCurrent: (code: number) => boolean, off: boolean, onpick: (code: number) => void)}
  <div class="groups">
    {#each GROUPS as g (g)}
      {@const items = filtered.filter((u) => u.group === g)}
      {#if items.length}
        <div class="group">
          <h3 class="small muted">{t(`group_${g}`)}</h3>
          <div class="grid">
            {#each items as u (u.code)}
              <button
                class="cap"
                class:current={isCurrent(u.code)}
                title={`${u.name} (0x${hex(u.code)})`}
                disabled={off}
                onclick={() => onpick(u.code)}
                data-testid={`usage-${hex(u.code)}`}
                >{#if u.shifted}<span class="shift">{u.shifted}</span>{/if}<span>{u.legend}</span></button
              >
            {/each}
          </div>
        </div>
      {/if}
    {/each}
  </div>
{/snippet}

<section class="inspector" data-testid="inspector">
  {#if !keyView || !slot}
    <header class="info" data-testid="no-key">
      <h2 class="muted">{t("no_key_title")}</h2>
      <span class="what muted">{t("no_key_selected")}</span>
    </header>
    <div class="body">
      <div class="tabs" role="tablist" aria-orientation="vertical" aria-disabled="true">
        {#each TABS as [id, label] (id)}
          <button role="tab" class="tab" aria-selected="false" disabled>{t(label)}</button>
        {/each}
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
    {#if tapHold}
      <div class="note firmware small" data-testid="taphold-note">
        <b>{t("taphold_title")}</b>
        <span
          >{t("taphold_tap", { tap: tapHold.tap })} ·
          {tapHold.hold_is_layer
            ? t("taphold_hold_layer", { layer: (tapHold.hold_layer ?? 0) + 1 })
            : t("taphold_hold_key", { hold: tapHold.hold })} ·
          {t("taphold_term", { ms: tapHold.term_ms })}</span
        >
        <span class="muted">{t("taphold_locked")}</span>
      </div>
    {/if}
    {#if fnEdit}
      <p class="note firmware small" data-testid="fn-edit-note">
        {fnEdit.code ? t("fn_edit_set", { code: fnEdit.code }) : t("fn_edit_removed")}
      </p>
    {/if}
    {#if keyView.unverified}
      <p class="note warn small">{t("unverified_key")}</p>
    {/if}
    {#if keyView.note}
      <p class="note small muted">{keyView.note}</p>
    {/if}
    {#if !slot.editable && !tapHold}
      <p class="note warn small">{t("not_editable", { kind: slot.kind })}</p>
    {/if}

    <div class="body">
      <div class="tabs" role="tablist" aria-orientation="vertical">
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
      </div>

      <div class="panel" role="tabpanel" bind:this={panel}>
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
          {@render keyGrid(
            (code) => current(keyAction(code)),
            disabled,
            (code) => assign(keyAction(code)),
          )}
        {:else if tab === "tap_hold"}
          {#if !layerTapHold}
            <p class="note small muted" data-testid="tap-hold-upgrade">{t("tap_hold_upgrade_note")}</p>
          {/if}
          <div class="row">
            <span class="label">{t("tap_hold_tap")}</span>
            <button
              class="chip"
              aria-pressed={thField === "tap"}
              disabled={thOff}
              onclick={() => (thField = "tap")}
              data-testid="th-tap">{thTap ? legendOf(thTap) : t("tap_hold_none")}</button
            >
            <span class="label">{t("tap_hold_hold")}</span>
            <button
              class="chip"
              aria-pressed={thField === "hold" && thHold < HOLD_LAYER}
              disabled={thOff}
              onclick={() => (thField = "hold")}
              data-testid="th-hold"
              >{thHold && thHold < HOLD_LAYER ? legendOf(thHold) : t("tap_hold_key")}</button
            >
            {#each model.layers as l, i (i)}
              <button
                class="chip"
                aria-pressed={thHold === HOLD_LAYER + i}
                disabled={thOff || i === layer}
                title={t("tap_hold_layer", { n: l.label })}
                onclick={() => setTapHold(thTap, HOLD_LAYER + i, thTerm)}
                data-testid={`th-layer-${i}`}>→ L{l.label}</button
              >
            {/each}
          </div>
          <div class="row">
            <span class="label">{t("tap_hold_term")}</span>
            <input
              class="num"
              type="number"
              min="10"
              max="2550"
              step="10"
              bind:value={thTerm}
              onchange={() => setTapHold(thTap, thHold, thTerm)}
              disabled={thOff}
              aria-label={t("tap_hold_term")}
              data-testid="th-term"
            />
            <span class="small">ms</span>
            <span class="small muted">{t("tap_hold_hint")}</span>
          </div>
          <div class="row">
            <input
              type="search"
              placeholder={t("search")}
              bind:value={query}
              aria-label={t("search")}
              data-testid="th-search"
            />
            <span class="small muted"
              >{t(thField === "tap" ? "tap_hold_pick_tap" : "tap_hold_pick_hold")}</span
            >
          </div>
          {@render keyGrid(
            (code) => (thField === "tap" ? thTap === code : thHold === code),
            thOff,
            pickTapHold,
          )}
        {:else if tab === "modifier"}
          <p class="small muted">{t("modifier_only")}</p>
          <div class="grid">
            {#each MOD_BITS as [bit, name] (bit)}
              <button class="cap" aria-pressed={(mods & bit) !== 0} onclick={() => (mods ^= bit)}
                >{name}</button
              >
            {/each}
          </div>
          <button
            class="primary assign"
            disabled={disabled || mods === 0}
            onclick={() => assign({ type: "key", mods, usage: 0 })}>{t("assign_modifiers")}</button
          >
        {:else if tab === "media"}
          {#each [true, false] as fa (fa)}
            <h3 class="small muted">{t(fa ? "media_filco" : "media_more")}</h3>
            <div class="grid">
              {#each catalog.consumer.filter((u) => u.filco_assist === fa) as u (u.code)}
                {@const a = consumerAction(u.code)}
                <button
                  class="cap"
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
                  class="cap"
                  class:current={current({ type: "mouse_click", buttons: bit })}
                  {disabled}
                  onclick={() => assign({ type: "mouse_click", buttons: bit })}>{t(label)}</button
                >
              {/each}
            </div>
            <div class="sub">
              <h3 class="small muted">{t("mouse_scroll")}</h3>
              <label class="small"
                >{t("amount")} <input type="number" min="1" max="127" bind:value={scrollAmount} /></label
              >
            </div>
            <div class="grid">
              <button
                class="cap"
                {disabled}
                class:current={current({ type: "mouse_scroll", amount: Math.abs(scrollAmount) })}
                onclick={() => assign({ type: "mouse_scroll", amount: Math.abs(scrollAmount) })}
                >{t("scroll_up")}</button
              >
              <button
                class="cap"
                {disabled}
                class:current={current({ type: "mouse_scroll", amount: -Math.abs(scrollAmount) })}
                onclick={() => assign({ type: "mouse_scroll", amount: -Math.abs(scrollAmount) })}
                >{t("scroll_down")}</button
              >
            </div>
            <div class="sub">
              <h3 class="small muted">{t("mouse_move")}</h3>
              <label class="small"
                >{t("amount")} <input type="number" min="1" max="127" bind:value={moveStep} /></label
              >
            </div>
            <div class="grid">
              <button
                class="cap"
                {disabled}
                class:current={current({ type: "mouse_move", dx: -moveStep, dy: 0 })}
                onclick={() => assign({ type: "mouse_move", dx: -moveStep, dy: 0 })}>←</button
              >
              <button
                class="cap"
                {disabled}
                class:current={current({ type: "mouse_move", dx: 0, dy: -moveStep })}
                onclick={() => assign({ type: "mouse_move", dx: 0, dy: -moveStep })}>↑</button
              >
              <button
                class="cap"
                {disabled}
                class:current={current({ type: "mouse_move", dx: 0, dy: moveStep })}
                onclick={() => assign({ type: "mouse_move", dx: 0, dy: moveStep })}>↓</button
              >
              <button
                class="cap"
                {disabled}
                class:current={current({ type: "mouse_move", dx: moveStep, dy: 0 })}
                onclick={() => assign({ type: "mouse_move", dx: moveStep, dy: 0 })}>→</button
              >
            </div>
          </div>
        {:else if tab === "system"}
          <p class="small warn-text">{t("system_note")}</p>
          <div class="grid">
            {#each catalog.system as u (u.code)}
              {@const a = systemAction(u.code)}
              <button
                class="cap"
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
  /* Read from the firmware info block (0x30), not editable here. */
  .note.firmware {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 10px;
    align-items: baseline;
    padding: 6px 10px;
    border: 1px solid color-mix(in srgb, var(--acc) 40%, var(--line));
    border-radius: 7px;
    background: color-mix(in srgb, var(--acc) 7%, var(--paper));
  }
  .note.firmware b {
    color: var(--acc);
  }
  /* VIA-like: the action categories in a column on the left, the picker fills the rest of the window. */
  .body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 164px minmax(0, 1fr);
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
    /* Short windows: the list scrolls rather than spilling under the window edge. */
    overflow-y: auto;
    min-height: 0;
  }
  .tab {
    border: 1px solid transparent;
    border-radius: 7px;
    padding: 3px 12px;
    background: transparent;
    font-size: 15px;
    text-align: left;
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
    /* A row cut by the bottom edge fades out instead of ending in a hard slice; at the end of the scroll the
       padding keeps the last row clear of the fade. */
    mask-image: linear-gradient(to bottom, #000 calc(100% - 18px), transparent);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-bottom: 10px;
  }
  .row input[type="search"] {
    width: 260px;
    height: 36px;
    margin-right: 10px;
  }
  #raw {
    width: 160px;
    height: 36px;
  }
  .group h3,
  .mouse h3,
  .panel > h3 {
    font: 500 12.5px var(--font-key);
    margin: 6px 0 6px;
  }
  /* Every cap is the same square whatever its text. The columns are fixed and spread over the full width, the
     same for every group and tab, so the caps line up from group to group. */
  .inspector {
    --cap: 60px;
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
  /* The Shift character above the plain one, as printed on the keycap. */
  .shift {
    font-size: 13px;
  }
  .assign {
    height: 40px;
    padding: 0 18px;
    font-size: 14.5px;
  }
  /* A group heading with its own setting (mouse step) on the same line. */
  .sub {
    display: flex;
    align-items: center;
    gap: 16px;
    margin: 6px 0;
  }
  .sub h3 {
    margin: 0;
  }
  .sub input {
    width: 64px;
    height: 30px;
    margin-left: 4px;
  }
  .cap:hover:not(:disabled) {
    background: var(--soft);
  }
  .cap.current {
    border-color: var(--acc);
    box-shadow: inset 0 0 0 1px var(--acc);
    font-weight: 700;
  }
  .chip {
    height: 36px;
    padding: 0 13px;
    font-size: 14px;
  }
  .label {
    font-weight: 500;
    font-size: 13px;
  }
  .label + .chip {
    min-width: 64px;
  }
  .chip + .label {
    margin-left: 14px;
  }
  .num {
    width: 76px;
    height: 36px;
  }
</style>
