<script lang="ts">
  // A file path that wraps only after a separator, never inside a folder name, a model id or a timestamp.
  let { path }: { path: string } = $props();
  const parts = $derived(path.split(/(?<=[\\/])/));
</script>

<code class="path"
  >{#each parts as p, i (i)}<span>{p}</span><wbr />{/each}</code
>

<style>
  span {
    white-space: nowrap;
  }
</style>
