# SVG Instrument Header Implementation Plan

> **Status:** Completed. Later meter-panel, status-button and control-surface corrections are reflected in [DESIGN.md](../../DESIGN.md) and [HANDOFF](../HANDOFF.md).

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rebuild the app’s upper instrument area to match the user-approved SVG and full-width PNG exactly while retaining real 5700i state, live meters and all existing safety guards.

**Architecture:** Add a semantic React instrument-header component and restyle the existing canvas meter renderer using tokens and proportions measured from the SVG. The current Rust API, SSE meter transport, channel mapping, coupling workflow and lower workspace remain intact. Live meter availability is surfaced from `MeterStrip` to the header without duplicating the device stream.

**Tech Stack:** React 19, TypeScript, Canvas 2D, CSS, Vite, Node test runner, Rust/AppKit packaging.

---

### Task 1: Lock the approved meter geometry and palette

**Files:**
- Modify: `packages/ui/src/meters.ts`
- Modify: `packages/ui/tests/meters.test.ts`

**Step 1: Write the failing geometry assertions**

Add assertions for the reference well width, illuminated bar width, two-lane spacing and stable group width across views. Add a small exported palette object and assert the approved greys, blue, amber, red and green tokens.

**Step 2: Run the meter tests and verify failure**

Run:

```sh
node --experimental-strip-types --test packages/ui/tests/meters.test.ts
```

Expected: failure until the reference geometry and tokens are implemented.

**Step 3: Implement the reference constants**

Keep every channel lane fixed. Use the SVG proportions for narrow illuminated bars inside a wider dark well, and export presentation tokens without changing meter channel IDs or curves.

**Step 4: Run the meter tests**

Expected: all meter tests pass.

**Step 5: Commit**

```sh
git add packages/ui/src/meters.ts packages/ui/tests/meters.test.ts
git commit -m "test: lock approved meter geometry"
```

### Task 2: Build the semantic instrument header

**Files:**
- Create: `packages/ui/src/InstrumentHeader.tsx`
- Modify: `packages/ui/src/main.tsx`
- Modify: `packages/ui/src/CouplingControl.tsx`
- Modify: `packages/ui/src/MeterStrip.tsx`

**Step 1: Add the header component contract**

The component receives connection state, meter-live state, preset name, active input, stereo mode and the existing coupling control props. The final implementation renders the supplied 5700 SVG and high-resolution Orban asset around the dynamic centre controls:

```tsx
<header className="instrument-header">
  <div className="optimod-mark">…</div>
  <div className="status-lcd">…</div>
  <CouplingControl … />
  <div className="instrument-connection">…</div>
  <div className="orban-mark">…</div>
</header>
```

**Step 2: Surface actual meter-live state**

Add `onLiveChange?: (live: boolean) => void` to `MeterStrip`. Invoke it only when the active/stale state changes and clear it on disconnect/unmount.

**Step 3: Replace the current topbar in `main.tsx`**

Derive the LCD second line from `ACTUAL A OR D`, `MOD TYPE` and `PILOT` without adding any API call. Pass the existing guarded coupling action through unchanged.

**Step 4: Preserve coupling safety**

Keep the field, session and preset captured-confirmation behavior. Only the button’s visual wrapper and displayed state change.

**Step 5: Build TypeScript**

Run `npm run build --prefix packages/ui`. Expected: success.

**Step 6: Commit**

```sh
git add packages/ui/src/InstrumentHeader.tsx packages/ui/src/main.tsx packages/ui/src/CouplingControl.tsx packages/ui/src/MeterStrip.tsx
git commit -m "feat: add SVG-inspired instrument header"
```

### Task 3: Match the live meter chassis

**Files:**
- Modify: `packages/ui/src/MeterStrip.tsx`
- Modify: `packages/ui/src/style.css`
- Test: `packages/ui/tests/meters.test.ts`

**Step 1: Rebuild the canvas drawing order**

Draw each group as an instrument bay with its own background and separator, then scale labels, ticks, wells, illuminated bars, peak markers and units. Use the approved colors from `meters.ts`.

**Step 2: Add zoned level bars**

Input, FM output and HD output use a green/yellow/red vertical gradient matching the reference. Reduction meters draw down from the top in blue. Loudness, enhance and composite meters draw amber from the bottom.

**Step 3: Retain real behavior**

Do not change SSE polling, channel IDs, curves, animation interpolation, reduced-motion handling or 1.2-second stale clearing.

**Step 4: Match chassis CSS**

Use the final measured `#353432` → `#32312f` outer gradient, `#20201e` → `#252523` chassis gradient, `#141414` wells, `#191919` separators and `#656565` ticks, an 8 px chassis radius, reference-height canvas and horizontal scrolling at fixed lane width. Use the supplied Liquid Crystal Display face only in the LCD and SF Compact Text in the remaining DOM/canvas labels.

**Step 5: Run tests and build**

Run the UI tests and production build. Expected: all pass.

**Step 6: Commit**

```sh
git add packages/ui/src/MeterStrip.tsx packages/ui/src/style.css packages/ui/src/meters.ts packages/ui/tests/meters.test.ts
git commit -m "feat: match approved live meter chassis"
```

### Task 4: Carry the instrument visual language through the workspace

**Files:**
- Modify: `packages/ui/src/style.css`
- Modify: `packages/ui/index.html`

**Step 1: Define shared visual tokens**

Add CSS custom properties for the approved surface, separator, LCD, blue, amber, green, red and magenta values, plus the SF Compact font stacks.

**Step 2: Restyle navigation and panels**

Apply the same surfaces, restrained radii, typography, active blue/magenta and border hierarchy to Connections, toolbar, processing groups, Presets, Outputs and Setup. Preserve every layout and accessible focus state from the previous audit.

**Step 3: Validate narrow native width**

At 1180×760, text must not overlap. The header may tighten its grid, while meters scroll instead of shrinking lanes.

**Step 4: Build and commit**

```sh
npm run build --prefix packages/ui
git add packages/ui/src/style.css packages/ui/index.html
git commit -m "style: extend instrument theme through workspace"
```

### Task 5: Visual regression, packaging and documentation

**Files:**
- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/verification/2026-09-19-ui-audit.md`

**Step 1: Run complete checks**

Run the ten existing UI tests plus the new meter assertions, Vite production build, Rust workspace tests, Clippy, formatting, Swift launcher test and shell syntax checks.

**Step 2: Reinstall the native app**

Run:

```sh
DEVELOPER_DIR=/Library/Developer/CommandLineTools sh scripts/install-macos.sh
```

Expected: a valid ad-hoc signed `/Applications/Open Optimod Remote.app`.

**Step 3: Run read-only visual regression**

Exercise disconnected and live states, FM/HD/Both meters, every Setup category and every FM/HD processing page at 1440×900 and 1180×760. Assert no unintended overflow or text overlap. Do not confirm a coupling, routing, preset or parameter write.

**Step 4: Compare against the supplied SVG**

Verify the header proportions, both product marks, LCD, coupling button, live status, SF Compact typography, panel radius, exact grey palette, meter colors, well widths, labels and separators against the rendered reference.

**Step 5: Update documentation and commit**

Document the approved reference, checks and remaining Windows-parity boundary, then commit the final milestone.
