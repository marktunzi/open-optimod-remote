# Native 5700i System Settings implementation

> **Status:** Completed. The maintained control count is 134 across ten primary tabs; current evidence is in [2026-09-20-native-system-settings.md](../verification/2026-09-20-native-system-settings.md).

## Goal

Add a separate native macOS window for the complete 5700i System Settings worksheet. The window is opened from the existing interface and exposes ten native tabs: Input, Output, Test, Utility, Network, Stereo Encoder, Remote I/O, HD / Digital Radio, RDS, and RDS AF.

## Architecture

- Keep `orban-web` as the only owner of the PC Remote connection.
- Load `/api/state` and `/api/profile` in the native window.
- Submit supported edits to the existing `/api/change` endpoint, including the expected host, session, field, and value. The Rust owner performs the fresh read, typed write, response-boundary crossing, and exact readback.
- Keep the Settings specification in a Foundation-only file so its tabs, sections, control types, ranges, and option lists can be tested without starting AppKit.
- Build the window with AppKit controls because the existing native shell uses AppKit. Use `NSTabViewController`, `NSScrollView`, `NSSlider`, `NSSwitch`, `NSSegmentedControl`, `NSPopUpButton`, and native text/number fields.
- Mark source-defined controls whose 5700i wire mapping is not present in the verified firmware profile as unavailable instead of inventing a device write.

## Implementation tasks

1. Encode all ten tabs and every requested control in `native/macos/SystemSettingsSpec.swift`.
2. Add Foundation-only specification tests for tab names, AES differences, GPI/tally options, special Off-plus-value controls, and all 24 AF pickers.
3. Implement API decoding and guarded change requests in `native/macos/SystemSettingsAPI.swift`.
4. Implement the native window and reusable control rows in `native/macos/SystemSettingsWindow.swift`.
5. Add the WebKit message bridge, application menu item, and existing-interface button.
6. Extend macOS build and test scripts, then run Swift, frontend, Rust, signing, and UI checks.
7. Update README, handoff, changelog, and verification documentation.
