"""Straight-line SH-2 code (tools/rom/dis.py's listing) as Rust statements on locals r0-r15, `mac`
and `t`, each with its address and instruction as a comment: for transliterating SGL's assembly
step for step where its registers must come out as SGL leaves them (sgl.rs's sl_inverse). Only
the instructions those routines use; branches, delay slots and anything else are printed as
`// TODO` lines for the hand to place.

    python3 tools/port/asm2rs.py LISTING > out.rs
"""
import re, sys
def r(n): return f"r{n}"


def parse(line):
    """A listing line: (address, opcode, mnemonic, operands without spaces, the literal comment),
    or None."""
    m = re.match(r"([0-9a-f]{8}): ([0-9a-f]{4})\s+(\S+)\s*(.*?)\s*(;.*)?$", line.rstrip())
    if not m:
        return None
    return m.group(1), int(m.group(2), 16), m.group(3), m.group(4).replace(" ", ""), m.group(5) or ""


def convert(addr, mn, args, lit):
    """One instruction as a Rust statement, or None (a branch, or one not handled)."""
    a = re.split(r",(?![^(]*\))", args)
    def reg(s): return "r" + s[1:] if s.startswith("r") else s
    s = None
    R = lambda x: re.fullmatch(r"r(\d+)", x)
    if mn == "mov.l" and "gbr" not in a[0] and re.fullmatch(r"@\((\d+),r(\d+)\)", a[0]) and R(a[1]):
        d, b = re.fullmatch(r"@\((\d+),r(\d+)\)", a[0]).groups(); s = f"{reg(a[1])} = cpu.read32(r{b}.wrapping_add({d}));"
    elif mn == "mov.l" and R(a[0]) and re.fullmatch(r"@\((\d+),r(\d+)\)", a[1]):
        d, b = re.fullmatch(r"@\((\d+),r(\d+)\)", a[1]).groups(); s = f"cpu.write32(r{b}.wrapping_add({d}), {reg(a[0])});"
    elif mn == "mov.l" and re.fullmatch(r"@r(\d+)", a[0]) and R(a[1]):
        b = a[0][2:]; s = f"{reg(a[1])} = cpu.read32(r{b});"
    elif mn == "mov.l" and R(a[0]) and re.fullmatch(r"@r(\d+)", a[1]):
        b = a[1][2:]; s = f"cpu.write32(r{b}, {reg(a[0])});"
    elif mn == "mov.l" and re.fullmatch(r"@r(\d+)\+", a[0]) and R(a[1]):
        b = a[0][2:-1]; s = f"{reg(a[1])} = cpu.read32(r{b}); r{b} = r{b}.wrapping_add(4);"
    elif mn == "mov" and R(a[0]) and R(a[1]): s = f"{reg(a[1])} = {reg(a[0])};"
    elif mn == "mov" and a[0].startswith("#"): s = f"{reg(a[1])} = {int(a[0][1:])}i32 as u32;"
    elif mn == "add" and a[0].startswith("#"): s = f"{reg(a[1])} = {reg(a[1])}.wrapping_add({int(a[0][1:])}i32 as u32);"
    elif mn == "add": s = f"{reg(a[1])} = {reg(a[1])}.wrapping_add({reg(a[0])});"
    elif mn == "sub": s = f"{reg(a[1])} = {reg(a[1])}.wrapping_sub({reg(a[0])});"
    elif mn == "neg": s = f"{reg(a[1])} = {reg(a[0])}.wrapping_neg();"
    elif mn == "not": s = f"{reg(a[1])} = !{reg(a[0])};"
    elif mn == "or" and R(a[0]): s = f"{reg(a[1])} |= {reg(a[0])};"
    elif mn == "and" and R(a[0]): s = f"{reg(a[1])} &= {reg(a[0])};"
    elif mn == "xor" and R(a[0]): s = f"{reg(a[1])} ^= {reg(a[0])};"
    elif mn == "swap.w": s = f"{reg(a[1])} = {reg(a[0])}.rotate_left(16);"
    elif mn == "swap.b": s = f"{reg(a[1])} = ({reg(a[0])} & 0xffff_0000) | (({reg(a[0])} & 0xff) << 8) | (({reg(a[0])} >> 8) & 0xff);"
    elif mn == "exts.w": s = f"{reg(a[1])} = {reg(a[0])} as i16 as i32 as u32;"
    elif mn == "exts.b": s = f"{reg(a[1])} = {reg(a[0])} as i8 as i32 as u32;"
    elif mn == "extu.w": s = f"{reg(a[1])} = {reg(a[0])} & 0xffff;"
    elif mn == "extu.b": s = f"{reg(a[1])} = {reg(a[0])} & 0xff;"
    elif mn in ("shll8","shll16","shll2","shlr8","shlr16","shlr2"):
        n = int(mn[4:]); d = "<<" if mn.startswith("shll") else ">>"; s = f"{reg(a[0])} {d}= {n};"
    elif mn == "shll": s = f"t = {reg(a[0])} >> 31 != 0; {reg(a[0])} <<= 1;"
    elif mn == "shlr": s = f"t = {reg(a[0])} & 1 != 0; {reg(a[0])} >>= 1;"
    elif mn == "shar": s = f"t = {reg(a[0])} & 1 != 0; {reg(a[0])} = ({reg(a[0])} as i32 >> 1) as u32;"
    elif mn == "dmuls.l": s = f"mac = {reg(a[0])} as i32 as i64 * {reg(a[1])} as i32 as i64;"
    elif mn == "dmulu.l": s = f"mac = ({reg(a[0])} as u64 * {reg(a[1])} as u64) as i64;"
    elif mn == "sts" and a[0] == "mach": s = f"{reg(a[1])} = (mac >> 32) as u32;"
    elif mn == "sts" and a[0] == "macl": s = f"{reg(a[1])} = mac as u32;"
    elif mn == "xtrct": s = f"{reg(a[1])} = ({reg(a[0])} << 16) | ({reg(a[1])} >> 16);"
    elif mn == "tst" and R(a[0]): s = f"t = {reg(a[0])} & {reg(a[1])} == 0;"
    elif mn.startswith("cmp/"):
        k = mn[4:]
        if k == "pz": s = f"t = {reg(a[0])} as i32 >= 0;"
        elif k == "pl": s = f"t = {reg(a[0])} as i32 > 0;"
        elif a[0].startswith("#"):
            s = f"t = r0 == {int(a[0][1:])}i32 as u32;"
        else:
            x, y = reg(a[0]), reg(a[1])
            s = {"eq": f"t = {y} == {x};", "hs": f"t = {y} >= {x};", "hi": f"t = {y} > {x};", "ge": f"t = {y} as i32 >= {x} as i32;", "gt": f"t = {y} as i32 > {x} as i32;"}[k]
    mm = None
    def mem(x):
        return re.fullmatch(r"@\((\d+),(r\d+|gbr)\)", x)
    if s is None and mn in ("mov.w", "mov.b", "mov.l") and R(a[1]) and re.fullmatch(r"@\(r0,r(\d+)\)", a[0]):
        b_ = re.fullmatch(r"@\(r0,r(\d+)\)", a[0]).group(1)
        rd, ty = {"mov.w": ("read16", " as i16 as i32 as u32"), "mov.b": ("read8", " as i8 as i32 as u32"), "mov.l": ("read32", "")}[mn]
        s = f"{reg(a[1])} = cpu.{rd}(r0.wrapping_add(r{b_})){ty};"
    elif s is None and mn in ("mov.w", "mov.b", "mov.l") and R(a[0]) and re.fullmatch(r"@\(r0,r(\d+)\)", a[1]):
        b_ = re.fullmatch(r"@\(r0,r(\d+)\)", a[1]).group(1)
        wr, ty = {"mov.w": ("write16", " as u16"), "mov.b": ("write8", " as u8"), "mov.l": ("write32", "")}[mn]
        s = f"cpu.{wr}(r0.wrapping_add(r{b_}), {reg(a[0])}{ty});"
    elif s is None and mn in ("mov.w", "mov.b") and R(a[1]) and re.fullmatch(r"@r(\d+)\+", a[0]):
        b_ = a[0][2:-1]; n = 2 if mn == "mov.w" else 1; ty = "i16" if n == 2 else "i8"; rd = "read16" if n == 2 else "read8"
        s = f"{reg(a[1])} = cpu.{rd}(r{b_}) as {ty} as i32 as u32; r{b_} = r{b_}.wrapping_add({n});"
    elif s is None and mn in ("mov.w", "mov.b", "mov.l") and R(a[1]) and re.fullmatch(r"@r(\d+)", a[0]):
        b_ = a[0][2:]; rd, ty = {"mov.w": ("read16", " as i16 as i32 as u32"), "mov.b": ("read8", " as i8 as i32 as u32"), "mov.l": ("read32", "")}[mn]
        s = f"{reg(a[1])} = cpu.{rd}(r{b_}){ty};"
    elif s is None and mn in ("mov.w", "mov.b", "mov.l") and R(a[1]) and mem(a[0]):
        d, b_ = mem(a[0]).groups(); base = "w" if b_ == "gbr" else b_
        rd, ty = {"mov.w": ("read16", " as i16 as i32 as u32"), "mov.b": ("read8", " as i8 as i32 as u32"), "mov.l": ("read32", "")}[mn]
        s = f"{reg(a[1])} = cpu.{rd}({base}.wrapping_add({d})){ty};"
    elif s is None and mn in ("mov.w", "mov.b", "mov.l") and R(a[0]) and mem(a[1]):
        d, b_ = mem(a[1]).groups(); base = "w" if b_ == "gbr" else b_
        wr, ty = {"mov.w": ("write16", " as u16"), "mov.b": ("write8", " as u8"), "mov.l": ("write32", "")}[mn]
        s = f"cpu.{wr}({base}.wrapping_add({d}), {reg(a[0])}{ty});"
    elif s is None and mn in ("mov.w", "mov.b") and R(a[0]) and re.fullmatch(r"@r(\d+)", a[1]):
        b_ = a[1][2:]; wr, ty = ("write16", " as u16") if mn == "mov.w" else ("write8", " as u8")
        s = f"cpu.{wr}(r{b_}, {reg(a[0])}{ty});"
    elif s is None and mn == "mov.l" and R(a[0]) and re.fullmatch(r"@-r15", a[1]):
        s = f"r15 = r15.wrapping_sub(4); cpu.write32(r15, {reg(a[0])});"
    elif s is None and mn == "mov.l" and re.fullmatch(r"@r15\+", a[0]) and R(a[1]):
        s = f"{reg(a[1])} = cpu.read32(r15); r15 = r15.wrapping_add(4);"
    elif s is None and mn in ("mov.l", "mov.w") and re.search(r"\[[0-9a-f]+\] = ([0-9a-f]+)", lit):
        v = re.search(r"\[[0-9a-f]+\] = ([0-9a-f]+)", lit).group(1)
        v = int(v, 16)
        if mn == "mov.w": v = v - 0x10000 if v & 0x8000 else v
        s = f"{reg(a[1])} = 0x{v & 0xffffffff:08x};"
    elif s is None and mn == "mova":
        v = re.search(r"= ([0-9a-f]+)", lit).group(1); s = f"r0 = 0x{int(v,16):08x};"
    elif s is None and mn == "mul.l": s = f"mac = (mac & !0xffff_ffff) | ({reg(a[1])}.wrapping_mul({reg(a[0])}) as i64 & 0xffff_ffff);"
    elif s is None and mn == "dt": s = f"{reg(a[0])} = {reg(a[0])}.wrapping_sub(1); t = {reg(a[0])} == 0;"
    elif s is None and mn == "rotcl": s = f"{{ let c = {reg(a[0])} >> 31 != 0; {reg(a[0])} = ({reg(a[0])} << 1) | t as u32; t = c; }}"
    elif s is None and mn == "rotl": s = f"t = {reg(a[0])} >> 31 != 0; {reg(a[0])} = {reg(a[0])}.rotate_left(1);"
    elif s is None and mn == "subc": s = f"{{ let (x, b1) = {reg(a[1])}.overflowing_sub({reg(a[0])}); let (y, b2) = x.overflowing_sub(t as u32); {reg(a[1])} = y; t = b1 || b2; }}"
    elif s is None and mn == "tst" and a[0].startswith("#"): s = f"t = r0 & {int(a[0][1:])} == 0;"
    elif s is None and mn == "and" and a[0].startswith("#"): s = f"r0 &= {int(a[0][1:])};"
    elif s is None and mn == "or" and a[0].startswith("#"): s = f"r0 |= {int(a[0][1:])};"
    elif s is None and mn == "xor" and a[0].startswith("#"): s = f"r0 ^= {int(a[0][1:])};"
    elif s is None and mn == "cmp/eq" and a[0].startswith("#"): s = f"t = r0 == {int(a[0][1:])}i32 as u32;"
    elif s is None and mn in ("clrt",): s = "t = false;"
    elif s is None and mn in ("sett",): s = "t = true;"
    elif s is None and mn == "nop": s = "// nop"
    return s


if __name__ == "__main__":
    out = []
    for line in open(sys.argv[1]):
        p = parse(line)
        if not p:
            continue
        addr, _, mn, args, lit = p
        s = convert(addr, mn, args, lit)
        if s is None:
            s = f"// TODO {mn} {args}"
        out.append(f"    {s} // {addr} {mn} {args}")
    print("\n".join(out))
