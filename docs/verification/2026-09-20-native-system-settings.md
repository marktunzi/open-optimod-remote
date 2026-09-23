# Native System Settings verification — 20 September 2026

> **Evidence status:** Current point-in-time evidence for the 134-control native Settings specification. Broader product status and remaining parity work are maintained in [HANDOFF](../HANDOFF.md).

Target: the native **Optimod 5700i Settings** window inside the installed macOS app, based on the complete “Optimod 5700i Instellingen Inputs Groepen” source transcript and the verified firmware 3.0.1.20 parameter profile.

## Implemented surface

- The main 5700i interface opens a separate AppKit window through a restricted WebKit message handler. Command-, opens the same window from the application menu.
- The window uses ten native toolbar tabs with SF Symbols: Input, Output, Test, Utility, Network, Stereo Encoder, Remote I/O, HD / Digital Radio, RDS and RDS AF.
- The specification contains 134 controls. Ranges use an `NSSlider` with an editable native numeric field; binary values use `NSSwitch`; short exclusive choices use `NSSegmentedControl`; longer lists use `NSPopUpButton`; numeric values use native fields and steppers.
- Input, Test, Network, Stereo Encoder and HD/Digital Radio show all of their grouped sections together on one page. Larger categories retain the second native section selector.
- The Test tab has a real Bypass switch backed by the verified `ALGORITHM` operate/bypass choices, plus the separate Bypass Gain range.
- Each field shows its value only in the editable control. The repeated value column and in-content Settings header are removed, and section cards use two columns where multiple groups are available.
- Multiplex Power Threshold, Dynamic PS Timeout and TA Timeout keep Off as a separate enabled state from the last numeric value.
- RDS AF contains 24 independent pickers. Each offers None or 87.6–107.9 MHz in 0.1 MHz steps.
- The window follows macOS appearance. Light appearance uses a continuous light-grey title bar and canvas with borderless white setting cards; Dark appearance uses adaptive dark system colors.

## Data and write boundary

The native window does not open another device socket. It reads `/api/state` and `/api/profile` from the loopback service and sends supported edits through the existing `/api/change` command. The Rust owner still requires the expected host, connection-session token, current field and typed value, then performs a fresh System read and exact readback.

No live setting write was submitted during this verification. A connected read-only UI inspection loaded current Input and HD values and confirmed that profile-backed controls were populated. Repeated bundle restarts later produced a transient device login timeout, while both device ports remained reachable; this was not treated as write evidence.

The later connection failure was isolated to a backend process left alive while its ad-hoc-signed application bundle was replaced. The installer now stops the previous launcher and backend before replacing the signed bundle. Saved access codes use the app-local owner-only credential file and never use Keychain.

The worksheet-only External AGC item was removed because the connected 5700i document exposes no corresponding System field. The eight GPI fields are real typed-text fields in the observed System document; their complete function lists and dynamic factory/user preset names write through the guarded typed-text route. The worksheet-only AES2 Ratings choice remains visible and disabled when the connected firmware profile does not expose it.

## Automated checks

- `SystemSettingsSpecTests`: 10 exact tab names, 10 unique symbol names, 134 controls, combined-page categories, AES1/AES2 source differences, three Off-plus-value controls, eight complete GPI lists, 24 AF pickers, exact coverage of every profile-backed field, and profile index checks for aliases, ports and AF endpoints.
- Native Swift application compile for macOS 13 with AppKit and WebKit.
- Existing 12 TypeScript UI tests and Vite production build.
- Complete Rust workspace test suite.
- Installed bundle build, property-list validation and strict deep code-signature verification.

Visual inspection confirmed native Light appearance, icon-backed tabs, explicit section headings, 22 pt left/right content margins, borderless white cards and a vertically scrolling two-column layout with every active-tab control expanded.
