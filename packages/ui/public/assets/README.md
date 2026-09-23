# Runtime identity assets

These four files were supplied specifically for the Open Optimod Remote interface:

- `optimod-5700i.svg` — the 5700 DIGITAL / OPTIMOD identity used at the left of the instrument header;
- `orban.png` — the high-resolution transparent Orban mark embedded in the final supplied UI reference;
- `orban-purple.png` — the purple Orban mark used by the approved instrument revision;
- `liquid-crystal-display.ttf` — the dotmatrix face used only inside the blue status LCD.

They are copied unchanged into the Vite output and the macOS application bundle. They are not covered by the repository's MIT license. Product names and marks remain the property of their respective owners; anyone redistributing the application must ensure they have the necessary rights for these assets and the supplied font.

The `optimod-5500i.svg`, `optimod-5500.svg`, `optimod-5700-fm.svg`, `optimod-5700-hd.svg`, `optimod-6300.svg`, `optimod-8500.svg`, `optimod-8600.svg`, `optimod-8700i.svg`, `optimod-9300.svg`, `optimod-9400.svg` and `optimod-neutral.svg` files are original text-based compatibility marks created for this project. They are separate runtime assets so every processor adapter can select its own product identity and skin. The underlying product names remain third-party marks and identify compatibility targets only.

`instrument-noise.svg` is an original runtime recreation of the final reference's documented Figma noise-filter parameters. `control-noise.svg` recreates the supplied slider-thumb texture at its specified 3% opacity and seed. Both are covered by the repository code license.

The separately supplied `native/macos/AppIcon.png` (the Orban mark on a graphite tile, laid out on the 1024 px macOS icon grid) is converted into `OpenOptimod.icns` by `scripts/build-macos.sh`. It is an application identity asset and is likewise outside the repository's MIT license.
