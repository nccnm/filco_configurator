<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    label: string;
    testid: string;
    disabled?: boolean;
    children: Snippet<[() => void]>;
  }
  let { label, testid, disabled = false, children }: Props = $props();
  let open = $state(false);
  let root: HTMLDivElement | undefined = $state();

  const close = () => (open = false);

  function onwindowclick(e: MouseEvent) {
    if (open && root && !root.contains(e.target as Node)) close();
  }
  function onkeydown(e: KeyboardEvent) {
    if (open && e.key === "Escape") {
      e.stopPropagation();
      close();
    }
  }
</script>

<svelte:window onclick={onwindowclick} />

<div class="menu" bind:this={root} {onkeydown} role="presentation">
  <button
    aria-haspopup="menu"
    aria-expanded={open}
    {disabled}
    onclick={() => (open = !open)}
    data-testid={testid}
  >
    {label}
    <svg width="9" height="6" viewBox="0 0 9 6" aria-hidden="true"
      ><path d="M1 1l3.5 3.5L8 1" fill="none" stroke="currentColor" stroke-width="1.4" /></svg
    >
  </button>
  {#if open}
    <div class="items" role="menu">{@render children(close)}</div>
  {/if}
</div>

<style>
  .menu {
    position: relative;
  }
  .items {
    position: absolute;
    right: 0;
    top: calc(100% + 4px);
    z-index: 20;
    min-width: 260px;
    background: var(--paper);
    border: 1px solid var(--line);
    border-radius: 8px;
    box-shadow: var(--pop-shadow);
    padding: 5px;
    display: grid;
  }
  .items :global(button) {
    border-color: transparent;
    justify-content: flex-start;
    width: 100%;
    padding: 6px 10px;
  }
  .items :global(button:hover:not(:disabled)) {
    background: var(--soft);
    border-color: transparent;
  }
  .items :global(hr) {
    border: none;
    border-top: 1px solid var(--soft);
    margin: 4px 2px;
  }
  .items :global(.label) {
    font: 500 12px var(--font-key);
    opacity: 0.65;
    padding: 4px 10px 2px;
  }
  .items :global(.row) {
    display: flex;
    gap: 4px;
    padding: 2px 6px 4px;
  }
  .items :global(.row button) {
    width: auto;
    border-color: var(--line);
    justify-content: center;
    flex: 1;
  }
</style>
