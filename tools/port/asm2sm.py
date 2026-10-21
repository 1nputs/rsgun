"""An SH-2 routine (tools/rom/dis.py's listing) as a Rust function that runs it block by block:
for SGL's hand-written assembly that jumps through tables and shares tails between entries,
where structured code would not say more than the blocks do. Locals r0-r15, `mac` and `t` hold
the registers; each block is a `match` arm on its address, ending in the branch the SH-2 code
takes (its delay slot first); calls (bsr, jsr) go through `cpu.call` with the registers handed
over and taken back; rts returns with the registers as the code leaves them. The routine's
own pushes are made by the blocks, so its entry must be in PROLOGUES with no pushes.

    python3 tools/port/asm2sm.py LISTING NAME ENTRY [TARGET...] > out.rs

LISTING: dis.py's listing of a region holding the routine (every halfword, data too). The blocks
are found from ENTRY by following the code: branches, delay slots, and the jump tables a
`jmp @rN` reads (a `mova` base and a `mov.l @(r0,rK),rN` before it: the table's longs, while they
are addresses in the listing). TARGET: more block starts. Instructions asm2rs.py cannot convert
stop the script (it names them).
"""
import importlib.util
import pathlib
import sys

spec = importlib.util.spec_from_file_location("asm2rs", pathlib.Path(__file__).with_name("asm2rs.py"))
asm2rs = importlib.util.module_from_spec(spec)
sys.argv, argv = sys.argv[:1], sys.argv
spec.loader.exec_module(asm2rs)
sys.argv = argv

DELAYED = {"bt/s", "bf/s", "bra", "braf", "bsr", "bsrf", "jmp", "jsr", "rts"}
BRANCHES = DELAYED | {"bt", "bf"}


def main():
    listing, name, entry = sys.argv[1], sys.argv[2], int(sys.argv[3], 16)
    extra = [int(x, 16) for x in sys.argv[4:]]
    listed, words = {}, {}
    for line in open(listing):
        p = asm2rs.parse(line)
        if p:
            a = int(p[0], 16)
            words[a] = p[1]
            if not p[2].startswith("."):
                listed[a] = p

    def long_at(a):
        if a in words and a + 2 in words:
            return words[a] << 16 | words[a + 2]
        return None

    # the code reached from the entry, block starts and all; a threaded jump (through a register
    # stepped along a table of handlers) takes the tables the reached code's `mova`s point at, so
    # the walk is made again until it reaches no new one
    movas = set()
    while True:
        code, leaders, todo, seen = {}, {entry, *extra}, [entry, *extra], set()
        while todo:
            a = todo.pop()
            while a not in seen:
                if a not in listed:
                    sys.exit(f"{a:08x}: reached, not code")
                seen.add(a)
                code[a] = listed[a]
                mn, args = listed[a][2], listed[a][3]
                if mn not in BRANCHES:
                    a += 2
                    continue
                if mn in DELAYED:
                    code[a + 2] = listed[a + 2]
                if mn in ("bt", "bf", "bt/s", "bf/s", "bra"):
                    t = int(args, 16)
                    leaders.add(t)
                    todo.append(t)
                if mn == "jmp":
                    for t in jump_table(a, listed, long_at, movas):
                        leaders.add(t)
                        todo.append(t)
                if mn in ("bra", "braf", "jmp", "rts"):
                    break
                a += 4 if mn in DELAYED else 2
                leaders.add(a)
        reached = {int(p[4].split("=")[1], 16) for p in code.values() if p[2] == "mova"}
        if reached <= movas:
            break
        movas |= reached
    leaders = sorted(leaders)

    def spin(at, target):
        """Whether the backward branch at `at` to `target` closes a wait loop: short, storing
        nothing (it reads what something else changes, or counts time). Such a loop lets the
        machine run on (`Cpu::wait`) each pass, or it would never end natively."""
        if target > at or at - target > 32:
            return False
        import re
        body = [code.get(b) for b in range(target, at + 4, 2)]
        if None in body:
            return False
        # a loop: nothing before its closing branch leaves it for good
        if any(code[b][2] in ("bra", "braf", "jmp", "rts") for b in range(target, at, 2)):
            return False
        written = set()
        for p in body:
            ops = re.split(r",(?![^(]*\))", p[3])
            if len(ops) == 2 and ops[1].startswith("@") or p[2] in ("mac.l", "tas.b") or p[2].startswith(("bsr", "jsr")):
                return False
            if re.fullmatch(r"r\d+", ops[-1]):
                written.add(ops[-1])
            written.update(re.findall(r"@(r\d+)\+", p[3]))
        reads, others = False, 0
        for p in body:
            ops = re.split(r",(?![^(]*\))", p[3])
            if ops[0].startswith("@") and "pc" not in p[3] and not p[4]:
                # a read at a place the loop does not move: something else changes it
                if not set(re.findall(r"r\d+", ops[0])) & written:
                    reads = True
            elif p[2] not in ("dt", "nop") and p[2] not in BRANCHES:
                others += 1
        # a poll reads what something else changes; a delay only counts down
        return reads or others == 0

    def wait_line(at, target):
        return f"cpu.wait(0x{target:08x}, {(at - target) // 2 + 2});"

    def stmt(a):
        addr, _, mn, args, lit = code[a]
        s = asm2rs.convert(addr, mn, args, lit) or extra_convert(mn, args)
        if s is None:
            missing.append(f"{addr}: {mn} {args}")
            return f"// MISSING {addr} {mn} {args}"
        return f"{s} // {addr} {mn} {args}"

    missing = []

    out = []
    out.append("#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]")
    out.append(f"pub fn {name}(cpu: &mut Cpu) {{")
    out.append("    let w = cpu.gbr();")
    out.append("    let [mut r0, mut r1, mut r2, mut r3, mut r4, mut r5, mut r6, mut r7, mut r8, mut r9, mut r10, mut r11, mut r12, mut r13, _, mut r15] = cpu.sh2.r;")
    out.append("    let mut r14 = cpu.caller_r14;")
    out.append("    let mut mac = ((cpu.sh2.mach as u64) << 32 | cpu.sh2.macl as u64) as i64;")
    out.append("    let mut t = cpu.sh2.sr & 1 != 0;")
    out.append("    let entry_pr = cpu.sh2.pr;")
    out.append("    let mut pr = entry_pr;")
    out.append("    macro_rules! hand_over { () => { cpu.sh2.r = [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15]; cpu.leave_mac(mac); cpu.leave_t(t); } }")
    out.append("    macro_rules! take_back { () => { [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15] = cpu.sh2.r; mac = ((cpu.sh2.mach as u64) << 32 | cpu.sh2.macl as u64) as i64; t = cpu.sh2.sr & 1 != 0; } }")
    out.append(f"    let mut pc: u32 = 0x{entry:08x};")
    out.append("    loop {")
    out.append("        match pc {")
    for k, start in enumerate(leaders):
        out.append(f"            0x{start:08x} => {{")
        a = start
        while True:
            if a not in code:
                sys.exit(f"{a:08x}: runs off the listing")
            addr, _, mn, args, lit = code[a]
            if mn not in BRANCHES:
                out.append("                " + stmt(a))
                a += 2
                if a in leaders:
                    out.append(f"                pc = 0x{a:08x};")
                    break
                continue
            slot = a + 2
            after = a + 4
            ind = "                "
            if mn in ("bt", "bf"):
                cond = "t" if mn == "bt" else "!t"
                x = int(args, 16)
                out.append(f"{ind}pc = if {cond} {{ 0x{x:08x} }} else {{ 0x{a + 2:08x} }}; // {addr} {mn} {args}")
                if spin(a, x):
                    out.append(f"{ind}if pc == 0x{x:08x} {{ {wait_line(a, x)} }}")
            elif mn in ("bt/s", "bf/s"):
                cond = "t" if mn == "bt/s" else "!t"
                x = int(args, 16)
                out.append(f"{ind}let take = {cond}; // {addr} {mn} {args}")
                out.append(ind + stmt(slot))
                out.append(f"{ind}pc = if take {{ 0x{x:08x} }} else {{ 0x{after:08x} }};")
                if spin(a, x):
                    out.append(f"{ind}if take {{ {wait_line(a, x)} }}")
            elif mn == "bra":
                x = int(args, 16)
                out.append(f"{ind}// {addr} bra {args}")
                out.append(ind + stmt(slot))
                out.append(f"{ind}pc = 0x{x:08x};")
                if spin(a, x):
                    out.append(f"{ind}{wait_line(a, x)}")
            elif mn == "jmp":
                rn = args[1:]
                out.append(f"{ind}let to = {rn}; // {addr} jmp {args}")
                out.append(ind + stmt(slot))
                out.append(f"{ind}pc = to;")
            elif mn == "rts":
                out.append(f"{ind}// {addr} rts")
                out.append(ind + stmt(slot))
                out.append(f"{ind}if pr == entry_pr {{ break; }}")
                out.append(f"{ind}pc = pr;")
            elif mn in ("bsr", "jsr"):
                to = f"0x{int(args, 16):08x}" if mn == "bsr" else args[1:]
                out.append(f"{ind}let to = {to}; // {addr} {mn} {args}")
                out.append(ind + stmt(slot))
                out.append(f"{ind}hand_over!(); cpu.call(to, &[]); take_back!();")
                out.append(f"{ind}pr = 0x{after:08x};")
                out.append(f"{ind}pc = 0x{after:08x};")
            else:
                sys.exit(f"{addr}: {mn} not handled")
            break
        out.append("            }")
    out.append('            _ => unreachable!("{pc:#010x}"),')
    out.append("        }")
    out.append("    }")
    out.append("    hand_over!();")
    out.append("    cpu.leave_r14(r14);")
    out.append("}")
    if missing:
        sys.exit("not converted:\n" + "\n".join(missing))
    print("\n".join(out))


def jump_table(at, listed, long_at, movas):
    """The targets of the `jmp @rN` at `at`: its table found from the instructions before it."""
    rn = listed[at][3][1:]
    base = index = None
    movas = []
    for back in range(2, 128, 2):
        p = listed.get(at - back)
        if not p:
            break
        mn, args, lit = p[2], p[3], p[4]
        if index is None and mn == "mov.l" and args.startswith("@(r0,") and args.endswith("," + rn):
            index = args
        if mn == "mova":
            movas.append(int(lit.split("=")[1], 16))
            if index is not None:
                base = movas[-1]
                break
    if base is None:
        setter = next((listed[at - b] for b in range(2, 256, 2) if at - b in listed and listed[at - b][3].endswith("," + rn)), None)
        if setter and setter[2] == "mov.l" and "] = " in setter[4]:
            # a literal's address
            v = int(setter[4].split("] = ")[1], 16)
            if v in listed:
                return [v]
        if setter and setter[2] == "mov.l" and setter[3].startswith("@r") and not setter[3].startswith("@(r0"):
            # threaded: the register steps through a table of handlers (each group ended by 0)
            out = set()
            for m in movas:
                out |= set(table_at(m, listed, long_at, zeros=True))
            return sorted(out)
        # no table: the jump goes where a `mova` before it pointed r0
        if rn == "r0" and movas and all(m in listed for m in movas):
            return sorted(set(movas))
        sys.exit(f"{at:08x}: jmp without a table found")
    out = table_at(base, listed, long_at)
    if not out:
        sys.exit(f"{at:08x}: empty table at {base:08x}")
    return out


def table_at(base, listed, long_at, zeros=False):
    """The code addresses in a table of longs at `base` (with `zeros`, groups ended by 0)."""
    out, k = [], 0
    first = long_at(base)
    if first is None or first & 1 or first not in listed:
        return []
    while True:
        v = long_at(base + 4 * k)
        if zeros and v == 0:
            k += 1
            continue
        if v is None or v & 1 or v not in listed:
            break
        out.append(v)
        k += 1
    return out



def extra_convert(mn, args):
    """What asm2rs.py does not convert and these routines use."""
    a = args.split(",")
    reg = lambda s: s
    if mn == "clrmac":
        return "mac = 0;"
    if mn == "mac.l":
        m, n = a[0][1:-1], a[1][1:-1]
        return (f"{{ let x = cpu.read32({m}) as i32 as i64; {m} = {m}.wrapping_add(4); let y = cpu.read32({n}) as i32 as i64; {n} = {n}.wrapping_add(4); "
                f"mac = mac.wrapping_add(x * y); if cpu.sh2.sr & 2 != 0 {{ mac = mac.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff); }} }}")
    if mn == "movt":
        return f"{a[0]} = t as u32;"
    if mn == "rotr":
        return f"t = {a[0]} & 1 != 0; {a[0]} = {a[0]}.rotate_right(1);"
    if mn == "shal":
        return f"t = {a[0]} >> 31 != 0; {a[0]} <<= 1;"
    if mn == "addc":
        return f"{{ let (x, c1) = {a[1]}.overflowing_add({a[0]}); let (y, c2) = x.overflowing_add(t as u32); {a[1]} = y; t = c1 || c2; }}"
    if mn == "negc":
        return f"{{ let (x, b1) = 0u32.overflowing_sub({a[0]}); let (y, b2) = x.overflowing_sub(t as u32); {a[1]} = y; t = b1 || b2; }}"
    if mn == "mulu.w":
        return f"mac = (mac & !0xffff_ffff) | ((({a[0]} & 0xffff) * ({a[1]} & 0xffff)) as i64 & 0xffff_ffff);"
    if mn == "muls.w":
        return f"mac = (mac & !0xffff_ffff) | ((({a[0]} as i16 as i32).wrapping_mul({a[1]} as i16 as i32)) as u32 as i64);"
    if mn == "cmp/str":
        return f"{{ let x = {a[0]} ^ {a[1]}; t = x & 0xff == 0 || x & 0xff00 == 0 || x & 0xff_0000 == 0 || x & 0xff00_0000 == 0; }}"
    if mn in ("mov.l", "mov.w", "mov.b") and a[1].startswith("@-r"):
        n, (k, wr, ty) = a[1][2:], {"mov.l": (4, "write32", ""), "mov.w": (2, "write16", " as u16"), "mov.b": (1, "write8", " as u8")}[mn]
        return f"{n} = {n}.wrapping_sub({k}); cpu.{wr}({n}, {a[0]}{ty});"
    if mn == "stc" and a[0] == "gbr":
        return f"{a[1]} = w;"
    if mn == "sts.l" and args == "pr,@-r15":
        return "r15 = r15.wrapping_sub(4); cpu.write32(r15, pr);"
    if mn == "lds.l" and args == "@r15+,pr":
        return "pr = cpu.read32(r15); r15 = r15.wrapping_add(4);"
    if mn == "lds" and a[1] == "pr":
        return f"pr = {a[0]};"
    if mn == "sts" and a[0] == "pr":
        return f"{a[1]} = pr;"
    return None


if __name__ == "__main__":
    main()
