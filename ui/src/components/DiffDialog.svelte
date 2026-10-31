<script lang="ts">
  import type { ModelView, PlanView, ProgressView } from "../bindings/ipc";
  import type { Key } from "../lib/i18n";
  import Modal from "./Modal.svelte";
  import Path from "./Path.svelte";
  import Progress from "./Progress.svelte";

  interface Props {
    plan: PlanView;
    model: ModelView;
    writing: boolean;
    progress: ProgressView | null;
    t: (key: Key, vars?: Record<string, string | number>) => string;
    oncancel: () => void;
    onwrite: () => void;
  }
  let { plan, model, writing, progress, t, oncancel, onwrite }: Props = $props();

  let cancelButton: HTMLButtonElement | undefined = $state();
  // The safe choice has the focus: Enter never writes by accident.
  $effect(() => cancelButton?.focus());

  const label = (text: string) => text.replace(/\n/g, " ");
  const linkedText = $derived(
    plan.linked.length
      ? t("linked_warning", {
          layers: plan.linked.map((l) => model.layers[l.layer]?.label ?? l.layer + 1).join(", "),
          profiles: plan.linked.map((l) => `"${l.profile}"`).join(", "),
        })
      : "",
  );
</script>

<Modal
  title={t("diff_title", { n: plan.changes.length })}
  testid="diff-dialog"
  wide
  onclose={writing ? undefined : oncancel}
>
  <p class="small">{t("diff_intro")} <Path path={plan.backup} /></p>
  {#if plan.experimental}
    <p class="banner warn small">{t("experimental")}</p>
  {/if}
  {#if linkedText}
    <p class="banner warn small" data-testid="linked-warning">{linkedText}</p>
  {/if}
  <div class="table-wrap">
    <table>
      <thead>
        <tr>
          <th>{t("diff_layer")}</th>
          <th>{t("diff_key")}</th>
          <th>{t("diff_old")}</th>
          <th class="arrow"></th>
          <th>{t("diff_new")}</th>
          <th class="fill">{t("diff_bytes")}</th>
        </tr>
      </thead>
      <tbody>
        {#each plan.changes as c (`${c.layer}-${c.index}`)}
          {@const l = model.layers[c.layer]}
          <tr class="l{c.layer}">
            <td
              ><span class="layer"
                ><span
                  class="led"
                  style:background={l?.led}
                  style:border-color={l?.led === "#ffffff" ? "" : l?.led}
                ></span>{l?.label}</span
              ></td
            >
            <td class="key">{c.legend ?? t("outside_layout", { index: c.index })}</td>
            <td class="old">{label(c.old.long)}</td>
            <td class="arrow">→</td>
            <td class="new">{label(c.new.long)}</td>
            <td class="mono bytes">{c.old.hex} → {c.new.hex}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
  {#if writing}
    <Progress {progress} {t} />
  {/if}
  {#snippet footer()}
    <button bind:this={cancelButton} onclick={oncancel} disabled={writing} data-testid="diff-cancel"
      >{t("cancel")}</button
    >
    <button class="primary" onclick={onwrite} disabled={writing} data-testid="diff-write"
      >{t("write_to_keyboard")}</button
    >
  {/snippet}
</Modal>

<style>
  .banner {
    margin: 0;
    padding: 7px 11px;
    border-radius: 6px;
  }
  .banner.warn {
    background: var(--warn-bg);
    color: var(--ink);
    border-left: 3px solid var(--warn);
  }
  .table-wrap {
    max-height: 46vh;
    overflow: auto;
  }
  table {
    border-collapse: collapse;
    width: 100%;
    font-size: 14px;
  }
  th {
    text-align: left;
    font: 500 12.5px var(--font-key);
    opacity: 0.65;
    padding: 0 18px 6px 0;
    border-bottom: 1px solid var(--line);
    position: sticky;
    top: 0;
    background: var(--paper);
  }
  td {
    padding: 7px 18px 7px 0;
    border-bottom: 1px solid var(--soft);
    vertical-align: top;
  }
  /* Every column hugs its content; the byte column takes what is left, so the arrow sits right next to the old
     and the new action. */
  th,
  td {
    width: 1%;
    white-space: nowrap;
  }
  th.fill,
  td.bytes {
    width: auto;
  }
  td.old,
  th:nth-child(3),
  .arrow {
    padding-right: 10px;
  }
  .layer {
    display: flex;
    align-items: center;
    gap: 7px;
    font: 600 14px var(--font-key);
  }
  .key {
    font: 600 14px var(--font-key);
  }
  .arrow {
    color: var(--line);
  }
  .new {
    font-weight: 600;
    color: var(--acc);
  }
  .bytes {
    white-space: nowrap;
    opacity: 0.8;
  }
</style>
