// App self-update via Tauri's updater plugin.
//
// This is the ONLY place the frontend talks to the updater/process plugins,
// mirroring how api.ts owns the dockwin-core bridge. It checks GitHub Releases
// for a newer SIGNED dockwin installer (see src-tauri/tauri.conf.json
// `plugins.updater`), and — only when the user asks — downloads, installs, and
// relaunches. Nothing here runs automatically in the background.
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

// A pending app update the user can choose to install.
export interface AppUpdateInfo {
  version: string; // the new version (e.g. "0.2.0")
  currentVersion: string; // what's running now
  notes: string | null; // release notes, when the manifest carries them
  date: string | null; // pub_date from the manifest
  // The live plugin handle, kept so install() can download + apply it.
  handle: Update;
}

// Download/install progress for the update bar (bytes-based).
export interface AppUpdateProgress {
  downloaded: number;
  total: number | null;
  done: boolean;
}

// Check for a newer signed release. Returns null when up to date (or when the
// updater endpoint is unreachable — callers treat a thrown error as "couldn't
// check" and stay quiet). Safe to call on launch.
export async function checkAppUpdate(): Promise<AppUpdateInfo | null> {
  const update = await check();
  if (!update) return null;
  return {
    version: update.version,
    currentVersion: update.currentVersion,
    notes: update.body ?? null,
    date: update.date ?? null,
    handle: update,
  };
}

// Download + install the update, reporting byte progress, then relaunch into
// the new version. Throws on signature mismatch or a failed download.
export async function installAppUpdate(
  info: AppUpdateInfo,
  onProgress?: (p: AppUpdateProgress) => void
): Promise<void> {
  let downloaded = 0;
  let total: number | null = null;
  await info.handle.downloadAndInstall((event) => {
    switch (event.event) {
      case "Started":
        total = event.data.contentLength ?? null;
        onProgress?.({ downloaded: 0, total, done: false });
        break;
      case "Progress":
        downloaded += event.data.chunkLength;
        onProgress?.({ downloaded, total, done: false });
        break;
      case "Finished":
        onProgress?.({ downloaded, total, done: true });
        break;
    }
  });
  // The new installer has been applied; restart into it.
  await relaunch();
}
