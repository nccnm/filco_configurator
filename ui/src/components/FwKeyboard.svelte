<script lang="ts">
  import type { FwKeyView, ModelView } from "../bindings/ipc";
  import { ARROWS, fit, geometry, neighbour, viewBox, type Direction } from "../lib/keycap";
  import FwIcon from "./FwIcon.svelte";

  interface Props {
    model: ModelView;
    /** Firmware view of every key, in the order of `model.keys`. */
    keys: FwKeyView[];
    selected: number | null;
    label: string;
    onselect: (i: number) => void;
  }
  let { model, keys, selected, label, onselect }: Props = $props();

  const geo = $derived(model.keys.map(geometry));
  const box = $derived(viewBox(model.width, model.height));
  let svg: SVGSVGElement | undefined = $state();

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
      const j = model.keys.indexOf(neighbour(model.keys, from, dir));
      onselect(j);
      queueMicrotask(() => svg?.querySelector<SVGGElement>(`[data-i="${j}"]`)?.focus());
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onselect(i);
    }
  }

  /** Arrows and other one-character symbols read better a little larger than words. */
  function mainSize(text: string, width: number): number {
    return ARROWS.has(text) || /^[^\w\s]$/u.test(text) ? 16 : Math.min(13, fit(text, width, 13));
  }

  function aria(legend: string, f: FwKeyView | undefined): string {
    if (!f) return legend;
    if (f.tap_hold) return `${legend}: ${f.tap_label ?? ""} / ${f.hold_label ?? ""}`;
    return f.fn_code ? `Fn + ${legend}: ${f.fn_code.name}` : legend;
  }
</script>

<svg
  bind:this={svg}
  class="kbd"
  viewBox={box}
  role="group"
  aria-label={label}
  data-testid="fw-keyboard"
  preserveAspectRatio="xMidYMid meet"
>
  {#each model.keys as k, i (k.id)}
    {@const g = geo[i] ?? geometry(k)}
    {@const f = keys[i]}
    {@const fnMine = !!f && f.fn_code?.code !== f.fn_stock?.code}
    <g
      class="key"
      class:mkey={k.m_key}
      class:custom={f?.custom}
      class:taphold={!!f?.tap_hold}
      class:selected={selected === i}
      role="button"
      tabindex={selected === i || (selected === null && i === 0) ? 0 : -1}
      aria-pressed={selected === i}
      aria-label={aria(k.legend, f)}
      data-i={i}
      data-key={k.id}
      data-testid={`fwkey-${k.id}`}
      onclick={() => onselect(i)}
      onkeydown={(e) => onkeydown(e, i)}
    >
      <rect class="outer" x={g.outer.x} y={g.outer.y} width={g.outer.w} height={g.outer.h} rx="7" />
      <rect class="inner" x={g.inner.x} y={g.inner.y} width={g.inner.w} height={g.inner.h} rx="5" />
      {#if f?.tap_hold}
        <text
          x={g.cx}
          y={g.cy - 10}
          class="lg-hold"
          font-size={fit(f.hold_label ?? "", g.inner.w, 9.5)}
          text-anchor="middle"
          dominant-baseline="central">{f.hold_label}</text
        >
        <text
          x={g.cx}
          y={g.cy + 5}
          class="lg-main"
          font-size={mainSize(f.tap_label ?? "", g.inner.w)}
          text-anchor="middle"
          dominant-baseline="central">{f.tap_label}</text
        >
      {:else}
        <!-- Read as "Fn + this key": the key itself small on top, its Fn function large in the middle. -->
        <text
          x={g.cx}
          y={g.cy - 10}
          class="lg-key"
          font-size={fit(k.legend, g.inner.w, 9.5)}
          text-anchor="middle"
          dominant-baseline="central">{k.legend}</text
        >
        {#if f?.fn_code?.icon}
          <FwIcon name={f.fn_code.icon} x={g.cx} y={g.cy + 5} scale={1.35} mine={fnMine} />
        {:else if f?.fn_code}
          <text
            x={g.cx}
            y={g.cy + 5}
            class="lg-main"
            class:mine={fnMine}
            font-size={mainSize(f.fn_code.legend, g.inner.w)}
            text-anchor="middle"
            dominant-baseline="central">{f.fn_code.legend}</text
          >
        {/if}
      {/if}
      {#if f?.changed}
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
  /* Differs from stock V72. */
  .key.custom .outer {
    stroke: var(--acc);
    stroke-width: 1.6;
  }
  .key.custom .inner {
    stroke: var(--acc);
  }
  .key.taphold .inner {
    fill: color-mix(in srgb, var(--acc) 7%, var(--paper));
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
  .lg-key {
    fill: var(--line);
    font-family: var(--font-key);
    font-weight: 500;
  }
  .lg-main {
    fill: var(--ink);
    font-family: var(--font-key);
    font-weight: 500;
  }
  .lg-main.mine {
    fill: var(--acc);
    font-weight: 700;
  }
  .lg-hold {
    fill: var(--acc);
    font-family: var(--font-key);
    font-weight: 600;
  }
  .key.taphold .lg-main {
    font-weight: 700;
  }
  .dot {
    fill: var(--acc);
    stroke: var(--paper);
    stroke-width: 1.5;
  }
</style>
