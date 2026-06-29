import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

export default {
  // Enable TypeScript-in-markup (<script lang="ts">) and other standard preprocessing.
  preprocess: vitePreprocess(),
};
