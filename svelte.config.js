import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
export default {
  // Enable TypeScript-in-markup (<script lang="ts">) and other standard preprocessing.
  preprocess: vitePreprocess(),

  kit: {
    // SPA: a single fallback shell, all routing happens client-side. There is no
    // server — Tauri loads the built assets straight from disk (see +layout.ts,
    // which disables SSR/prerendering app-wide).
    adapter: adapter({ fallback: "index.html" }),
    // Emit relative asset URLs so Tauri can load the bundle from its asset protocol.
    paths: { relative: true },
  },
};
