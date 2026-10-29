#!/usr/bin/env python3
"""Writes a profile's factory-presets.json from the `presets` folder of an
original Optimod PC Remote installation.

Each factory preset has a `.<ext>50` and a `.<ext>75` file (the system
setting PRE-E picks one). Where the app recalls a factory preset without a
status port, it does what the original client does: it uploads that file
over PC Remote. Line ends become LF, as in the client's upload.

usage: extract_factory_presets.py PRESETS_DIR EXTENSION OUTPUT
  e.g. ... ".../Optimod 5700i PC Remote/presets" orb57f profiles/5700i/3.0.1.20/factory-presets.json
       ... ".../Optimod 2300 PC Remote/presets" orb23f profiles/2300/2.0.3.85/factory-presets.json
"""
import json, pathlib, sys

presets, extension, target = pathlib.Path(sys.argv[1]), sys.argv[2], pathlib.Path(sys.argv[3])
out = {}
for f in sorted(presets.glob(f"*.{extension}50")):
    name = f.name[: -len(f".{extension}50")]
    out[name] = {
        f"{emphasis}us": (presets / f"{name}.{extension}{emphasis}")
        .read_bytes()
        .decode("latin-1")
        .replace("\r\n", "\n")
        for emphasis in ("50", "75")
    }
target.write_text(json.dumps(out, indent=1, ensure_ascii=False) + "\n")
print(f"{len(out)} factory presets -> {target}")
