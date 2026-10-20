# rsgun

Radiant Silvergun (Sega ST-V, 1998) reimplemented natively in Rust, verified against an emulator,
the way `md-recomp` did NHL '94 and Gunstar Heroes for the Mega Drive. Early days: the runtime crate
`stv` has its SH-2 core, checked against the oracle's traces; the rest of this repository is tooling
(the oracle, Ghidra, the ROM tools). The plan and the living guide are in `../playbook/rsgun/`.

```
crates/stv          the ST-V as a library (no game knowledge): so far the SH-2 core
crates/stv-check    checks the runtime against the oracle's records
tools/              the oracle, traces, ROM and Ghidra tools (Python and shell)
```

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
| `cargo test` | the runtime's unit tests |
| `tools/trace/check_all.sh` | records 19 trace windows from power on to attract play and replays each through the SH-2 core (`stv-check trace`); fails on any difference |
| `tools/oracle/run.py --frames N --dump F,... --trace F:N,...` | runs the game on the oracle headless; dumps RAM, VDP memory, registers and the screen at frames F; traces both SH-2s through windows of frames |
| `target/release/stv-check trace DIR` | replays one trace through the SH-2 core |
| `tools/trace/show.py TRACE [--from N --count N]` | summarises or prints a trace |
| `tools/survey/loadmap.py DUMP` | which parts of work RAM are copies of the cart |
| `tools/ghidra/decomp.py IMAGE OFF SIZE BASE FUNC...` | Ghidra's C for functions in a raw image |
| `tools/rom/dis.py IMAGE OFF BASE COUNT` | SH-2 disassembly with literals resolved |
| `tools/rom/coffscan.py`, `coffdump.py`, `textruns.py`, `blockmap.py` | find and read what the cart's padding holds |

The oracle is Mednafen 1.32.1 (GPL-2.0+), patched by `tools/patches/mednafen/` (`mkpatch.sh` remakes
the patch from `ref/`): a development tool, never shipped or linked into the port. The port is MIT.

Setup also needs Rust (stable, 2024 edition).
