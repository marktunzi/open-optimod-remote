# Approved follow-up: controls, routing and metering

> **Status:** Historical approved plan, implemented across later milestones. The Keychain-preservation instruction below was superseded by the owner-only Application Support credential file requested on 20 September. Use [HANDOFF](../HANDOFF.md) and the [compatibility matrix](../compatibility.md) for current behavior and open gates.

The user authorized implementation of the existing plan and clarified that FM/HD control must include physical outputs. Product text is English; the interface is dark with familiar grouped Optimod meters and a more modern control surface. Saved named connections must remain intact.

Implementation:
1. Read the authenticated device preset catalog and provide selection plus explicit on-air Recall, with acknowledgment and fresh readback.
2. Keep FM/HD display selection local. Provide separate Analog, AES1, AES2 and headphones routing controls using actual system fields.
3. Expose processing and system settings through validated editors. Use reference enum values, compact integer ranges and exact sample-delay conversion; preserve free-text wire types.
4. Correct per-channel meter conversion and orientation, reduce smoothing latency, and target 20 polls/second. Preserve genuine stream liveness.
5. Validate using synthetic commands and a mocked browser for audio-changing actions; use only read-only checks on the user's running processor. Preserve existing connections and Keychain entries.
6. Record unsupported workflows and distinguish implemented control paths from hardware-verified writes.

Full parity remains a separate acceptance gate: additional processing structures, complete preset/file management, maintenance, optional hardware, all indicators, external changes, fault recovery, long-duration tests and clean-machine distribution.
