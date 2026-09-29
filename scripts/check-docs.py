#!/usr/bin/env python3
"""Validate maintained Markdown facts and local links."""

from __future__ import annotations

import re
import sys
from pathlib import Path
from urllib.parse import unquote


ROOT = Path(__file__).resolve().parents[1]
EXCLUDED_PARTS = {"dist", "node_modules", "target"}


def markdown_files() -> list[Path]:
    return sorted(
        path
        for path in ROOT.rglob("*.md")
        if not EXCLUDED_PARTS.intersection(path.relative_to(ROOT).parts)
    )


def check_local_links(files: list[Path]) -> list[str]:
    errors: list[str] = []
    pattern = re.compile(r"!?(?:\[[^\]]*\])\(([^)]+)\)")
    for path in files:
        text = path.read_text(encoding="utf-8")
        for raw_target in pattern.findall(text):
            target = raw_target.strip().split(maxsplit=1)[0].strip("<>")
            if not target or target.startswith(("#", "http://", "https://", "mailto:")):
                continue
            local_part = unquote(target.split("#", 1)[0])
            resolved = (path.parent / local_part).resolve()
            if not resolved.exists():
                errors.append(f"{path.relative_to(ROOT)}: missing link target {target}")
    return errors


def require(path: str, needle: str, errors: list[str]) -> None:
    source = ROOT / path
    if not source.exists():
        errors.append(f"{path}: missing maintained document")
        return
    text = source.read_text(encoding="utf-8")
    if needle not in text:
        errors.append(f"{path}: missing current fact: {needle}")


def forbid(path: str, needle: str, errors: list[str]) -> None:
    text = (ROOT / path).read_text(encoding="utf-8")
    if needle in text:
        errors.append(f"{path}: stale fact remains: {needle}")


def main() -> int:
    files = markdown_files()
    errors = check_local_links(files)

    require("PRODUCT.md", "owner-only Application Support credential file", errors)
    forbid("PRODUCT.md", "belong in the macOS Keychain", errors)
    require("CHANGELOG.md", "0.1.0 — 23 September 2026", errors)
    require("CHANGELOG.md", "0.2.0 — 29 September 2026", errors)
    require("README.md", "6300, 8500, 8600, 8700i, 9300 and 9400", errors)
    require("PRODUCT.md", "Every processor model has its own skin", errors)
    require("CHANGELOG.md", "134 specification-backed controls", errors)
    require("skills/optimod-5700i-control/references/verification.md", "75 presets listed", errors)
    require("skills/optimod-5700i-control/references/verification.md", "B2 Output Mix", errors)
    require("skills/optimod-5700i-control/references/metering.md", "900 ms", errors)
    require("docs/verification/2026-09-21-native-connections-presets.md", "Superseded", errors)
    require("docs/verification/2026-09-23-multi-model-foundation.md", "Inactive layouts", errors)
    require("docs/verification/2026-09-23-multi-model-foundation.md", "eleven corresponding terminal banners", errors)
    require("skills/optimod-5700i-control/references/model-families.md", "`9400.30`", errors)
    require("skills/optimod-5500-control/SKILL.md", "5500 V 1.2.8.24", errors)
    require("skills/optimod-5500-control/references/evidence.md", "8300.10", errors)
    require("skills/optimod-8700hd-control/SKILL.md", "8700HD V 1.0.2.161", errors)
    require("docs/adding-a-model.md", "scripts/extract_pc_remote.py", errors)
    require("README.md", "not yet been verified on hardware", errors)
    require("README.md", "Saving, renaming and deleting presets on the processor works only on the 5500.", errors)
    require("README.md", "all twelve selectable PC Remote models", errors)
    forbid("README.md", "all eleven selectable", errors)
    forbid("README.md", "backup and restore, and maintenance", errors)
    require("docs/compatibility.md", "Stand: 29 september 2026", errors)
    forbid("docs/HANDOFF.md", "feature/native-remote", errors)
    require("docs/research/2026-09-29-5500-8700hd-static-analysis.md", "295 registraties", errors)
    require("docs/research/2026-09-29-5500-8700hd-static-analysis.md", "## Presets op het apparaat opslaan en verwijderen", errors)
    forbid("skills/optimod-5700i-control/SKILL.md", "AES67", errors)
    forbid("skills/optimod-5700i-control/SKILL.md", "read-only adapters for 5500i, 5500,", errors)
    require("skills/optimod-5500-control/SKILL.md", "## Presets on the processor", errors)
    require("skills/optimod-8700hd-control/SKILL.md", "## Presets on the processor", errors)
    require("docs/verification/2026-09-29-preset-management-backup.md", "Evidence status", errors)
    forbid("PRODUCT.md", "Connections, Presets and System Settings use native macOS windows", errors)
    forbid("DESIGN.md", "Connections, Presets and System Settings use native macOS windows", errors)
    forbid("docs/research/2026-09-29-5500-8700hd-static-analysis.md", "8,19 s", errors)
    require("docs/research/2026-09-29-5500-8700hd-static-analysis.md", "8,557 s in plaats van 16,250 s", errors)
    forbid("skills/optimod-5700i-control/references/state-and-controls.md", "must not be followed by a redundant warning sheet", errors)
    require("CHANGELOG.md", "the signed release has not yet been published", errors)

    for path in sorted((ROOT / "docs/plans").glob("*.md")):
        if "> **Status:**" not in path.read_text(encoding="utf-8"):
            errors.append(f"{path.relative_to(ROOT)}: missing historical plan status")

    for path in sorted((ROOT / "docs/verification").glob("*.md")):
        text = path.read_text(encoding="utf-8")
        if "> **Evidence status:**" not in text and "> **Superseded" not in text:
            errors.append(f"{path.relative_to(ROOT)}: missing point-in-time evidence status")

    if errors:
        print("Documentation audit failed:")
        for error in errors:
            print(f"- {error}")
        return 1

    print(f"Documentation audit passed: {len(files)} Markdown files")
    return 0


if __name__ == "__main__":
    sys.exit(main())
