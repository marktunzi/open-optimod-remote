# M4B: Help and About

> **Evidence status:** run on 28 October 2026 in the real macOS app (test build, port 15701), connected to the scratch 5700i 3.0.1.20.

| Client (5700iPC.exe) | Mac app | Result |
|---|---|---|
| Help › Operating Manual: opens `5700i_3.0_Operating_Manual.pdf` from the client's install folder | Help › Operating Manual (⌘?) opens the connected model's manual; the first time, or with Help › Choose Operating Manual…, the user picks the PDF (Orban's manual is not shipped with this app) and it is remembered per model | PASS: the installer's 5700i manual (343 pages) opened in Preview and was stored for `optimod-5700i` |
| Help › About: client name and version | Open Optimod Remote › About (standard panel: name, version, copyright) | PASS |

Test hook: `manual` opens it, `files PATH` answers the choose panel.
