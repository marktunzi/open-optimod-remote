# Native Connections and Presets Design

> **Status:** Implemented. Both windows now use `NSSplitViewController` with real `.sidebar` split items; current evidence is in [2026-09-22-native-browser-redesign.md](../verification/2026-09-22-native-browser-redesign.md).

## Goal

Rebuild the Connections and Presets windows as compact native macOS browsers that match the supplied references, preserve all existing device safeguards, and remove the redundant Recall confirmation sheet.

## Shared window structure

Both windows use a unified AppKit toolbar above a native split view. The sidebar is a source list backed by system colors and materials so active, inactive, accent, high-contrast, and dark appearances are handled by AppKit. Main content uses standard tables, system typography, SF Symbols, and point-based spacing. Custom selection painting is removed.

Stored window frames remain supported, but restored dimensions are clamped to the visible screen and versioned so the previously oversized frames no longer reopen. Each browser opens at a compact useful size.

## Connections

- Open at 920 × 500 points with a 780 × 430 minimum.
- Use a 220-point resizable sidebar with All Connections and Network rows, plus native add/remove controls at the bottom.
- Put the sidebar toggle and tracking separator in the unified toolbar. Place the All Connections or Network title in the main toolbar area, followed by list controls, an actions menu, Add, and Search.
- Remove the window subtitle that currently places saved-processor text beside the traffic lights.
- Use 52-point connection rows with a 20-point `server.rack` symbol, a 13-point medium name, an 11-point secondary host, a 13-point trailing state, and a 28-point `info.circle` button.
- Keep 12–16 points of content padding.
- Remove the large duplicate Edit/Connect inspector. Double-click connects, the row information button edits, the actions menu exposes written Connect, Edit, Remove, and New commands, and Return connects the selected row.
- Network remains automatic discovery. Opening Network starts discovery and presents discovered processors in the same list.
- Use system source-list and table selection colors. No text is manually changed to white.

## Presets

- Open at 900 × 560 points with a 760 × 460 minimum.
- Use a 188-point native source-list sidebar for All Presets, Factory, User, and Modified.
- Keep one prominent written Recall toolbar item with `play.fill`. Move Apply File and Save to Mac to a standard actions menu, keep Refresh compact, and retain the native search item.
- Remove the duplicated 22-point All Presets content heading and the permanent explanatory footer.
- Present a dense native list with 40-point rows. Use 13-point medium preset names, 11-point secondary source labels, appropriate 16-point SF Symbols, and a trailing green On Air state.
- Let rows fill the available width without oversized rounded custom selection cards.
- Selection never changes the processor. Recall is performed only from the written Recall action or Return.

## Recall behavior

The Recall button is itself the explicit confirmation. It sends the existing guarded request with `confirmed: true` immediately and does not show an `NSAlert`. The existing expected host, session, current preset, access level, processor response, meter resubscription, and error-log behavior remain unchanged. Double-click does not recall a preset.

During Recall, only mutation controls are disabled. Progress is shown in the toolbar or content status, and errors are recorded in Error Log. There is no success or warning modal.

## Accessibility and keyboard behavior

- Search uses Command-F.
- Return activates Connect in Connections and Recall in Presets when the relevant selection is valid.
- Command-N adds a saved connection.
- Import uses Command-O and export uses Command-Shift-S through the actions menu.
- Every symbol has an accessibility description and every icon-only control has a tooltip.

## Verification

Native tests assert the presentation constants, absence of the Recall confirmation, toolbar hierarchy, keyboard actions, and compact window defaults. The full Swift, web, Rust, bundle, code-signature, and installer checks run before completion. Hardware verification is read-only; no real preset is recalled.
