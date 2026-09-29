# Preset Recall continuity

> **Evidence status:** Synthetic continuity evidence as of 22 September plus a read-only post-build meter observation. No on-air hardware Recall was performed. The Recall exchange itself is unchanged; since 29 September the workspace asks for one confirmation before it. Since 29 September the Presets workflow is a workspace of the interface, also in the macOS app, and the native Presets window is only a fallback; see [preset management and backup](2026-09-29-preset-management-backup.md).

Target: the Rust session owner and native Presets workflow for an OPTIMOD 5700i running the supported firmware profile.

The earlier Recall path performed an LP/AP catalog refresh before every write and then performed RP/AP for the Recall itself. Both terminal exchanges requested a complete processing document. The owner also continued sending 50 ms PC Remote meter polls throughout the preset switch. On real hardware that combination can pause meters repeatedly and accumulate receive timeouts while the processor is busy loading the preset.

Recall now validates the expected device session, host, current preset and target against the already loaded session state. It performs exactly one RP/AP terminal exchange. PC Remote polling remains paused until the returned AP document confirms the target, after which the owner sends one meter-subscription command, clears prior heartbeat failures and schedules the next poll 50 ms later. An unusual terminal response may trigger one direct AP read, but Recall is never replayed.

The regression test uses independent synthetic PC Remote and terminal servers. It asserts one terminal connection, zero PC Remote polls during the delayed RP/AP exchange, one meter resubscription, a connected final session and the confirmed target as both the live document and new preset baseline. The installed build then reconnected read-only to the reference device and delivered 182 live meter events over ten seconds, with no reported error and no disconnect. No Recall was sent to the on-air hardware during this verification.
