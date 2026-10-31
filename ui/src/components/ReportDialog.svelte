<script lang="ts">
  import type { ModelView, ReportView } from "../bindings/ipc";
  import type { Key } from "../lib/i18n";
  import Modal from "./Modal.svelte";
  import Path from "./Path.svelte";

  interface Props {
    report: ReportView;
    model: ModelView;
    t: (key: Key, vars?: Record<string, string | number>) => string;
    onclose: () => void;
  }
  let { report, model, t, onclose }: Props = $props();

  let closeButton: HTMLButtonElement | undefined = $state();
  $effect(() => closeButton?.focus());

  const outcomeKey = (o: string) => `outcome_${o}` as Key;
</script>

<Modal title={t(report.succeeded ? "report_ok_title" : "report_fail_title")} testid="report-dialog" {onclose}>
  <p class:ok={report.succeeded} class:bad={!report.succeeded} class="lead" data-testid="report-summary">
    {#if report.succeeded}
      {t("report_ok_text")}
    {:else if report.consistent}
      {t("report_fail_consistent")}
    {:else}
      {t("report_fail_inconsistent")}
    {/if}
  </p>
  <ul class="outcomes">
    {#each report.outcomes as o (o.layer)}
      {@const l = model.layers[o.layer]}
      <li class="l{o.layer}" data-outcome={o.outcome}>
        <span class="led" style:background={l?.led} style:border-color={l?.led === "#ffffff" ? "" : l?.led}
        ></span>
        <b>{t("layer", { n: l?.label ?? o.layer + 1 })}</b>
        <span class:bad={o.outcome === "restore_failed" || o.outcome === "unknown"}
          >{t(outcomeKey(o.outcome))}</span
        >
      </li>
    {/each}
  </ul>
  {#if report.error}
    <details>
      <summary class="small">{t("details")}</summary>
      <code class="small">{report.error}</code>
    </details>
  {/if}
  <p class="small muted">{t("backup_at")} <Path path={report.backup} /></p>
  {#snippet footer()}
    <button bind:this={closeButton} class="primary" onclick={onclose} data-testid="report-close"
      >{t("close")}</button
    >
  {/snippet}
</Modal>

<style>
  .lead {
    margin: 0;
    font-weight: 500;
  }
  .ok {
    color: var(--good);
  }
  .bad {
    color: var(--bad);
  }
  .outcomes {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    gap: 4px;
  }
  .outcomes li {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  b {
    font: 600 14px var(--font-key);
    min-width: 64px;
  }
  p {
    margin: 0;
  }
  details code {
    overflow-wrap: anywhere;
  }
</style>
