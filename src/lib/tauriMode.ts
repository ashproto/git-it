// Sets data-tauri="true" on <html> when running inside the Tauri shell.
// CSS in +page.svelte uses that attribute to opt into the glassy/translucent look
// without affecting normal browser dev (npm run dev). The attribute is set as
// early as possible via a module side effect so styles apply before paint.
//
// Three independent signals are checked so detection can't miss:
//   1. Build-time:  import.meta.env.TAURI_ENV_PLATFORM (requires envPrefix in vite.config.js)
//   2. Runtime:     a few globals Tauri may inject (covers withGlobalTauri variations)
//   3. Manual:      ?translucent query param — useful for previewing glass mode in a browser

const BUILD_INSIDE_TAURI = Boolean(
  (import.meta as unknown as { env?: Record<string, unknown> }).env?.TAURI_ENV_PLATFORM,
);

function markIfTauri(): boolean {
  if (typeof document === "undefined") return false;
  const w = window as unknown as Record<string, unknown>;
  const runtimeTauri =
    "__TAURI_INTERNALS__" in w ||
    "__TAURI__" in w ||
    "isTauri" in w ||
    "__TAURI_IPC__" in w ||
    "__TAURI_METADATA__" in w;
  const forcedByQuery = new URL(window.location.href).searchParams.has("translucent");
  if (BUILD_INSIDE_TAURI || runtimeTauri || forcedByQuery) {
    document.documentElement.setAttribute("data-tauri", "true");
    return true;
  }
  return false;
}

if (!markIfTauri() && typeof window !== "undefined") {
  // Retry on the next microtask and on DOMContentLoaded — the runtime globals
  // are sometimes injected slightly after module evaluation.
  queueMicrotask(markIfTauri);
  window.addEventListener("DOMContentLoaded", markIfTauri, { once: true });
}
