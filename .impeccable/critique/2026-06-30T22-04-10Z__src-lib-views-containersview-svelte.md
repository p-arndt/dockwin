---
target: Containers view
total_score: 26
p0_count: 0
p1_count: 2
timestamp: 2026-06-30T22-04-10Z
slug: src-lib-views-containersview-svelte
---
#### Design Health Score

| # | Heuristic | Score | Key Issue |
|---|-----------|-------|-----------|
| 1 | Visibility of System Status | 3 | No distinct loading state — first paint / slow poll renders identically to "no containers" |
| 2 | Match System / Real World | 3 | `isOfficial` badge fabricates a trust signal for any image without a `/` in its name |
| 3 | User Control and Freedom | 3 | Strong remove-confirm + rename escape; filter input has no clear ("×") affordance |
| 4 | Consistency and Standards | 2 | Sheet scrim uses backdrop-blur (DESIGN.md bans this by name); the selected-row lime rail is wired in CSS but never triggered |
| 5 | Error Prevention | 3 | Remove confirmation is state-aware ("it will be stopped and then deleted"); Stop has no dependency warning (acceptable for the genre) |
| 6 | Recognition Rather Than Recall | 3 | Restart/Remove only appear on hover/focus — no static affordance hints they exist |
| 7 | Flexibility and Efficiency | 2 | No multi-select/bulk start-stop-remove, no keyboard shortcuts (e.g. `/` to focus filter) |
| 8 | Aesthetic and Minimalist Design | 3 | Dense but calm; 4-card stat grid varies treatment per card rather than templating identically |
| 9 | Error Recovery | 2 | Raw `errText(e)` strings surfaced directly; top-level error banner has no retry (Inspect tab's does) |
| 10 | Help and Documentation | 2 | No inline help beyond port tooltips — fitting restraint for the audience, but scores low literally |
| **Total** | | **26/40** | **Acceptable — solid bones, real consistency gaps** |

#### Anti-Patterns Verdict

**LLM assessment**: Mostly holds the line dockwin's own DESIGN.md sets — no hero metrics, no gradient text, no glowing pill-bands, uppercase reserved for table headers only. One direct, citable contradiction: the shadcn Sheet primitive (`src/lib/components/ui/sheet/sheet-overlay.svelte:15`) ships `backdrop-blur-xs` on the scrim behind the Container Details panel — the exact glassmorphism/backdrop-blur pattern DESIGN.md bans by name, live on this screen's primary interaction (opening container details).

**Deterministic scan**: `detect.mjs` against the three target view files (ContainersView, ContainerList, ContainerDetails) returned 0 findings across 0 rules — a clean scan. This is not a contradiction of Assessment A's finding: the backdrop-blur violation lives in a shared shadcn primitive (`sheet-overlay.svelte`) one level below the three files the scan targeted, so the detector's file-scoped pattern matching had no visibility into it. No false positives to flag — there were no findings to question.

**Visual overlays**: Not available. No browser automation tool was exposed in this session, so no live-server injection or in-browser overlay was attempted. No fallback signal beyond that.

#### Overall Impression

The Containers screen is a genuinely disciplined product surface — restrained color use, real progressive disclosure, state-aware confirmation copy — undercut by two specific, fixable consistency breaks: a banned visual effect leaking in from a shared primitive, and the system's own flagship example of "the one selected-row lime role" being dead code. Neither is a redesign; both are surgical. The bigger structural opportunity is efficiency for the actual target user (someone managing compose stacks) — there's no bulk action path, which is exactly the kind of one-at-a-time friction dockwin exists to remove from the Docker Desktop experience.

#### What's Working

- **Port tooltip honesty** (`ContainerList.svelte:70-73`, `ContainerDetails.svelte:339-343`) — explicitly states "Bound to X — NOT forwarded to Windows localhost" for non-wildcard bindings. Precise, Docker-literate, trust-building microcopy.
- **State-aware remove confirmation** — copy changes based on running state ("it will be stopped and then deleted" vs. a generic warning) rather than a one-size-fits-all dialog. Rare and well-executed reassurance at the screen's one destructive action.
- **4-card stat grid with varied treatment** (`ContainerDetails.svelte:488-533`) — CPU alone carries the sparkline (the system's one designated "focused chart"), Memory gets a bar, Network/Block stay plain numerics. Avoids the identical-card-grid tell while respecting the ≤4 chunking rule.

#### Priority Issues

**[P1] Sheet overlay uses backdrop-blur, contradicting DESIGN.md's explicit ban.**
- **Why it matters**: DESIGN.md states in bold that the system uses no glassmorphism and no backdrop-blur anywhere — this is a direct, easily-screenshotted contradiction in the app's most-used interaction (opening container details).
- **Fix**: Drop the blur utility on `sheet-overlay.svelte:15`; keep the flat scrim only.
- **Suggested command**: `/impeccable polish`

**[P1] Selected-row lime rail is dead code.**
- **Why it matters**: DESIGN.md names the selected-table-row rail as one of exactly four canonical lime placements in the whole system. `ContainerList.svelte:121` has the `data-[sel=true]:shadow-[inset_2px_0_0_var(--primary)]` styling ready, but `ContainersView.svelte` never passes `selected` down into `ContainerList`, so `data-sel` is never set. The rule's flagship example doesn't work.
- **Fix**: Pass `selected?.id` into `ContainerList`, set `data-sel={c.id === selectedId}` per row.
- **Suggested command**: `/impeccable polish`

**[P2] Loading and empty states render identically.**
- **Why it matters**: `containers` starts as `[]` and stays that way until the first poll resolves, so "No containers." is indistinguishable from "still connecting." Violates visibility-of-system-status and specifically harms first-time users who can't tell the two apart.
- **Fix**: Thread an explicit loading boolean through and show "Loading containers…" until the first successful fetch.
- **Suggested command**: `/impeccable clarify`

**[P2] No bulk actions for multi-container workflows.**
- **Why it matters**: PRODUCT.md's target user manages compose stacks; every Start/Stop/Restart/Remove is single-row only, forcing one-by-one clicking for what's usually a whole-project operation — the exact friction dockwin exists to remove from Docker Desktop.
- **Fix**: Add row checkboxes + a bulk action bar, or at minimum surface a compose-project-level action from this screen.
- **Suggested command**: `/impeccable shape`

**[P3] `isOfficial` badge gives a false signal.**
- **Why it matters**: `ContainerDetails.svelte:74-77` marks any image lacking a `/` as "Official," including locally built/tagged images like `myapp:latest` — fabricating a provenance signal in a tool whose value proposition is being more honest than Docker Desktop.
- **Fix**: Remove the badge, or gate it on actual registry namespace data if available.
- **Suggested command**: `/impeccable polish`

#### Persona Red Flags

**Alex (Power User)**: No multi-select/bulk start-stop-remove for a compose stack's containers, forcing N individual clicks. The modal Sheet (with its blur) takes over the screen on every "peek at details," and because the selected-row rail never fires, Alex loses track of which row they were on after closing it — repeated friction across a normal triage session.

**Sam (Accessibility-Dependent)**: `ContainerList.svelte:120-133` puts `role="button" tabindex={0}` directly on a `<tr>` — breaks native table semantics (row/column announcement, arrow-key navigation) for screen reader users, with no `aria-label` describing the action ("Open details for nginx-proxy"). Restart/Remove buttons are `opacity-0` until hover/focus-within; keyboard users do reach them via focus-within, but tooling relying on visible affordances may skip them.

**Riley (Stress Tester)**: The Ports cell (`ContainerList.svelte:203-229`, fixed-width column, `flex flex-wrap`) has no cap — a container with a dozen+ exposed ports balloons that row's height unpredictably, breaking the table's consistent row rhythm. Combined with the loading/empty conflation, a fast engine restart or a heavily-port-mapped dev stack are the two edge cases most likely to make this screen look broken rather than dense.

#### Minor Observations

- `StatusDot.svelte` is the documented shared status component, but `ContainerList.svelte` and `ContainerDetails.svelte` both hand-roll their own dot spans instead of importing it — duplicated logic and a drift risk (e.g. the halo-ring option only exists on the shared component).
- The Name cell's leading lamp dot and the separate Status-column dot+word repeat the same signal in two adjacent columns — minor redundancy, though it aids left-edge scanning.
- Filter `Input` (`ContainersView.svelte:68`) has no clear ("×") affordance once text is entered.
- Top-level error `Alert` in `ContainersView.svelte:72-76` has no retry button, while the Inspect tab's equivalent error state does (`ContainerDetails.svelte:662-664`) — inconsistent recovery pattern within the same screen.

#### Questions to Consider

- If the design system's own shared primitive (`sheet-overlay.svelte`) violates a rule stated in bold in DESIGN.md, who is actually auditing the primitives versus just the screens assembled from them?
- DESIGN.md names the selected-row rail as one of exactly four canonical lime placements in the whole app — if it's dead code on the very screen that rule was written for, does it hold in practice or only on paper?
- Is a fully modal, scrim-and-blur Sheet the right model for "details of the row I clicked," or would a non-modal split pane (list and details visible together) fit the brand's own anti-webby principle better than a borrowed SaaS dialog pattern?
