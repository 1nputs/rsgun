"""Dump an SH COFF object's symbols as C-like declarations: coffdump.py IMAGE OFFSET [--raw | --header].

--header writes the types as a C header a parser takes (Ghidra's: tools/ghidra/project.py): names
without the leading underscore COFF adds, anonymous tags named anonN, the symbols left out; each
struct's size stays in a comment after it, to check the parser's layout against.
"""
import re, sys, struct
d = open(sys.argv[1], "rb").read()
base = int(sys.argv[2], 16)
raw = "--raw" in sys.argv
header = "--header" in sys.argv
magic, nscns, tdat, symptr, nsyms, opthdr, flags = struct.unpack(">HHIIIHH", d[base:base + 20])
secs = []
for i in range(nscns):
    s = base + 20 + opthdr + 40 * i
    nm, pa, va, size, rawp, relp, lnp, nrel, nln, fl = struct.unpack(">8sIIIIIIHHI", d[s:s + 40])
    secs.append((nm.rstrip(b"\0").decode(), size, rawp, relp, nrel, lnp, nln))
    print(f"/* section {secs[-1][0]} size={size:#x} raw@{rawp:#x} relocs={nrel} lines={nln} */")
st = base + symptr
strtab = base + symptr + nsyms * 18
def sname(e):
    if e[:4] == b"\0\0\0\0":
        off = struct.unpack(">I", e[4:8])[0]
        return d[strtab + off: d.index(b"\0", strtab + off)].decode("latin1")
    return e[:8].split(b"\0")[0].decode("latin1")
syms = []
i = 0
while i < nsyms:
    e = d[st + 18 * i: st + 18 * i + 18]
    name = sname(e)
    val, scn, typ, scl, naux = struct.unpack(">IhHBB", e[8:18])
    aux = [d[st + 18 * (i + 1 + k): st + 18 * (i + 2 + k)] for k in range(naux)]
    syms.append((i, name, val, scn, typ, scl, aux))
    i += 1 + naux
byidx = {s[0]: s for s in syms}
BASE = ["?", "void", "char", "short", "int", "long", "float", "double", "struct", "union", "enum", "moe",
        "unsigned char", "unsigned short", "unsigned int", "unsigned long"]
def tname(typ, aux):
    b = typ & 0xF
    t = BASE[b]
    if b in (8, 9, 10) and aux:
        tag = struct.unpack(">I", aux[0][0:4])[0]
        t = f"{t} {byidx[tag][1] if tag in byidx else '#%d' % tag}"
    decl, dims, k = "", [], 0
    if aux:
        dims = [x for x in struct.unpack(">4H", aux[0][8:16]) if x]
    ds = []
    t2 = typ >> 4
    while t2:
        ds.append(t2 & 3); t2 >>= 2
    return t, ds, dims
def fmt(name, typ, aux):
    t, ds, dims = tname(typ, aux)
    s = name
    di = 0
    for dk in ds:
        if dk == 1: s = "*" + s
        elif dk == 2: s = f"({s})()" if s.startswith("*") else f"{s}()"
        elif dk == 3:
            s = f"({s})" if s.startswith("*") else s
            s += f"[{dims[di] if di < len(dims) else ''}]"; di += 1
    return f"{t} {s}"
SCL = {1: "auto", 2: "extern", 3: "static", 4: "register", 8: "member", 9: "arg", 10: "struct", 11: "umember",
       12: "union", 13: "typedef", 15: "enum", 16: "enumval", 18: "field", 100: "block", 101: "fcn", 102: "eos", 103: "file", 6: "label", 17: "regparm"}
if header:
    _print = print
    def unmangle(text):
        text = re.sub(r"\.(\d+)fake", r"anon\1", text)
        return re.sub(r"(?<![\w.])_(?=[A-Za-z_])", "", text)
    def print(line=""):
        if line.startswith(("/* file", "/* section", "/* extern", "/* static")):
            return
        line = re.sub(r"= (\d+),$", lambda m: f"= {struct.unpack('>i', struct.pack('>I', int(m.group(1))))[0]},", line)
        _print(unmangle(line).replace("? ", "int "))
    _print("/* The types of the cart's leftover dma.c object (tools/rom/coffdump.py --header). */")
for (i, name, val, scn, typ, scl, aux) in syms:
    k = SCL.get(scl, str(scl))
    if raw:
        print(f"{i:5d} {k:9s} scn={scn:3d} val={val:#010x} typ={typ:#06x} {name}")
        continue
    if k in ("struct", "union", "enum"):
        print(f"{k} {name} {{")
    elif k in ("member", "umember"):
        print(f"    /* +{val:#05x} */ {fmt(name, typ, aux)};")
    elif k == "field":
        print(f"    /* bit {val} */ {fmt(name, typ, aux)} : {struct.unpack('>H', aux[0][6:8])[0] if aux else '?'};")
    elif k == "enumval":
        print(f"    {name} = {val},")
    elif k == "eos":
        size = struct.unpack(">H", aux[0][6:8])[0] if aux else 0
        print(f"}}; /* size {size:#x} */")
    elif k == "typedef":
        print(f"typedef {fmt(name, typ, aux)};")
    elif k == "file":
        print(f"/* file {aux[0].split(b'\\0')[0].decode() if aux else name} */")
    elif k in ("extern", "static") and name not in (".text", ".data", ".bss"):
        sec = {0: "undef", -1: "abs", -2: "debug"}.get(scn, secs[scn - 1][0] if 0 < scn <= len(secs) else scn)
        print(f"/* {k} {sec}+{val:#x} */ {fmt(name, typ, aux)};")
