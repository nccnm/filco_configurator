<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    title: string;
    testid: string;
    wide?: boolean;
    /** Esc and the close button; omitted while the dialog must not be closed (e.g. while writing). */
    onclose?: () => void;
    children: Snippet;
    footer?: Snippet;
  }
  let { title, testid, wide = false, onclose, children, footer }: Props = $props();

  let el: HTMLDialogElement | undefined = $state();

  $effect(() => {
    if (el && !el.open) el.showModal();
  });

  function oncancel(e: Event) {
    e.preventDefault();
    onclose?.();
  }
</script>

<dialog bind:this={el} class:wide {oncancel} data-testid={testid} aria-labelledby={`${testid}-title`}>
  <header>
    <h2 id={`${testid}-title`}>{title}</h2>
  </header>
  <div class="body">{@render children()}</div>
  {#if footer}
    <footer>{@render footer()}</footer>
  {/if}
</dialog>

<style>
  dialog {
    width: min(620px, calc(100vw - 48px));
    max-height: calc(100vh - 64px);
    padding: 0;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--paper);
    color: var(--ink);
    box-shadow: var(--dialog-shadow);
    display: flex;
    flex-direction: column;
  }
  dialog:not([open]) {
    display: none;
  }
  dialog.wide {
    width: min(920px, calc(100vw - 48px));
  }
  dialog::backdrop {
    background: var(--scrim);
  }
  header {
    padding: 16px 22px 10px;
    border-bottom: 1px solid var(--soft);
  }
  h2 {
    font: 600 19px var(--font-key);
  }
  .body {
    padding: 14px 22px;
    overflow-y: auto;
    display: grid;
    gap: 10px;
  }
  footer {
    padding: 12px 22px 16px;
    border-top: 1px solid var(--soft);
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
