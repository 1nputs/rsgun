"""What a sound log had set up at a sample: each slot's registers and key state, the common
registers, whether the DSP has a program.

    python3 tools/scsp/logstate.py WRITES.bin SAMPLE [--keys]

--keys lists every key-on execution (KYONEX) up to the sample with the slots it keyed on.
"""
import struct
import sys

FIELDS = ["sbctl ssctl lpctl pcm8 sa_hi", "sa", "lsa", "lea", "d2r d1r eghold ar", "lpslnk krs dl rr",
          "stwinh sdir tl", "mdl mdxsl mdysl", "oct fns", "lfore lfof plfows plfos alfows alfos", "isel imxl",
          "disdl dipan efsdl efpan"]


def decode(w):
    oct_ = ((w[8] >> 11) & 0xF)
    oct_ = oct_ - 16 if oct_ & 8 else oct_
    return (f"SA {((w[0] & 0xF) << 16) | w[1]:05x} {'8' if w[0] & 0x10 else '16'}bit LP{(w[0] >> 5) & 3} "
            f"SS{(w[0] >> 7) & 3} SB{(w[0] >> 9) & 3} LSA {w[2]:04x} LEA {w[3]:04x} | AR {w[4] & 31} D1R {(w[4] >> 6) & 31} "
            f"D2R {w[4] >> 11} HOLD {(w[4] >> 5) & 1} RR {w[5] & 31} DL {(w[5] >> 5) & 31} KRS {(w[5] >> 10) & 15} "
            f"LNK {(w[5] >> 14) & 1} | TL {w[6] & 255} SDIR {(w[6] >> 8) & 1} STWINH {(w[6] >> 9) & 1} | "
            f"MDL {w[7] >> 12} X {(w[7] >> 6) & 63} Y {w[7] & 63} | OCT {oct_} FNS {w[8] & 0x3FF:03x} | "
            f"LFO re{w[9] >> 15} f{(w[9] >> 10) & 31} p{(w[9] >> 8) & 3}/{(w[9] >> 5) & 7} a{(w[9] >> 3) & 3}/{w[9] & 7} | "
            f"ISEL {(w[10] >> 3) & 15} IMXL {w[10] & 7} | DISDL {w[11] >> 13} DIPAN {(w[11] >> 8) & 31:02x} "
            f"EFSDL {(w[11] >> 5) & 7} EFPAN {w[11] & 31:02x}")


def main():
    log = open(sys.argv[1], "rb").read()
    at = int(sys.argv[2])
    keys = "--keys" in sys.argv
    regs = bytearray(0x1000)
    keyed = [None] * 32  # sample of the last key on, or None if off
    for i in range(0, len(log), 16):
        s, tag, size, a, v = struct.unpack_from(">IBBxxII", log, i)
        if s > at:
            break
        if tag == ord("R"):
            regs = bytearray(0x1000)
            keyed = [None] * 32
            continue
        if tag != ord("W") or a < 0x100000:
            continue
        r = a & 0xFFF
        if size == 1:
            regs[r] = v
        else:
            regs[r & ~1] = v >> 8
            regs[(r & ~1) + 1] = v & 0xFF
        w0 = (regs[r & 0xFE0 & ~1] << 8) | regs[(r & 0xFE0) + 1] if r < 0x400 else 0
        if r < 0x400 and (r & 0x1F) in (0, 1) and w0 & 0x1000:
            on = []
            for n in range(32):
                kb = regs[0x20 * n] & 0x08
                if kb and keyed[n] is None:
                    keyed[n] = s
                    on.append(n)
                elif not kb:
                    keyed[n] = None
            if keys and on:
                print(f"{s:9d} key on {on}")
            regs[r & 0xFE0] &= 0xEF
    print(f"at sample {at}: MVOL {regs[1] & 15} RBP {regs[0x403] & 0x7F:02x} RBL {(regs[0x402] << 8 | regs[0x403]) >> 7 & 3} "
          f"DSP program {'yes' if any(regs[0x800:0xC00]) else 'no'}")
    for n in range(32):
        w = [(regs[0x20 * n + 2 * k] << 8) | regs[0x20 * n + 2 * k + 1] for k in range(12)]
        if keyed[n] is not None:
            print(f"slot {n:2d} keyed at {keyed[n]}: {decode(w)}")


main()
