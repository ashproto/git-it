// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a fallback to index.html to put the site in SPA mode
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
import "$lib/tauriMode";
import "$lib/theme/themeMode"; // sets data-theme before paint (no-FOUC)
import "$lib/theme/nerv.css";  // global NERV stylesheet

export const ssr = false;
