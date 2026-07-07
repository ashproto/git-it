// Sets data-theme="nerv" on <html> BEFORE first paint when the saved theme is NERV,
// so launch never flashes Classic. Mirrors tauriMode.ts: a synchronous module
// side-effect, guarded for SSR/build/test (no document/localStorage). The durable
// source of truth is the Tauri store (hydrated a tick later by store.svelte.ts); we
// also persist the theme to localStorage (see store.svelte.ts persistTheme, Task A4)
// so this synchronous read is current on the next launch.
const THEME_KEY = "gitit.theme.v1";

function applySavedTheme(): void {
  if (typeof document === "undefined") return;
  try {
    if (typeof localStorage === "undefined") return;
    if (localStorage.getItem(THEME_KEY) === "nerv") {
      document.documentElement.setAttribute("data-theme", "nerv");
    }
    // "classic" / null → attribute absent (Classic is the default)
  } catch {
    // ignore — Classic is a safe default
  }
}

applySavedTheme();
