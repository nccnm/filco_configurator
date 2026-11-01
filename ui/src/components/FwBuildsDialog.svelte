<script lang="ts">
  import type { FwBuiltView } from "../bindings/ipc";
  import type { Key } from "../lib/i18n";
  import Modal from "./Modal.svelte";

  interface Props {
    builds: FwBuiltView[];
    t: (key: Key, vars?: Record<string, string | number>) => string;
    onopenfolder: (dir: string) => void;
    onclose: () => void;
  }
  let { builds, t, onopenfolder, onclose }: Props = $props();

  function date(s: number): string {
    return new Date(s * 1000).toLocaleString();
  }
</script>

<Modal title={t("fw_builds_title")} testid="fw-builds-dialog" wide {onclose}>
  {#if builds.length === 0}
    <p class="muted">{t("fw_builds_empty")}</p>
  {:else}
    <table>
      <tbody>
        {#each builds as b (b.dir)}
          <tr>
            <td class="mono name">{b.name}</td>
            <td class="mono"
              >{b.tag != null ? `0x${b.tag.toString(16).toUpperCase().padStart(4, "0")}` : ""}</td
            >
            <td class="small">{date(b.modified)}</td>
            <td class="small" class:good={b.passed} class:bad={!b.passed}
              >{t(b.passed ? "fw_builds_passed" : "fw_builds_failed")}</td
            >
            <td class="small on">{b.on_keyboard ? t("fw_builds_on_keyboard") : ""}</td>
            <td><button onclick={() => onopenfolder(b.dir)}>{t("fw_open_folder")}</button></td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
  {#snippet footer()}
    <button onclick={() => onopenfolder("")}>{t("fw_open_folder")}</button>
    <span class="sp"></span>
    <button class="primary" onclick={onclose}>{t("close")}</button>
  {/snippet}
</Modal>

<style>
  table {
    width: 100%;
    border-collapse: collapse;
  }
  td {
    padding: 5px 8px;
    border-bottom: 1px solid var(--soft);
  }
  .name {
    font-weight: 600;
  }
  .good {
    color: var(--good);
  }
  .bad {
    color: var(--bad);
  }
  .on {
    color: var(--acc);
    font-weight: 600;
  }
  .sp {
    flex: 1;
  }
</style>
