# Update feed

`appcast.xml` is the signed Sparkle feed of Open Optimod Remote for macOS. The app reads it from this branch; every entry and the feed itself are signed with the project's update key. Do not edit it by hand: `scripts/release-macos.sh` on `main` produces it.
