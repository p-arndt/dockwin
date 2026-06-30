// Theme foundation store (Svelte 5 runes module).
//
// Owns a single axis:
//   • theme — "dark" (the hero) | "light", applied as the `.dark` class on
//     <html> (the shadcn-svelte convention) plus a matching color-scheme.
//
// Persists to localStorage and re-applies on construction (boot). The palette
// itself is defined (per theme) in app.css; there is no runtime swap.

export type Theme = "dark" | "light";

const THEME_KEY = "dockwin.theme";

function readStoredTheme(): Theme {
  try {
    const v = localStorage.getItem(THEME_KEY);
    if (v === "light" || v === "dark") return v;
  } catch {
    /* ignore (SSR / privacy mode) */
  }
  return "dark";
}

function applyTheme(theme: Theme): void {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  root.classList.toggle("dark", theme === "dark");
  root.style.colorScheme = theme;
}

class ThemeStore {
  theme = $state<Theme>(readStoredTheme());

  constructor() {
    // Apply the persisted value immediately on construction (module boot).
    applyTheme(this.theme);
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
}

// Singleton — import { theme } and read theme.theme reactively.
export const theme = new ThemeStore();
