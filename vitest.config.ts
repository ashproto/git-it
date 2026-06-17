import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  // The svelte plugin compiles `.svelte` and `.svelte.ts`/`.svelte.js` (runes)
  // modules, so a runes store like `src/lib/store.svelte.ts` ($state/$derived)
  // can be imported and exercised in unit tests. Pure `.ts` tests are unaffected
  // (vitest's own esbuild handles them). hot:false — no HMR under test.
  plugins: [svelte({ hot: false })],
  test: {
    include: ["src/**/*.test.ts"],
    // The store is pure logic (no DOM rendering); all browser globals it touches
    // (window/localStorage) are typeof-guarded, so the node environment is enough.
    environment: "node",
  },
  resolve: {
    // Resolve Svelte's client build so $state/$derived signal reactivity works in
    // tests (the canonical Svelte-5-runes-under-Vitest setup). Additive to Vite's
    // base conditions. Caveat: this makes the web resolver prefer "browser" exports
    // even under the node environment — fine for everything imported today (the
    // existing pure-.ts deps have no browser/node-divergent exports), but a future
    // test importing such a dep would get its browser build.
    conditions: ["browser"],
  },
});
