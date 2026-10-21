#!/usr/bin/env python3
"""Find the extractor addresses of a new PC Remote build by matching known functions.

Usage:
    python3 scripts/match_pc_remote_functions.py KNOWN.exe NEW.exe ADDRESS[:NAME] ...

Each ADDRESS is a function of KNOWN.exe from its CONFIGS entry (constructor,
string helpers, binders, meter functions). The first instructions of that
function, with call targets and absolute addresses masked, are searched in
NEW.exe. A single hit is the same function in the new build; use two known
builds and accept only addresses both agree on.
"""
import re, sys, pefile
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
md = Cs(CS_ARCH_X86, CS_MODE_32); md.detail = True
def load(p):
    pe = pefile.PE(p); base = pe.OPTIONAL_HEADER.ImageBase
    t = [s for s in pe.sections if s.Name.startswith(b'.text')][0]
    return pe, base, base + t.VirtualAddress, t.get_data()
def pattern(code, start, nbytes=56):
    out = b''; n = 0
    for ins in md.disasm(code, start):
        b = bytes(ins.bytes); mask = bytearray(b)
        size = len(b)
        # mask rel32 targets and absolute 32-bit operands
        if ins.mnemonic in ('call', 'jmp') and size >= 5 and b[0] in (0xe8, 0xe9):
            mask[1:5] = b'\0\0\0\0'; wild = set(range(1, 5))
        else:
            wild = set()
            for i in range(size - 3):
                v = int.from_bytes(b[i:i+4], 'little')
                if 0x400000 <= v < 0x800000: wild |= set(range(i, i+4))
            if ins.mnemonic.startswith('j') and size >= 2 and ins.mnemonic != 'jmp': wild |= set(range(1, size)) if size > 2 else {1}
        for i, x in enumerate(b): out += b'.' if i in wild else re.escape(bytes([x]))
        n += size
        if n >= nbytes or ins.mnemonic == 'ret': break
    return out
ref = load(sys.argv[1]); tgt = load(sys.argv[2])
for arg in sys.argv[3:]:
    a, _, name = arg.partition(':'); a = int(a, 16)
    code = ref[3][a - ref[2]: a - ref[2] + 400]
    pat = pattern(code, a)
    hits = [tgt[2] + m.start() for m in re.finditer(pat, tgt[3], re.S)]
    print(f'{name or hex(a)} ref={a:#x} hits={[hex(h) for h in hits[:6]]}{" ..." if len(hits) > 6 else ""}')
