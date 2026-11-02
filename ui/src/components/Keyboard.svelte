<script lang="ts">
  import type { KeyView, ModelView, SlotView, TapHoldView } from "../bindings/ipc";
  import { ARROWS, fit, geometry, lineYs, neighbour, viewBox, type Direction } from "../lib/keycap";

  interface Props {
    model: ModelView;
    slots: SlotView[];
    selected: number | null;
    label: string;
    /** Tap-hold keys the firmware holds itself (an older build): they override the layer slot. */
    tapHolds?: TapHoldView[];
    /** Current layer index, so a key that holds into this layer is drawn bold. */
    layer?: number;
    /** Positions of keys whose tap-hold slot in some layer turns on the current layer while held. */
    holdsInto?: Set<number>;
    onselect: (i: number) => void;
  }
  let {
    model,
    slots,
    selected,
    label,
    tapHolds = [],
    layer = 0,
    holdsInto = new Set(),
    onselect,
  }: Props = $props();

  const tapHoldByIndex = $derived(new Map(tapHolds.map((t) => [t.index, t])));

  const geo = $derived(model.keys.map(geometry));
  const box = $derived(viewBox(model.width, model.height));
  let svg: SVGSVGElement | undefined = $state();

  function mainSize(text: string, width: number): number {
    return fit(text, width, ARROWS.has(text) ? 18 : 14.5);
  }

  const DIRS: Record<string, Direction> = {
    ArrowLeft: "left",
    ArrowRight: "right",
    ArrowUp: "up",
    ArrowDown: "down",
  };

  function onkeydown(e: KeyboardEvent, i: number) {
    const dir = DIRS[e.key];
    if (dir) {
      e.preventDefault();
      const from = model.keys[i];
      if (!from) return;
      const to = neighbour(model.keys, from, dir);
      const j = model.keys.indexOf(to);
      onselect(j);
      focusKey(j);
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onselect(i);
    }
  }

  function focusKey(i: number) {
    queueMicrotask(() => svg?.querySelector<SVGGElement>(`[data-i="${i}"]`)?.focus());
  }

  /** Keycap text with the Shift character on its own line above, as printed on the key: "{" over "[". */
  function stack(shifted: string | null | undefined, base: string): string {
    return shifted ? `${shifted}\n${base}` : base;
  }

  function aria(k: KeyView, s: SlotView | undefined, th: TapHoldView | undefined): string {
    if (th) {
      const hold = th.hold_is_layer ? `hold layer ${(th.hold_layer ?? 0) + 1}` : `hold ${th.hold}`;
      return `${k.legend}: tap ${th.tap}, ${hold}`;
    }
    if (!s || s.kind === "empty") return k.legend;
    return `${k.legend}: ${s.long}`;
  }
</script>

<svg
  bind:this={svg}
  class="kbd"
  viewBox={box}
  role="group"
  aria-label={label}
  data-testid="keyboard"
  preserveAspectRatio="xMidYMid meet"
>
  {#each model.keys as k, i (k.id)}
    {@const g = geo[i] ?? geometry(k)}
    {@const s = slots[i]}
    {@const th = tapHoldByIndex.get(k.index)}
    {@const empty = !s || s.kind === "empty"}
    {@const text = stack(s?.cap_shift, s?.short ?? "")}
    {@const printed = stack(k.shifted, k.legend)}
    {@const showPrint = !empty && s?.short !== k.legend}
    {@const slotTh = s?.kind === "tap_hold"}
    {@const holdsHere = (th?.hold_is_layer && th.hold_layer === layer) || holdsInto.has(i)}
    <g
      class="key"
      class:mkey={k.m_key}
      class:assigned={th ? true : !empty}
      class:taphold={!!th || slotTh}
      class:holds-here={holdsHere}
      class:pending={s?.changed}
      class:selected={selected === i}
      class:locked={!th && s && !s.editable}
      role="button"
      tabindex={selected === i || (selected === null && i === 0) ? 0 : -1}
      aria-pressed={selected === i}
      aria-label={aria(k, s, th)}
      data-i={i}
      data-key={k.id}
      data-testid={`key-${k.id}`}
      onclick={() => onselect(i)}
      onkeydown={(e) => onkeydown(e, i)}
    >
      <rect class="outer" x={g.outer.x} y={g.outer.y} width={g.outer.w} height={g.outer.h} rx="7" />
      <rect class="inner" x={g.inner.x} y={g.inner.y} width={g.inner.w} height={g.inner.h} rx="5" />
      {#if th}
        {@const holdSize = fit(th.hold, g.inner.w, 9.5)}
        <text
          x={g.cx}
          y={g.cy - 10}
          class="lg-hold"
          font-size={holdSize}
          text-anchor="middle"
          dominant-baseline="central">{th.hold}</text
        >
        {@const tapSize = Math.min(13, mainSize(th.tap, g.inner.w))}
        <text
          x={g.cx}
          y={g.cy + 5}
          class="lg-main"
          font-size={tapSize}
          text-anchor="middle"
          dominant-baseline="central">{th.tap}</text
        >
      {:else if slotTh && s}
        {@const hold = s.cap_sub ?? ""}
        <text
          x={g.cx}
          y={g.cy - 10}
          class="lg-hold"
          font-size={fit(hold, g.inner.w, 9.5)}
          text-anchor="middle"
          dominant-baseline="central">{hold}</text
        >
        <text
          x={g.cx}
          y={g.cy + 5}
          class="lg-main"
          font-size={Math.min(13, mainSize(s.short, g.inner.w))}
          text-anchor="middle"
          dominant-baseline="central">{s.short}</text
        >
      {:else if empty}
        {@const size = fit(printed, g.inner.w, 12)}
        {#each lineYs(printed, g.cy, size) as y, li (li)}
          <text
            x={g.cx}
            {y}
            class="lg-faint"
            font-size={size}
            text-anchor="middle"
            dominant-baseline="central">{printed.split("\n")[li]}</text
          >
        {/each}
      {:else}
        {#if showPrint}
          <text
            x={g.inner.x + 4}
            y={g.inner.y + 8}
            class="lg-print"
            font-size="8.5"
            dominant-baseline="central">{k.legend}</text
          >
        {/if}
        {#if s?.cap_sub}
          {@const subSize = fit(s.cap_sub, g.inner.w, 9.5)}
          <text
            x={g.cx}
            y={g.cy - 2}
            class="lg-sub"
            font-size={subSize}
            text-anchor="middle"
            dominant-baseline="central">{s.cap_sub}</text
          >
        {/if}
        {@const size = s?.cap_sub ? Math.min(13, mainSize(text, g.inner.w)) : mainSize(text, g.inner.w)}
        {#each lineYs(text, g.cy + (s?.cap_sub ? 12 : showPrint ? 3 : 0), size) as y, li (li)}
          <text x={g.cx} {y} class="lg-main" font-size={size} text-anchor="middle" dominant-baseline="central"
            >{text.split("\n")[li]}</text
          >
        {/each}
      {/if}
      {#if s?.changed}
        <circle class="dot" cx={g.outer.x + g.outer.w - 7} cy={g.outer.y + 7} r="3.2" />
      {/if}
    </g>
  {/each}
</svg>

<style>
  .kbd {
    display: block;
    width: 100%;
    height: 100%;
    user-select: none;
  }
  .key {
    cursor: pointer;
    outline: none;
  }
  .key .outer {
    fill: none;
    stroke: var(--line);
    stroke-width: 1;
  }
  .key .inner {
    fill: var(--paper);
    stroke: var(--line);
    stroke-width: 1;
  }
  .key.mkey .outer {
    fill: var(--soft);
  }
  .key.assigned .outer {
    stroke: var(--acc);
    stroke-width: 1.6;
  }
  .key.assigned .inner {
    stroke: var(--acc);
  }
  .key.locked .inner {
    stroke-dasharray: 3 3;
  }
  .key:hover .outer {
    stroke: var(--ink);
  }
  .key.selected .outer {
    fill: color-mix(in srgb, var(--acc) 16%, var(--paper));
    stroke: var(--ink);
    stroke-width: 2;
  }
  .key:focus-visible .outer {
    stroke: var(--ink);
    stroke-width: 2.4;
    stroke-dasharray: 4 3;
  }
  .lg-main {
    fill: var(--ink);
    font-family: var(--font-key);
    font-weight: 500;
  }
  .key.assigned .lg-main {
    font-weight: 700;
    stroke: var(--ink);
    stroke-width: 0.4px;
    paint-order: stroke;
  }
  .lg-sub {
    fill: var(--ink);
    font-family: var(--font-key);
    font-weight: 500;
    opacity: 0.8;
  }
  .lg-hold {
    fill: var(--acc);
    font-family: var(--font-key);
    font-weight: 600;
  }
  /* A tap-hold key, set in the firmware or in the layer slot. */
  .key.taphold .inner {
    fill: color-mix(in srgb, var(--acc) 7%, var(--paper));
  }
  /* A key whose hold turns on the layer being viewed: emphasised on that layer. */
  .key.holds-here .outer {
    fill: color-mix(in srgb, var(--acc) 22%, var(--paper));
    stroke-width: 2;
  }
  .key.holds-here .lg-main {
    font-weight: 700;
  }
  .lg-print {
    fill: var(--line);
    font-family: var(--font-key);
  }
  .lg-faint {
    fill: var(--faint);
    font-family: var(--font-key);
  }
  .key:hover .lg-faint,
  .key.selected .lg-faint {
    fill: var(--line);
  }
  .dot {
    fill: var(--acc);
    stroke: var(--paper);
    stroke-width: 1.5;
  }
</style>
