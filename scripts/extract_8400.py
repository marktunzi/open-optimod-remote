#!/usr/bin/env python3
"""Rebuild the OPTIMOD-FM 8400 3.0.5 (software 3.0.5.7) parameter profile for Open Optimod Remote.

Static analysis of ORIGINAL files only: no unit emulator, no Wine, no network, no
git writes. Output: parameters.json, layouts.json, report.json in --out.

Run with the Optimod Emulator venv (capstone, unicorn, pefile):
    "<Optimod Emulator>/.venv/bin/python" -I scripts/extract_8400.py [--out DIR] [--<source> PATH ...]

How the facts are derived
-------------------------
Parameters (PROVEN by code):
  Every 8400 control is a 0x58-byte object built by one constructor, the same
  PEG code base in the unit firmware (8400_ets.exe, flat image base 0x100000) and
  in the PC Remote (8400_pc.exe, PE base 0x400000):
      ctor(name, value_count, default_index, flag, conversion, f5, f6)
        firmware 0x107e20, PC Remote 0x407c90
  conversion(index, value*) is a pure function filling a value object:
      +0x00 kind  1 Int, 2 Cent-from-double, 3 FixPoint, 4 String, 5 Cent, 6 UserString
      +0x04 string object {vtable, char*, cap, len}
      +0x18 int32 (kinds 1, 5) or IEEE double (kind 2)
  The document writer (firmware 0x107410, PC Remote 0x407490) emits kind 1 as
  "Int:%d", 5 as "Cent:%d", 2 as "Cent:" + ftol(floor(x*100.0 + 0.5)), 4 as
  "String:<s>", 6 as "UserString:<s>". Every conversion is executed for every index
  in a Unicorn x86 emulator over the original image (CRT and string class run
  natively; only malloc/free are replaced by a bump allocator); kind 2 is formatted
  by running the writer's own x87 sequence (fld/fmul 100.0/fadd 0.5/floor/ftol).
  The PC Remote tables are computed the same way and must be identical (raw kind,
  integer, double bits and text) to the firmware tables.
Scope (PROVEN by code + documents):
  Firmware registration function 0x151d30 = processing parameters (onair.orb and
  every preset), 0x1593e0 = system controls (system.orb). Other groups belong to
  time.orb (0x15f3e0), test.orb (0x15e220), automation (0x15fd70) and remote
  contact actions (0x15ea80, 0x1609c0); the app has no scope for those, so they are
  excluded and listed in report.json.
Evidence: each table is checked against every C:<NAME>TYPE:VALUE;D:INDEX; record in
  Orban's factory presets (firmware package + PC Remote installer, incl. the LM
  Less-More files) and the documents a 3.0.5 unit wrote (onair/next/system/time/test).
  "confirmed" = every record equals the table at its index, "unobserved" = the name is
  in no such document, "conflict" = a record disagrees (the code-proven table is kept
  and the records are listed). User presets (possibly from older firmware) are only
  reported. A parameter without a provable table is excluded with its reason.
Units: from the V3.0 worksheet row of the parameter; a parameter without a row takes
  the unit of a worksheet parameter that uses the same conversion routine.
Pages: the PC Remote builds its PEG screens in code; every control is a call
  0x41a8a0(name, label, x, y, style, font, 0) and every Advanced Modify list row a
  call 0x40a200(&pos, name, ...), whose label is the parameter's short label. Page
  titles are the "Basic/Intermediate/Advanced Modify n", "Input/Output n" and
  "System Setup: ..." strings pushed before the controls of a page.
"""
from __future__ import annotations

import argparse
import bisect
import collections
import hashlib
import json
import re
import struct
from pathlib import Path

import capstone
import pefile
from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_HOOK_MEM_READ
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_EIP

HOME = Path.home()
EMU = HOME / "Dropbox/03 Sideprojects/App projects/Optimod Emulator"
REPO = Path(__file__).resolve().parent.parent
CLIENT_DIR = HOME / "Library/Application Support/CrossOver/Bottles/Optimod 8400/drive_c/Program Files (x86)/Orban/Optimod 8400 PC Remote"
DEFAULTS = dict(
    image=EMU / "research/8400/firmware/derived/optimod_image.bin",
    ets=EMU / "research/8400/firmware/unpacked/8400_upd/bin/8400_ets.exe",
    fw_presets=EMU / "research/8400/firmware/unpacked/8400_upd/presets",
    fw_user=EMU / "research/8400/firmware/unpacked/8400_upd/user",
    client=CLIENT_DIR / "8400_pc.exe",
    client_presets=CLIENT_DIR / "presets",
    worksheet=CLIENT_DIR / "8400_V305_Worksheet.pdf",
    unit_user=HOME / "werk/oor-p2.noindex/units/8400-p2/flash/8400/user",
    onair_live=HOME / "werk/oor-p2.noindex/units/8400-p2/flash/8400/user/onair.orb",
    out=REPO / "profiles/8400/3.0.5.7",
)

FW = dict(
    label="firmware 8400_ets.exe",
    ctor=0x107E20, malloc=0x1DFE30, free=0x1DFE24, str_ctor=0x17CE44, floor=0x1E305C, ftol=0x1E312C,
    c100=0x1F77A8, c05=0x1F77A0, set_short=0x108230, set_long=0x108210, set_help=0x1081D0, writer=0x107410,
)
PC = dict(
    label="PC Remote 8400_pc.exe",
    ctor=0x407C90, malloc=0x497211, free=0x496D44, str_ctor=0x473F00, floor=0x4980DC, ftol=0x49820C,
    c100=0x4B52F0, c05=0x4B52E8, set_short=0x407F30, set_long=0x407F10, set_help=0x407EE0, writer=0x407490,
    builder=0x41A8A0, row=0x40A200,
)
FW_GROUPS = {
    0x151D30: "processing", 0x1593E0: "system", 0x15E220: "test", 0x15EA80: "contact-actions",
    0x15F3E0: "time", 0x15FD70: "automation", 0x1609C0: "automation-actions",
}
SCOPE = {"processing": "Processing", "system": "System"}

md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
md.skipdata = True


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


class Image:
    """A 32-bit x86 image in memory: the firmware flat image or a mapped PE."""

    def __init__(self, path: Path, flat: bool):
        if flat:
            self.base, self.mem = 0x100000, path.read_bytes()
            self.code = (0x1006D0, 0x3251FB)          # CODE ends where the DATA record stream starts
            self.bss = 0x3A9800                       # .bss: zero at load, runtime state
        else:
            pe = pefile.PE(str(path))
            self.base, self.mem = pe.OPTIONAL_HEADER.ImageBase, bytes(pe.get_memory_mapped_image())
            text = pe.sections[0]
            self.code = (self.base + text.VirtualAddress, self.base + text.VirtualAddress + text.Misc_VirtualSize)
            data = [s for s in pe.sections if s.Name.rstrip(b"\0") == b".data"][0]
            self.bss = self.base + data.VirtualAddress + data.SizeOfRawData
        self.top = (self.base + len(self.mem) + 0xFFF) & ~0xFFF
        lo, hi = self.code
        self.ins = list(md.disasm(self.mem[lo - self.base:hi - self.base], lo))
        self.addrs = [i.address for i in self.ins]

    def cstr(self, va, n=400):
        if not isinstance(va, int):
            return None
        o = va - self.base
        if not 0 <= o < len(self.mem):
            return None
        e = self.mem.find(b"\0", o, o + n)
        if e < 0:
            return None
        s = self.mem[o:e]
        return s.decode("latin1") if all(32 <= c < 127 for c in s) else None

    def calls_to(self, target):
        t = hex(target)
        return [k for k, i in enumerate(self.ins) if i.mnemonic == "call" and i.op_str == t]

    def pushes_before(self, k, n, window=40):
        out, j = [], k - 1
        while len(out) < n and j > k - window:
            if self.ins[j].mnemonic == "push":
                out.append(self.ins[j].op_str)
            j -= 1
        return out  # nearest first = first argument first

    def function_start(self, va):
        """MSVC function start at or below va: a prologue after padding/ret."""
        o = va - self.base
        lo = self.code[0] - self.base
        while o > lo:
            if self.mem[o - 1] in (0xCC, 0x90, 0xC3) or (self.mem[o - 3] == 0xC2 and self.mem[o - 1] == 0):
                if self.mem[o:o + 3] in (b"\x6a\xff\x68", b"\x55\x8b\xec") or self.mem[o:o + 6] == b"\x64\xa1\x00\x00\x00\x00":
                    return o + self.base
            o -= 1
        return 0

    def ebx_is_zero(self, k):
        """The nearest instruction before k that writes ebx is `xor ebx, ebx` (ebx is callee-saved)."""
        j = k - 1
        while j > max(0, k - 30000):
            i = self.ins[j]
            if i.op_str.startswith("ebx,") and i.mnemonic not in ("cmp", "test"):
                return i.mnemonic == "xor" and i.op_str == "ebx, ebx"
            if i.mnemonic == "pop" and i.op_str == "ebx":
                return False
            j -= 1
        return False


def imm(op):
    try:
        return int(op, 0)
    except ValueError:
        return None


def registrations(img: Image, cfg, groups=None):
    sites = img.calls_to(cfg["ctor"])
    starts = sorted(groups) if groups else []
    out = []
    for n, k in enumerate(sites):
        pushes = img.pushes_before(k, 7)
        args, unresolved = [], []
        for p in pushes:
            v = imm(p)
            if v is None and p == "ebx" and img.ebx_is_zero(k):
                v = 0
            if v is None:
                unresolved.append(p)
            args.append(v)
        while len(args) < 7:
            args.append(None)
        group = None
        if groups:
            below = [s for s in starts if s <= img.ins[k].address]
            group = groups[below[-1]] if below else "other"
        end = sites[n + 1] if n + 1 < len(sites) else k + 200
        labels = {}
        for m in range(k + 1, min(end, k + 200)):
            i = img.ins[m]
            if i.mnemonic != "call":
                continue
            key = {hex(cfg["set_short"]): "short_label", hex(cfg["set_long"]): "label",
                   hex(cfg["set_help"]): "help"}.get(i.op_str)
            if key:
                q = m - 1
                while q > k and img.ins[q].mnemonic != "push":
                    q -= 1
                labels.setdefault(key, img.cstr(imm(img.ins[q].op_str)))
        out.append(dict(ordinal=n, site=img.ins[k].address, group=group, name=img.cstr(args[0]),
                        count=args[1], default=args[2], flag=args[3], conv=args[4],
                        unresolved=unresolved, **labels))
    return out


STACK, HEAP, OUT, STOP, STUB, EMPTY = 0x10000000, 0x11000000, 0x12000000, 0x13000000, 0x14000000, 0x15000000


class Emulator:
    def __init__(self, img: Image, cfg):
        self.img, self.cfg = img, cfg
        uc = self.uc = Uc(UC_ARCH_X86, UC_MODE_32)
        uc.mem_map(0, 0x1000)  # fs:[0] exception-frame chain (FS base 0)
        uc.mem_map(img.base, img.top - img.base)
        uc.mem_write(img.base, img.mem)
        for a in (STACK - 0x100000, OUT, STOP, STUB, EMPTY):
            uc.mem_map(a, 0x100000)
        uc.mem_map(HEAP, 0x1000000)
        uc.mem_write(EMPTY, b"\0" * 16)
        # Cent-from-double exactly as the document writer does it.
        code = bytes.fromhex("dd05" + struct.pack("<I", OUT + 0x18).hex()      # fld qword [value]
                             + "dc0d" + struct.pack("<I", cfg["c100"]).hex()    # fmul qword [100.0]
                             + "83ec08"                                         # sub esp, 8
                             + "dc05" + struct.pack("<I", cfg["c05"]).hex()     # fadd qword [0.5]
                             + "dd1c24")                                        # fstp qword [esp]
        code += b"\xe8" + struct.pack("<i", cfg["floor"] - (STUB + len(code) + 5))
        code += b"\x83\xc4\x08"
        code += b"\xe8" + struct.pack("<i", cfg["ftol"] - (STUB + len(code) + 5))
        code += b"\xe9" + struct.pack("<i", STOP - (STUB + len(code) + 5))
        uc.mem_write(STUB, code)
        self.heap = HEAP
        self.hooks = {cfg["malloc"]: self._malloc, cfg["free"]: lambda p: 0}
        uc.hook_add(UC_HOOK_CODE, self._code)
        self.reads, self.watch = set(), None
        uc.hook_add(UC_HOOK_MEM_READ, self._read)

    def _read(self, uc, access, address, size, value, _):
        # Globals read by the conversion routine itself (not by the CRT or string class).
        if self.watch and address >= self.img.bss and address < self.img.top:
            pc = uc.reg_read(UC_X86_REG_EIP)
            if self.watch[0] <= pc < self.watch[1]:
                self.reads.add(address)

    def _code(self, uc, address, size, _):
        h = self.hooks.get(address)
        if h is None:
            return
        esp = uc.reg_read(UC_X86_REG_ESP)
        ret, arg0 = struct.unpack("<II", uc.mem_read(esp, 8))
        uc.reg_write(UC_X86_REG_EAX, h(arg0))
        uc.reg_write(UC_X86_REG_ESP, esp + 4)  # cdecl: the caller pops
        uc.reg_write(UC_X86_REG_EIP, ret)

    def _malloc(self, size):
        p = self.heap
        self.heap += (size + 15) & ~15
        if self.heap > HEAP + 0xF00000:
            self.heap = HEAP
        return p

    def run(self, fn, args, ecx=0, limit=500000):
        esp = STACK - 0x2000
        self.uc.mem_write(esp, struct.pack("<I", STOP) + b"".join(struct.pack("<I", a & 0xFFFFFFFF) for a in args))
        self.uc.reg_write(UC_X86_REG_ESP, esp)
        self.uc.reg_write(UC_X86_REG_EBP, esp + 0x1000)
        self.uc.reg_write(UC_X86_REG_ECX, ecx)
        self.uc.emu_start(fn, STOP, count=limit)
        if self.uc.reg_read(UC_X86_REG_EIP) != STOP:
            raise RuntimeError("did not return")

    def convert(self, conv, index):
        """(kind, raw, wire) for one index. raw is comparable across images."""
        self.uc.mem_write(OUT, b"\0" * 0x60)
        self.run(self.cfg["str_ctor"], [EMPTY], ecx=OUT + 4)
        self.uc.mem_write(OUT, struct.pack("<I", 0xFFFF))
        self.watch = (conv, conv + 0x600)
        try:
            self.run(conv, [index, OUT])
        finally:
            self.watch = None
        kind = struct.unpack("<I", self.uc.mem_read(OUT, 4))[0]
        if kind in (1, 5, 3):
            v = struct.unpack("<i", self.uc.mem_read(OUT + 0x18, 4))[0]
            return kind, v, v
        if kind == 2:
            raw = bytes(self.uc.mem_read(OUT + 0x18, 8)).hex()
            self.uc.reg_write(UC_X86_REG_ESP, STACK - 0x2000)
            self.uc.emu_start(STUB, STOP, count=100000)
            v = self.uc.reg_read(UC_X86_REG_EAX)
            v = v - (1 << 32) if v & 0x80000000 else v
            return kind, raw, v
        if kind in (4, 6):
            ptr, length = struct.unpack("<I", self.uc.mem_read(OUT + 8, 4))[0], struct.unpack("<I", self.uc.mem_read(OUT + 0x10, 4))[0]
            if length > 400:
                raise RuntimeError("bad string")
            s = bytes(self.uc.mem_read(ptr, length)).decode("latin1")
            return kind, s, s
        raise RuntimeError("kind %d" % kind)


def emulate(img, cfg, regs):
    emu = Emulator(img, cfg)
    for r in regs:
        r["table"], r["error"], r["runtime_reads"] = None, None, []
        if r["unresolved"] or None in (r["name"], r["count"], r["conv"]):
            r["error"] = "constructor arguments in registers (%s)" % ",".join(r["unresolved"])
            continue
        emu.reads, table = set(), []
        try:
            for i in range(r["count"]):
                table.append(emu.convert(r["conv"], i))
        except (UcError, RuntimeError) as e:
            r["error"] = "conversion did not complete at index %d (%s)" % (len(table), e)
            continue
        r["table"] = table
        r["runtime_reads"] = sorted(hex(x) for x in emu.reads)
    return regs


WIRE = {1: "Int", 2: "Cent", 5: "Cent", 4: "String", 6: "UserString", 3: "FixPoint"}
PROFILE_TYPE = {"Int": "Int", "Cent": "Cent", "String": "Choice", "UserString": "Text"}
RECORD = re.compile(r"^C:<([^>]*)>(Int|Cent|String|UserString|FixPoint):(<[^>]*>|[^;]*);D:(\d+);\s*$")


def read_documents(roots):
    docs = []
    for root in roots:
        paths = [root] if root.is_file() else sorted(p for p in root.rglob("*") if p.is_file())
        for p in paths:
            raw = p.read_bytes()
            if not raw.startswith(b"OptimodVersion="):
                continue
            text = raw.decode("latin1")
            m = re.search(r"^Preset Type= ?<([^>]*)>", text, re.M)
            recs = []
            for line in text.splitlines():
                r = RECORD.match(line)
                if r:
                    name, kind, value, index = r.groups()
                    recs.append((name, kind, value[1:-1] if value.startswith("<") else value, int(index)))
            docs.append(dict(path=str(p), sha256=hashlib.sha256(raw).hexdigest(), version=text.split("\n", 1)[0].strip(),
                             type=m.group(1) if m else "", records=recs))
    return docs


DOC_SCOPE = {"FACTORY PRESET": "Processing", "LESS MORE PRESET": "Processing", "USER PRESET": "Processing",
             "ON AIR PRESET": "Processing", "NEXT PRESET": "Processing", "SYSTEM PRESET": "System",
             "TIME PRESET": "time", "TEST PRESET": "test"}
# Documents that decide the evidence: Orban's factory presets (both packages) and the
# documents a 3.0.5 unit wrote itself. User presets can come from older firmware
# (YORIN FM 2003 *.orb: AGC DRIVE Cent:1100 at D:22, a 1.x table); their mismatches are
# reported, not used to exclude.
PRIMARY = {"FACTORY PRESET", "LESS MORE PRESET", "ON AIR PRESET", "NEXT PRESET", "SYSTEM PRESET",
           "TIME PRESET", "TEST PRESET"}


def wire_value(entry):
    kind, _, v = entry
    return WIRE[kind], str(v)


# --------------------------------------------------------------------------- units (worksheet)
# Units per parameter from the 8400 V3.0 Setup/Parameters Worksheet (8400_V305_Worksheet.pdf)
# matched by its row label. Two worksheet units contradict the value tables and the
# screen and are corrected (listed in report.json): PEQ HIGH FREQ (values 1.00..15.00,
# worksheet "Hz") is kHz; the PEQ widths (4.0..0.8, worksheet "dB") are octaves.
UNITS = {
    "AI CLIP LEVEL": ("Clip Level", "dBu"), "AI BALANCE": ("Right Channel Balance", "dB"),
    "DI BALANCE": ("Right Channel Balance", "dB"),
    "AI REF LEVEL": ("Ref. Level VU/PPM", "dBu"), "AI REF PPM LEVEL": ("Ref. Level VU/PPM", "dBu"),
    "DI REF LEVEL": ("Ref. Level VU/PPM", "dBFS"), "DI REF PPM LEVEL": ("Ref. Level VU/PPM", "dBFS"),
    "ANALOG OUT": ("Out Level", "dBu"), "DO LEVEL": ("Out Level", "dBFS"),
    "DO SAMPLE RATE": ("Sample Rate", ""), "PILOT LEVEL": ("Pilot Level", "%"),
    "MOD REDUCE 1": ("Mod. Red. 1", "%"), "MOD REDUCE 2": ("Mod. Red. 2", "%"),
    "COMP1 OUT": ("Level", "dBu"), "COMP2 OUT": ("Level", "dBu"),
    "ITU412 THR": ("Multiplex Power Threshold", "dB"),
    "IBOC AO LEVEL": ("Out Level", "dBu"), "IBOC DO LEVEL": ("Out Level", "dBFS"),
    "AGC DRIVE": ("Drive", "dB"), "AGC RELEASE": ("Release", "dB/s"), "GATE THRESH": ("Gate Threshold", "dB"),
    "AGC BASS COUPLING": ("Bass Coupling", "%"), "AGC DIFF GR": ("MaxDelta GR", "dB"),
    "AGC WINDOW THRESH": ("Window Size", "dB"), "AGC WINDOW RELEASE": ("Window Release", "dB/s"),
    "AGC BASS TH": ("Bass Thresh", "dB"),
    "AGC BASS ATTACK": ("Bass Attack", "s"), "AGC BASS RELEASE": ("Bass Release", "s"),
    "AGC MASTER ATTACK": ("Mast. Attack", "s"), "IDLE GR": ("Idle Gain", "dB"),
    "AGC MAST DELTA TH": ("MstrDeltaThre", "dB"), "AGC BASS DELTA TH": ("BassDeltaThr", "dB"),
    "SE RATIO WIDTH": ("Ratio Limit", "%"),
    "LOW BASS GAIN": ("Bass Gain", "dB"), "LOW BASS FREQ": ("Bass Freq", "Hz"), "LOW BASS Q": ("Bass Slope", "dB/oct"),
    "PEQ LOW GAIN": ("Low Gain", "dB"), "PEQ LOW FREQ": ("Low Freq", "Hz"), "PEQ LOW WIDTH": ("Low Width", "oct"),
    "PEQ MID GAIN": ("Mid Gain", "dB"), "PEQ MID FREQ": ("Mid Freq", "Hz"), "PEQ MID WIDTH": ("Mid Width", "oct"),
    "PEQ HIGH GAIN": ("High Gain", "dB"), "PEQ HIGH FREQ": ("High Freq", "kHz"), "PEQ HIGH WIDTH": ("High Width", "oct"),
    "IBOC EQ GAIN": ("HD EQ gain", "dB"), "IBOC EQ FREQ": ("HD EQ freq", "kHz"),
    "MB DRIVE": ("MB Drive", "dB"), "DWNWRD EXP": ("Down Expand", "dB"), "MB GATE THR": ("Gate Threshold", "dB"),
    "B1 COMP THRSH": ("B1 Compression Threshold", "dB"), "B2 COMP THRSH": ("B2 Compression Threshold", "dB"),
    "B3 COMP THRSH": ("B3 Compression Threshold", "dB"), "B4 COMP THRSH": ("B4 Compression Threshold", "dB"),
    "MULTIBAND CLIP": ("MB Clipping", "dB"), "B3 CLIP THRSH": ("MB Limit Threshold", "dB"),
    "MAX DIST CONTRL": ("Max Dist Ctrl", "dB"), "SPEECH THR": ("Speech Thr", "dB"),
    "B5 CLIP THRSH": ("HF Clip Thr", "dB"), "HF LIMITER": ("HF Limiter", "dB"),
    "B1 ATTACK": ("B1 Attack", "k"), "B2 ATTACK": ("B2 Attack", "k"), "B3 ATTACK": ("B3 Attack", "k"),
    "B4 ATTACK": ("B4 Attack", "k"),
    "B1 LIMIT ATTACK": ("B1 Lim Att", "%"), "B2 LIMIT ATTACK": ("B2 Lim Att", "%"),
    "B3 LIMIT ATTACK": ("B3 Lim Att", "%"), "B4 LIMIT ATTACK": ("B4 Lim Att", "%"),
    "B1 OUTPUT MIX": ("B1 Out Mix", "dB"), "B2 OUTPUT MIX": ("B2 Out Mix", "dB"), "B3 OUTPUT MIX": ("B3 Out Mix", "dB"),
    "B4 OUTPUT MIX": ("B4 Out Mix", "dB"), "B5 OUTPUT MIX": ("B5 Out Mix", "dB"),
    "B1 DIFF GR": ("B1MaxDeltGR", "dB"), "B2 DIFF GR": ("B2MaxDeltGR", "dB"), "B3 DIFF GR": ("B3MaxDeltGR", "dB"),
    "B4 DIFF GR": ("B4MaxDeltGR", "dB"), "B5 DIFF GR": ("B5MaxDeltGR", "dB"),
    "BASS COUPLING": ("B2/B1 Coupl", "%"), "BAND 23 COUPL": ("B2/B3 Coupl", "%"), "BAND 32 COUPL": ("B3/B2 Coupl", "%"),
    "BAND 34 COUPL": ("B3/B4 Coupl", "%"), "BAND 45 COUPL": ("B4/B5 Coupl", "%"),
    "2B DRIVE": ("Drive", "dB"), "2B RELEASE": ("Release", "dB/s"), "2B BASS COUPLE": ("Bass Coupling", "%"),
    "2B LOOKAHEAD": ("Lookahead", "ms"), "2B BASS COMP THR": ("BassCmp Thr", "dB"),
    "2B MASTER COMP THR": ("MastCmp Thr", "dB"), "2B CLIPPING": ("2B Clipping", "dB"),
    "2B BASS ATTACK": ("Bass Attack", "k"), "2B MASTR ATTACK": ("Master Attack", "k"),
    "BASS CLIP": ("Bass Clip Threshold", "dB"), "2B BASS CLIP": ("Bass Clip Threshold", "dB"),
    "SPEECH BC THR": ("SpeechBCThr", "dB"), "2B HF LIMITING": ("HF Limiting", "dB"),
    "FINAL CLIP DRV": ("Final Clip Drive", "dB"), "DRIVE W2": ("OSComp Dr", "dB"), "COMP DRIVE": ("Composite Drive", "dB"),
    "IBOC LIM DR": ("HD Limiter Drive", "dB"),
}


def worksheet_rows(path: Path):
    import subprocess
    try:
        text = subprocess.run(["pdftotext", "-layout", str(path), "-"], capture_output=True, text=True, check=True).stdout
    except (OSError, subprocess.CalledProcessError):
        return None
    rows = collections.defaultdict(set)
    for line in text.splitlines():
        m = re.match(r"^\s{0,4}(\S.*?)\s{2,}(\S.*?)(?:\s{2,}(\S+))?\s*$", line)
        if m:
            label = re.sub(r"\s*\((?:5|2) Band(?: only|Only)?\)|\s*\(HD only\)", "", m.group(1), flags=re.I).strip()
            rows[label].add((m.group(2).strip(), (m.group(3) or "").strip()))
    return rows


# --------------------------------------------------------------------------- structure
# From the worksheet's "(5 Band only)" / "(2 Band only)" markings (sections Multiband,
# MB Attack/Rel, Band Mix, Two Band; rows MstrDeltaThre, BassDeltaThr, Brilliance,
# DJ Bass, HardClipShape, Crossover, HF Limiting). Every processing parameter is stored
# in every preset regardless of the structure (2B/5B SWITCH selects it).
FIVE_BAND_ONLY = {
    "AGC MAST DELTA TH", "AGC BASS DELTA TH", "BRILLIANCE", "DJ BASS BOOST", "MB DRIVE", "DWNWRD EXP",
    "B1 COMP THRSH", "B2 COMP THRSH", "B3 COMP THRSH", "B4 COMP THRSH", "MULTIBAND CLIP", "HF LIMITER",
    "DE STEREO COUPL", "B12 CROSSOVER", "MB RELEASE", "B1 ATTACK", "B2 ATTACK", "B3 ATTACK", "B4 ATTACK",
    "B1 LIMIT ATTACK", "B2 LIMIT ATTACK", "B3 LIMIT ATTACK", "B4 LIMIT ATTACK", "COMP LOOKAHEAD SW",
    "B1 DELTA REL", "B2 DELTA REL", "B3 DELTA REL", "B4/5 DELTA REL", "B1 OUTPUT MIX", "B2 OUTPUT MIX",
    "B3 OUTPUT MIX", "B4 OUTPUT MIX", "B5 OUTPUT MIX", "B1 ON/OFF", "B2 ON/OFF", "B3 ON/OFF", "B4 ON/OFF",
    "B5 ON/OFF", "B1 DIFF GR", "B2 DIFF GR", "B3 DIFF GR", "B4 DIFF GR", "B5 DIFF GR", "BASS COUPLING",
    "BAND 23 COUPL", "BAND 32 COUPL", "BAND 34 COUPL", "BAND 45 COUPL", "BASS CLIP SHAPE",
}
TWO_BAND_ONLY = {"AGC XOVER", "2B XOVER", "2B DRIVE", "2B RELEASE", "2B BASS COUPLE", "2B BASS COMP THR",
                 "2B MASTER COMP THR", "2B BASS CLIP", "2B BASS CLIP MODE", "2B HF LIMITING", "2B CLIPPING",
                 "2B BASS ATTACK", "2B MASTR ATTACK", "2B LOOKAHEAD"}


def preset_pairs(root: Path):
    """Which records differ between the 50 us and 75 us copies, and between X and X LL."""
    def rd(p):
        return {m.group(1): (m.group(2), int(m.group(3))) for m in
                re.finditer(r"^C:<([^>]*)>(.*);D:(\d+);", p.read_text("latin1"), re.M)}
    out = dict(pre_emphasis=collections.Counter(), pairs=0, low_latency={})
    a50, a75 = root / "50us presets", root / "75us presets"
    for f in sorted(a50.glob("*.orb")):
        g = a75 / f.name
        if g.exists():
            x, y = rd(f), rd(g)
            out["pairs"] += 1
            for k in set(x) | set(y):
                if x.get(k) != y.get(k):
                    out["pre_emphasis"][k] += 1
        if " LL" in f.stem and (a50 / (f.stem.replace(" LL", "") + ".orb")).exists():
            x, y = rd(a50 / (f.stem.replace(" LL", "") + ".orb")), rd(f)
            out["low_latency"][f.name] = {k: "%s -> %s" % (x.get(k), y.get(k)) for k in sorted(set(x) | set(y))
                                          if x.get(k) != y.get(k)}
    out["pre_emphasis"] = dict(sorted(out["pre_emphasis"].items(), key=lambda kv: (-kv[1], kv[0])))
    return out


# --------------------------------------------------------------------------- pages (PC Remote)
TITLE = re.compile(r"^(?:(?:Basic|Intermediate|Advanced) Modify \d+|Input/Output \d+|System Setup: .+)$")


def pages_from_client(img: Image, cfg, names_short):
    """Walk the PC Remote code in address order: a title string starts a page, every
    builder / list-row call adds a control to the current page."""
    events = []
    builder, row = hex(cfg["builder"]), hex(cfg["row"])
    for k, i in enumerate(img.ins):
        if i.mnemonic == "push" and i.op_str.startswith("0x"):
            s = img.cstr(imm(i.op_str))
            if s and TITLE.match(s):
                events.append(("title", i.address, s))
        elif i.mnemonic == "call" and i.op_str in (builder, row):
            p = img.pushes_before(k, 7 if i.op_str == builder else 4, window=24)
            if i.op_str == builder:
                name, label = img.cstr(imm(p[0])), img.cstr(imm(p[1])) if len(p) > 1 else None
                xy = [imm(v) for v in p[2:5]]
                if name:
                    events.append(("control", i.address, dict(name=name, label=label, x=xy[0], y=xy[1], style=xy[2])))
            else:
                name = img.cstr(imm(p[1])) if len(p) > 1 else None
                if name:
                    events.append(("row", i.address, dict(name=name, label=names_short.get(name) or name)))
    pages, current = [], None
    for kind, addr, v in events:
        fn = img.function_start(addr)
        if kind == "title":
            current = dict(title=v, site=addr, fn=fn, controls=[])
            pages.append(current)
            continue
        if current is None or current["fn"] != fn:
            # Controls of a screen function that pushes no page title.
            current = dict(title=None, site=fn, fn=fn, controls=[])
            pages.append(current)
        if kind == "row":
            v = dict(v, x=None, y=None, style=0x22, row=True)
        current["controls"].append(dict(v, site=addr))
    return [p for p in pages if p["controls"]]


OPEN_QUESTIONS = [
    "orban-protocol has no 8400 DeviceModel: 8400 documents start 'OptimodVersion=<00.08>' and files end "
    "'End Preset\\r\\n' (no '<end>'); the onair.orb read from a unit (scratchpad p84/onair0.orb) has LF line "
    "ends and 0xFF padding. Document::parse needs a prefix/terminator for the 8400 before readback works. "
    "All 974 documents parse with document.rs parse_lines rules (types Int, Cent, String, UserString only).",
    "'30 HZ HPF' is registered twice: processing (onair.orb/presets) and system (system.orb, Test Modes page). "
    "Message 48 'NAME;INDEX;TYPE:VALUE;' carries no scope: which one the unit changes is unproven.",
    "reference_id is the ordinal of the firmware constructor call (0..273), not a protocol identifier: "
    "message 48 addresses parameters by NAME.",
    "Writable but never stored in a document (evidence 'unobserved'): processing FC HEADROOM, CLIP DENS THR, "
    "CLIP W1, DRIVE W1, DRIVE W3, MIX W3 (registered, absent from every preset); system 400 HERTZ TONE, OPERATE, "
    "BYPASS, TONE, ACTUAL A OR D, MOD REDUC SW1/SW2, OPERATE MODE, TEST MODE, MUTE, MONITOR MUTE (transient, not "
    "in system.orb). A write to these cannot be confirmed by reading the document back. ACTUAL A OR D looks like a "
    "status; MONITOR MUTE has the same text at both indices (an action).",
    "VU PPM and LANGUAGE are registered with value_count 1 ('VU', 'English') in both firmware and PC Remote, "
    "although the VU PPM conversion also returns 'PPM' for index 1; the profile keeps the registered count.",
    "FREQUENCY (test tone) index 31 is the string '13.58676' verbatim from the firmware table 0x384a84 "
    "(index 32 = '15kHz'; '16kHz' at 33 is beyond value_count 33).",
    "PEQ HIGH FREQ indices 0 and 1 both format as Cent:100 (firmware table); readback by D:index tells them apart.",
    "COMP DRIVE: IMPACT LM100.orb (75 us, in both packages) holds Cent:1000 at D:18 where the table has Cent:170; "
    "941 other records agree. Kept with evidence 'conflict'.",
    "AGC DRIVE: the YORIN FM 2003 user presets hold Cent:1100 at D:22 (3.0.5 table: Cent:1200); they come from "
    "older firmware. Factory presets and unit documents agree with the 3.0.5 table.",
    "LO/HI LATEN SW appears in the MINIMUM-DELAY factory presets (String:<High>;D:1) but has no registration in "
    "3.0.5 firmware or PC Remote; it is not writable.",
    "TOTAL MOD LEVEL (system.orb Cent:10000;D:0) is computed from MOD LEVEL and TEST PILOT at run time "
    "(conversion 0x1481e0 reads other parameters); excluded.",
    "Text parameters (UserString: MY STATION, NETWORK IP/SUBNET/GATEWAY, MODEM INIT STRING, REMOTE CONTACT 1-8, "
    "passcode) and time.orb/test.orb/automation parameters are excluded from parameters.json; tables of the "
    "latter are in other_documents_tables.",
]


# --------------------------------------------------------------------------- main
def main():
    ap = argparse.ArgumentParser()
    for k, v in DEFAULTS.items():
        ap.add_argument("--" + k.replace("_", "-"), type=Path, default=v)
    a = ap.parse_args()
    a.out.mkdir(parents=True, exist_ok=True)

    fw = Image(a.image, flat=True)
    pc = Image(a.client, flat=False)
    fw_regs = emulate(fw, FW, registrations(fw, FW, FW_GROUPS))
    pc_regs = emulate(pc, PC, registrations(pc, PC))

    # ---- PC Remote agreement: same name -> identical raw tables (per occurrence).
    pc_by_name = collections.defaultdict(list)
    for r in pc_regs:
        if r["name"]:
            pc_by_name[r["name"]].append(r)
    for r in fw_regs:
        cands = pc_by_name.get(r["name"], [])
        if not r["table"]:
            r["client"] = "n/a"
        elif not cands:
            r["client"] = "absent"
        elif any(c["table"] == r["table"] and c["count"] == r["count"] and c["default"] == r["default"] for c in cands):
            r["client"] = "identical"
        else:
            r["client"] = "differs"

    # ---- documents
    roots = [a.fw_presets, a.fw_user, a.client_presets, a.unit_user, a.onair_live]
    docs = read_documents(roots)
    observed = collections.defaultdict(list)  # (scope, name) -> [(kind, value, index, path)] primary
    secondary = collections.defaultdict(list)  # the same from user presets
    for doc in docs:
        scope = DOC_SCOPE.get(doc["type"], "?" + doc["type"])
        target = observed if doc["type"] in PRIMARY else secondary
        for name, kind, value, index in doc["records"]:
            target[(scope, name)].append((kind, value, index, doc["path"]))

    # ---- fields
    fields, excluded, evidence_count, notes = [], [], collections.Counter(), []
    other_tables = []
    user_mismatch = collections.defaultdict(list)
    conflicts = {}
    unit_shared = {}
    seen = set()
    for r in fw_regs:
        scope = SCOPE.get(r["group"])
        name = r["name"]
        if scope is None:
            reason = "belongs to the %s registration group (%s), not to onair.orb or system.orb" % (
                r["group"], {"time": "time.orb", "test": "test.orb"}.get(r["group"], "not a stored document"))
            excluded.append(dict(name=name, group=r["group"], reason=reason))
            if r["table"]:
                other_tables.append(dict(group=r["group"], name=name,
                                         values=[dict(type=WIRE[k], value=v) for k, _, v in r["table"]]))
            continue
        if r["error"]:
            excluded.append(dict(name=name, scope=scope, reason=r["error"]))
            continue
        if r["runtime_reads"]:
            excluded.append(dict(name=name, scope=scope,
                                 reason="conversion reads runtime state %s (text taken from the unit, not an index table)" % ",".join(r["runtime_reads"])))
            continue
        if any(k == 6 for k, _, _ in r["table"]):
            excluded.append(dict(name=name, scope=scope, reason="UserString (free text, written as text, not an index table)"))
            continue
        if r["client"] == "differs":
            excluded.append(dict(name=name, scope=scope, reason="PC Remote conversion table differs from the firmware"))
            continue
        if (scope, name) in seen:
            excluded.append(dict(name=name, scope=scope, reason="duplicate registration"))
            continue
        seen.add((scope, name))
        wire = [wire_value(e) for e in r["table"]]
        recs = observed.get((scope, name), [])
        bad = [(k, v, i, p) for k, v, i, p in recs if i >= len(wire) or wire[i] != (k, v)]
        if bad:
            # The table is proven by code (and identical in the PC Remote); a factory
            # record that disagrees is kept as evidence "conflict" and listed.
            conflicts["%s/%s" % (scope, name)] = dict(count=len(bad), of=len(recs), records=sorted({
                "%s: %s:%s at D:%d (table: %s)" % (Path(p).name, k, v, i, ("%s:%s" % wire[i]) if i < len(wire) else "no such index")
                for k, v, i, p in bad}))
        evidence = "conflict" if bad else "confirmed" if recs else "unobserved"
        for k, v, i, p in secondary.get((scope, name), []):
            if i >= len(wire) or wire[i] != (k, v):
                user_mismatch[(scope, name)].append("%s: %s:%s at D:%d (table: %s)" % (
                    Path(p).name, k, v, i, ("%s:%s" % wire[i]) if i < len(wire) else "no such index"))
        evidence_count[evidence] += 1
        unit = UNITS.get(name, ("", ""))[1]
        if not unit:
            same = sorted({UNITS[o["name"]][1] for o in fw_regs if o["conv"] == r["conv"] and o["name"] in UNITS})
            if len(same) == 1 and same[0]:
                unit = same[0]
                unit_shared[name] = {"unit": unit, "conversion": hex(r["conv"]),
                                     "from": [o["name"] for o in fw_regs if o["conv"] == r["conv"] and o["name"] in UNITS]}
        values = [dict(type=PROFILE_TYPE[t], value=(int(v) if t in ("Int", "Cent") else v)) for t, v in wire]
        f = dict(scope=scope, reference_id=r["ordinal"], name=name, evidence=evidence, values=values)
        if unit:
            f["unit"] = unit
        fields.append(dict(f, _reg=r, _records=len(recs), _indices=sorted({i for _, _, i, _ in recs})))
    fields.sort(key=lambda f: (f["scope"], f["reference_id"]))

    # ---- names stored in a document but without a provable table
    proven = {(f["scope"], f["name"]) for f in fields}
    stored_without = sorted({"%s/%s" % k for k in list(observed) + list(secondary)
                             if k[0] in ("Processing", "System") and k not in proven})

    # ---- pages
    short = {}
    for r in pc_regs:
        if r["name"] and r["name"] not in short:
            short[r["name"]] = r.get("short_label") or r["name"]
    raw_pages = pages_from_client(pc, PC, short)
    field_by_name = collections.defaultdict(dict)
    for f in fields:
        field_by_name[f["name"]][f["scope"]] = f
    layout, page_report, seen_titles = [], [], collections.Counter()
    for n, p in enumerate(raw_pages):
        if p["title"] is None:
            page_report.append(dict(title=None, code=hex(p["site"]), controls=[c["name"] for c in p["controls"]],
                                    dropped="all (screen function pushes no page title)"))
            continue
        system_page = p["title"].startswith(("Input/Output", "System Setup"))
        want = "System" if system_page else "Processing"
        controls, dropped = [], []
        for c in p["controls"]:
            f = field_by_name.get(c["name"], {}).get(want)
            if f is None and c["name"] == "TEST PILOT":
                f = None
            if f is None:
                dropped.append(c["name"])
                continue
            opts = [v["value"] for v in f["values"]] if all(v["type"] == "Choice" for v in f["values"]) else []
            kind = "radio" if opts and len(opts) <= 4 else "slider"
            label = (c["label"] or "").strip() or short.get(c["name"], c["name"])
            if any(x["name"] == c["name"] for x in controls):
                continue
            controls.append(dict(kind=kind, name=c["name"], label=label,
                                 x=c["x"] if c["x"] is not None else 0,
                                 y=c["y"] if c["y"] is not None else 20 * len(controls),
                                 w=60, id=c["site"], reference_id=f["reference_id"], options=opts if kind == "radio" else []))
        has5 = any(c["name"] in FIVE_BAND_ONLY for c in controls)
        has2 = any(c["name"] in TWO_BAND_ONLY or c["name"].startswith("2B ") for c in controls)
        variant = "Two Band" if has2 and not has5 else ("Five Band" if has5 and not has2 else None)
        page_report.append(dict(title=p["title"], code=hex(p["site"]), variant=variant, controls=[c["name"] for c in p["controls"]],
                                dropped=dropped))
        if not controls:
            continue
        sig = tuple(c["name"] for c in controls)
        if any(tuple(c["name"] for c in q["controls"]) == sig and q["_title"] == p["title"] for q in layout):
            continue  # an identical variant of the same page (only a soft key differs)
        title = p["title"]
        seen_titles[title] += 1
        layout.append(dict(_title=p["title"], _variant=variant, title=title,
                           path="Shared" if want == "Processing" else "System",
                           width=320, height=240, reference_dialog=p["site"], controls=controls, groups=[]))
    # Distinguish the structure variants of a title. Of two variants, the one without
    # any 5-band-only control next to a Five Band one is the Two Band page (and the
    # other way round); this pairing is inferred from the worksheet markings.
    by_title = collections.defaultdict(list)
    for q in layout:
        by_title[q["_title"]].append(q)
    for title, qs in by_title.items():
        if len(qs) == 2:
            known = {q["_variant"] for q in qs}
            for q in qs:
                if q["_variant"] is None and len(known - {None}) == 1:
                    q["_variant"] = ({"Five Band", "Two Band"} - known).pop()
                    q["_inferred"] = True
        if len(qs) > 1:
            for q in qs:
                q["title"] = "%s (%s)" % (title, q["_variant"] or "variant %x" % q["reference_dialog"])
    order = {"Basic": 0, "Intermediate": 1, "Advanced": 2, "Input/Output": 3, "System": 4}

    def key(q):
        m = re.match(r"^(Basic|Intermediate|Advanced|Input/Output|System)\D*?(\d+)?(?:\s|$)", q["_title"])
        return (order[m.group(1)], int(m.group(2)) if m.group(2) else 0, q["_title"],
                {"Five Band": 0, None: 1, "Two Band": 2}[q["_variant"]])
    layout.sort(key=key)
    inferred = [q["title"] for q in layout if q.get("_inferred")]
    # The app shows the Advanced Modify pages only (they hold every processing control
    # but LESS MORE), with Less-More as its own first page as on the other models. A
    # Two Band or Five Band page is shown only while that structure is on air.
    structure = {"Five Band": "5band", "Two Band": "2band"}
    shown = []
    for q in layout:
        if q["path"] == "Shared" and not q["_title"].startswith("Advanced"):
            continue
        if q["path"] == "Shared" and q["_variant"] in structure and q["title"] != q["_title"]:
            q["title"] = q["_title"]
            q["when"] = {"field": "2B/5B SWITCH", "value": structure[q["_variant"]]}
        shown.append(q)
    less_more = next(f for f in fields if f["name"] == "LESS MORE" and f["scope"] == "Processing")
    shown.insert(0, dict(title="Less More", path="Shared", width=320, height=240, reference_dialog=0,
                         controls=[dict(kind="slider", name="LESS MORE", label="Less-More", x=0, y=0, w=60, id=0,
                                        reference_id=less_more["reference_id"], options=[])],
                         groups=[]))
    layout = shown
    for q in layout:
        for k in ("_title", "_variant", "_inferred"):
            q.pop(k, None)

    # ---- worksheet check of units / ranges (report only)
    ws = worksheet_rows(a.worksheet)
    ws_check = []
    if ws is not None:
        for name, (label, unit) in sorted(UNITS.items()):
            rows = ws.get(label)
            ws_check.append(dict(name=name, worksheet_label=label, worksheet=sorted("%s [%s]" % r for r in rows) if rows else None,
                                 unit=unit))

    # ---- write
    status = ("Statically derived from the 8400 3.0.5 firmware (8400_ets.exe) parameter conversion routines, "
              "identical in the original PC Remote (8400_pc.exe), and cross-checked against the factory presets, "
              "user presets and unit documents. Not verified on hardware.")
    params = dict(model="8400", firmware="3.0.5.7", reference_sha256=sha256(a.client), firmware_sha256=sha256(a.ets),
                  status=status,
                  fields=[{k: v for k, v in f.items() if not k.startswith("_")} for f in fields])
    (a.out / "parameters.json").write_text(json.dumps(params, indent=2, ensure_ascii=False) + "\n")
    (a.out / "layouts.json").write_text(json.dumps(layout, indent=2, ensure_ascii=False) + "\n")

    families = collections.Counter(d["version"] for d in docs)
    types = collections.Counter(d["type"] for d in docs)
    report = dict(
        model="8400", firmware="3.0.5.7", banner_note="firmware package 'Optimod 8400 Update 3.0.5'; software version 3.0.5.7",
        reference_sha256=sha256(a.client), firmware_sha256=sha256(a.ets), image_sha256=sha256(a.image),
        document_family="OptimodVersion=<00.08>",
        document_family_note=("8400 documents start with OptimodVersion=<00.08>, not <8400.: "
                              "orban-protocol document.rs has no 8400 DeviceModel/prefix yet."),
        registrations=dict(firmware=len(fw_regs), pc_remote=len(pc_regs),
                           firmware_groups=dict(collections.Counter(r["group"] for r in fw_regs))),
        pc_remote_agreement=dict(collections.Counter(r["client"] for r in fw_regs)),
        pc_remote_only=sorted({r["name"] for r in pc_regs if r["name"]} - {r["name"] for r in fw_regs if r["name"]}),
        factory_presets=sum(1 for d in docs if d["type"] in ("FACTORY PRESET", "LESS MORE PRESET")),
        documents=dict(files=len(docs), families=dict(families), types=dict(types),
                       records=sum(len(d["records"]) for d in docs)),
        fields=len(fields), fields_by_scope=dict(collections.Counter(f["scope"] for f in fields)),
        field_evidence=dict(evidence_count),
        excluded=excluded,
        conflicts=conflicts,
        structure=dict(
            five_band_only=sorted(FIVE_BAND_ONLY), two_band_only=sorted(TWO_BAND_ONLY),
            note=("All processing parameters are stored in every preset and every structure; 2B/5B SWITCH "
                  "(String 5band/2band) selects the structure. The tables do not depend on structure, "
                  "pre-emphasis or low latency: no conversion reads runtime state. 'LL' (low-latency) presets "
                  "differ by values only, e.g. BASS CLIP MODE LLHard (index 3); 50/75 us factory presets differ "
                  "by values only (PRE-E itself is the system parameter)."),
            factory_preset_comparison=preset_pairs(a.fw_presets)),
        user_preset_mismatches={"%s/%s" % k: dict(count=len(v), first=v[:3]) for k, v in sorted(user_mismatch.items())},
        stored_without_table=stored_without,
        pages=[q["title"] + " (" + q["path"] + ")" for q in layout],
        page_sources=page_report,
        layout_notes=[
            "Pages, order, titles, labels and x/y come from the PC Remote's PEG screen code "
            "(builder 0x41a8a0 args name,label,x,y; Advanced Modify list rows 0x40a200 use the parameter's short label). "
            "x/y are PEG screen pixels of the builder call; list rows have no fixed position (y is the row order x 20). "
            "w is a nominal 60, id is the builder call address, reference_dialog the address of the page title push; "
            "groups are not modelled (empty). kind is 'radio' for a choice table of <= 4 entries, else 'slider'.",
            "Pages with the same title for the 2-band and 5-band structures are split as '(Two Band)'/'(Five Band)' "
            "when their controls differ; identical variants (only a soft key differs) are merged.",
            "System pages (Input/Output, System Setup) have path 'System'; the UI's processing view only shows 'Shared'.",
            "Order: Basic, Intermediate, Advanced Modify, Input/Output, System Setup, by page number; Five Band before Two Band.",
            "Structure of these pages inferred as the counterpart of a Five/Two Band page with the same title: %s" % ", ".join(inferred),
        ],
        units_note=("Units from the 8400 V3.0 Setup/Parameters Worksheet by row label. Corrected against the tables: "
                    "PEQ HIGH FREQ worksheet 'Hz' -> kHz (values 1.00..15.00); PEQ LOW/MID/HIGH WIDTH worksheet 'dB' -> oct; "
                    "LOW BASS Q worksheet 'dB/Oct' kept as dB/oct; '%'-unit Cent values are percent x100."),
        worksheet_units=ws_check,
        units_by_shared_conversion=unit_shared,
        other_documents_tables=other_tables,
        sources={
            "firmware_image": dict(file=a.image.name, sha256=sha256(a.image)),
            "firmware_exe": dict(file=a.ets.name, sha256=sha256(a.ets)),
            "pc_remote": dict(file=a.client.name, sha256=sha256(a.client)),
            "worksheet": dict(file=a.worksheet.name, sha256=sha256(a.worksheet)),
            "document_sets": [dict(set="/".join(r.parts[-2:]), files=sum(1 for d in docs if d["path"].startswith(str(r)))) for r in roots],
            "documents_sha256": hashlib.sha256("".join(sorted(d["sha256"] for d in docs)).encode()).hexdigest(),
        },
        open_questions=OPEN_QUESTIONS,
    )
    (a.out / "report.json").write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
    print("fields", len(fields), dict(evidence_count), "excluded", len(excluded), "pages", len(layout))


if __name__ == "__main__":
    main()
