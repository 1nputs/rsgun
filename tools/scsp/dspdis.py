"""The DSP program, coefficients and addresses a sound log had set up at a sample, one step a line.

    python3 tools/scsp/dspdis.py WRITES.bin SAMPLE
"""
import struct
import sys


def main():
    log = open(sys.argv[1], "rb").read()
    at = int(sys.argv[2])
    regs = bytearray(0x1000)
    for i in range(0, len(log), 16):
        s, tag, size, a, v = struct.unpack_from(">IBBxxII", log, i)
        if s > at:
            break
        if tag == ord("R"):
            regs = bytearray(0x1000)
        if tag != ord("W") or a < 0x100000:
            continue
        r = a & 0xFFF
        if size == 1:
            regs[r] = v
        else:
            regs[r & ~1], regs[(r & ~1) + 1] = v >> 8, v & 0xFF
    w = lambda a: (regs[a] << 8) | regs[a + 1]
    coef = [w(0x700 + 2 * i) for i in range(64)]
    madrs = [w(0x780 + 2 * i) for i in range(32)]
    print("RBP", hex(w(0x402) & 0x7F), "RBL", (w(0x402) >> 7) & 3)
    print("COEF", " ".join(f"{(c >> 3) if c < 0x8000 else (c >> 3) - 8192}" for c in coef))
    print("MADRS", " ".join(f"{m:04x}" for m in madrs))
    for st in range(128):
        i = [w(0x800 + 8 * st + 2 * k) for k in range(4)]
        if not any(i):
            continue
        f = dict(TRA=(i[0] >> 8) & 0x7F, TWT=(i[0] >> 7) & 1, TWA=i[0] & 0x7F, XSEL=(i[1] >> 15) & 1, YSEL=(i[1] >> 13) & 3,
                 IRA=(i[1] >> 6) & 0x3F, IWT=(i[1] >> 5) & 1, IWA=i[1] & 0x1F, TABLE=(i[2] >> 15) & 1, MWT=(i[2] >> 14) & 1,
                 MRD=(i[2] >> 13) & 1, EWT=(i[2] >> 12) & 1, EWA=(i[2] >> 8) & 0xF, ADRL=(i[2] >> 7) & 1, FRCL=(i[2] >> 6) & 1,
                 SHIFT=(i[2] >> 4) & 3, YRL=(i[2] >> 3) & 1, NEGB=(i[2] >> 2) & 1, ZERO=(i[2] >> 1) & 1, BSEL=i[2] & 1,
                 NOFL=(i[3] >> 15) & 1, COEF=(i[3] >> 9) & 0x3F, MASA=(i[3] >> 2) & 0x1F, ADREB=(i[3] >> 1) & 1, NXADR=i[3] & 1)
        shown = " ".join(f"{k}={v}" for k, v in f.items() if v or k in ("TRA", "IRA", "XSEL", "YSEL"))
        print(f"{st:3d}: {' '.join(f'{x:04x}' for x in i)}  {shown}")


main()
