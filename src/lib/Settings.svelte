<script lang="ts">
  /** Settings screen. */
  import { t, getLocale, setLocale, LANGUAGES } from "./i18n/index.svelte";
  import { profiles, saveSettings } from "./profiles.svelte";

  function changeLanguage(code: string) {
    setLocale(code);
    profiles.settings.language = code;
    saveSettings();
  }
</script>

<h2>{t("nav.settings")}</h2>

<section>
  <h3>{t("settings.language")}</h3>
  <select value={getLocale()} onchange={(e) => changeLanguage(e.currentTarget.value)} aria-label={t("settings.language")}>
    {#each LANGUAGES as l (l.code)}<option value={l.code}>{l.name}</option>{/each}
  </select>
</section>

<style>
  section { margin-bottom: 1.4rem; }
</style>
