# rsgun

Radiant Silvergun (Sega ST-V, 1998) reimplemented natively in Rust, verified against an emulator,
the way `md-recomp` did NHL '94 and Gunstar Heroes for the Mega Drive. So far the runtime crate `stv`
is a whole ST-V machine (two SH-2s, the 68000, SCU, SMPC, VDP1, VDP2, the SCSP with its synthesis
and effect DSP), checked against the oracle, that runs the game from power on through the real BIOS:
`rsgun` plays it in a window, with sound. The port runs the game's code as Rust in place of the SH-2
code (`--native`): a mechanical translation of every function the evidence saw, generated from your
ROM into `build/port` and never committed, being replaced function by function with a rewrite meant
to be read (`crates/rsgun/src/game`: the game's own names where its leftovers give them, its state
named). Every call of either is checked against the SH-2 code it replaces (`tools/port/check.sh`).
The plan and the living guide are in `../playbook/rsgun/`.

```
crates/stv          the ST-V as a library (no game knowledge)
crates/rsgun        the game in a window: controls, saves, input scripts; the rewrite (src/game)
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

## Playing

```
cargo run --release -p rsgun            # a window; the first time, the BIOS takes about 19 seconds (hold Tab to hurry it)
cargo run --release -p rsgun -- --native   # the same, the game's code running as Rust (tools/port)
```

| Keys | | Pad | |
|---|---|---|---|
| arrows | move | d-pad, left stick | move |
| Z, X, C | A (vulcan), B (homing), C (spread) | south, west, north | A, B, C |
| A, S, D | A+B, A+C, B+C | shoulders, east | A+B, A+C; B+C |
| space | A+B+C | right trigger | A+B+C |
| 5, 6 | coin 1, 2 | back | coin |
| 1, 2 | start 1, 2 | start | start |
| F2, 9 | test, service | | |
| P, Tab, F11, F12, Esc | pause, fast while held, full screen, screenshot, quit | | |

The settings EEPROM and backup RAM are kept in `~/.local/share/rsgun/` (`--save DIR`), and with them
the boot state: the machine as the BIOS hands over to the game, saved the first time a run gets
there, so later runs start at the game (`--cold` boots from power on). `--script FILE`
plays an input script instead (`crates/rsgun/src/controls.rs`); `--headless --frames N --shot F.ppm
--wav F.wav` runs without a window; `--mute` plays no sound; `--cpu-speed N` runs the SH-2s N times
as fast as the hardware (no slowdown where the game runs out of time). With a 59-61 Hz display the
game runs a frame a refresh; otherwise by the clock at 59.94.

## Tools

| Command | What it does |
|---|---|
| `cargo test` | the runtime's unit tests |
| `tools/port/check_native.sh` | play1 and the attract mode natively and on the SH-2 code (CPUs 16 times as fast, so neither slows down): the same last pictures |
| `tools/ghidra/project.py` | the Ghidra project of the port's functions with the leftover dma.c's types, and Ghidra's C for each in `build/ghidra/c` |
| `tools/port/calls.py ADDR` | a function's callers and callees, from the evidence |
| `tools/port/check.sh` | translates the game's functions to Rust (`tools/port/sh2rust.py`, into `build/port`) and checks every call the port has against the SH-2 code, over play1 and the attract mode |
| `target/release/stv-check state --frames N --at F,F` | save states: a machine made from the state at each frame F must stay equal to the one it was saved from |
| `tools/trace/check_spin.sh` | a machine that plays the CPUs' wait loops beside one that executes everything: equal after every frame |
| `tools/scsp/probes.py` | crafted one-voice SCSP logs run on the oracle's SCSP (`tools/scsp/build.sh` builds it) and on ours: every sample must match |
| `target/release/stv-check scsp LOG` | replays a sound log (`run.py --sound N`) through the SCSP against the oracle's samples |
| `tools/vdp1/probes.py` | crafted VDP1 command lists drawn by the oracle's VDP1 (`tools/vdp1/build.sh` builds it) and by ours: every pixel must match |
| `tools/trace/check_vdp2.sh` | VDP2's composition of the oracle's own state, pixel for pixel against its pictures |
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
