// Theme + accent foundation store (Svelte 5 runes module).
//
// Owns two independent axes:
//   • theme   — "dark" (the hero) | "light", written to <html data-theme>.
//   • accent  — one of three lime shades, applied as inline --lime* vars on
//               <html>. Shade 0 is the default and writes NO inline vars, so the
//               per-theme default accent defined in app.css (incl. the light
//               override) is respected. Shades 1/2 write inline overrides.
//
// Both axes persist to localStorage and re-apply on construction (boot).

export type Theme = "dark" | "light";

// The three swappable accent shades. Index 0 = default (CSS-driven, no inline).
export interface AccentShade {
  l: string; // --lime
  b: string; // --lime-bright
  d: string; // --lime-deep
}
export const ACCENT_SHADES: readonly AccentShade[] = [
  { l: "#a6e35b", b: "#b7ee70", d: "#7eb83c" }, // default
  { l: "#c3ee82", b: "#d2f59b", d: "#9bcc55" }, // brighter
  { l: "#8fd95f", b: "#a3e377", d: "#6cb53e" }, // greener
] as const;

const THEME_KEY = "dockwin.theme";
const ACCENT_KEY = "dockwin.accent";

function readStoredTheme(): Theme {
  try {
    const v = localStorage.getItem(THEME_KEY);
    if (v === "light" || v === "dark") return v;
  } catch {
    /* ignore (SSR / privacy mode) */
  }
  return "dark";
}

function readStoredAccent(): number {
  try {
    const v = Number(localStorage.getItem(ACCENT_KEY));
    if (Number.isInteger(v) && v >= 0 && v < ACCENT_SHADES.length) return v;
  } catch {
    /* ignore */
  }
  return 0;
}

function applyTheme(theme: Theme): void {
  if (typeof document === "undefined") return;
  document.documentElement.setAttribute("data-theme", theme);
}

function applyAccent(index: number): void {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  if (index <= 0) {
    // Default: clear inline overrides so the CSS (theme-aware) default wins.
    root.style.removeProperty("--lime");
    root.style.removeProperty("--lime-bright");
    root.style.removeProperty("--lime-deep");
    return;
  }
  const s = ACCENT_SHADES[index];
  root.style.setProperty("--lime", s.l);
  root.style.setProperty("--lime-bright", s.b);
  root.style.setProperty("--lime-deep", s.d);
}

class ThemeStore {
  theme = $state<Theme>(readStoredTheme());
  accent = $state<number>(readStoredAccent());

  constructor() {
    // Apply the persisted values immediately on construction (module boot).
    applyTheme(this.theme);
    applyAccent(this.accent);
  }

  setTheme(theme: Theme): void {
    this.theme = theme;
    applyTheme(theme);
    try {
      localStorage.setItem(THEME_KEY, theme);
    } catch {
      /* ignore */
    }
  }

  toggleTheme(): void {
    this.setTheme(this.theme === "dark" ? "light" : "dark");
  }

  setAccent(index: number): void {
    if (index < 0 || index >= ACCENT_SHADES.length) return;
    this.accent = index;
    applyAccent(index);
    try {
      localStorage.setItem(ACCENT_KEY, String(index));
    } catch {
      /* ignore */
    }
  }
}

// Singleton — import { theme } and read theme.theme / theme.accent reactively.
export const theme = new ThemeStore();
