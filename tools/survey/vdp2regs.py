"""Decodes VDP2's registers from a display dump (DIR/display/vdp2-regs.bin): what each screen is set
to, to see which features a game uses.

    .venv/bin/python tools/survey/vdp2regs.py DUMP_DIR...
"""
import pathlib
import struct
import sys


def main():
    for d in sys.argv[1:]:
        r = struct.unpack(">256H", (pathlib.Path(d) / "display/vdp2-regs.bin").read_bytes())
        reg = lambda off: r[off // 2]
        tvmd, bgon, chctla, chctlb = reg(0), reg(0x20), reg(0x28), reg(0x2A)
        on = [n for i, n in enumerate(["NBG0", "NBG1", "NBG2", "NBG3", "RBG0", "RBG1"]) if bgon & (1 << i)]
        transp = [n for i, n in enumerate(["NBG0", "NBG1", "NBG2", "NBG3", "RBG0"]) if bgon & (1 << (8 + i))]
        depth = lambda v: ["4bpp", "8bpp", "11bit", "RGB15", "RGB24", "?", "?", "?"][v]
        print(f"== {d}")
        print(f"  TVMD {tvmd:04x}: display {'on' if tvmd & 0x8000 else 'off'} hreso {tvmd & 7} vreso {(tvmd >> 4) & 3} lsmd {(tvmd >> 6) & 3}")
        print(f"  BGON {bgon:04x}: on {on}, transparent-code display {transp}")
        print(f"  NBG0 {depth((chctla >> 4) & 7)} {'bitmap' if chctla & 2 else 'cell'} chsz {chctla & 1}; NBG1 {depth((chctla >> 12) & 3)} {'bitmap' if chctla & 0x200 else 'cell'} chsz {(chctla >> 8) & 1}")
        print(f"  NBG2 {depth((chctlb >> 1) & 1)} chsz {chctlb & 1}; NBG3 {depth((chctlb >> 5) & 1)} chsz {(chctlb >> 4) & 1}; RBG0 {depth((chctlb >> 12) & 7)} {'bitmap' if chctlb & 0x200 else 'cell'} chsz {(chctlb >> 8) & 1}")
        print(f"  PNCN0-3 {reg(0x30):04x} {reg(0x32):04x} {reg(0x34):04x} {reg(0x36):04x} PNCR {reg(0x38):04x} PLSZ {reg(0x3A):04x} MPOFN {reg(0x3C):04x} MPOFR {reg(0x3E):04x}")
        print(f"  scroll N0 {reg(0x70):04x}.{reg(0x72):04x},{reg(0x74):04x}.{reg(0x76):04x} zoom {reg(0x78):04x}.{reg(0x7A):04x},{reg(0x7C):04x}.{reg(0x7E):04x}; N1 {reg(0x80):04x},{reg(0x84):04x} zoom {reg(0x88):04x}.{reg(0x8A):04x}; N2 {reg(0x90):04x},{reg(0x92):04x} N3 {reg(0x94):04x},{reg(0x96):04x}")
        print(f"  ZMCTL {reg(0x98):04x} SCRCTL {reg(0x9A):04x} (line/vcell scroll) VCSTA {reg(0x9C):04x}{reg(0x9E):04x} LSTA0 {reg(0xA0):04x}{reg(0xA2):04x} LSTA1 {reg(0xA4):04x}{reg(0xA6):04x}")
        print(f"  LCTA {reg(0xA8):04x}{reg(0xAA):04x} BKTA {reg(0xAC):04x}{reg(0xAE):04x} RPMD {reg(0xB0):04x} RPRCTL {reg(0xB2):04x} KTCTL {reg(0xB4):04x} RPTA {reg(0xBC):04x}{reg(0xBE):04x}")
        print(f"  windows WCTLA-D {reg(0xD0):04x} {reg(0xD2):04x} {reg(0xD4):04x} {reg(0xD6):04x} W0 {reg(0xC0)},{reg(0xC2)}-{reg(0xC4)},{reg(0xC6)} W1 {reg(0xC8)},{reg(0xCA)}-{reg(0xCC)},{reg(0xCE)} LWTA0 {reg(0xD8):04x}{reg(0xDA):04x}")
        print(f"  SPCTL {reg(0xE0):04x} SDCTL {reg(0xE2):04x} CRAOFA {reg(0xE4):04x} CRAOFB {reg(0xE6):04x} LNCLEN {reg(0xE8):04x} SFPRMD {reg(0xEA):04x} CCCTL {reg(0xEC):04x} SFCCMD {reg(0xEE):04x}")
        print(f"  PRIS {reg(0xF0):04x} {reg(0xF2):04x} {reg(0xF4):04x} {reg(0xF6):04x} PRIN {reg(0xF8):04x} {reg(0xFA):04x} PRIR {reg(0xFC):04x}")
        print(f"  CCRS {reg(0x100):04x} {reg(0x102):04x} {reg(0x104):04x} {reg(0x106):04x} CCRN {reg(0x108):04x} {reg(0x10A):04x} CCRR {reg(0x10C):04x} CCRLB {reg(0x10E):04x}")
        print(f"  CLOFEN {reg(0x110):04x} CLOFSL {reg(0x112):04x} COA {reg(0x114):04x} {reg(0x116):04x} {reg(0x118):04x} COB {reg(0x11A):04x} {reg(0x11C):04x} {reg(0x11E):04x} MZCTL {reg(0x22):04x} SFSEL {reg(0x24):04x}")
        tvmr, fbcr = (pathlib.Path(d) / "display/vdp1-regs.bin").read_bytes()
        print(f"  VDP1 TVMR {tvmr:02x} FBCR {fbcr:02x}")


if __name__ == "__main__":
    main()
