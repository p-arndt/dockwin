---
name: dockwin
description: A lightweight, native Windows 11 GUI for a single dedicated WSL2 Docker engine.
colors:
  refined-lime-light: "#7fb53a"
  refined-lime-dark: "#a6e35b"
  neutral-bg-light: "#f5f6f8"
  neutral-bg-dark: "#08090c"
  surface-card-light: "#ffffff"
  surface-card-dark: "#15171d"
  surface-popover-light: "#fafbfc"
  surface-popover-dark: "#1b1e26"
  ink-light: "#161a20"
  ink-dark: "#f3f4f6"
  ink-muted-light: "#7b828d"
  ink-muted-dark: "#727a86"
  border-light: "rgba(16, 24, 40, 0.09)"
  border-dark: "rgba(255, 255, 255, 0.065)"
  status-ok-light: "#179a5f"
  status-ok-dark: "#54c98a"
  status-warn-light: "#b07816"
  status-warn-dark: "#e0aa46"
  status-err-light: "#cf3f50"
  status-err-dark: "#e5727f"
  status-off-light: "#9aa2ad"
  status-off-dark: "#5a626d"
typography:
  display:
    fontFamily: "Segoe UI Variable Display, Segoe UI, system-ui, -apple-system, sans-serif"
    fontSize: "1rem"
    fontWeight: 600
    lineHeight: 1.3
    letterSpacing: "normal"
  body:
    fontFamily: "Segoe UI Variable Display, Segoe UI, system-ui, -apple-system, sans-serif"
    fontSize: "13.5px"
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: "normal"
  label:
    fontFamily: "Segoe UI Variable Display, Segoe UI, system-ui, -apple-system, sans-serif"
    fontSize: "10.5px"
    fontWeight: 600
    lineHeight: 1.2
    letterSpacing: "0.05em"
  mono:
    fontFamily: "Cascadia Code, JetBrains Mono, ui-monospace, Consolas, monospace"
    fontSize: "12.5px"
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: "normal"
rounded:
  sm: "0.25rem"
  md: "0.35rem"
  lg: "0.45rem"
  xl: "0.65rem"
  full: "999px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "13px"
  lg: "16px"
  xl: "24px"
components:
  button-primary:
    backgroundColor: "{colors.refined-lime-light}"
    textColor: "#11210a"
    rounded: "{rounded.lg}"
    height: "32px"
    padding: "0 10px"
  button-secondary:
    backgroundColor: "{colors.surface-popover-light}"
    textColor: "{colors.ink-light}"
    rounded: "{rounded.lg}"
    height: "32px"
    padding: "0 10px"
  button-success:
    backgroundColor: "{colors.status-ok-light}"
    textColor: "{colors.status-ok-light}"
    rounded: "{rounded.lg}"
    height: "32px"
    padding: "0 10px"
  button-destructive:
    backgroundColor: "{colors.status-err-light}"
    textColor: "{colors.status-err-light}"
    rounded: "{rounded.lg}"
    height: "32px"
    padding: "0 10px"
  badge-default:
    backgroundColor: "{colors.refined-lime-light}"
    textColor: "#11210a"
    rounded: "{rounded.full}"
    height: "20px"
    padding: "2px 8px"
  card:
    backgroundColor: "{colors.surface-card-light}"
    rounded: "{rounded.xl}"
    padding: "13px"
---

# Design System: dockwin

## 1. Overview

**Creative North Star: "The Honest Workshop"**

dockwin's interface is the workshop of someone who builds tools for their own use first: dense, accurate data surfaces (container tables, live stats, log tails, inspect JSON) housed in a shell that's warm and rounded rather than terminal-cold, but never decorated past the point of usefulness. Every visual choice answers to the product's thesis — the wrapper around Docker Engine should be as free and unencumbered as the engine itself — which translates directly into design restraint: no chrome for chrome's sake, no upsell surfaces, no glow for glow's sake.

The system explicitly rejects Docker Desktop's heavy, nag-laden chrome; it rejects the generic AI-tool aesthetic of ambient lime-on-dark glow and glowing filled status pill-bands; and it rejects the cookie-cutter SaaS admin template — identical card grids, gradient text, uppercase eyebrow labels. dockwin reads as crafted by a single attentive developer, not generated from a dashboard kit.

Dark is the hero surface (the default), with light fully first-class — both driven by the same token set via the `.dark` class, so neither is a second-class afterthought.

**Key Characteristics:**
- Dense, table-and-stat-driven data views, not marketing-style whitespace
- One accent color (lime), deployed in exactly four roles, never as ambient decoration
- Quiet status communication: a small dot, one word, a muted sub-line — never a glowing badge
- Native-desktop feel over webby SaaS feel: thin themed scrollbars, system typography, no user-select sprawl
- Dark-first, light fully supported, both built from the same semantic token set

## 2. Colors

The palette is a tinted near-black/near-white neutral pair carrying one deliberate accent; status colors are separated from the brand accent so they never compete with it.

### Primary
- **Refined Lime** (`#7fb53a` light / `#a6e35b` dark): the single brand accent. **The One Voice Rule** governs it — it appears in exactly four roles: the primary action button, the active-nav rail, the selected-table-row rail, and one focused chart series. It is never used as background wash, glow, or ambient decoration.

### Neutral
- **Workshop Ink** (`#161a20` light text / `#f3f4f6` dark text): primary foreground text.
- **Muted Ink** (`#7b828d` light / `#727a86` dark): secondary text, sub-labels, timestamps.
- **Paper / Void** (`#f5f6f8` light bg / `#08090c` dark bg): the app's outermost background, lit by a faint radial lime glow at the top edge only (`backdrop-glow`, 5.5–7% opacity) — never a flat color, never a saturated wash.
- **Card Surface** (`#ffffff` light / `#15171d` dark): elevated content surfaces (tables, panels, dialogs).
- **Popover Surface** (`#fafbfc` light / `#1b1e26` dark): menus, dropdowns, secondary chrome (sidebar accent, secondary buttons).
- **Hairline Border** (`rgba(16,24,40,.09)` light / `rgba(255,255,255,.065)` dark): all dividers and card borders — always translucent over the surface, never a flat gray.

### Status colors (kept separate from the brand accent)
- **Running / OK** (`#179a5f` light / `#54c98a` dark, "emerald"): live/running state. Deliberately distinct from lime so brand and state never get confused.
- **Warning** (`#b07816` light / `#e0aa46` dark, "amber"): paused / restarting.
- **Error** (`#cf3f50` light / `#e5727f` dark, "destructive red"): dead / failed / destructive actions.
- **Off / Idle** (`#9aa2ad` light / `#5a626d` dark, "slate"): exited / stopped / inactive.

### Named Rules
**The One Voice Rule.** Refined Lime appears in exactly four places per screen: primary action, active-nav rail, selected-row rail, one focused chart. Anywhere else it shows up is a bug, not a feature.

**The Lifecycle Override.** Start/stop container actions are the one deliberate exception to brand-accent restraint: start = emerald (`success` button variant), stop = destructive red — borrowed from status color, not lime — because the color needs to encode the action's consequence, not the brand.

## 3. Typography

**Body & Display Font:** Segoe UI Variable Display (with Segoe UI, system-ui, -apple-system, sans-serif fallback)
**Mono Font:** Cascadia Code (with JetBrains Mono, ui-monospace, Consolas fallback)

**Character:** A native Windows system stack, deliberately — it makes the GUI feel like part of the OS rather than a cross-platform web shell. Cascadia Code carries IDs, hashes, ports, and log/inspect output, separating "data to read" from "data to scan."

### Hierarchy
- **Display** (600 weight, ~1rem, 1.3 line-height): section/panel titles, dialog headings — there is no oversized hero type anywhere in the app.
- **Body** (400 weight, 13.5px base, 1.5 line-height): the default reading size for all primary content; small by web standards, sized for a dense desktop tool used all day.
- **Label** (600 weight, 10.5px, 0.05em tracking, uppercase): table column headers and section labels only — the one place uppercase+tracking is allowed, because it marks structural metadata, not body copy.
- **Mono** (400 weight, 12.5px, 1.5 line-height): container/image IDs, ports, log tails, inspect JSON, hashes.

### Named Rules
**The Structural Uppercase Rule.** Uppercase, letter-spaced labels are reserved for table headers and structural metadata. They are never used as a decorative "eyebrow" above section headings — that pattern is explicitly rejected as a generic-SaaS tell.

## 4. Elevation

Quiet depth: surfaces are not flat, but the lift is subtle and reserved for content that benefits from separation from the backdrop. The app's outer background is a soft radial-gradient void (a faint lime glow top-right fading into the base tone), and cards sit on it with a three-layer shadow — a tight contact shadow, a soft ambient falloff, and a 1px inset top highlight that reads as a hairline catching light. Dialogs and popovers use the same vocabulary at a higher resting elevation. There is no glassmorphism and no backdrop-blur anywhere in the system.

### Shadow Vocabulary
- **Card / data-table elevation** (`box-shadow: 0 1px 2px rgba(0,0,0,.45), 0 10px 28px -12px rgba(0,0,0,.6), inset 0 1px 0 rgba(255,255,255,.04)`): primary data surfaces — container/image/volume/network tables, the main content cards.
- **Ambient backdrop glow** (`radial-gradient(1100px 520px at 88% -8%, rgba(166,227,91,var(--backdrop-glow)), transparent 60%)`, glow opacity 5.5–7%): the app shell's outermost background only — never repeated on inner surfaces.

### Named Rules
**The One Lift Rule.** The layered card shadow is reserved for primary data surfaces (tables, main panels). Secondary chrome — sidebar, popovers, badges — stays flush against its parent surface, distinguished by a hairline border and a one-step-lighter/darker fill, not by a second shadow language.

## 5. Components

### Buttons
- **Shape:** rounded corners at `0.45rem` (`--radius`), slightly tighter (`min(--radius-md,10px/12px)`) at the `xs`/`sm` sizes.
- **Default (primary):** filled Refined Lime background, near-black text (`#11210a` on light-lime) — reserved for the one primary action per view.
- **Secondary / Outline / Ghost:** popover-surface or transparent fill, border-on-hover only for outline; used for every non-primary action, which is most of them.
- **Success / Destructive:** soft-filled (10% tint at rest, 20% on hover, 30% in dark), text in the status color itself rather than white-on-solid — quieter than a fully filled button, used specifically for container start/stop.
- **Hover / Focus:** background opacity steps up one tier on hover; focus shows a 3px ring in the variant's own color at 50% opacity plus a 1px border-color shift. Active state nudges the button down 1px (`translate-y-px`) instead of scaling — a tactile press, not a bounce.

### Badges / Chips
- **Style:** fully rounded (`rounded-4xl`, effectively pill-shaped), 20px tall, 8px horizontal padding, 12px text. Default variant uses solid Refined Lime; other variants follow the same soft-tint pattern as buttons (destructive, secondary, outline, ghost).
- **State:** chips are static labels (compose-project tags, counts) — they do not carry their own hover-interactive state unless wrapped as a link.

### Cards / Data Tables
- **Corner Style:** `rounded-xl` (~0.65rem).
- **Background:** Card Surface token, with a 1px hairline border in the Hairline Border token.
- **Shadow Strategy:** the card-elevation shadow from Elevation §4 — contact + ambient + inset highlight.
- **Internal Padding:** 13px vertical rhythm inside table cells (`py-[13px]`); section padding generally on an 8/16/24px scale.

### Status Indicator (signature component)
The **quiet status dot** (`StatusDot.svelte`) is dockwin's answer to status communication: a small (7px default) filled circle in one of four tones (running=emerald, warning=amber, error=destructive-red, off=slate), optionally with a soft pulsing halo ring (`eng-dot-ring` keyframe — scale 0.6→1.5, opacity 0.6→0, on the engine-live indicator only). It is always paired with a single status word and a muted sub-line in the surrounding markup. **It is never built as a glowing filled pill-band** — that's the explicit anti-pattern this component exists to avoid.

### Navigation (sidebar)
- Built on the shadcn `Sidebar` primitive, `collapsible="icon"`. Default state, hover state (muted-surface fill), and active state (Refined Lime inset rail — `inset 2px 0 0 var(--primary)` — plus icon/text in the accent-foreground tone) are the only three states; no secondary highlight layer.
- Footer carries the engine status pod (hidden when collapsed to icon mode), using the StatusDot + word + sub-line pattern.

## 6. Do's and Don'ts

### Do:
- **Do** keep Refined Lime to its four roles: primary action, active-nav rail, selected-row rail, one focused chart (**The One Voice Rule**).
- **Do** use the soft-filled success/destructive button pattern for container start/stop — color encodes consequence, not brand.
- **Do** communicate status with a dot + word + muted sub-line.
- **Do** reserve uppercase, letter-spaced label type for structural metadata (table headers), never as decorative section eyebrows.
- **Do** keep the layered card shadow exclusive to primary data surfaces; everything else stays flush with a hairline border.
- **Do** support both themes as first-class — dark is the hero/default, light is fully realized, not a stripped-down fallback.

### Don't:
- **Don't** let lime become ambient decoration — no lime backgrounds, no lime glows beyond the single top-edge backdrop gradient, no lime-tinted cards "for vibe."
- **Don't** build glowing filled status pill-bands. This was explicitly rejected once already as the generic-AI-dashboard tell.
- **Don't** reach for Docker Desktop's pattern of heavy chrome, nags, or upsell surfaces — any new UI should justify its presence by utility, not polish-for-its-own-sake.
- **Don't** ship identical card grids, gradient text, or tiny uppercase tracked "eyebrow" labels above section headings — the generic-SaaS-template tell this system is built to avoid.
- **Don't** use glassmorphism or backdrop-blur; the elevation system is shadow + hairline border, not frosted glass.
- **Don't** use a `border-left`/`border-right` colored stripe as a status or selection indicator — selection uses an inset shadow rail (`shadow-[inset_2px_0_0_var(--primary)]`), not a border-side accent.
