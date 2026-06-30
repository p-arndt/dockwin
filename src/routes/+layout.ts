// SPA mode: no server-side rendering, no prerendering. The app runs entirely in
// the browser/webview and talks to dockwin-core over Tauri IPC, which only exists
// at runtime. adapter-static emits a single fallback shell (see svelte.config.js).
export const ssr = false;
export const prerender = false;
