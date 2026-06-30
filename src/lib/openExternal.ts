// Tiny helper to open a URL in the OS default browser via the Tauri opener
// plugin. Centralised so view components don't each import the plugin directly.
// (The plugin install/registration is owned by a separate agent.)
import { openUrl } from "@tauri-apps/plugin-opener";

/**
 * Open `url` in the user's default external browser.
 * Swallows/rethrows nothing on the happy path; returns the underlying promise so
 * callers may await + surface failures if they care.
 */
export async function openExternal(url: string): Promise<void> {
  await openUrl(url);
}
