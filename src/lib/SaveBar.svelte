<script lang="ts">
  /** Name + tags form that turns the current drawing into a library animation. */
  import type { Role } from "./api";
  import { t } from "./i18n/index.svelte";

  let {
    onsave,
    defaultName = "",
    defaultTags = [],
    label = "",
    busy = false,
    canOverwrite = false,
    overwriteName = "",
    defaultRole = "theme",
  }: {
    onsave: (name: string, tags: string[], asNew: boolean, role: Role) => unknown;
    defaultName?: string;
    defaultTags?: string[];
    label?: string;
    busy?: boolean;
    /** Editing an existing library entry: offer "Update" next to "Save as new". */
    canOverwrite?: boolean;
    overwriteName?: string;
    /** Where it shows up in Quick: base look ("theme") or stackable overlay ("effect"). */
    defaultRole?: Role;
  } = $props();

  // svelte-ignore state_referenced_locally
  let name = $state(defaultName);
  // svelte-ignore state_referenced_locally
  let tags = $state(defaultTags.join(", "));
  // svelte-ignore state_referenced_locally
  let role = $state<Role>(defaultRole);
  let submitter: "update" | "new" = "new";

  function submit(e: SubmitEvent) {
    e.preventDefault();
    onsave(name.trim(), tags.split(",").map((x) => x.trim().replace(/^#/, "")).filter(Boolean), !(canOverwrite && submitter === "update"), role);
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
  <div class="role" role="radiogroup" aria-label={t("quick.role")}>
    <span>{t("quick.role")}</span>
    <div class="seg">
      {#each ["theme", "effect"] as const as r (r)}
        <button type="button" class="pill" class:on={role === r} role="radio" aria-checked={role === r} data-role-pick={r} title={t(r === "theme" ? "quick.roleThemeHint" : "quick.roleEffectHint")} onclick={() => (role = r)}>{t(r === "theme" ? "quick.roleTheme" : "quick.roleEffect")}</button>
      {/each}
    </div>
  </div>
  {#if canOverwrite}
    <button class="primary" type="submit" disabled={busy || !name.trim()} onclick={() => (submitter = "update")}>{t("save.update", { name: overwriteName })}</button>
    <button type="submit" disabled={busy || !name.trim()} onclick={() => (submitter = "new")}>{t("save.asNew")}</button>
  {:else}
    <button class="primary" type="submit" disabled={busy || !name.trim()} onclick={() => (submitter = "new")}>{label || t("save.saveAs")}</button>
  {/if}
</form>

<style>
  .save { display: flex; gap: 0.6rem; align-items: flex-end; flex-wrap: wrap; }
  label { display: flex; flex-direction: column; gap: 0.15rem; font-size: 0.85rem; color: var(--muted); min-width: 0; }
  .role { display: flex; flex-direction: column; gap: 0.15rem; font-size: 0.85rem; color: var(--muted); }
  .seg { display: flex; gap: 0.3rem; }
  .pill { padding: 0.3rem 0.8rem; border-radius: 999px; }
  .pill.on { border-color: var(--ok); color: var(--ok); box-shadow: 0 0 0 1px var(--ok) inset; }
  input { min-width: 9rem; padding: 0.35rem 0.5rem; color: var(--text); }
</style>
