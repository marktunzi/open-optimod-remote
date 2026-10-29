# 8400 session stability (29 October 2026)

> **Evidence status:** 29 October 2026. Virtual 8400 running original firmware 3.0.5.7 (192.168.1.176, no audio), alone and next to a virtual 2300 (.174) and two virtual 6300s (.173, .188), each with its own isolated service instance (ports 15702–15705, own data folders). The owner's app and settings were not used. Old build = `f18d5da`; fixed build = this commit.

**Result: the app-side cause of the soak drops is fixed; a remaining drop under heavy host overload is the unit's own session end and is recovered by the automatic reconnect.**

## The six soak drops

In the Phase 9 soak (build-10 twin, 10:38–11:55) the 8400 session was lost six times between 10:40 and 11:08: five times *The processor has not answered for 20 seconds*, once *Broken pipe*. The virtual 8400 3.0.5.7 (.176) ran next to two virtual 6300s. Its own clock log shows it mostly at real time with slow stretches (1.1–1.9 s per virtual second from 10:50) and one 47 s stall at 11:06 (`LIVESTALL`). The soak's packet log was overwritten by a later start of the unit, so the cause was reproduced instead.

## Reproduction (scratch 8400, original firmware, no audio)

The unit's packet log (`netlog.jsonl`, every frame at its network interface) was read with a TCP timeline decoder.

| Run | Load | Result |
|---|---|---|
| Old build, 8400 alone | unit at 1.00× | connected 13 min; about 3 % of segments each way sent again; the unit sometimes acknowledged a segment only after the PC's fourth copy. The keepalive went every **4 s**, not 2 s |
| Old build, with 2300 + two 6300s running | unit at 0.65× | **lost after about 1 min** (*not answered for 20 seconds*) while the unit was still answering in bursts |
| Fixed build, same four units | unit at 0.63–0.75× | 20 min; 252 of 479 segments to the unit sent again; four gaps of **21–25 s** between keepalive answers, all survived. **One loss** (*Connection reset by peer*): see below; the automatic reconnect brought the session back |

## Causes

1. **The app gave up before the unit did.** The unit keeps a session until 30 s without any received byte (watchdog 0x1370b0), and its 30 s run on its own clock, which is slower than real time under load. The unit drops part of the segments it receives; TCP then backs off, and answers come seconds late. The app counted an 8400 session lost after 20 s without a keepalive answer, so it ended sessions the unit still held, and the reconnect then had to wait for them.
2. **Keepalive every 4 s instead of 2 s.** The tick came a few milliseconds less than 2 s after the last answer and was skipped every second time.
3. **The unit's own end of the session.** In the fixed run, from unit time 1224.7 s the unit's log shows every segment from the PC arriving but none acknowledged or answered for 30 s; then the unit reset the connection (its watchdog). Its network stack stalled under the load of four emulated processors. The *Broken pipe* in the soak is the same unit-side end. The app cannot prevent it; it reconnects (120 s later, as for every model). The original 8400 PC Remote 3.0 meets the same end: during the Phase 10 capture (unit at about 1.0×, a virtual 8700i starting next to it) the unit received the client's keepalives from unit time 2115 s on without acknowledging them, closed the connection 30 s later, and the client showed *The 8400 Has Disconnected* (it does not reconnect by itself).

## Fix

- `crates/orban-web/src/o8400.rs`: an 8400 session is lost only when the connection closes or after **35 s** without a keepalive answer (the unit's 30 s plus a margin). The keepalive goes every 2 s, as the original client's (`KEEPALIVE_SLACK`).
- Tests: `a_late_keepalive_answer_keeps_the_session` (25 s and 34 s keep the session, 36 s ends it, a closed connection ends it at once), `the_keepalive_goes_every_two_seconds`.

Physical 8400 hardware: **BLOCKED** (none available); whether a real unit drops segments the same way is not known.
