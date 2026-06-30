// Tiny helpers around the Tauri opener plugin. Centralised so view components
// don't each import the plugin directly.
import { openUrl, openPath } from "@tauri-apps/plugin-opener";

// Schemes we are willing to hand to the OS shell. Several call sites open URLs
// derived from engine/container data (published-port links), so we never let a
// `file:`, `javascript:`, `vbscript:` … scheme through to ShellExecute.
const ALLOWED_URL_SCHEMES = new Set(["http:", "https:"]);

/**
 * Open `url` in the user's default external browser, restricted to http(s).
 * Returns the underlying promise so callers may await + surface failures.
 */
export async function openExternal(url: string): Promise<void> {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    throw new Error(`refusing to open malformed URL: ${url}`);
  }
  if (!ALLOWED_URL_SCHEMES.has(parsed.protocol)) {
    throw new Error(`refusing to open non-web URL (${parsed.protocol}): ${url}`);
  }
  await openUrl(parsed.toString());
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

// Final path segments we refuse to hand to the OS shell: openPath() routes
// through ShellExecute, which would EXECUTE these rather than reveal a folder.
const EXECUTABLE_EXTENSIONS = new Set([
  "exe", "com", "bat", "cmd", "ps1", "psm1", "vbs", "vbe", "js", "jse", "wsf",
  "wsh", "msi", "msp", "msc", "scr", "lnk", "url", "hta", "cpl", "reg", "pif",
  "jar", "scf", "inf",
]);

/**
 * Open a folder in the OS file manager (Explorer).
 *
 * The `path` here is a container-controlled compose label
 * (`com.docker.compose.project.working_dir`), so a hostile image could set it to
 * a UNC share, an executable, or a `.lnk` — and openPath() would hand that to
 * ShellExecute and run it. A genuine compose working_dir is always an engine-side
 * `/mnt/<drive>/…` mount path, so we (1) require exactly that shape (blocking UNC
 * and arbitrary Windows paths) and (2) refuse paths ending in an executable
 * extension, before translating to the Windows path and opening it.
 */
export async function openFolder(path: string): Promise<void> {
  if (!/^\/mnt\/[a-z](\/|$)/i.test(path)) {
    throw new Error(`refusing to open untrusted path: ${path}`);
  }
  const ext = path.split("/").pop()?.split(".").pop()?.toLowerCase() ?? "";
  if (EXECUTABLE_EXTENSIONS.has(ext)) {
    throw new Error(`refusing to open executable path: ${path}`);
  }
  await openPath(wslToWindowsPath(path));
}
