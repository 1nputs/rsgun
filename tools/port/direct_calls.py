"""Lists the calls from one rewrite to another that do not go through `nested!` (crates/rsgun's
game/: a rewrite calls another as its SH-2 function calls it, so the callee makes its SH-2 prologue;
see game/mod.rs's Prologue). Each line: file:line, the callee and its SH-2 entry. A call the SH-2
function does not make (the same code inline in it) is listed too: mark it with a comment
`// inline` on the line before to leave it out. Exit status 1 if any are listed.

    python3 tools/port/direct_calls.py
"""
import pathlib
import re
import sys

GAME = pathlib.Path(__file__).resolve().parents[2] / "crates/rsgun/src/game"


def main() -> int:
    table = (GAME / "mod.rs").read_text()
    entries = {}
    for m in re.finditer(r"0x([0-9a-f]{4})_([0-9a-f]{4}) => (\w+)::(\w+)\[", table):
        entries.setdefault((m.group(3), m.group(4)), f"0x{m.group(1)}_{m.group(2)}")
    call = re.compile(r"(?<![\w:!.])((?:super::)?(?:(\w+)::)?(\w+))\(cpu\b")
    found = 0
    for path in sorted(GAME.glob("*.rs")):
        if path.name == "mod.rs":
            continue
        module = path.stem
        lines = path.read_text().split("\n")
        for i, line in enumerate(lines):
            if line.strip().startswith("//") or re.match(r"\s*(pub )?fn ", line):
                continue
            if i > 0 and lines[i - 1].strip().startswith("// inline"):
                continue
            for m in call.finditer(line):
                key = (m.group(2) or module, m.group(3))
                prefix = line[:m.start()]
                if key not in entries or re.search(r"nested!\(cpu,\s*[^,()]+,\s*$", prefix):
                    continue
                if prefix.rstrip().endswith("|cpu|"):
                    continue
                print(f"{path.name}:{i + 1}: {key[0]}::{key[1]} ({entries[key]})")
                found += 1
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
