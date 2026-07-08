// Sets data-theme="nerv" on <html> BEFORE first paint when the saved theme is NERV,
// so launch never flashes Classic. Mirrors tauriMode.ts: a synchronous module
// side-effect, guarded for SSR/build/test (no document/localStorage). The durable
// source of truth is the Tauri store (hydrated a tick later by store.svelte.ts); we
// also persist the theme to localStorage (see store.svelte.ts persistTheme, Task A4)
// so this synchronous read is current on the next launch.
const THEME_KEY = "gitit.theme.v1";

// Same pre-paint mechanism for the NERV color scheme preset (see store.svelte.ts
// persistScheme, Task 4). Setting data-scheme unconditionally (even "orange", the
// default) is harmless — it's only consumed by CSS when data-theme="nerv" is also set.
const SCHEME_KEY = "gitit.scheme.v1";
const VALID_SCHEMES = new Set(["orange", "phosphor", "steel", "amber", "violet", "crimson"]);

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

function applySavedScheme(): void {
  if (typeof document === "undefined") return;
  try {
    if (typeof localStorage === "undefined") return;
    const saved = localStorage.getItem(SCHEME_KEY);
    document.documentElement.dataset.scheme = saved && VALID_SCHEMES.has(saved) ? saved : "orange";
  } catch {
    // ignore — "orange" is a safe default
  }
}

applySavedTheme();
applySavedScheme();
