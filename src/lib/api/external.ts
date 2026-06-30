// Tiny helpers around the Tauri opener plugin. Centralised so view components
// don't each import the plugin directly.
import { openUrl, openPath } from "@tauri-apps/plugin-opener";

/**
 * Open `url` in the user's default external browser.
 * Returns the underlying promise so callers may await + surface failures.
 */
export async function openExternal(url: string): Promise<void> {
  await openUrl(url);
}

/**
 * Translate an engine-side (WSL) path to a Windows path so Explorer can open it.
 * `/mnt/e/foo/bar` → `E:\foo\bar`; UNC and already-Windows paths pass through.
 */
export function wslToWindowsPath(p: string): string {
  const m = /^\/mnt\/([a-z])(\/.*)?$/i.exec(p);
  if (m) {
    const drive = m[1].toUpperCase();
    const rest = (m[2] ?? "").replace(/\//g, "\\");
    return `${drive}:${rest || "\\"}`;
  }
  return p;
}

/**
 * Open a folder in the OS file manager (Explorer). Engine-side `/mnt/<drive>/…`
 * paths are translated to their Windows equivalent first.
 */
export async function openFolder(path: string): Promise<void> {
  await openPath(wslToWindowsPath(path));
}
