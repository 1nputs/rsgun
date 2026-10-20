"""SH-2 machine code to Rust: each function of the game as a Rust function over the CPU's registers
(stv::sh2::Sh2) and the bus, instruction for instruction as stv's interpreter runs them, so it can
replace the SH-2 code and be checked against it call by call (stv::native).

    python3 tools/port/sh2rust.py --image WRAM_H.bin --evidence EVIDENCE.txt[,EVIDENCE.txt...] --out build/port/generated.rs
        [--range 06003000-06080000]

WRAM_H.bin: work RAM-H as the game had it loaded (rsgun --dump); EVIDENCE.txt: rsgun --evidence. The
functions are the targets of the calls the evidence saw within the range; jump tables' targets come
from it too. Code in the overlay window is translated once per overlay the evidence saw (by the
window's fingerprint), from the window rsgun saved beside the evidence (EVIDENCE.overlays/), and
registered for that overlay only. The output is ROM-derived: it stays in build/ (never committed), and
crates/rsgun's build script includes it when it is there.

What a function is: its entry and every block reached from it by branches (both ways of a
conditional), up to returns, calls' returns, and jumps; a BRA or computed jump to another function's
entry is a tail call. Code the translator cannot place (an undecodable word, SLEEP, TRAPA, a
stack switch) leaves the function to the interpreter. Interrupt handlers (the evidence's) return by RTE.

Waits stay as the SH-2 code had them: each block charges its cycles through stv::native::tick, and
native code runs on a stack of its own, so a polling loop polls while the machine runs on.
"""
import argparse
import collections
import pathlib
import struct
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
# the overlay window (crates/rsgun's overlay())
WINDOW = (0x06080000, 0x060A5000)


class Untranslatable(Exception):
    pass


def sext8(v):
    return v - 0x100 if v & 0x80 else v


def sext12(v):
    return v - 0x1000 if v & 0x800 else v


def h(v):
    return f"0x{v & 0xFFFFFFFF:08X}_u32"


class Image:
    def __init__(self, data, base):
        self.data, self.base = data, base

    def word(self, a):
        o = (a & 0x07FFFFFF) - (self.base & 0x07FFFFFF)
        if not 0 <= o < len(self.data) - 1:
            raise Untranslatable(f"code outside the image at {a:08x}")
        return struct.unpack_from(">H", self.data, o)[0]


# ---- control flow -------------------------------------------------------------------------------

def flow(op, a):
    """How an instruction leaves: (kind, target) — kind in None (falls through), 'cond' (BT/BF,
    target), 'conds' (BT/S, BF/S: delay slot), 'bra', 'braf', 'jmp', 'call' (BSR), 'callr' (JSR,
    BSRF), 'rts', 'rte', 'stop' (SLEEP, TRAPA)."""
    if op & 0xF000 == 0xA000:
        return "bra", a + 4 + 2 * sext12(op & 0xFFF)
    if op & 0xF000 == 0xB000:
        return "call", a + 4 + 2 * sext12(op & 0xFFF)
    if op & 0xFF00 in (0x8900, 0x8B00):
        return "cond", a + 4 + 2 * sext8(op & 0xFF)
    if op & 0xFF00 in (0x8D00, 0x8F00):
        return "conds", a + 4 + 2 * sext8(op & 0xFF)
    if op & 0xF0FF == 0x0023:
        return "braf", None
    if op & 0xF0FF == 0x402B:
        return "jmp", None
    if op & 0xF0FF in (0x0003, 0x400B):
        return "callr", None
    if op == 0x000B:
        return "rts", None
    if op == 0x002B:
        return "rte", None
    if op == 0x001B or op & 0xFF00 == 0xC300:
        return "stop", None
    return None, None


def has_slot(kind):
    return kind in ("conds", "bra", "braf", "jmp", "call", "callr", "rts", "rte")


def loads_sp(op):
    """Loads R15 from memory (a stack switch: the function does not return as functions do)."""
    return (op & 0xFF0F in (0x6F02, 0x6F06) or op & 0xFF00 in (0x5F00, 0xDF00) or op & 0xFF0F == 0x0F0E)


# ---- instructions -----------------------------------------------------------------------------------

def stmt(op, a, pc4):
    """One instruction (not a branch) as Rust statements. `pc4`: the PC-relative base expression."""
    n, m = (op >> 8) & 0xF, (op >> 4) & 0xF
    d4, d8 = op & 0xF, op & 0xFF
    R = lambda i: f"c.r[{i}]"
    t = lambda e: f"c.set_t({e});"
    top = op >> 12
    if top == 0x0:
        lo = op & 0xF
        if lo == 0x2 and m < 3:
            return f"{R(n)} = c.{['sr', 'gbr', 'vbr'][m]};"
        if lo == 0x4:
            return f"m.write8({R(n)}.wrapping_add({R(0)}), {R(m)} as u8);"
        if lo == 0x5:
            return f"m.write16({R(n)}.wrapping_add({R(0)}), {R(m)} as u16);"
        if lo == 0x6:
            return f"m.write32({R(n)}.wrapping_add({R(0)}), {R(m)});"
        if lo == 0x7:
            return f"c.macl = {R(n)}.wrapping_mul({R(m)});"
        if op == 0x0008:
            return t("false")
        if op == 0x0018:
            return t("true")
        if op == 0x0028:
            return "c.mach = 0; c.macl = 0;"
        if op == 0x0009:
            return ""
        if op == 0x0019:
            return "c.set_m(false); c.set_q(false); c.set_t(false);"
        if op & 0xFF == 0x29:
            return f"{R(n)} = c.sr & 1;"
        if lo == 0xA and m < 3:
            return f"{R(n)} = c.{['mach', 'macl', 'pr'][m]};"
        if lo == 0xC:
            return f"{R(n)} = m.read8({R(m)}.wrapping_add({R(0)})) as i8 as u32;"
        if lo == 0xD:
            return f"{R(n)} = m.read16({R(m)}.wrapping_add({R(0)})) as i16 as u32;"
        if lo == 0xE:
            return f"{R(n)} = m.read32({R(m)}.wrapping_add({R(0)}));"
        if lo == 0xF:
            return f"c.mac_l(m, {n}, {m});"
    elif top == 0x1:
        return f"m.write32({R(n)}.wrapping_add({d4 << 2}), {R(m)});"
    elif top == 0x2:
        lo = op & 0xF
        if lo == 0x0:
            return f"m.write8({R(n)}, {R(m)} as u8);"
        if lo == 0x1:
            return f"m.write16({R(n)}, {R(m)} as u16);"
        if lo == 0x2:
            return f"m.write32({R(n)}, {R(m)});"
        if lo in (0x4, 0x5, 0x6):
            sz, w = {0x4: (1, "write8"), 0x5: (2, "write16"), 0x6: (4, "write32")}[lo]
            cast = {1: " as u8", 2: " as u16", 4: ""}[sz]
            return f"{{ let a = {R(n)}.wrapping_sub({sz}); m.{w}(a, {R(m)}{cast}); {R(n)} = a; }}"
        if lo == 0x7:
            return f"{{ let q = {R(n)} >> 31 != 0; let mm = {R(m)} >> 31 != 0; c.set_q(q); c.set_m(mm); c.set_t(q != mm); }}"
        if lo == 0x8:
            return t(f"{R(n)} & {R(m)} == 0")
        if lo == 0x9:
            return f"{R(n)} &= {R(m)};"
        if lo == 0xA:
            return f"{R(n)} ^= {R(m)};"
        if lo == 0xB:
            return f"{R(n)} |= {R(m)};"
        if lo == 0xC:
            return (f"{{ let x = {R(n)} ^ {R(m)}; c.set_t(x & 0xFF00_0000 == 0 || x & 0x00FF_0000 == 0 || "
                    f"x & 0x0000_FF00 == 0 || x & 0x0000_00FF == 0); }}")
        if lo == 0xD:
            return f"{R(n)} = ({R(m)} << 16) | ({R(n)} >> 16);"
        if lo == 0xE:
            return f"c.macl = ({R(n)} & 0xFFFF).wrapping_mul({R(m)} & 0xFFFF);"
        if lo == 0xF:
            return f"c.macl = ({R(n)} as i16 as i32).wrapping_mul({R(m)} as i16 as i32) as u32;"
    elif top == 0x3:
        lo = op & 0xF
        ab = f"let (a, b) = ({R(n)}, {R(m)});"
        body = {
            0x0: t("a == b"),
            0x2: t("a >= b"),
            0x3: t("a as i32 >= b as i32"),
            0x6: t("a > b"),
            0x7: t("a as i32 > b as i32"),
            0x8: f"{R(n)} = a.wrapping_sub(b);",
            0xC: f"{R(n)} = a.wrapping_add(b);",
            0x5: "let p = (a as u64) * (b as u64); c.mach = (p >> 32) as u32; c.macl = p as u32;",
            0xD: "let p = (a as i32 as i64) * (b as i32 as i64); c.mach = (p >> 32) as u32; c.macl = p as u32;",
            0xA: f"let t1 = a.wrapping_sub(b); let res = t1.wrapping_sub(c.t() as u32); {R(n)} = res; c.set_t(a < t1 || t1 < res);",
            0xB: f"let res = a.wrapping_sub(b); {R(n)} = res; c.set_t(((a ^ b) & (a ^ res)) >> 31 != 0);",
            0xE: f"let t1 = a.wrapping_add(b); let res = t1.wrapping_add(c.t() as u32); {R(n)} = res; c.set_t(a > t1 || t1 > res);",
            0xF: f"let res = a.wrapping_add(b); {R(n)} = res; c.set_t((!(a ^ b) & (a ^ res)) >> 31 != 0);",
        }
        if lo == 0x4:
            return f"c.div1({n}, {m});"
        if lo in body:
            return f"{{ {ab} {body[lo]} }}"
    elif top == 0x4:
        if op & 0xF == 0xF:
            return f"c.mac_w(m, {n}, {m});"
        low = op & 0xFF
        v = R(n)
        simple = {
            0x00: f"c.set_t({v} >> 31 != 0); {v} <<= 1;",
            0x20: f"c.set_t({v} >> 31 != 0); {v} <<= 1;",
            0x10: f"{v} = {v}.wrapping_sub(1); c.set_t({v} == 0);",
            0x01: f"c.set_t({v} & 1 != 0); {v} >>= 1;",
            0x11: t(f"{v} as i32 >= 0"),
            0x21: f"c.set_t({v} & 1 != 0); {v} = (({v} as i32) >> 1) as u32;",
            0x04: f"c.set_t({v} >> 31 != 0); {v} = {v}.rotate_left(1);",
            0x24: f"{{ let t = c.t() as u32; c.set_t({v} >> 31 != 0); {v} = ({v} << 1) | t; }}",
            0x05: f"c.set_t({v} & 1 != 0); {v} = {v}.rotate_right(1);",
            0x15: t(f"{v} as i32 > 0"),
            0x25: f"{{ let t = c.t() as u32; c.set_t({v} & 1 != 0); {v} = ({v} >> 1) | (t << 31); }}",
            0x08: f"{v} <<= 2;", 0x18: f"{v} <<= 8;", 0x28: f"{v} <<= 16;",
            0x09: f"{v} >>= 2;", 0x19: f"{v} >>= 8;", 0x29: f"{v} >>= 16;",
            0x0A: f"c.mach = {v};", 0x1A: f"c.macl = {v};", 0x2A: f"c.pr = {v};",
            0x0E: f"c.sr = {v} & 0x3F3;", 0x1E: f"c.gbr = {v};", 0x2E: f"c.vbr = {v};",
            0x1B: f"{{ let a = {v}; let b = m.read8(a); c.set_t(b == 0); m.write8(a, b | 0x80); }}",
        }
        if low in simple:
            return simple[low]
        stores = {0x02: "c.mach", 0x12: "c.macl", 0x22: "c.pr", 0x03: "c.sr", 0x13: "c.gbr", 0x23: "c.vbr"}
        if low in stores:
            return f"{{ let a = {v}.wrapping_sub(4); m.write32(a, {stores[low]}); {v} = a; }}"
        loads = {0x06: "c.mach = x;", 0x16: "c.macl = x;", 0x26: "c.pr = x;", 0x07: "c.sr = x & 0x3F3;",
                 0x17: "c.gbr = x;", 0x27: "c.vbr = x;"}
        if low in loads:
            return f"{{ let x = m.read32({v}); {v} = {v}.wrapping_add(4); {loads[low]} }}"
    elif top == 0x5:
        return f"{R(n)} = m.read32({R(m)}.wrapping_add({d4 << 2}));"
    elif top == 0x6:
        lo = op & 0xF
        v = f"let v = {R(m)};"
        body = {
            0x0: f"{R(n)} = m.read8(v) as i8 as u32;",
            0x1: f"{R(n)} = m.read16(v) as i16 as u32;",
            0x2: f"{R(n)} = m.read32(v);",
            0x3: f"{R(n)} = v;",
            0x4: f"{R(n)} = m.read8(v) as i8 as u32;" + (f" {R(m)} = v.wrapping_add(1);" if n != m else ""),
            0x5: f"{R(n)} = m.read16(v) as i16 as u32;" + (f" {R(m)} = v.wrapping_add(2);" if n != m else ""),
            0x6: f"{R(n)} = m.read32(v);" + (f" {R(m)} = v.wrapping_add(4);" if n != m else ""),
            0x7: f"{R(n)} = !v;",
            0x8: f"{R(n)} = (v & 0xFFFF_0000) | ((v & 0xFF) << 8) | ((v >> 8) & 0xFF);",
            0x9: f"{R(n)} = v.rotate_left(16);",
            0xA: f"let t0 = 0u32.wrapping_sub(v); let res = t0.wrapping_sub(c.t() as u32); {R(n)} = res; c.set_t(0 < t0 || t0 < res);",
            0xB: f"{R(n)} = 0u32.wrapping_sub(v);",
            0xC: f"{R(n)} = v & 0xFF;",
            0xD: f"{R(n)} = v & 0xFFFF;",
            0xE: f"{R(n)} = v as u8 as i8 as u32;",
            0xF: f"{R(n)} = v as u16 as i16 as u32;",
        }
        return f"{{ {v} {body[lo]} }}"
    elif top == 0x7:
        return f"{R(n)} = {R(n)}.wrapping_add({h(sext8(d8))});"
    elif top == 0x8:
        if n == 0x0:
            return f"m.write8({R(m)}.wrapping_add({d4}), {R(0)} as u8);"
        if n == 0x1:
            return f"m.write16({R(m)}.wrapping_add({d4 << 1}), {R(0)} as u16);"
        if n == 0x4:
            return f"{R(0)} = m.read8({R(m)}.wrapping_add({d4})) as i8 as u32;"
        if n == 0x5:
            return f"{R(0)} = m.read16({R(m)}.wrapping_add({d4 << 1})) as i16 as u32;"
        if n == 0x8:
            return t(f"{R(0)} == {h(sext8(d8))}")
    elif top == 0x9:
        return f"{R(n)} = m.read16(({pc4}).wrapping_add({d8 << 1})) as i16 as u32;"
    elif top == 0xC:
        g = "c.gbr"
        if n == 0x0:
            return f"m.write8({g}.wrapping_add({d8}), {R(0)} as u8);"
        if n == 0x1:
            return f"m.write16({g}.wrapping_add({d8 << 1}), {R(0)} as u16);"
        if n == 0x2:
            return f"m.write32({g}.wrapping_add({d8 << 2}), {R(0)});"
        if n == 0x4:
            return f"{R(0)} = m.read8({g}.wrapping_add({d8})) as i8 as u32;"
        if n == 0x5:
            return f"{R(0)} = m.read16({g}.wrapping_add({d8 << 1})) as i16 as u32;"
        if n == 0x6:
            return f"{R(0)} = m.read32({g}.wrapping_add({d8 << 2}));"
        if n == 0x7:
            return f"{R(0)} = (({pc4}) & !3).wrapping_add({d8 << 2});"
        if n == 0x8:
            return t(f"{R(0)} & {d8} == 0")
        if n == 0x9:
            return f"{R(0)} &= {d8};"
        if n == 0xA:
            return f"{R(0)} ^= {d8};"
        if n == 0xB:
            return f"{R(0)} |= {d8};"
        rmw = {0xC: "c.set_t(b & {d} == 0);", 0xD: "m.write8(a, (b & {d}) as u8);", 0xE: "m.write8(a, (b ^ {d}) as u8);",
               0xF: "m.write8(a, (b | {d}) as u8);"}
        if n in rmw:
            return f"{{ let a = {g}.wrapping_add({R(0)}); let b = m.read8(a) as u32; {rmw[n].format(d=d8)} }}"
    elif top == 0xD:
        return f"{R(n)} = m.read32((({pc4}) & !3).wrapping_add({d8 << 2}));"
    elif top == 0xE:
        return f"{R(n)} = {h(sext8(d8))};"
    raise Untranslatable(f"no translation for {op:04x} at {a:08x}")


# ---- a function --------------------------------------------------------------------------------------

class Function:
    def __init__(self, entry, img, entries, jump_targets, return_targets, overlay=0, handler=False):
        self.entry = entry
        self.overlay = overlay          # the overlay's fingerprint, for code in the window
        self.handler = handler          # an interrupt handler (it returns by RTE)
        self.img = img
        self.entries = entries          # every function entry (tail-call targets)
        self.jump_targets = jump_targets  # site -> targets seen
        self.return_targets = return_targets  # RTS site -> targets seen
        self.blocks = {}                # start -> list of instruction addresses
        self.find_blocks()

    def find_blocks(self):
        starts, todo, seen = {self.entry}, [self.entry], set()
        while True:
            self.explore(starts, todo, seen)
            # an RTS that the evidence saw land within the function (PR set to a point in it, then a
            # jump to code that returns there) goes on in the function
            code = {x for addrs in self.blocks.values() for x in addrs}
            more = {tg for x in code if flow(self.img.word(x), x)[0] == "rts"
                    for tg in self.return_targets.get(x, []) if tg in code and tg not in starts}
            if not more:
                break
            starts |= more
            todo += sorted(more)
        self.split(starts)

    def explore(self, starts, todo, seen):
        while todo:
            a = todo.pop()
            if a in seen:
                continue
            seen.add(a)
            addrs, x = [], a
            while True:
                op = self.img.word(x)
                kind, target = flow(op, x)
                addrs.append(x)
                if kind == "stop":
                    raise Untranslatable(f"SLEEP or TRAPA at {x:08x}")
                if loads_sp(op):
                    raise Untranslatable(f"a stack switch at {x:08x}")
                if kind is None:
                    x += 2
                    if x in starts:
                        break
                    continue
                if has_slot(kind):
                    slot = self.img.word(x + 2)
                    if flow(slot, x + 2)[0] is not None:
                        raise Untranslatable(f"a branch in a delay slot at {x + 2:08x}")
                    addrs.append(x + 2)
                nexts = []
                if kind in ("cond", "conds"):
                    nexts = [target, x + 2 if kind == "cond" else x + 4]
                elif kind == "bra" and target not in self.entries:
                    nexts = [target]
                elif kind in ("call", "callr"):
                    nexts = [x + 4]
                elif kind in ("braf", "jmp"):
                    for tg in self.jump_targets.get(x, []):
                        if tg not in self.entries:
                            nexts.append(tg)
                for nx in nexts:
                    if nx not in starts:
                        starts.add(nx)
                    todo.append(nx)
                break
            self.blocks[a] = addrs

    def split(self, starts):
        """Splits blocks that run into a later start."""
        for s in sorted(starts):
            for b, addrs in list(self.blocks.items()):
                if s != b and s in addrs[1:]:
                    i = addrs.index(s)
                    # a jump into a branch's delay slot: the branch keeps its slot, and the jump's
                    # block (explored from there) runs it as an ordinary instruction
                    if i > 0 and has_slot(flow(self.img.word(addrs[i - 1]), addrs[i - 1])[0]):
                        continue
                    self.blocks[b] = addrs[:i]
                    self.blocks.setdefault(s, addrs[i:])
        if self.entry in {t for addrs in self.blocks.values() for t in self.local_targets(addrs)}:
            raise Untranslatable("the entry is a loop head")

    def local_targets(self, addrs):
        last = addrs[-1]
        op = self.img.word(last)
        kind, target = flow(op, last)
        if kind is None and len(addrs) > 1:
            kind, target = flow(self.img.word(addrs[-2]), addrs[-2])
        return [target] if kind in ("cond", "conds", "bra") and target is not None else []

    def rust(self):
        name = f"f_{self.entry & 0x07FFFFFF:08x}" + (f"_{self.overlay:016x}" if self.overlay else "")
        # where the function returns to, for a computed jump the translation has no block for
        out = [f"pub fn {name}(c: &mut Sh2, m: &mut dyn Bus) {{", "    let (ret, sp) = (c.pr, c.r[15]);",
               f"    let mut pc: u32 = {h(self.entry)};", "    loop {", "        match pc {"]
        for b in sorted(self.blocks):
            out.append(f"            {h(b)} => {{")
            out.extend("                " + line for line in self.block(self.blocks[b]))
            out.append("            }")
        out += ["            _ => unreachable!(),", "        }", "    }", "}"]
        return name, "\n".join(out)

    def block(self, addrs):
        lines, i = [], 0
        count = 0
        while i < len(addrs):
            a = addrs[i]
            op = self.img.word(a)
            kind, target = flow(op, a)
            count += 1
            if kind is None:
                s = stmt(op, a, h(a + 4))
                if s:
                    lines.append(s + f" // {a:08x}")
                i += 1
                continue
            # a transfer: its slot (if any) comes after the decision
            slot = None
            if has_slot(kind):
                sa = addrs[i + 1]
                count += 1
                slot_target = None
                slot = (sa, self.img.word(sa))
            lines.append(f"tick(c, m, {count});")
            lines.extend(self.transfer(kind, target, a, op, slot))
            return lines
        # fell into the next block
        nxt = addrs[-1] + 2
        lines.append(f"tick(c, m, {count});")
        lines.append(f"pc = {h(nxt)};")
        return lines

    def slot_code(self, slot, pc4):
        sa, sop = slot
        s = stmt(sop, sa, pc4)
        return [s + f" // {sa:08x} (slot)"] if s else []

    def tail(self, target_expr):
        return [f"call(c, m, {target_expr});", "return;"]

    def transfer(self, kind, target, a, op, slot):
        n = (op >> 8) & 0xF
        if kind == "cond":
            want = "true" if op & 0xFF00 == 0x8900 else "false"
            return [f"pc = if c.t() == {want} {{ {h(target)} }} else {{ {h(a + 2)} }};"]
        if kind == "conds":
            want = "true" if op & 0xFF00 == 0x8D00 else "false"
            # the slot's PC-relative base is the branch's destination + 2 when taken: the
            # interpreter's rule (stv::sh2)
            # (not taken, the slot runs as an ordinary instruction: its own address + 4)
            return [f"let taken = c.t() == {want};", f"let dest: u32 = if taken {{ {h(target)} }} else {{ {h(a + 4)} }};"] + \
                self.slot_code(slot, f"if taken {{ dest.wrapping_add(2) }} else {{ {h(a + 6)} }}") + ["pc = dest;"]
        if kind == "bra":
            code = self.slot_code(slot, h(target + 2))
            if target in self.entries:
                return code + self.tail(h(target))
            return code + [f"pc = {h(target)};"]
        if kind in ("braf", "jmp"):
            dest = f"c.r[{n}]" if kind == "jmp" else f"c.r[{n}].wrapping_add({h(a + 4)})"
            code = [f"let dest = {dest};"] + self.slot_code(slot, "dest.wrapping_add(2)")
            # a target the evidence saw within the function goes on here; any other is interpreted
            # until the function returns (a tail call returns there too)
            local = [tg for tg in self.jump_targets.get(a, []) if tg not in self.entries]
            other = self.elsewhere()
            if local:
                arms = " ".join(f"{h(tg)} => pc = {h(tg)}," for tg in sorted(set(local)))
                code += [f"match dest {{ {arms} _ => {{ {other} }} }}"]
            else:
                code += [other]
            return code
        if kind == "call":
            return [f"c.pr = {h(a + 4)};"] + self.slot_code(slot, h(target + 2)) + [f"call(c, m, {h(target)});", f"pc = {h(a + 4)};"]
        if kind == "callr":
            dest = f"c.r[{n}]" if op & 0xF0FF == 0x400B else f"c.r[{n}].wrapping_add({h(a + 4)})"
            return [f"let dest = {dest};", f"c.pr = {h(a + 4)};"] + self.slot_code(slot, "dest.wrapping_add(2)") + \
                ["call(c, m, dest);", f"pc = {h(a + 4)};"]
        if kind == "rts":
            # the function's return, or a return within it (see find_blocks); anywhere else is
            # interpreted until the function returns
            code = ["let dest = c.pr;"] + self.slot_code(slot, "dest.wrapping_add(2)") + \
                ["if dest == ret { c.pc = dest; return; }"]
            local = sorted({tg for tg in self.return_targets.get(a, []) if tg in self.blocks})
            other = self.elsewhere()
            if local:
                arms = " ".join(f"{h(tg)} => pc = {h(tg)}," for tg in local)
                return code + [f"match dest {{ {arms} _ => {{ {other} }} }}"]
            return code + [other]
        if kind == "rte":
            # as the interpreter: PC, then SR, from the stack; the slot runs with the new SR
            return ["let dest = m.read32(c.r[15]);", "c.r[15] = c.r[15].wrapping_add(4);",
                    "c.sr = m.read32(c.r[15]) & 0x3F3;", "c.r[15] = c.r[15].wrapping_add(4);"] + \
                self.slot_code(slot, "dest.wrapping_add(2)") + ["c.pc = dest;", "return;"]
        raise Untranslatable(kind)

    def elsewhere(self):
        """A transfer to code the translation has no block for: interpreted until the function returns
        (a handler returns by RTE to the PC on its stack at entry)."""
        if self.handler:
            return "goto(c, m, dest, m.read32(sp), sp.wrapping_add(8)); return;"
        return "goto(c, m, dest, ret, sp); return;"


# ---- the evidence and the module ---------------------------------------------------------------------------

def read_evidence(paths):
    """Per overlay fingerprint (0: code outside the window): the call targets, each site's jump
    targets, each RTS site's return targets, and the interrupt handlers."""
    ev = collections.defaultdict(lambda: (set(), collections.defaultdict(set), collections.defaultdict(set), set()))
    for line in (line for path in paths for line in open(path)):
        f = line.split()
        if f[0] == "i":
            handler = int(f[4], 16) & 0x07FFFFFF
            ev[int(f[2], 16) if in_window(handler) else 0][3].add(handler)
        if f[0] != "t":
            continue
        fp, kind, site, target = int(f[2], 16), f[3], int(f[4], 16) & 0x07FFFFFF, int(f[5], 16) & 0x07FFFFFF
        if kind == "c":
            ev[fp if in_window(target) else 0][0].add(target)
        elif kind == "j":
            ev[fp if in_window(site) else 0][1][site].add(target)
        elif kind == "r" and target:
            ev[fp if in_window(site) else 0][2][site].add(target)
    return ev


def in_window(a):
    return WINDOW[0] <= a < WINDOW[1]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--image", required=True)
    ap.add_argument("--evidence", required=True, help="rsgun --evidence files, comma-separated (merged)")
    ap.add_argument("--out", default=str(ROOT / "build/port/generated.rs"))
    ap.add_argument("--range", default="06000C00-06003000,06003000-0600A9A4,0603327C-0603C000,06040000-060735E8",
                    help="the code regions: the BIOS's code in RAM (its interrupt dispatch and library, there from "
                         "the first frames), then the game's load map (the rest of work RAM held other code in the boot)")
    ap.add_argument("--only", default="", help="comma-separated entries to translate (hex)")
    a = ap.parse_args()
    regions = [tuple(int(x, 16) for x in r.split("-")) for r in a.range.split(",")]
    main_image = open(a.image, "rb").read()
    paths = [pathlib.Path(x) for x in a.evidence.split(",")]
    ev = read_evidence(paths)
    # each overlay's window, saved beside the evidence that saw it
    windows = {int(f.stem, 16): f for path in paths for f in path.with_suffix(".overlays").glob("*.bin")}
    main_calls = {t for t in ev[0][0] | ev[0][3] if any(lo <= t < hi for lo, hi in regions)}
    only = {int(x, 16) for x in a.only.split(",")} if a.only else None
    done, skipped = [], collections.Counter()
    for fp in sorted(ev):
        calls, jumps, returns, handlers = ev[fp]
        calls = calls | handlers
        if fp == 0:
            img, wanted, entries = Image(main_image, 0x06000000), main_calls, main_calls
        else:
            window = windows.get(fp)
            if window is None:
                skipped["an overlay with no window saved"] += len(calls)
                continue
            data = bytearray(main_image)
            code = window.read_bytes()
            data[WINDOW[0] - 0x06000000:WINDOW[0] - 0x06000000 + len(code)] = code
            img, wanted, entries = Image(bytes(data), 0x06000000), calls, calls | main_calls
        for e in sorted(wanted):
            if only is not None and e not in only:
                continue
            try:
                done.append((e, fp, Function(e, img, entries, jumps, returns, fp, e in handlers)))
            except Untranslatable as err:
                skipped[str(err).split(" at ")[0]] += 1
    fns = [(e, fp) + f.rust() for e, fp, f in done]
    out = pathlib.Path(a.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    body = ["// Generated by tools/port/sh2rust.py from the game's code (ROM-derived: build/ only).",
            "#[allow(unused_variables, unused_mut, unreachable_code, clippy::all)]", "pub mod generated {",
            "use stv::sh2::{Bus, Sh2};", "use stv::native::{call, goto, tick, Natives};", ""]
    body += [src for _, _, _, src in fns]
    body += ["", "pub fn register(n: &mut Natives) {"]
    body += [f"    n.register({h(e)}, {name}, \"{name}\", {f'Some(0x{fp:016x})' if fp else 'None'});" for e, fp, name, _ in fns]
    body += ["}", "}"]
    out.write_text("\n".join(body) + "\n")
    n_overlay = sum(1 for _, fp, _, _ in fns if fp)
    print(f"{len(fns)} functions translated ({n_overlay} in {len({fp for _, fp, _, _ in fns if fp})} overlays), "
          f"{sum(skipped.values())} left to the interpreter: {dict(skipped)}")


if __name__ == "__main__":
    main()
