<script lang="ts">
  import type { BackupView } from "../bindings/ipc";
  import type { Key } from "../lib/i18n";
  import Modal from "./Modal.svelte";

  interface Props {
    backups: BackupView[];
    t: (key: Key, vars?: Record<string, string | number>) => string;
    onload: (path: string) => void;
    onopenfolder: () => void;
    onpickfolder: () => void;
    onclose: () => void;
  }
  let { backups, t, onload, onopenfolder, onpickfolder, onclose }: Props = $props();

  function when(created: string): string {
    const m = /^(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})/.exec(created);
    return m ? `${m[1]}-${m[2]}-${m[3]} ${m[4]}:${m[5]}:${m[6]}` : created;
  }

  function reason(r: string | null): string {
    if (r === "before-write") return t("backup_reason_before_write");
    if (r === "manual") return t("backup_reason_manual");
    return r ?? t("backup_reason_other");
  }
</script>

<Modal title={t("backups_title")} testid="backups-dialog" {onclose}>
  <p class="small">{t("backups_intro")}</p>
  {#if backups.length === 0}
    <p class="muted">{t("backups_empty")}</p>
  {:else}
    <ul>
      {#each backups as b (b.path)}
        <li>
          <span class="when">{when(b.created)}</span>
          <span class="small muted">{reason(b.reason)}</span>
          {#if b.problem}
            <span class="small bad" title={b.problem}>{b.problem}</span>
          {/if}
          <button disabled={b.problem !== null} onclick={() => onload(b.path)}>{t("load")}</button>
        </li>
      {/each}
    </ul>
  {/if}
  {#snippet footer()}
    <button class="ghost" onclick={onopenfolder}>{t("open_backup_folder")}</button>
    <button class="ghost" onclick={onpickfolder}>{t("load_backup_folder")}</button>
    <span class="spacer"></span>
    <button onclick={onclose}>{t("close")}</button>
  {/snippet}
</Modal>

<style>
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 50vh;
    overflow-y: auto;
  }
  li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 7px 0;
    border-bottom: 1px solid var(--soft);
  }
  .when {
    font: 500 14px var(--font-mono);
  }
  li button {
    margin-left: auto;
  }
  .bad {
    color: var(--bad);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 260px;
  }
  .spacer {
    flex: 1;
  }
</style>
