"""Disassemble SH-2 code from a big-endian image, with PC-relative literals resolved.

    python3 tools/rom/dis.py IMAGE FILE_OFFSET LOAD_ADDRESS COUNT

FILE_OFFSET and LOAD_ADDRESS are hex: the instruction at FILE_OFFSET is shown at LOAD_ADDRESS.
Needs Capstone 5 (tools/setup_venv.sh).
"""
import struct
import sys

import capstone as cs


def main() -> None:
    data = open(sys.argv[1], "rb").read()
    off, base, count = int(sys.argv[2], 16), int(sys.argv[3], 16), int(sys.argv[4])
    md = cs.Cs(cs.CS_ARCH_SH, cs.CS_MODE_BIG_ENDIAN | cs.CS_MODE_SH2)

    def at(addr: int, size: int):
        o = addr - base + off
        if 0 <= o <= len(data) - size:
            return int.from_bytes(data[o:o + size], "big")
        return None

    for k in range(count):
        o = off + 2 * k
        pc = base + 2 * k
        w = struct.unpack(">H", data[o:o + 2])[0]
        insn = next(md.disasm(data[o:o + 2], pc), None)
        text = f"{insn.mnemonic} {insn.op_str}" if insn else f".word 0x{w:04x}"
        note = ""
        if w >> 12 == 0xD:                          # mov.l @(disp,PC),Rn
            lit = (pc & ~3) + 4 + (w & 0xFF) * 4
            v = at(lit, 4)
            note = f"[{lit:08x}] = {v:08x}" if v is not None else ""
        elif w >> 12 == 0x9:                        # mov.w @(disp,PC),Rn
            lit = pc + 4 + (w & 0xFF) * 2
            v = at(lit, 2)
            note = f"[{lit:08x}] = {v:04x}" if v is not None else ""
        elif w >> 8 == 0xC7:                        # mova @(disp,PC),R0
            note = f"= {(pc & ~3) + 4 + (w & 0xFF) * 4:08x}"
        print(f"{pc:08x}: {w:04x}  {text:28s}{'; ' + note if note else ''}")


if __name__ == "__main__":
    main()
