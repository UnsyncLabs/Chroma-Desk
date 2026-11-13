<script lang="ts">
  /** Name + tags form that turns the current drawing into a library animation. */
  import { t } from "./i18n/index.svelte";

  let {
    onsave,
    defaultName = "",
    label = "",
    busy = false,
  }: {
    onsave: (name: string, tags: string[]) => void | Promise<void>;
    defaultName?: string;
    label?: string;
    busy?: boolean;
  } = $props();

  let name = $state("");
  let tags = $state("");
  $effect(() => {
    if (!name) name = defaultName;
  });

  function submit(e: SubmitEvent) {
    e.preventDefault();
    onsave(name.trim(), tags.split(",").map((x) => x.trim().replace(/^#/, "")).filter(Boolean));
  }
</script>

<form class="save" onsubmit={submit}>
  <label>
    <span>{t("anim.name")}</span>
    <input type="text" bind:value={name} maxlength="64" required placeholder={t("save.namePlaceholder")} />
  </label>
  <label>
    <span>{t("save.tags")}</span>
    <input type="text" bind:value={tags} placeholder={t("save.tagsPlaceholder")} />
  </label>
  <button class="primary" type="submit" disabled={busy || !name.trim()}>{label || t("save.saveAs")}</button>
</form>

<style>
  .save { display: flex; gap: 0.6rem; align-items: flex-end; flex-wrap: wrap; }
  label { display: flex; flex-direction: column; gap: 0.15rem; font-size: 0.85rem; color: var(--muted); min-width: 0; }
  input { min-width: 9rem; padding: 0.35rem 0.5rem; color: var(--text); }
</style>
