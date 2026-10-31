<script lang="ts">
  import type { ProgressView } from "../bindings/ipc";
  import type { Key } from "../lib/i18n";

  interface Props {
    progress: ProgressView | null;
    t: (key: Key, vars?: Record<string, string | number>) => string;
  }
  let { progress, t }: Props = $props();

  const fraction = $derived.by(() => {
    if (!progress || progress.steps === 0) return 0;
    const within = progress.done === progress.total ? 0 : progress.done / progress.total;
    return Math.min(1, (progress.step + within) / progress.steps);
  });
  const phaseKey = $derived(
    (progress ? `write_phase_${progress.phase}` : "write_phase_read") as
      "write_phase_read" | "write_phase_write" | "write_phase_verify",
  );
</script>

<div class="progress" data-testid="progress">
  <div
    class="bar"
    role="progressbar"
    aria-valuemin="0"
    aria-valuemax="100"
    aria-valuenow={Math.round(fraction * 100)}
  >
    <span style:width={`${fraction * 100}%`}></span>
  </div>
  <span class="small muted">
    {#if progress}
      {t("layer", { n: progress.layer + 1 })} · {t(phaseKey)}
    {:else}
      …
    {/if}
  </span>
</div>

<style>
  .progress {
    display: grid;
    gap: 5px;
  }
  .bar {
    height: 6px;
    border-radius: 3px;
    background: var(--soft);
    overflow: hidden;
    border: 1px solid var(--line);
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--acc);
    transition: width 80ms linear;
  }
</style>
