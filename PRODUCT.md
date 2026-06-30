# Product

## Register

product

## Users

Developers and small/medium engineering teams on Windows 11 who currently run, or are evaluating an alternative to, Docker Desktop — often because of its per-seat licensing cost at scale, or because they want a leaner local setup. They use dockwin to provision a dedicated WSL2 Docker engine and then manage day-to-day container work: starting/stopping containers and compose stacks, checking logs and stats, pulling/pruning images, managing volumes and networks. They're comfortable with Docker concepts (and often the CLI) but want a fast native GUI for the parts that benefit from one — lists, status at a glance, log tail, port links — without the GUI getting in the way of the engine underneath.

## Product Purpose

dockwin is a lightweight, native Windows 11 GUI + CLI for a single dedicated, stock-`dockerd`-in-WSL2 engine — built so the wrapper around Docker Engine is as free and unencumbered as the engine itself. It exists to give Docker Desktop refugees a small, honest alternative: no persistent background service, no VPN proxy, no telemetry, no auto-updater, no upsell surface. Success looks like a developer forgetting dockwin is "an app" at all — it provisions once, then gets out of the way, surfacing exactly the container/image/volume/network state they need and nothing else.

## Brand Personality

Refined, warm, professional. The interface pairs dense, no-nonsense data views (tables, stats, logs — the stuff a Docker power user actually needs) with a shell that feels considered and friendly rather than cold or terminal-grim. Confidence comes from restraint and precision, not decoration: one accent color used sparingly, quiet status indicators, no chrome for chrome's sake. It should read as crafted by someone who cares, not generated from a dashboard template.

## Anti-references

- **Docker Desktop's bloat** — heavy chrome, nags/upsells, telemetry-vibe surfaces, sluggish feel. The entire reason dockwin exists is to not be this.
- **Generic AI-lime-glow dashboards** — overused lime-on-dark glow effects and glowing filled status pill-bands. Already explicitly rejected once during the UI overhaul; the accent (lime) is restricted to ~4 deliberate roles (primary action, active-nav rail, selected-row rail, one focused chart) and must never become ambient decoration.
- **Generic SaaS admin templates** — cookie-cutter identical card grids, gradient text, tiny uppercase eyebrow labels, hero-metric clichés. dockwin's views are real dense data tools, not a SaaS marketing dashboard wearing a tool's clothes.

## Design Principles

- **The wrapper should be as free as the engine** — every UI decision should reinforce "lightweight and honest," not "feature-maximalist." When in doubt, cut chrome rather than add it.
- **Dense data, friendly shell** — don't sacrifice information density (tables, stats, logs) to chase visual minimalism; the warmth comes from shell-level craft (rounding, color restraint, motion), not from hiding data.
- **Accent is a signal, not a paint job** — lime marks the handful of things that matter (primary action, active state, selection, one chart) precisely so it keeps meaning when it appears.
- **Status is quiet** — communicate state with a small dot, a word, and a muted sub-line, never a glowing badge. Loud status UI is a tell, not a feature.
- **Native-feeling over webby** — prefer patterns that feel like a real Windows desktop tool (thin themed scrollbars, no user-select sprawl, system-ish typography) over generic web-app conventions.

## Accessibility & Inclusion

Standard desktop-app baseline: full keyboard navigation, visible focus states, and body text contrast sufficient for extended reading sessions (a developer may have this open all day). No formal WCAG audit target at this stage, but no avoidable barriers — reduced-motion alternatives for any animation, and status communicated by more than color alone (dot + word + sub-line, not color-only).
