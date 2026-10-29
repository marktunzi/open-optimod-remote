#!/usr/bin/env python3
"""Writes profiles/5700i/3.0.1.20/factory-presets.json from the `presets`
folder of the original Optimod 5700i PC Remote 3.0.1.20 installation.

Each factory preset has a `.orb57f50` and a `.orb57f75` file (the system
setting PRE-E picks one). Without its status port the app recalls a factory
preset as the original client does: by uploading that file over PC Remote.
Line ends become LF, as in the client's upload.

usage: extract_5700i_factory.py PRESETS_DIR
"""
import json, pathlib, sys

presets = pathlib.Path(sys.argv[1])
out = {}
for f in sorted(presets.glob("*.orb57f50")):
    name = f.name[: -len(".orb57f50")]
    files = {}
    for emphasis in ("50", "75"):
        text = (presets / f"{name}.orb57f{emphasis}").read_bytes().decode("latin-1")
        files[f"{emphasis}us"] = text.replace("\r\n", "\n")
    out[name] = files
target = pathlib.Path(__file__).resolve().parent.parent / "profiles/5700i/3.0.1.20/factory-presets.json"
target.write_text(json.dumps(out, indent=1, ensure_ascii=False) + "\n")
print(f"{len(out)} factory presets -> {target}")
