"""Lists the `cpu.call` sites in crates/rsgun's game/ whose target (a constant of the file's, or a
literal) is now a rewrite of the game's own code: such a call should be the rewrite's, through
`nested!`. SGL's code is left out (0x06030000-0x06040000, 0x06066000-0x06080000): its asm hands
registers on, and `cpu.call` loads the argument registers as the SH-2 caller does (HANDOFF
section 5). `cpu.call_returning` is left out too: it gives the callee the SH-2 call's return
address to push, for a stale read of it (HANDOFF section 3). Each line: file:line, the constant
and the rewrite. Exit status 1 if any are listed.

    python3 tools/port/called_rewrites.py
"""
import pathlib
import re
import sys

GAME = pathlib.Path(__file__).resolve().parents[2] / "crates/rsgun/src/game"
SGL = [(0x0603_0000, 0x0604_0000), (0x0606_6000, 0x0608_0000)]


def main() -> int:
    table = (GAME / "mod.rs").read_text()
    entries = {int(a + b, 16): f"{m}::{f}"
               for a, b, m, f in re.findall(r"0x([0-9a-f]{4})_([0-9a-f]{4}) => (\w+)::(\w+)", table)}
    found = 0
    for path in sorted(GAME.glob("*.rs")):
        if path.name == "mod.rs":
            continue
        text = path.read_text()
        consts = {k: int(v.replace("_", ""), 16) for k, v in re.findall(r"const (\w+): u32 = 0x([0-9a-f_]+);", text)}
        for i, line in enumerate(text.split("\n")):
            for m in re.finditer(r"cpu\.call\(\s*([A-Z_0-9]+|0x[0-9a-f_]+)", line):
                t = m.group(1)
                addr = int(t.replace("_", ""), 16) if t.startswith("0x") else consts.get(t)
                if addr not in entries or any(lo <= addr < hi for lo, hi in SGL):
                    continue
                print(f"{path.name}:{i + 1}: {t} ({entries[addr]})")
                found += 1
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
