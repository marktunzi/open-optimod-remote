# Native Connections and Presets browser verification

> **Evidence status:** Current structural, build and installation evidence for the two native browser windows. Pixel-level visual acceptance remains separate.

Date: 2026-09-22

## Scope

The native Connections and Presets windows were rebuilt from the approved macOS composition. Both now use `NSSplitViewController` and `NSSplitViewItem(sidebarWithViewController:)`, giving the sidebar AppKit's real `.sidebar` behavior, translucent material, collapse rules and toolbar-divider tracking. The Connections browser opens at 920 × 500 points with a 200–260 point system-resizable sidebar, unified toolbar, 52-point rows, compact typography, system active/inactive selection colors and no duplicate bottom inspector. The Presets browser opens at 900 × 560 points with a 168–240 point system-resizable sidebar, 40-point rows, one written Recall action, an import/export action menu and no duplicated content heading or permanent footer.

Preset selection and double-click do not alter processing. Clicking Recall or pressing Return sends the existing guarded Recall request directly with `confirmed: true`; the extra AppKit `NSAlert` was removed. Host, session, current-preset, access-level, processor acknowledgement, meter-resubscription and Error Log safeguards remain in the backend.

Both windows use new V3 frame autosave identifiers so previously stored oversized frames do not override the corrected composition.

## Automated evidence

- 43 TypeScript UI tests passed.
- The TypeScript production build completed with 49 transformed modules.
- 47 Rust protocol/backend tests passed.
- Rust Clippy passed with warnings denied.
- Rust formatting passed.
- Eight native test executables passed: LauncherCore, SystemSettingsSpec, ConnectionsAPI, ConnectionsWindow, PresetsAPI, NetworkDiscovery, ErrorLog and PresetsWindow.
- ConnectionsWindowTests verifies the 920 × 500 declared default, 780 × 430 content minimum, real `.sidebar` behavior, 200–260 point sidebar bounds, 52-point list rows and native sidebar toolbar item.
- PresetsWindowTests verifies the 900 × 560 declared default, 760 × 460 content minimum, real `.sidebar` behavior, 168–240 point sidebar bounds, 40-point list rows, compact actions menu and absence of table double-click Recall.
- Shell syntax checks and `git diff --check` passed.
- The complete one-click macOS bundle build completed successfully.

## Installation and runtime evidence

`scripts/install-macos.sh` replaced the development bundle at `/Applications/Open Optimod Remote.app`. The nested ad-hoc signatures passed strict deep verification and the installed `Info.plist` passed `plutil` validation. Launching the installed app started its bundled local service successfully and returned a valid disconnected state with no error.

No real processor preset was recalled and no processing, setup, routing or coupling write was submitted during this verification.

The AppKit hierarchy, dimensions, actions, build and installed bundle are verified. Final pixel-level visual acceptance of the two windows remains a separate review step.
