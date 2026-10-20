"""Lists SH-2 calls that use the result of a rewrite registered as giving none (`-> ()` in
crates/rsgun's game/mod.rs): compare mode does not check r0 for those, so a caller that reads it
(the SH-2 function left something there) gets whatever the rewrite left. Every call through a
literal (`mov.l @(disp,PC),Rn` ... `jsr @Rn`) in the cart (the main program and every overlay) is
looked at: r0 read after the call's delay slot before anything writes it. Exit status 1 if any
are listed; each needs a look (a call site can be data that reads as code).

    python3 tools/port/void_results.py [CART]   # default: build/rom/rsgun-cart.bin
"""
import pathlib
import re
import sys

import capstone as cs

ROOT = pathlib.Path(__file__).resolve().parents[2]
REG = re.compile(r"^\s*0x([0-9a-f]{4})_([0-9a-f]{4}) => (\w+::\w+)\[[^\]]*\] -> \(\),", re.M)
MOVES = ("mov", "mov.b", "mov.w", "mov.l", "mova", "sts", "stc", "movt", "swap.b", "swap.w", "extu.b",
         "extu.w", "exts.b", "exts.w", "neg", "not")


def writes_r0_only(t):
    """Whether the instruction writes r0 without reading it (a move into r0 from elsewhere)."""
    op, _, args = t.partition(" ")
    src, _, dst = args.rpartition(",")
    return op in MOVES and (dst or src) == "r0" and not re.search(r"\br0\b", src if dst else "")


STOPS = ("bt", "bf", "bra", "braf", "bsr", "bsrf", "jmp", "jsr", "rts", "rte")


def voids():
    text = (ROOT / "crates/rsgun/src/game/mod.rs").read_text()
    return {int(a + b, 16): name for a, b, name in REG.findall(text)}


def main() -> int:
    cart = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ROOT / "build/rom/rsgun-cart.bin").read_bytes()
    targets = voids()
    md = cs.Cs(cs.CS_ARCH_SH, cs.CS_MODE_BIG_ENDIAN | cs.CS_MODE_SH2)

    def insn(o):
        i = next(md.disasm(cart[o:o + 2], o), None)
        return f"{i.mnemonic} {i.op_str}".strip() if i else ""

    found = 0
    for o in range(0, len(cart) - 2, 2):
        w = int.from_bytes(cart[o:o + 2], "big")
        if w >> 12 != 0xD:
            continue
        lit = (o & ~3) + 4 + (w & 0xFF) * 4
        if lit + 4 > len(cart):
            continue
        target = int.from_bytes(cart[lit:lit + 4], "big")
        if target not in targets:
            continue
        rn = (w >> 8) & 0xF
        for k in range(1, 8):                       # the jsr through Rn, a few instructions on
            j = o + 2 * k
            t = insn(j)
            if t == f"jsr @r{rn}":
                break
            if re.search(rf"r{rn}\b", t.split(",")[-1]) or t.split(" ")[0] in STOPS:
                j = None
                break
        else:
            j = None
        if j is None:
            continue
        for k in range(2, 10):                      # after the delay slot
            t = insn(j + 2 * k)
            if not re.search(r"\br0\b", t):
                if t.split(" ")[0] in STOPS:
                    break
                continue
            if not writes_r0_only(t):
                found += 1
                print(f"cart +{j:#08x}: {targets[target]} ({target:#010x}) then `{t}`")
            break
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
