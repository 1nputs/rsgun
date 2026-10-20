"""Find SH COFF object headers (big-endian, magic 0x0500) in an image and summarise each."""
import sys, struct
d = open(sys.argv[1], "rb").read()
found = []
for o in range(0, len(d) - 20, 2):
    if d[o:o + 2] != b"\x05\x00":
        continue
    magic, nscns, tdat, symptr, nsyms, opthdr, flags = struct.unpack(">HHIIIHH", d[o:o + 20])
    if not (1 <= nscns <= 12 and 0 < symptr < 0x200000 and 0 < nsyms < 50000 and opthdr in (0, 28)):
        continue
    sh = o + 20 + opthdr
    names = []
    ok = True
    for i in range(nscns):
        nm = d[sh + 40 * i: sh + 40 * i + 8].rstrip(b"\0")
        if not nm.startswith(b".") or not nm[1:].isalnum():
            ok = False; break
        names.append(nm.decode())
    if not ok:
        continue
    st = o + symptr + nsyms * 18
    strsz = struct.unpack(">I", d[st:st + 4])[0] if st + 4 <= len(d) else 0
    first = d[st + 4: st + 40].split(b"\0")[0]
    found.append(o)
    print(f"{o:07x} nscns={nscns} sects={names} symptr={symptr:#x} nsyms={nsyms} opthdr={opthdr} flags={flags:#x} strtab@{st:07x} size={strsz:#x} first={first[:30]!r}")
