<svelte:options namespace="svg" />

<script lang="ts">
  import { ICONS } from "../lib/fwicons";

  interface Props {
    name: string;
    /** Centre of the icon in the parent SVG. */
    x?: number;
    y?: number;
    scale?: number;
    /** Changed in the draft: drawn in the accent colour. */
    mine?: boolean;
  }
  let { name, x = 0, y = 0, scale = 1, mine = false }: Props = $props();
</script>

<g class="ic" class:mine transform={`translate(${x} ${y}) scale(${scale})`} aria-hidden="true">
  {#each ICONS[name] ?? [] as s, i (i)}
    {#if "d" in s}
      <path d={s.d} class:solid={s.solid} />
    {:else}
      <rect x={s.x} y={s.y} width={s.w} height={s.h} rx={s.rx} />
    {/if}
  {/each}
</g>

<style>
  .ic {
    color: var(--ink);
    fill: none;
    stroke: currentColor;
    stroke-width: 1.2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .ic.mine {
    color: var(--acc);
    stroke-width: 1.5;
  }
  .solid {
    fill: currentColor;
  }
</style>
