#!/usr/bin/env python3
"""Derive an Open Optimod Remote model profile from an official Orban PC Remote installer.

Usage:
    python3 scripts/extract_pc_remote.py INSTALLER.exe OUTPUT_DIR [--work DIR]

The installer is unpacked with ``innoextract`` into a temporary work directory.
Nothing from the installer is copied into OUTPUT_DIR; only derived facts are
written:

    parameters.json   wire names, scopes and index-to-value tables
    layouts.json      processing pages from the PC Remote dialog resources
    meters.json       meter banks, channel groups, orientation and curves
    report.json       hashes, counts and every field that was left out

Requirements: innoextract, and the Python packages pefile, capstone and unicorn.

How the facts are derived
-------------------------
* Parameters: every PC Remote parameter is registered through one constructor
  ``ctor(reference_id, name, value_count, default_index, conversion, fn2)``.
  Each conversion routine is a pure function ``conversion(index, out)`` and is
  executed for every index in a Unicorn x86 emulator. String assignment,
  ``sprintf`` and ``itoa`` are implemented in Python; CRT maths runs natively.
* Every result is cross-checked against the factory presets in the same
  installer, which are documents produced by the processor firmware. Fields
  that disagree are either explained by one consistent transform (``Int n`` on
  screen is ``Cent n*100`` on the wire, or a constant offset) or left out.
* Pages: control-to-parameter bindings (slider, radio group and combo box
  binders) are grouped per dialog initialisation function and matched to the
  dialog resource with the largest control overlap.
* Meters: meter bars are subclassed progress controls. Their channel, curve
  table and orientation come from the dialog initialisation code, or from the
  per-member update function where the channel is assigned there.

Adding a model: run the script once with a new installer. An unknown PC Remote
executable stops with its SHA-256 and the call targets it found; add a CONFIGS
entry with the addresses the analysis prints (see docs/adding-a-model.md).
"""
from __future__ import annotations

import argparse
import collections
import hashlib
import json
import re
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

import pefile
from capstone import CS_ARCH_X86, CS_MODE_32, Cs
from unicorn import Uc, UcError, UC_ARCH_X86, UC_HOOK_CODE, UC_MODE_32
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_ECX, UC_X86_REG_EIP, UC_X86_REG_ESP

# Per PC Remote executable (SHA-256). Addresses are fixed facts of one build.
CONFIGS = {
    "c8f138357e9b8758af16e1c756e88258cecb41b762b9daf7112b66649565a6a1": dict(
        model="5500", adapter="pc-remote-5500-1.2.8.24", firmware="1.2.8.24",
        banner="5500 V 1.2.8.24", document_family="8300.",
        ctor=0x48F930, assign=0x46C750, sprintf=0x4FE2A7, itoa=0x504413,
        binders={0x41E780: ("slider", 5), 0x41E930: ("radio", 7), 0x41EB00: ("combo", 3)},
        processing_dialogs=[232, 233, 234, 235, 240, 243, 245, 246, 377, 387, 400, 381],
        meter=dict(subclass=0x51A87D, init=0x436F80, chan=0x438C90, curve=0x437620,
                   dialogs=[247, 373, 399], primary=247, max_values=79),
        overrides={},
    ),
    "63def9bbe3ea4cc5714b958a330f511e9f8540b973f0cedaf596dd7956054b43": dict(
        model="5700i", adapter="pc-remote-5700i-3.0.1.20", firmware="3.0.1.20",
        # A hardware-verified profile exists: write static-*.json supplements
        # next to it instead of replacing parameters.json.
        supplement=True,
        banner="5700i V 3.0.1.20", document_family="5700.",
        ctor=0x4B2E20, assign=0x47C400, sprintf=0x52A3D8, itoa=0x530481,
        binders={0x4293D0: ("slider", 5), 0x429580: ("radio", 7), 0x429750: ("combo", 3)},
        processing_dialogs=[232, 233, 234, 235, 240, 244, 245, 246, 362, 368, 370, 375, 377, 380,
                            387, 388, 393, 394, 395, 396, 399, 400],
        meter=dict(subclass=0x546767, init=0x442810, chan=0x4447F0, curve=0x442EB0,
                   getbyte=0x4B8EB0, setvalue=0x442D70,
                   dialogs=[247, 373, 378], primary=247, max_values=112),
        # Register-pushed constructor arguments. DIVERSITY DELAY ADJ uses model
        # branch 2 (0x4ab0ff), the 5700i count; BS1770 counts are 0xc9 (0x608340).
        overrides={
            ("AGC BASS COUPLE", 17): dict(count=14, default=13, conv=0x490650),
            ("DIVERSITY DELAY ADJ", 266): dict(count=0xF833E, default=0x797B6, conv=0x4986D0),
            ("BS1770 LDNES CTRL THR", 426): dict(count=201, default=0x82, conv=0x498070),
            ("FM BS1770 LDNES CTRL THR", 428): dict(count=201, default=0x82, conv=0x498070),
        },
    ),
    "6ff1f203b45345553459e70f043ae681514c98ba130058d677e70170a72ee5b5": dict(
        model="8700hd", adapter="pc-remote-8700hd-1.0.2.161", firmware="1.0.2.161",
        banner="8700HD V 1.0.2.161", document_family="8700.",
        ctor=0x4AEDA0, assign=0x479150, sprintf=0x5251E8, itoa=0x52B131,
        binders={0x4285F0: ("slider", 5), 0x4287A0: ("radio", 7), 0x428970: ("combo", 3)},
        processing_dialogs=[232, 233, 234, 235, 240, 244, 245, 246, 362, 368, 370, 375, 377,
                            387, 388, 393, 394, 395, 396, 399, 400],
        meter=dict(subclass=0x541417, init=0x441460, chan=0x443280, curve=0x441B00,
                   getbyte=0x4B4280, setvalue=0x4419C0,
                   dialogs=[247, 373, 378], primary=247, max_values=105),
        # Register-pushed constructor arguments, resolved by hand from the call sites.
        overrides={
            ("AGC BASS COUPLE", 16): dict(count=14, default=13, conv=0x48E130),
            ("DIVERSITY DELAY ADJ", 259): dict(count=0x100000, default=0x797B6, conv=0x4959E0),
            ("BS1770 LDNES CTRL THR", 402): dict(count=201, default=0x82, conv=0x495380),
            ("FM BS1770 LDNES CTRL THR", 404): dict(count=201, default=0x82, conv=0x495380),
        },
    ),
}
# Page titles where the dialog caption is empty, generic or shared by variants.
PAGE_TITLES = {246: "Less More", 381: "Clipper Options", 393: "Equalizer", 396: "MX Distortion Control",
               388: "2 Band", 395: "Speech Mode", 399: "MX Speech Mode", 370: "Speech Mode",
               368: "Distortion Control"}
PAGE_ORDER = ["Less More", "Stereo Enhancer", "AGC", "Equalizer", "Multiband", "Compressors", "Band Mix",
              "2 Band", "Distortion Control", "2 Band Distortion", "Final Clipping", "Clipper Options",
              "Speech Mode", "HD Limiting"]

md = Cs(CS_ARCH_X86, CS_MODE_32)


# --------------------------------------------------------------------------- image helpers
class Image:
    def __init__(self, path: Path):
        self.path = path
        self.pe = pefile.PE(str(path))
        self.base = self.pe.OPTIONAL_HEADER.ImageBase
        self.mem = bytearray(self.pe.get_memory_mapped_image())
        text = self.pe.sections[0]
        self.text_start = self.base + text.VirtualAddress
        self.text = bytes(self.mem[text.VirtualAddress:text.VirtualAddress + text.Misc_VirtualSize])

    def cstr(self, va: int, limit: int = 200):
        offset = va - self.base
        if not 0 <= offset < len(self.mem):
            return None
        end = self.mem.find(b"\0", offset, offset + limit)
        return self.mem[offset:end].decode("latin1") if end >= 0 else None

    def dis(self, va: int, count: int):
        out = []
        for ins in md.disasm(bytes(self.mem[va - self.base:va - self.base + count * 8]), va):
            out.append(ins)
            if len(out) >= count:
                break
        return out

    def linear(self):
        return list(md.disasm(self.text, self.text_start))

    def function_start(self, va: int) -> int:
        offset = va - self.base
        while offset > 0 and not (self.mem[offset - 1] in (0xCC, 0xC3, 0x90) and self.mem[offset:offset + 3] == b"\x55\x8b\xec"):
            offset -= 1
        return offset + self.base

    def callers(self, target: int) -> list[int]:
        starts = set()
        for match in re.finditer(rb"\xe8(....)", self.text, re.S):
            address = self.text_start + match.start()
            if (address + 5 + struct.unpack("<i", match.group(1))[0]) & 0xFFFFFFFF == target:
                starts.add(self.function_start(address))
        return sorted(starts)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


# --------------------------------------------------------------------------- parameters
NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9 +\-/&.#%()_<>:]{1,30}")


def registrations(image: Image, ctor: int):
    """Yield (name, reference_id, count, default, conversion) for every constructor call."""
    found = []
    # Lookahead: "push count; push name; push id" must not hide the name push.
    for match in re.finditer(rb"(?=\x68(....)(\x68(....)|\x6a(.)))", image.text, re.S):
        name = image.cstr(struct.unpack("<I", match.group(1))[0], 40)
        if not name or not NAME.fullmatch(name):
            continue
        reference = struct.unpack("<I", match.group(3))[0] if match.group(3) else match.group(4)[0]
        site = image.text_start + match.start()
        width = 5 + (5 if match.group(3) is not None else 2)
        following = image.dis(site + width, 3)
        call = next((i for i in following if i.mnemonic == "call"), None)
        if not call or call.op_str != hex(ctor):
            continue
        pushes = None
        for back in range(14, 40):
            window = image.dis(site - back, 30)
            addresses = [i.address for i in window]
            if site in addresses:
                before = [i for i in window[:addresses.index(site)] if i.mnemonic == "push"][-4:]
                if len(before) == 4 and all(i.op_str.startswith("0x") or i.op_str.isdigit() for i in before):
                    pushes = [int(i.op_str, 0) for i in before]
                    break
        if pushes:
            _, conversion, default, count = pushes
            found.append(dict(name=name, id=reference, count=count, default=default, conv=conversion))
        else:
            found.append(dict(name=name, id=reference, count=None))
    return found


STACK, OUT, HEAP, STOP = 0x0F000000, 0x0E000000, 0x0D000000, 0x0C000000


def c_format(fmt: str, next_arg) -> str:
    out = []
    for match in re.finditer(r"%([-+ 0#]*)(\d*)(?:\.(\d+))?(l|h|I64)?([diouxXfFeEgGsc%])|[^%]+", fmt):
        token = match.group(0)
        if not token.startswith("%"):
            out.append(token)
            continue
        conversion = match.group(5)
        if conversion == "%":
            out.append("%")
            continue
        spec = "%" + (match.group(1) or "") + (match.group(2) or "") + ("." + match.group(3) if match.group(3) is not None else "")
        if conversion in "fFeEgG":
            out.append((spec + conversion) % next_arg("double"))
        elif conversion == "s":
            out.append((spec + "s") % next_arg("str"))
        elif conversion == "c":
            out.append(chr(next_arg("int") & 0xFF))
        else:
            value = next_arg("int")
            if conversion in "di" and value >= 2**31:
                value -= 2**32
            out.append((spec + ("d" if conversion in "iu" else conversion)) % value)
    return "".join(out)


class Emulator:
    def __init__(self, image: Image, config: dict):
        self.uc = Uc(UC_ARCH_X86, UC_MODE_32)
        self.uc.mem_map(image.base, (len(image.mem) + 0xFFF) & ~0xFFF)
        self.uc.mem_write(image.base, bytes(image.mem))
        for area in (STACK - 0x100000, OUT, HEAP, STOP):
            self.uc.mem_map(area, 0x100000)
        self.hooks = {config["assign"]: (self._assign, 4), config["sprintf"]: (self._sprintf, 0), config["itoa"]: (self._itoa, 0)}
        self.strings: dict[int, str] = {}
        self.error = None
        self.uc.hook_add(UC_HOOK_CODE, self._code)

    def arg(self, n: int) -> int:
        esp = self.uc.reg_read(UC_X86_REG_ESP)
        return struct.unpack("<I", self.uc.mem_read(esp + 4 + 4 * n, 4))[0]

    def cstr(self, va: int) -> str:
        data = bytes(self.uc.mem_read(va, 512))
        return data[:data.index(b"\0")].decode("latin1")

    def _code(self, uc, address, size, _):
        if address not in self.hooks:
            return
        handler, pop = self.hooks[address]
        try:
            result = handler()
        except Exception as error:  # noqa: BLE001 - reported per field
            self.error = f"hook {address:x}: {error}"
            uc.emu_stop()
            return
        esp = uc.reg_read(UC_X86_REG_ESP)
        ret = struct.unpack("<I", uc.mem_read(esp, 4))[0]
        if result is not None:
            uc.reg_write(UC_X86_REG_EAX, result & 0xFFFFFFFF)
        uc.reg_write(UC_X86_REG_ESP, esp + 4 + pop)
        uc.reg_write(UC_X86_REG_EIP, ret)

    def _assign(self):
        this = self.uc.reg_read(UC_X86_REG_ECX)
        pointer = self.arg(0)
        self.strings[this] = self.cstr(pointer) if pointer else ""
        return this

    def _sprintf(self):
        buffer, fmt, cursor = self.arg(0), self.cstr(self.arg(1)), [2]

        def next_arg(kind):
            if kind == "double":
                low, high = self.arg(cursor[0]), self.arg(cursor[0] + 1)
                cursor[0] += 2
                return struct.unpack("<d", struct.pack("<II", low, high))[0]
            value = self.arg(cursor[0])
            cursor[0] += 1
            return self.cstr(value) if kind == "str" else value

        text = c_format(fmt, next_arg)
        self.uc.mem_write(buffer, text.encode("latin1") + b"\0")
        return len(text)

    def _itoa(self):
        value, buffer, radix = self.arg(0), self.arg(1), self.arg(2)
        if radix != 10:
            raise ValueError("unsupported radix")
        if value >= 2**31:
            value -= 2**32
        self.uc.mem_write(buffer, str(value).encode() + b"\0")
        return buffer

    def convert(self, function: int, index: int):
        """Run conversion(index, OUT) and return (kind, number, string) or raise."""
        self.strings, self.error = {}, None
        self.uc.mem_write(OUT, b"\0" * 0x100)
        esp = STACK - 0x1000
        self.uc.mem_write(esp, struct.pack("<III", STOP, index, OUT))
        self.uc.reg_write(UC_X86_REG_ESP, esp)
        self.uc.reg_write(UC_X86_REG_EBP, esp + 0x800)
        try:
            self.uc.emu_start(function, STOP, count=200000)
        except UcError as error:
            self.error = self.error or f"emulation {error}"
        if self.error or self.uc.reg_read(UC_X86_REG_EIP) != STOP:
            raise RuntimeError(self.error or "conversion did not return")
        kind = struct.unpack("<I", self.uc.mem_read(OUT, 4))[0]
        number = struct.unpack("<i", self.uc.mem_read(OUT + 0x18, 4))[0]
        return kind, number, self.strings.get(OUT + 4, "")


def emulate(image: Image, config: dict):
    emulator = Emulator(image, config)
    fields = []
    for registration in registrations(image, config["ctor"]):
        override = config["overrides"].get((registration["name"], registration["id"]))
        if override:
            registration.update(override)
        if registration.get("count") is None:
            fields.append(dict(registration, values=None, error="register-pushed arguments"))
            continue
        values, error = [], None
        limit = registration["count"] if registration["count"] <= 70000 else 3
        for index in range(limit):
            try:
                kind, number, text = emulator.convert(registration["conv"], index)
            except RuntimeError as failure:
                error = str(failure)
                break
            if kind == 1:
                values.append({"type": "Int", "value": number, "unit": text})
            elif kind in (5, 6):  # both serialise as Cent:%i
                values.append({"type": "Cent", "value": number, "unit": text})
            elif kind == 4:
                values.append({"type": "Choice", "value": text})
            elif kind == 7:
                values.append({"type": "Text", "value": text})
            else:
                error = f"value kind {kind} at index {index}"
                break
        fields.append(dict(registration, values=values, error=error))
    return fields


RECORD = re.compile(r"^C:<([^>]*)>(Int|Cent|String|UserString|Float|FixPoint):(<[^>]*>|[^;]*);D:(\d+);")


def factory_observations(root: Path):
    observed, files, families = collections.defaultdict(set), 0, collections.Counter()
    for path in sorted((root / "app" / "presets").rglob("*")):
        if not path.is_file():
            continue
        text = path.read_text("latin1")
        files += 1
        families[text.split("\n", 1)[0].strip()] += 1
        for line in text.splitlines():
            match = RECORD.match(line)
            if match:
                name, kind, value, index = match.groups()
                observed[name].add((kind, value.strip("<>"), int(index)))
    return observed, files, dict(families)


def fits(values, records, factor, offset):
    for kind, value, index in records:
        if index >= len(values):
            return False
        entry = values[index]
        if entry["type"] == "Choice":
            if (kind, value) != ("String", entry["value"]):
                return False
        elif entry["type"] in ("Int", "Cent"):
            wire = "Cent" if factor == 100 or entry["type"] == "Cent" else "Int"
            if kind != wire or int(value) != entry["value"] * factor + offset:
                return False
        else:
            return False
    return True


def transform(values, factor, offset):
    out = []
    for entry in values:
        if entry["type"] in ("Int", "Cent"):
            out.append({"type": "Cent" if factor == 100 or entry["type"] == "Cent" else "Int", "value": entry["value"] * factor + offset})
        else:
            out.append({"type": entry["type"], "value": entry["value"]})
    return out


def build_parameters(raw, observed, reference_scopes):
    report, notes, fields = collections.Counter(), [], []
    candidates = collections.defaultdict(list)
    for registration in raw:
        if registration.get("error") or not registration.get("values"):
            report["not-convertible"] += 1
            notes.append((registration["name"], "not convertible"))
            continue
        if any(v["type"] == "Text" for v in registration["values"]):
            report["text"] += 1  # handled by the typed text path, not by an index table
            continue
        candidates[registration["name"]].append(registration)
    for name, registrations_for_name in candidates.items():
        records = observed.get(name, set())
        if len(registrations_for_name) > 1:
            ordered = sorted(registrations_for_name, key=lambda r: len(r["values"]))
            shortest = ordered[0]
            if all(r["values"][:len(shortest["values"])] == shortest["values"] for r in ordered[1:]):
                registration = shortest  # every allowed index means the same in all variants
            else:
                matching = [r for r in ordered if records and fits(r["values"], records, 1, 0)]
                if len(matching) != 1 and records:
                    # Otherwise the variant that explains the most factory-preset records.
                    scores = sorted(((sum(1 for rec in records if fits(r["values"], [rec], 1, 0)), r) for r in ordered),
                                    key=lambda item: -item[0])
                    if scores[0][0] > 0 and scores[0][0] > scores[1][0]:
                        matching = [scores[0][1]]
                if len(matching) != 1:
                    report["ambiguous-duplicate"] += 1
                    notes.append((name, "ambiguous duplicate ids " + ",".join(str(r["id"]) for r in ordered)))
                    continue
                registration = matching[0]
        else:
            registration = registrations_for_name[0]
        values, evidence = registration["values"], "unobserved"
        # An index whose conversion yields no text is not defined by PC Remote.
        while values and values[-1]["type"] == "Choice" and not values[-1]["value"]:
            values = values[:-1]
        if any(v["type"] == "Choice" and not v["value"] for v in values):
            report["undefined-index"] += 1
            notes.append((name, "undefined index inside the value table"))
            continue
        if records:
            numeric_nonzero = any(k in ("Int", "Cent") and int(v) != 0 for k, v, _ in records)
            if fits(values, records, 1, 0):
                evidence = "confirmed"
            elif numeric_nonzero and fits(values, records, 100, 0):
                values, evidence = transform(values, 100, 0), "transform x100"
            else:
                # A constant offset between PC Remote and presets is not trusted: on the
                # hardware-verified 5700i the presets came from older firmware and such
                # an offset was wrong (MPX PWR OFFSET).
                report["conflict"] += 1
                notes.append((name, "conflicts with factory presets"))
                continue
        report[evidence.split(" ")[0]] += 1
        field = {"reference_id": registration["id"], "name": name, "evidence": evidence}
        units = collections.Counter(v.get("unit") for v in registration["values"][:len(values)] if v["type"] in ("Int", "Cent") and v.get("unit"))
        clean = [{"type": v["type"], "value": v["value"]} for v in values]
        if name == "DIVERSITY DELAY ADJ":
            first, second = float(registration["values"][0]["value"]), float(registration["values"][1]["value"])
            rate = round(1 / (second - first))
            # Formatting to nine decimals blurs the step; snap to a real sample rate.
            rate = min((32000, 44100, 48000, 64000, 96000, 192000), key=lambda known: abs(known - rate))
            field.update(values=[], unit="s", sample_delay={"max_index": registration["count"] - 1, "offset": round(first * rate), "rate": rate})
        elif len(clean) > 1000:
            if len(clean) == registration["count"] and all(v == {"type": "Int", "value": i} for i, v in enumerate(clean)):
                # Port 0 is not accepted by the hardware-verified 5700i.
                low = 1 if name.upper().endswith("PORT") else 0
                field.update(values=[], integer_range={"min": low, "max": registration["count"] - 1})
            else:
                report["large-unmapped"] += 1
                notes.append((name, "large range not representable"))
                continue
        else:
            field["values"] = clean
            if units:
                field["unit"] = units.most_common(1)[0][0]
        if name in observed:
            scopes = ["Processing"]  # factory presets are processing documents
        elif reference_scopes.get(name):
            scopes = sorted(reference_scopes[name])
        else:
            # Unknown scope: list both. A write still needs the field in the live
            # document of that scope, so a wrong guess is refused, never written.
            scopes = ["Processing", "System"]
        for scope in scopes:
            fields.append({"scope": scope, **field})
    fields.sort(key=lambda f: (f["scope"], f["reference_id"]))
    return fields, dict(report), notes


# --------------------------------------------------------------------------- dialogs and pages
def _string(data: bytes, offset: int):
    if struct.unpack_from("<H", data, offset)[0] == 0xFFFF:
        return struct.unpack_from("<H", data, offset + 2)[0], offset + 4
    end = offset
    while struct.unpack_from("<H", data, end)[0] != 0:
        end += 2
    return data[offset:end].decode("utf-16le"), end + 2


def parse_dialog(data: bytes):
    extended = struct.unpack_from("<HH", data, 0) == (1, 0xFFFF)
    if extended:
        _, _, _, _, style, count, x, y, cx, cy = struct.unpack_from("<HHIIIHhhhh", data, 0)
        offset = 26
    else:
        style, _, count, x, y, cx, cy = struct.unpack_from("<IIHhhhh", data, 0)
        offset = 18
    _, offset = _string(data, offset)
    _, offset = _string(data, offset)
    title, offset = _string(data, offset)
    if style & 0x40:
        offset += 6 if extended else 2
        _, offset = _string(data, offset)
    classes = {0x80: "Button", 0x81: "Edit", 0x82: "Static", 0x83: "ListBox", 0x84: "ScrollBar", 0x85: "ComboBox"}
    items = []
    for _ in range(count):
        offset = (offset + 3) & ~3
        if extended:
            _, _, item_style, ix, iy, icx, icy, identifier = struct.unpack_from("<IIIhhhhI", data, offset)
            offset += 24
        else:
            item_style, _, ix, iy, icx, icy, identifier = struct.unpack_from("<IIhhhhH", data, offset)
            offset += 18
        cls, offset = _string(data, offset)
        text, offset = _string(data, offset)
        offset += 2 + struct.unpack_from("<H", data, offset)[0]
        items.append(dict(id=identifier, cls=classes.get(cls, cls), text=text if isinstance(text, str) else "",
                          style=item_style, x=ix, y=iy, w=icx, h=icy))
    return dict(title=title, w=cx, h=cy, items=items)


def dialogs(image: Image):
    out = {}
    for resource_type in image.pe.DIRECTORY_ENTRY_RESOURCE.entries:
        if resource_type.id != 5:
            continue
        for entry in resource_type.directory.entries:
            for language in entry.directory.entries:
                data = language.data.struct
                key = entry.id if entry.id is not None else str(entry.name)
                out[key] = parse_dialog(image.pe.get_data(data.OffsetToData, data.Size))
    return out


def binder_calls(image: Image, binders: dict):
    instructions = image.linear()
    calls = collections.defaultdict(list)
    for k, ins in enumerate(instructions):
        if ins.mnemonic != "call" or not ins.op_str.startswith("0x"):
            continue
        target = int(ins.op_str, 16)
        if target not in binders:
            continue
        j = k - 1
        while j >= 0 and k - j < 5 and instructions[j].mnemonic in ("mov", "lea", "add") and instructions[j].op_str.startswith(("ecx", "eax", "edx")):
            j -= 1
        args = []
        while j >= 0 and instructions[j].mnemonic == "push" and (instructions[j].op_str.startswith("0x") or instructions[j].op_str.isdigit()):
            args.append(int(instructions[j].op_str, 0))
            j -= 1
        kind, arity = binders[target]
        if len(args) == arity:
            calls[image.function_start(ins.address)].append((ins.address, kind, args))
    return calls


def title_case(name: str) -> str:
    words = []
    for word in re.sub(r"^HD ", "", name).split():
        words.append(word if re.fullmatch(r"[A-Z0-9/]{1,3}\d*|HD|FM|MX|SP|PEQ|HF|AGC|MB|B\d.*", word) else word.capitalize())
    return " ".join(words)


def build_layouts(image: Image, config: dict, profile_fields, raw):
    resources = dialogs(image)
    names = {f["name"] for f in profile_fields if f["scope"] == "Processing"}
    by_id = {r["id"]: r["name"] for r in raw}
    pages = []
    for function, binds in binder_calls(image, config["binders"]).items():
        controls_used = {c for _, _, args in binds for c in args[1:] if c}
        best = None
        for dialog_id, dialog in resources.items():
            overlap = len(controls_used & {i["id"] for i in dialog["items"]})
            if overlap and (best is None or overlap > best[0] or (overlap == best[0] and len(dialog["items"]) < best[2])):
                best = (overlap, dialog_id, len(dialog["items"]))
        if not best or best[1] not in config["processing_dialogs"]:
            continue
        dialog = resources[best[1]]
        items = {i["id"]: i for i in dialog["items"]}
        controls = []
        for _, kind, args in sorted(binds):
            name = by_id.get(args[0])
            if not name or name not in names:
                continue
            if kind == "slider":
                edit, label, track, _ = args[1:5]
                item = items.get(track) or items.get(edit)
                if not item:
                    continue
                text = items.get(label, {}).get("text", "").strip() or title_case(name)
                controls.append(dict(kind="slider", name=name, label=text.replace("&", ""), x=item["x"], y=item["y"], w=item["w"],
                                     id=item["id"], reference_id=args[0], options=[]))
            elif kind == "radio":
                # Only real radio buttons are options; another control in the
                # binder arguments is the caption of the group.
                referenced = [items[c] for c in args[1:] if c and c in items]
                buttons = [i for i in referenced if i["cls"] == "Button" and i["style"] & 0xF in (4, 9)]
                captions = [i["text"].replace("&", "").strip() for i in referenced if i not in buttons and i["text"].strip()]
                if not buttons:
                    continue
                x, y = min(b["x"] for b in buttons), min(b["y"] for b in buttons)
                boxes = sorted((g for g in dialog["items"] if g["cls"] == "Button" and g["style"] & 0xF == 7 and g["text"].strip()
                                and g["x"] <= x <= g["x"] + g["w"] and g["y"] <= y <= g["y"] + g["h"]), key=lambda g: g["w"] * g["h"])
                label = captions[0] if captions else boxes[0]["text"].replace("&", "").strip() if boxes else title_case(name)
                controls.append(dict(kind="radio", name=name, label=label, x=x, y=y, w=max(b["x"] + b["w"] for b in buttons) - x,
                                     id=buttons[0]["id"], reference_id=args[0], options=[b["text"].replace("&", "").strip() for b in buttons]))
            else:
                item = items.get(args[1]) or items.get(args[2])
                if item:
                    controls.append(dict(kind="slider", name=name, label=title_case(name), x=item["x"], y=item["y"], w=item["w"],
                                         id=item["id"], reference_id=args[0], options=[]))
        if not controls:
            continue
        hd = sum(c["name"].startswith(("HD ", "IBOC")) for c in controls)
        path = "Shared" if best[1] in (233, 246) else ("HD" if hd * 2 > len(controls) else "FM")
        groups = [dict(x=g["x"], y=g["y"], w=g["w"], h=g["h"], text=g["text"].replace("&", ""))
                  for g in dialog["items"] if g["cls"] == "Button" and g["style"] & 0xF == 7]
        pages.append(dict(title=dialog["title"].strip() or "Settings", path=path, width=dialog["w"], height=dialog["h"],
                          reference_dialog=best[1], controls=controls, groups=groups))
    # Drop pages that only repeat a subset of a larger page on the same path.
    kept = []
    for page in pages:
        names_here = {c["name"] for c in page["controls"]}
        if not any(other is not page and other["path"] == page["path"] and names_here < {c["name"] for c in other["controls"]} for other in pages):
            kept.append(page)
    for page in kept:
        page["title"] = PAGE_TITLES.get(page["reference_dialog"], page["title"])
        names_here = {c["name"] for c in page["controls"]}
        if page["reference_dialog"] == 400 and any(n.startswith("MX ") for n in names_here):
            page["title"] = "MX 2 Band Distortion"
        if page["reference_dialog"] == 375 and "B4/5 DELTA REL" in names_here:
            page["title"] = "Compressors (B4/5 Linked)"
        if all(n.startswith(("SP ", "SPEECH")) for n in names_here):
            page["title"] = "Speech Mode"
    seen = collections.Counter()
    for page in sorted(kept, key=lambda p: -len(p["controls"])):
        key = (page["path"], page["title"])
        if seen[key]:
            page["title"] = f"{page['title']} {seen[key] + 1}"
        seen[key] += 1

    def rank(page):
        path_rank = {"Shared": 0, "FM": 1}.get(page["path"], 2)
        order = next((i for i, t in enumerate(PAGE_ORDER) if page["title"].startswith(t) or page["title"].endswith(t)), 99)
        return path_rank, order, page["title"]

    return sorted(kept, key=rank)


# --------------------------------------------------------------------------- meters
def _meter_members(image: Image, start: int, meter: dict):
    """Track calls on dialog members (this+offset) and helper arguments inside one function."""
    instructions = image.dis(start, 0x3000 // 3)
    members, helpers, pushes, registers = collections.defaultdict(dict), [], [], {}
    for ins in instructions:
        mnemonic, operands = ins.mnemonic, ins.op_str
        if mnemonic == "push":
            if operands.startswith("0x") or operands.isdigit():
                pushes.append(int(operands, 0))
            elif operands in ("eax", "ecx", "edx"):
                pushes.append(("reg", registers.get(operands)))
            elif "ebp + 8" in operands:
                pushes.append(("arg", 0))
            elif "ebp + 0xc" in operands:
                pushes.append(("arg", 1))
            elif "ebp + 0x10" in operands:
                pushes.append(("arg", 2))
            else:
                pushes.append(("mem", operands))
            continue
        if mnemonic == "mov" and operands.split(",")[0] in ("eax", "ecx", "edx"):
            register, source = operands.split(",")[0], operands.split(", ", 1)[1]
            registers[register] = (("this", 0) if source in ("dword ptr [ebp - 4]", "dword ptr [ebp - 8]")
                                   else ("arg", 0) if source == "dword ptr [ebp + 8]"
                                   else ("arg", 2) if source == "dword ptr [ebp + 0x10]" else None)
            continue
        if mnemonic == "add" and operands.split(",")[0] in ("eax", "ecx", "edx") and registers.get(operands.split(",")[0]) and operands.split(", ")[1].startswith("0x"):
            register = operands.split(",")[0]
            registers[register] = (registers[register][0], registers[register][1] + int(operands.split(", ")[1], 16))
            continue
        if mnemonic == "call":
            target = int(operands, 16) if operands.startswith("0x") else None
            member, args, pushes = registers.get("ecx"), list(reversed(pushes)), []
            if target == meter["subclass"] and member:
                members[member]["ctl"] = args[0]
            elif target == meter["init"] and member:
                members[member]["init"] = args[:3]
            elif target == meter["chan"] and member:
                members[member]["chan"] = args[0]
            elif target == meter["curve"] and member:
                members[member]["curve"] = args[:2]
            elif target:
                helpers.append((target, args))
            registers = {}
            continue
        if mnemonic == "ret":
            break
    return members, helpers


def _update_channels(image: Image, meter: dict):
    if not meter.get("getbyte"):
        return []
    maps = []
    for start in image.callers(meter["getbyte"]):
        instructions = image.dis(start, 0x4000 // 3)
        channels = {}
        for k in range(len(instructions) - 8):
            if instructions[k].mnemonic == "ret":
                break
            w = instructions[k:k + 9]
            if (w[0].mnemonic == "push" and (w[0].op_str.startswith("0x") or w[0].op_str.isdigit()) and w[4].mnemonic == "call"
                    and w[4].op_str == hex(meter["getbyte"]) and w[8].mnemonic == "call" and w[8].op_str == hex(meter["setvalue"]) and w[7].mnemonic == "add"):
                channels[("this", int(w[7].op_str.split(", ")[1], 16))] = int(w[0].op_str, 0)
        if channels:
            maps.append(channels)
    return maps


def _curve(image: Image, table: int, count: int):
    offset = table - image.base
    stored = [struct.unpack_from("<i", image.mem, offset + 4 * k)[0] for k in range(count)]
    return [stored[count - 1 - raw] for raw in range(count)]  # reference uses table[max - raw]


def extract_meter_bars(image: Image, meter: dict):
    resources = dialogs(image)
    updates = _update_channels(image, meter)
    helper_bodies, bars = {}, []
    for start in image.callers(meter["subclass"]):
        members, helpers = _meter_members(image, start, meter)
        own = {k: v for k, v in members.items() if k[0] == "this"}
        if updates and own:
            def score(update):
                if any(k in own and "chan" in own[k] and own[k]["chan"] != ch for k, ch in update.items()):
                    return -1
                return sum(1 for k, ch in update.items() if k in own and own[k].get("chan") == ch)
            best = max(updates, key=score)
            if score(best) >= 1:
                for key, channel in best.items():
                    if key in own and "chan" not in own[key]:
                        own[key]["chan"] = channel
        if ("arg", 0) in members:
            helper_bodies[start] = members[("arg", 0)]
        for key, state in own.items():
            if isinstance(state.get("ctl"), int):
                bars.append(dict(func=start, **state))
        for target, args in helpers:
            bars.append(dict(func=start, helper=target, args=args))
    resolved = []
    for bar in bars:
        if "helper" in bar:
            body = helper_bodies.get(bar["helper"])
            if not body or len(bar["args"]) < 3 or not isinstance(bar["args"][1], int):
                continue
            channel = bar["args"][2] if body.get("chan") in (("arg", 2), ("reg", ("arg", 2))) else body.get("chan")
            bar = dict(func=bar["func"], ctl=bar["args"][1], chan=channel, init=body.get("init"), curve=body.get("curve"))
        resolved.append(bar)
    per_function = collections.defaultdict(list)
    for bar in resolved:
        per_function[bar["func"]].append(bar)
    views = {}
    for dialog_id in meter["dialogs"]:
        dialog = resources[dialog_id]
        items = {i["id"]: i for i in dialog["items"]}
        progress = {i["id"] for i in dialog["items"] if i["cls"] == "msctls_progress32"}
        owner = max(per_function, key=lambda f: (len({b["ctl"] for b in per_function[f]} & progress), -len({b["ctl"] for b in per_function[f]} - progress)))
        entries = []
        for bar in per_function[owner]:
            item = items.get(bar["ctl"])
            if not item or item["cls"] != "msctls_progress32" or not isinstance(bar.get("chan"), int) or any(e["ctl"] == bar["ctl"] for e in entries):
                continue
            init = bar.get("init") or [None, None, None]
            curve = _curve(image, bar["curve"][0], bar["curve"][1]) if bar.get("curve") and isinstance(bar["curve"][0], int) else None
            entries.append(dict(ctl=bar["ctl"], channel=bar["chan"], x=item["x"], y=item["y"], h=item["h"],
                                orientation={1: "down", 2: "up"}.get(init[1] if isinstance(init[1], int) else None), curve=curve))
        labels = [dict(text=s["text"].strip(), x=s["x"], y=s["y"], w=s["w"]) for s in dialog["items"]
                  if s["cls"] == "Static" and s["text"].strip() and "--" not in s["text"]]
        views[dialog_id] = dict(title=dialog["title"], labels=labels, bars=sorted(entries, key=lambda e: e["x"]))
    return views


def _meter_kind(title: str, bars) -> str:
    lower = title.lower()
    if "enhance" in lower:
        return "enhance"
    if "comp" in lower:
        return "composite"
    if "loudness level" in lower or lower in ("loudness", "mpx") or "power" in lower:
        return "loudness"
    if any(k in lower for k in ("reduction", "agc", "limit", "g/r")):
        return "reduction"
    if "input" in lower or "output" in lower:
        return "level"
    return "reduction" if all(b["orientation"] == "down" for b in bars) else "level"


def _meter_groups(view):
    titles = sorted((l for l in view["labels"] if l["y"] < 10 or (l["y"] < 100 and len(l["text"]) > 5 and not l["text"].endswith("Gate"))), key=lambda l: l["x"])
    lane_labels = [l for l in view["labels"] if l["y"] >= 60 and len(l["text"]) <= 5]
    bars = sorted((b for b in view["bars"] if b["h"] >= 50), key=lambda b: b["x"])  # 6 px bars are gate LEDs
    clusters = []
    for bar in bars:
        signature = (bar["orientation"], len(bar["curve"] or []), bar["h"])
        last = clusters[-1][-1] if clusters else None
        if last and bar["x"] - last["x"] <= 32 and (last["orientation"], len(last["curve"] or []), last["h"]) == signature:
            clusters[-1].append(bar)
        else:
            clusters.append([bar])
    grouped = []
    for cluster in clusters:
        center = (cluster[0]["x"] + cluster[-1]["x"] + 10) / 2
        title = min(titles, key=lambda l: abs(l["x"] + l["w"] / 2 - center))["text"] if titles else "Meters"
        group = next((g for g in grouped if g["name"] == title), None)
        if not group:
            group = {"name": title, "bars": []}
            grouped.append(group)
        group["bars"].extend(cluster)
    result = []
    for group in grouped:
        ordered, channels, i = sorted(group["bars"], key=lambda b: b["x"]), [], 0
        while i < len(ordered):
            pair = [ordered[i]]
            if i + 1 < len(ordered) and ordered[i + 1]["x"] - ordered[i]["x"] <= 8:
                pair.append(ordered[i + 1])
                i += 1
            i += 1
            under = [l["text"] for l in lane_labels if abs(l["x"] - pair[0]["x"]) <= 10]
            label = under[0] if under and (len(pair) == 2 or under[0] not in ("L", "R")) else ""
            channels.append({"label": label, "ids": [b["channel"] for b in pair], "bars": pair})
        if len(channels) == 2 and all(len(c["ids"]) == 1 for c in channels):
            under = [[l["text"] for l in lane_labels if abs(l["x"] - c["bars"][0]["x"]) <= 10][:1] for c in channels]
            if under == [["L"], ["R"]]:
                channels = [{"label": "", "ids": [channels[0]["ids"][0], channels[1]["ids"][0]], "laneLabels": ["L", "R"],
                             "bars": channels[0]["bars"] + channels[1]["bars"]}]
        if group["name"].endswith("Gain Reduction") and len(channels) == 5 and not any(c["label"] for c in channels):
            for n, channel in enumerate(channels):
                channel["label"] = str(n + 1)
        result.append({"name": group["name"], "kind": _meter_kind(group["name"], group["bars"]), "channels": channels})
    return result


def build_meters(image: Image, meter: dict):
    views = extract_meter_bars(image, meter)
    primary = _meter_groups(views[meter["primary"]])
    seen = {i for g in primary for c in g["channels"] for i in c["ids"]}
    extra = []
    for dialog_id, view in views.items():
        if dialog_id == meter["primary"]:
            continue
        for group in _meter_groups(view):
            ids = {i for c in group["channels"] for i in c["ids"]}
            if ids and not ids & seen:
                taken = {g["name"] for g in primary + extra}
                name = group["name"] if group["name"] not in taken else ("2-Band " + group["name"] if group["name"].endswith("Gain Reduction") else group["name"] + " (alt)")
                extra.append(dict(group, name=name))
                seen |= ids
    groups, orientation, curves = primary + extra, {}, {}
    for group in groups:
        for channel in group["channels"]:
            for bar in channel.pop("bars"):
                orientation[str(bar["channel"])] = bar["orientation"]
                if bar["curve"]:
                    curves[str(bar["channel"])] = bar["curve"]
    return {"banks": [1, 2], "max_values": meter["max_values"], "groups": groups, "orientation": orientation, "curves": curves}


# --------------------------------------------------------------------------- main
def reference_scopes(repository: Path):
    scopes = collections.defaultdict(set)
    reference = repository / "profiles" / "5700i" / "3.0.1.20" / "parameters.json"
    for field in json.loads(reference.read_text())["fields"]:
        scopes[field["name"]].add(field["scope"])
    return scopes


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("installer", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--work", type=Path, help="keep the unpacked installer here")
    args = parser.parse_args()
    work = args.work or Path(tempfile.mkdtemp(prefix="optimod-extract-"))
    try:
        if not (work / "app").is_dir():
            subprocess.run(["innoextract", "-s", "-d", str(work), str(args.installer)], check=True)
        executables = [p for p in (work / "app").glob("*PC.exe")]
        if len(executables) != 1:
            sys.exit("Expected exactly one PC Remote executable in the installer")
        executable = executables[0]
        digest = sha256(executable)
        config = CONFIGS.get(digest)
        if not config:
            sys.exit(f"Unknown PC Remote build {executable.name} sha256={digest}; add a CONFIGS entry (docs/adding-a-model.md)")
        image = Image(executable)
        firmware_zip = work / "app" / "update" / "update.zip"
        firmware_hash = None
        if firmware_zip.is_file():
            subprocess.run(["unzip", "-oq", str(firmware_zip), "bin/_optimod.exe", "-d", str(work / "firmware")], check=True)
            firmware_hash = sha256(work / "firmware" / "bin" / "_optimod.exe")
        raw = emulate(image, config)
        observed, preset_files, families = factory_observations(work)
        repository = Path(__file__).resolve().parents[1]
        fields, counts, notes = build_parameters(raw, observed, reference_scopes(repository))
        header = {"model": config["model"], "firmware": config["firmware"], "reference_sha256": digest, "firmware_sha256": firmware_hash}
        args.output.mkdir(parents=True, exist_ok=True)
        prefix = "static-" if config.get("supplement") else ""
        (args.output / f"{prefix}parameters.json").write_text(json.dumps({
            **header,
            "status": "Statically derived from the official PC Remote conversion routines and cross-checked against the "
                      "factory presets in the same installer. Not verified on hardware.",
            "fields": fields}, indent=2) + "\n")
        layouts = build_layouts(image, config, fields, raw)
        (args.output / f"{prefix}layouts.json").write_text(json.dumps(layouts, indent=2) + "\n")
        meters = build_meters(image, config["meter"])
        (args.output / f"{prefix}meters.json").write_text(json.dumps({**header, **meters}, indent=1) + "\n")
        (args.output / f"{prefix}report.json").write_text(json.dumps({
            **header, "installer_sha256": sha256(args.installer), "banner": config["banner"],
            "document_family": config["document_family"], "registrations": len(raw),
            "factory_presets": preset_files, "preset_families": families,
            "preset_fields": len(observed), "field_evidence": counts,
            "excluded": [{"name": n, "reason": r} for n, r in notes],
            "pages": [p["title"] + " (" + p["path"] + ")" for p in layouts],
            "meter_groups": [g["name"] for g in meters["groups"]]}, indent=2) + "\n")
        print(json.dumps(counts), len(fields), "profile entries,", len(layouts), "pages,", len(meters["groups"]), "meter groups")
    finally:
        if not args.work:
            shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    main()
