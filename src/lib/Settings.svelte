<script lang="ts">
  /** Settings screen. */
  import { t, getLocale, setLocale, LANGUAGES } from "./i18n/index.svelte";
</script>

<h2>{t("nav.settings")}</h2>

<section>
  <h3>{t("settings.language")}</h3>
  <select value={getLocale()} onchange={(e) => setLocale(e.currentTarget.value)} aria-label={t("settings.language")}>
    {#each LANGUAGES as l (l.code)}<option value={l.code}>{l.name}</option>{/each}
  </select>
</section>

<style>
  section { margin-bottom: 1.4rem; }
</style>
