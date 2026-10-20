# rsgun

Radiant Silvergun (Sega ST-V, 1998) reimplemented natively, verified against an emulator, the way
`md-recomp` did NHL '94 and Gunstar Heroes for the Mega Drive. Early days: this repository holds the
tooling (the oracle, Ghidra, the ROM tools); the plan and the living guide are in `../playbook/rsgun/`.

You need your own copy of MAME's `rsgun` set and the ST-V BIOS set (`rsgun.zip`, `stvbios.zip`), in
`roms/` here or in `../roms/`. Nothing derived from them is committed.

## Setup

```
tools/setup_venv.sh                    # .venv: Capstone, Pillow
tools/fetch_refs.sh                    # ref/: Mednafen with the oracle hooks, a JDK 21, Ghidra 12.1.4, PyGhidra
.venv/bin/python tools/rom/mkcart.py   # build/rom/: the cart as the SH-2 sees it, the BIOS images (SHA-1 checked)
```

Needs curl, unzip, a C++ compiler, SDL2 and ALSA development headers.

## Tools

| Command | What it does |
|---|---|
| `tools/oracle/run.py --frames N --dump F,...` | runs the game on the oracle headless; dumps RAM, VDP memory, registers and the screen at frames F |
| `tools/survey/loadmap.py DUMP` | which parts of work RAM are copies of the cart |
| `tools/ghidra/decomp.py IMAGE OFF SIZE BASE FUNC...` | Ghidra's C for functions in a raw image |
| `tools/rom/dis.py IMAGE OFF BASE COUNT` | SH-2 disassembly with literals resolved |
| `tools/rom/coffscan.py`, `coffdump.py`, `textruns.py`, `blockmap.py` | find and read what the cart's padding holds |

The oracle is Mednafen 1.32.1 (GPL-2.0+), patched by `tools/patches/mednafen/`: a development tool,
never shipped or linked into the port.
