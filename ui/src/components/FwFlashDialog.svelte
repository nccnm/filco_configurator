<script lang="ts">
  import type { FwFlashView, ModelView } from "../bindings/ipc";
  import type { Key } from "../lib/i18n";
  import Modal from "./Modal.svelte";

  interface Props {
    flash: FwFlashView;
    model: ModelView;
    busy: boolean;
    t: (key: Key, vars?: Record<string, string | number>) => string;
    onlaunch: () => void;
    onopenfolder: () => void;
    onclose: () => void;
  }
  let { flash, model, busy, t, onlaunch, onopenfolder, onclose }: Props = $props();

  const tag = $derived(flash.tag.toString(16).toUpperCase().padStart(4, "0"));
  const done = $derived(flash.step === "done");
  const layers = $derived(
    flash.layers_changed.map((l) => model.layers[l]?.label ?? String(l + 1)).join(", "),
  );
</script>

<Modal
  title={t("fw_flash_title", { name: flash.name })}
  testid="fw-flash-dialog"
  wide
  onclose={flash.step === "waiting" ? undefined : onclose}
>
  <ol class="steps" data-step={flash.step}>
    <li class="ok">
      <b>1. {t("fw_flash_s1")}</b>
      {#if flash.backup}<span class="small muted path">{flash.backup}</span>{/if}
    </li>
    <li class="ok"><b>2. {t("fw_flash_s2")}</b></li>
    <li class:ok={flash.step !== "ready"} class:now={flash.step === "ready" || flash.step === "waiting"}>
      <b>3. {t("fw_flash_s3")}</b>
      <div class="tag">CheckSum : <span class="mono">0x{tag}</span></div>
      <p class="small">{t("fw_flash_s3_text", { tag })}</p>
      <div class="buttons">
        <button class="primary" disabled={busy || done} onclick={onlaunch} data-testid="fw-flash-launch"
          >{t("fw_flash_open")}</button
        >
        <button onclick={onopenfolder}>{t("fw_open_folder")}</button>
      </div>
      {#if flash.step === "waiting"}
        <p class="small wait" data-testid="fw-flash-waiting">
          {flash.saw_unplug ? t("fw_flash_gone") : t("fw_flash_waiting")}
        </p>
      {/if}
    </li>
    <li
      class:now={done}
      class:ok={done && flash.firmware_ok === true}
      class:bad={done && flash.firmware_ok === false}
    >
      <b>4. {t("fw_flash_s4")}</b>
      {#if done}
        <p class="small" data-testid="fw-flash-result">
          {flash.firmware_ok
            ? t("fw_flash_ok", { name: flash.name, tag })
            : t("fw_flash_bad", { name: flash.firmware_name ?? t("fw_unknown") })}
        </p>
        <p class="small">
          {flash.layers_changed.length === 0 ? t("fw_flash_layers_ok") : t("fw_flash_layers_bad", { layers })}
        </p>
      {/if}
    </li>
  </ol>
  {#snippet footer()}
    {#if flash.step !== "waiting"}
      <button class={done ? "primary" : ""} onclick={onclose} data-testid="fw-flash-close"
        >{done ? t("close") : t("cancel")}</button
      >
    {/if}
  {/snippet}
</Modal>

<style>
  .steps {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 8px;
  }
  .steps li {
    padding: 8px 12px;
    border: 1px solid var(--line);
    border-radius: 7px;
    display: grid;
    gap: 4px;
  }
  .steps li.ok {
    border-color: color-mix(in srgb, var(--good) 60%, var(--line));
  }
  .steps li.now {
    border-color: var(--acc);
    box-shadow: inset 0 0 0 1px var(--acc);
  }
  .steps li.bad {
    border-color: var(--bad);
    background: var(--bad-bg);
  }
  .steps li.ok > b::after {
    content: " ✓";
    color: var(--good);
  }
  .path {
    word-break: break-all;
  }
  .tag {
    font: 600 22px var(--font-key);
  }
  .tag .mono {
    font-size: 22px;
  }
  p {
    margin: 0;
  }
  .buttons {
    display: flex;
    gap: 8px;
    margin-top: 4px;
  }
  .wait {
    color: var(--acc);
  }
</style>
