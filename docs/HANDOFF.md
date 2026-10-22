# rsgun (Radiant Silvergun, ST-V): working guide and handoff

Read this first. `PLAN.md` beside it has the owner's decisions and the phases, `SURVEY.md` what the
game asks of the hardware; the method is md-recomp's playbook (`md-recomp/playbook/PLAYBOOK.md`, a
sibling project: not needed to carry on). Paths are from the repository's root.

**Now (22 October 2026, afternoon).** Everything the game runs is readable Rust now but its stage
overlays (and, on purpose, the .control interrupt layer and the BIOS's own services). The work is
split as docs/AGENTS.md describes: this session orchestrates, two Sonnet agents do one batch each.

Overlays (fingerprint, game mode, marathon segments, worktree and branch, where it stands):
- TITLE, OPENING, STG00: done.
- STG01 (259ec12a68feaeb3, stage 3, f000000-f060000 and f200000; ../rsgun-stg01, wip-stg01): done.
  Every function the marathon runs is rewritten. Left to the SH-2 code (never run): 0x0608e044,
  0x0609a888, 0x0609b8a4, 0x0609b8d0, 0x0609c400, 0x0609cf60, 0x0609d124, 0x0609d25c,
  0x0609d2a4, 0x0609d83c, 0x0609d984, 0x0609db48, 0x0609dc58, 0x0609ddb4, 0x0609de74,
  0x0609e0d8, 0x0609e2e8, 0x0609ee24, 0x0609ef70 and the never-called ones.
- STG02 (1a653c2164fc4f52, f060000-f140000; ../rsgun-stg02, wip-stg02): every function that runs
  is rewritten (last runs out/port/stg02-33, all nine segments equal). Held back (never run):
  0x060a19d0-0x060a2504 (the grid scene's other half), 0x060a282c, 0x060a2834; and, uncommitted
  in the worktree, stage_boss_event 0x06097610, last_part_lost 0x060979cc, twelve mid-boss steps
  0x0608a054-0x0608b0c0.
- STG04 (5c18364ee98aacb1, game mode 10, f150000-f170000; ../rsgun-stg04, wip-stg04): every
  function that runs is rewritten (last runs out/port/stg04r-13). Held back (never run):
  0x06082520, 0x060828f4, 0x06084bc4, 0x060877b0, 0x060877bc.
- STG05 (2adefae5540dd556, game mode 11, f170000-f200000; ../rsgun-stg05, wip-stg05): every
  function that runs is rewritten (last runs out/port/stg05-19). Held back (never run):
  0x060894b0, 0x0608a510, 0x0608d104, 0x0608d48c.
- RANK (4635b7419f6fc1a7, the ranking after a game ends, f200000: 16 functions run; the
  marathon reports label many of them with STG05's fingerprint; ../rsgun-rank, wip-rank):
  scaffolded (rank.rs, a marker in mod.rs).
- STG03 (game mode 9) is stage 4: STG01's end asks "SELECT NEXT STAGE" (stage 2 or 4; up or down
  turns it, a button or 300 frames takes it) and the marathon took stage 2 (its weaving turned it
  at random). out/evidence/route4-input.txt is the marathon's input with the controls let go from
  65500 and down pressed at 66900 (stage 4 taken, STG03 loaded at 67050); its run from the
  f060000 state (evidence out/evidence/route4.txt, states out/evidence/route4-states every 10000
  frames from 70000) is STG03's evidence. It is also the attract mode's third demo (mode 14).
The whole-game check on main 96b4bf4 (out/port/marathon-96b4bf4): all 21 segments every call equal;
still run unrewritten there: RANK's 16 and STG02's 0x06097610 (f140000) and 0x060a19d0-0x060a2504
(f150000, before STG04 loads), both in hand.
Check each overlay's name with `states::overlay_name` before registering (a block registered
under the wrong name never runs; its report shows only `f_...` names). The marathon reports in
out/port/marathon-898fd01 label an address by one overlay's fingerprint only: in a segment where
two overlays run, check your own compare runs' reports. How each function is done: section 6,
step 1, "The overlay loop". The branch is main.

Before the overlays (history in `git log`): the main image's game code, the game's part of
.control but its interrupt layer, SGL's library and core, and Sega's sound library's set-up were
rewritten and checked; after the BIOS's handover a native run interprets nothing of the game's or
SGL's code (section 6, step 4). The last whole-game check before the overlay work
(`check_marathon.sh 7`, reports in out/port/marathon-898fd01, the reference the overlay work
reads) had every call equal in all 21 segments. The native check runs play1 and the attract mode
at `--cpu-speed 16` against the interpreter's pictures. `check.sh` is the quick gate on main.
Left besides the overlays: the BIOS's services (the machine's code, not the game's), and a
readable pass over the routines asm2sm.py generated (the slave's polygon pipeline, the VDP
set-up). When the overlays are done, run the whole-game check again on main.

## 1. Layout and constraints

```
roms/            rsgun.zip and stvbios.zip (MAME sets), here or beside the checkout; in no repository
docs/            these notes
crates/, tools/  the runtime, the game, the tools
ref/, build/, out/, .venv/, target/   fetched, built or ROM-derived: git-ignored
```

- Nothing derived from the ROM is committed: images, dumps, the leftovers in `build/leftovers/`
  (Treasure's source and debug information) stay in `build/` and `out/`. The leftovers are a
  reference for names and types, not text to copy into the port.
- The oracle (Mednafen, GPL-2.0+) is a development tool: never shipped, never linked into the port.
- Rust (a Cargo workspace), MIT; the SH-2 interpreter scaffold; smooth 60; one agent at a time
  (`PLAN.md`, the owner's decisions of 19 October).
- Commits name no person or machine (the repository's identity is `1nputs`); `Co-Authored-By` as in
  md-recomp, no session links.

## 2. Setup from a clean checkout

`tools/setup_debian.sh` does all of this on Debian (and Ubuntu): the packages, Rust, the steps
below, the translation and the overlays' disassembly. By hand:

```
cd rsgun                               # with roms/ (or ../roms/) holding rsgun.zip and stvbios.zip
tools/setup_venv.sh                    # .venv: Capstone, Pillow
tools/fetch_refs.sh                    # ref/: Mednafen 1.32.1 + oracle patch, JDK 21, Ghidra 12.1.4, PyGhidra (~4 min)
.venv/bin/python tools/rom/mkcart.py   # build/rom/rsgun-cart.bin ($02000000), bios-*.bin; SHA-1 checked
.venv/bin/python tools/oracle/run.py --frames 7200 --dump 1800,7200   # out/oracle/: title, attract play
.venv/bin/python tools/oracle/eeprom.py   # build/rom/eeprom.bin: the settings EEPROM as the oracle formats it
cargo test                             # the runtime's unit tests
tools/trace/check_all.sh               # 19 trace windows replayed through the SH-2 and 68000 cores (~1 min, 260 MB)
.venv/bin/python tools/oracle/run.py --frames 3600 --hashes out/oracle/boot.hashes --out out/oracle/boot
target/release/stv-check boot out/oracle/boot.hashes --frames 3600 [--dump F,F]   # the machine from power on
```

Needs curl, unzip, g++, SDL2 and ALSA headers; no root. Downloads are pinned by SHA-256 (Mednafen
publishes none, so its pin is the hash of the first download).

### On another machine

The port's inputs are ROM-derived and in no repository; regenerating them takes hours, copying them
takes a minute. On the machine that has them, `tools/carry.sh` packs them into `out/carry.tar`
(about 200 MB): `roms/`, `out/evidence/` (`play1.txt`, `attract.txt`, `marathon.txt`,
`marathon-end.txt`: the evidence the translation is made from, with their `*.overlays/`, each
overlay image the run saw; `dumps/f003000/`, work RAM-H at frame 3000 of play1, the image sh2rust
and Ghidra read; `marathon-input.txt`, the marathon's input script; `marathon-states/`, 20 save
states, frames 10000-200000, for the compares; `marathon-140000.state`), the last whole-game
check's reports (`out/port/marathon-*/f*.txt`: which functions each segment calls),
`build/leftovers/` and `build/ghidra/c/`. On the new machine, in a fresh clone:
`tools/setup_debian.sh out/carry.tar` (or any path to it): packages, Rust, ref/, build/rom, the
translation, the build, the overlays' disassembly. Without the tarball, `--make-evidence` records
the evidence there instead (play1 and the attract mode in minutes; the marathon and its states
about 8 hours at the speeds below).

Then `tools/port/check.sh && tools/port/check_native.sh`, and `tools/port/check_marathon.sh JOBS`.
Times on the first machine (8 threads): check.sh 11 minutes (the 391k-line translation builds in
about 4), check_native 6, check_marathon about 45 minutes a segment with 6 at once (21 segments:
about 3 hours); the evidence runs 113-132 ms a frame (marathon.txt 4.7 hours, marathon-end.txt
2.5); the states run 45 minutes. The commands are in check.sh's and check_marathon.sh's headers.

## 3. Tools

- `tools/oracle/run.py`: the game on the oracle, headless (SDL dummy drivers, no sound, no frame
  skip, unthrottled: about 3× real time). `--dump F,...` writes `out/oracle/fFFFFFF/`: work RAM-L/H,
  VDP1 VRAM, VDP2 VRAM and CRAM, sound RAM (all big-endian), both SH-2s' registers, the screen (PPM
  and PNG). `--window` plays it in a window. Mednafen's home (settings, BIOS chips copied from
  `stvbios.zip`) is `out/mednafen-home`.
- `tools/patches/mednafen/`: `oracle.patch` (hook calls in `ss.cpp`, `debug.inc`, `sh7095.inc`,
  `sh7095_ops.inc`; accessors in `vdp2`, `sound`, `scsp.h`) and `oracle.inc` (the hooks themselves,
  copied in by `fetch_refs.sh`; its header specifies the dump and the trace format, version 3). To
  change the hooks: edit `oracle.inc` (or the tree under `ref/mednafen-1.32.1/src/ss/` for a call
  site), copy `oracle.inc` in, `make -j8 && make install` there, then `tools/patches/mednafen/mkpatch.sh`
  remakes `oracle.patch` and checks it applies to a fresh tree.
- Traces (`run.py --trace F:N,F:N`): per CPU, the debugger's step records (the PC, Mednafen's pipeline
  operation index, the instruction word), every data read and write with its value, exceptions with
  the saved PC, interrupt levels; `start/` is a dump from before the window. About 5 MB per CPU per
  frame. `tools/trace/show.py` prints them.
- `crates/stv`: the runtime (no game knowledge). `sh2/` the SH-2 core; `m68k/` the 68000 core; `machine.rs`
  the ST-V (bus map, DMA, interrupts, the line scheduler, frames ending where the oracle's do, memory
  hashes and dumps); `onchip.rs` each SH-2's modules; `scu.rs`; `smpc.rs` (commands, the clock, the
  ST-V's port wiring to the EEPROM and the sound resets); `eeprom.rs` (93C46); `scsp.rs` (sound RAM,
  registers, timers, interrupts; no synthesis); `video.rs` (VDP1/VDP2 memories and registers);
  `vdp1render.rs`, `vdp2render.rs` (the pictures); `spin.rs` (wait loops played instead of executed).
- `crates/rsgun`: the player (SDL2): keys and pads as the cabinet's controls (controls.rs, which also
  reads input scripts), sound through an SDL queue (audio.rs), saves in `~/.local/share/rsgun/`,
  `--headless --frames N --shot F.ppm --wav F.wav`. Keys are in the README. Runs start from the boot
  state (boot.rs: `boot.state` beside the saves, the machine at the last frame's end before the BIOS
  jumps to the game's entry 0x06004000, frame 1159), made by the first run that passes the handover
  with nothing pressed and made again when the ROMs or the EEPROM change; the backup RAM is put back
  from the saves after it is loaded. `--cold` boots from power on (so do `--evidence` and `--compare`,
  whose runs must cover the BIOS's code too). `--frames N` runs to frame N counted from power on, and
  scripts count from power on too.
- `rsgun --headless --load-state FILE`, `--save-state FILE` (at the end), `--states F,F` (into the
  `--dump-dir`, as fFFFFFF.state): runs from and to save states, e.g. the marathon's
  (`out/evidence/marathon-states/`), so compare and evidence runs can cover later stages without
  playing up to them again.
- Save states (stv::state, `Machine::save_state`, `load_state`): every part names its fields with
  `saved!`, the saved ones and the skipped ones (ROMs, caches, development tools); a field in neither
  list does not compile. About 4.5 MB, 1 ms to save. Not while a native call is in progress (its Rust
  stack is not state): in native play the game's main loop is always in one, so states in native play
  need a resumable point first. `stv-check state --frames N --at F,F` saves at each F, makes a second
  machine from it, and requires the two equal (states byte for byte, sound) every frame to N.
- VDP1: `tools/vdp1/build.sh` builds `build/vdp1_oracle`, the oracle's VDP1 linked on its own (its
  compiled objects from ref/, with stubs): `vdp1_oracle DIR` draws DIR/draw/ (a draw dump's layout)
  into fb-oracle.bin. `tools/vdp1/probes.py` is the suite (textures whose texels hold their own (u, v));
  `stv-check vdp1 DIR` compares ours with DIR/draw/fb-after.bin and writes a three-panel vdp1.ppm.
- Sound: `run.py --sound N --input SCRIPT` logs every write into the oracle's SCSP from power on
  (sample-stamped, with its resets) and its raw output (`out/.../sound/`). `tools/scsp/build.sh`
  builds `build/scsp_oracle`, Mednafen's SCSP on its own (GPL, a dev tool): it replays a log bit for
  bit, and dumps RAM and the register view (`SCSP_ORACLE_REGS=N:PATH`) to compare internals.
  `stv-check scsp LOG [SAMPLES]` replays a log through ours (`STV_SCSP_REGS`, `STV_SCSP_RAM`,
  `STV_SCSP_SAVE`, `STV_SCSP_DEBUG=N` for the same). `tools/scsp/probe.py` writes crafted logs;
  `tools/scsp/probes.py` is the suite (19 probes, all matching); `logstate.py` and `dspdis.py` show a
  log's slots and DSP program at a sample.
- `crates/stv-check`: `stv-check trace DIR` replays a trace through the cores (the SH-2s with every data
  read answered by the record, the 68000 matching each instruction's accesses in any order); fails at
  the first difference, printing the records around it. `stv-check boot HASHES` runs the machine from
  power on and compares each frame's memory hashes with the oracle's (`run.py --hashes`); `--dump`
  writes its memories for `tools/survey/memdiff.py`. `tools/trace/check_all.sh` records 19 windows in
  one run and checks them all. `stv-check spin` (`tools/trace/check_spin.sh`) runs a machine that plays
  the wait loops beside one that executes every instruction and requires them equal every frame
  (memories, CPUs, steps, timers): run it after touching `machine.rs`, `spin.rs` or the bus.
- Survey tools: `iomap.py` (every hardware register the traces touched), `memdiff.py` (two dumps' ranges
  that differ), `loadmap.py`.
- `tools/survey/loadmap.py DUMP`: which parts of work RAM are copies of the cart (about 1.5 minutes).
- `tools/ghidra/decomp.py IMAGE OFF SIZE BASE FUNC...`: Ghidra's C for functions in a raw image
  (a throwaway project under `build/ghidra`).
- `tools/ghidra/project.py`: the Ghidra project for the rewrite (`build/ghidra/rsgun`): work RAM-H
  from the play1 dump, each overlay the evidence saw as an overlay block, the leftover dma.c's types
  (`coffdump.py --header` writes `build/leftovers/dma_types.h`; every struct's size and offsets
  checked), a function at each one the port translated, and Ghidra's C for each in
  `build/ghidra/c/f_ADDR[_FP].c` (about 30 s). The literals the code loads PC-relative are marked
  constant and the decompiler takes its options from the program: without both, the C shows the
  literal pool's cells instead of the addresses they hold.
- `tools/rom/`: `mkcart.py`; `dis.py` (SH-2, literals resolved); `blockmap.py`, `coffscan.py`,
  `coffdump.py`, `textruns.py` (what the padding holds).
- The port (phase 5). `rsgun --headless --evidence F` writes what the run executed (stv::evidence:
  addresses per CPU and overlay fingerprint, call/jump/return sites and targets, interrupts);
  `--dump F,F --dump-dir D` the memories. `tools/port/sh2rust.py` translates every function the
  evidence called (in the main program's regions) from a work RAM-H dump into
  `build/port/generated.rs` (ROM-derived, never committed), instruction for instruction as the
  interpreter runs them; `crates/rsgun/build.rs` brings it in. `rsgun --native` runs it in place of
  the SH-2 code; `rsgun --compare REPORT` runs the SH-2 code and checks each call the port has against
  it (stv::compare: the call's reads and writes logged, the native function replayed against the log,
  same accesses in the same order and the same registers at the end). `tools/port/check.sh` does it
  all over play1 (4000 frames) and the attract mode (7200): it must say "every call equal". Evidence
  runs also save each overlay's window by fingerprint (`EVIDENCE.overlays/`), which the translator
  translates on its own and registers for that overlay only. `tools/port/overlays.py` finds which
  cart offset an overlay dump came from. `tools/port/calls.py ADDR` lists a function's callers (the
  function each call site is in) and callees from the evidence. `tools/port/marathon.py` writes an
  input script that plays the whole game without skill (fires, weaves, coins and starts often, so
  each game over is continued; bosses leave when their time is up): evidence of every stage.
  `tools/port/check.sh` translates the marathon's evidence too when it is in out/evidence, and runs
  a third check, a boot from the backup RAM play1 left (the scores read back).
  `tools/port/check_marathon.sh [JOBS]` checks the whole game: the marathon's script from power on
  and from each of its 20 save states, 10000 frames each, JOBS at once, on a copy of the binary (so
  it can be rebuilt meanwhile); reports in out/port/marathon/.
- The rewrite (`crates/rsgun/src/game/`): functions written to be read, each registered at its SH-2
  function's entry over the translation (`rewrites!` in `game/mod.rs`: entry, Rust function,
  arguments, result), keeping the calling convention (arguments r4-r7, result r0, r8-r15 kept). A
  rewrite takes a `Cpu` (the bus, and the registers for `Cpu::call` into code not rewritten yet and
  `Cpu::wait` in polling loops). Compare mode checks a rewrite, and any call with one inside it, by
  what it does (stv::native::Contract::Abi/AbiVoid): its stack frame and the SH-2 code's literal
  loads are left out; memory (RAMs, video and sound memories) is checked by value (reads answered
  from what it held, the bytes written must end the same, in any order); hardware registers and
  memory that changed under the call (interrupts, the other CPU, the sound CPU) keep their order;
  at the end r0 (unless void), r8-r15, GBR, VBR and SR's mask must match. A rewrite's SH-2 entry
  makes its SH-2 function's prologue (`Prologue` in game/mod.rs, read from the code at the first
  call: the pushes, with the registers it was called with, and the locals' room), so its callees
  get the stack the SH-2 code gave them (some SGL code hands the DMA a buffer on the stack, and the
  address shows) and the stack holds what the SH-2 code's pushes left: the game reads stack it
  never set (an effect's scale left out of its struct gets an older push; the whole-game check
  found two). A rewrite calls another as its SH-2 function calls it through `nested!(cpu, ENTRY,
  f(cpu, ..))` (the same prologue), every time, and only where the SH-2 code makes that call: code
  the SH-2 function has inline stays a plain Rust call with a `// inline` comment on the line before
  (`tools/port/direct_calls.py` lists calls between rewrites that are neither; `nested_calls.py`
  lists `nested!` calls whose entry is not among the SH-2 function's literals, i.e. code it has
  inline; `void_results.py` lists SH-2 calls, anywhere in the cart, that read r0 after calling a
  rewrite registered `-> ()`: compare mode does not check r0 for those, so give it the result the
  SH-2 function leaves, as `models::follow_parent` gives the parent the stages' code takes;
  `called_rewrites.py` lists `cpu.call` sites, older than the rewrite they reach, whose target is
  now one of the game's rewrites: make them `nested!` calls. It leaves SGL's targets alone:
  `cpu.call` loads r4-r7 with the arguments as the SH-2 caller does, and SGL's asm reads registers
  it is handed (section 5), so those stay calls. `rewrites!` refuses an address registered twice
  (a second entry silently replaced the first: the heavy bullet's behaviour had been rewritten
  twice, and only one ever ran)). While a rewrite runs, r14 holds the bottom of its frame (GCC's frame pointer: a callee's
  pushes of r14 then push what the SH-2 code's did), and the main loop keeps the SH-2 loop's
  constants in r8-r13 (`hold` in main_loop.rs), so the pushes below it match.
  Stale stack: elsewhere a rewrite's r8-r13 are whatever it was called with, and a call from
  rewritten code pushes PR as RETURN, so a callee's prologue can leave other values than the SH-2
  code's; the game reads some of those slots later as struct fields it never set (the sweep found
  six). Where compare mode shows one (`obj_exec` or a caller "leaves memory otherwise" with no
  callee diverging): dump the call, find the read of the stale slot in the SH-2 log (the value
  that went into the diverging write), the last writes of that slot in both logs (`awk` on `W`
  lines covering it), and the SH-2 push that wrote it (the `P` lines before: which function's
  prologue, which register); then make the rewrite that calls that function hold the SH-2 code's
  value there: `cpu.holding(&[(n, value)], |cpu| ..)` for r8-r13 (what the SH-2 caller keeps in
  that register at the call: send's address in start_seq, the game in game_exec, the effect in
  se_set, the sprite in draw_model, a frame pointer in beam_exec, the list, object and next list
  in obj_exec, the actor in actor_exec/numbered_exec/plain_exec, the corner's registers in draw's
  loop), and write the frame's locals the SH-2 code writes even where the rewrite keeps them in
  Rust (turn_toward's ease and acceleration: an explosion's struct took one of them), or `cpu.call_returning(PR, f,
  args)` for a pushed return address (register_boxes' SL_ROTATE calls). `Cpu::push`/`Cpu::pop`
  leave stack arguments pushed across calls where the SH-2 code does (ribbon_edges), and push what
  SH-2 code pushes mid-function (`Cpu::reg`, `Cpu::pr`: math.rs's turn_point pushes r11, r12 and
  PR only when it turns). `call_returning` works because the interpreter takes a call's return
  as PR reached with the entry's stack pointer, so any PR does. While a call is logged,
  each write to work RAM is preceded in the log by what it overwrote (`O`), so a rewrite may read a
  variable the SH-2 code only wrote. `STV_COMPARE_DUMP=ENTRY STV_COMPARE_DUMP_DIR=DIR` writes the
  first diverging call to ENTRY: the SH-2 code's accesses and the native run's.
- Fast type-check of the rewrite: a git worktree of the repo without `build/port/generated.rs`
  (build.rs then substitutes an empty port) runs `cargo check -p rsgun` in about 1.5 s instead of
  the minutes the translation takes to compile; symlink `out` into it for the tools that read the
  evidence, and never commit the link. Linking `build/port` and `build/rom` too (build/ is
  ignored) lets the worktree build and sweep a full binary of its own while main's checks run.
- `tools/port/asm2sm.py LISTING NAME ENTRY`: SGL's hand-written assembly (the slave's pipeline,
  its handlers' threaded jumps through tables of handlers, tails shared between entries) as a
  Rust function run block by block: locals for the registers, MAC, T and PR, a `match` arm per
  block on its address ending in the branch the SH-2 code takes (delay slot first), calls by
  `cpu.call` with the registers handed over, rts back to the caller when PR is the entry's. The
  blocks are found from the entry over dis.py's listing of a region (data islands skipped by
  following the code; jump tables from a `mova` and the load before the `jmp`; threaded tables
  from the `mova`s of the code reached, to a fixed point). A short backward branch whose loop
  reads a fixed place or only counts down gets a `cpu.wait` (natively a poll never ends
  otherwise). The routine makes its own pushes, so its entry goes in PROLOGUES with none (or its
  wrapper takes over after a push the prologue made); it leaves r8-r14 as the code does
  (`Cpu::leave_r14` for r14). asm2rs.py converts each instruction (`convert`).
- Which instructions ran: the evidence's `x CPU FINGERPRINT ADDR COUNT` lines (out/evidence/
  marathon.txt and the others) count every instruction executed, so a function's paths that never
  ran show as gaps (SGL's PCM streams and its sound queue never run in this game: written all the
  same, marked "not seen run").
- `tools/port/check_marathon.sh [JOBS [FRAMES [OUT]]]` (RSGUN=binary): FRAMES a segment makes a
  quick sweep of every stage (3000: about an hour at 7 jobs).
- `tools/port/check_native.sh`: play1 and the attract mode natively and on the interpreter, both
  with `--cpu-speed 16`, must end on the same pictures (a lockstep run of both showed them equal
  every frame of the attract mode at 16; at 4 the change of mode at frame 5544 still slows both, by
  a frame more on the interpreter). Rewrites cost the machine less time than the
  SH-2 code, so at the built speed a native run slows down less (the interpreter's attract mode
  loses 5 frames by frame 4000, the native 1) and the game goes differently from there: the game
  runs per main-loop pass, not per VBlank. `--cpu-speed N` (stv's `Machine::cpu_speed`) runs the
  SH-2s N times as fast as the rest of the machine (their timers count their clock): no slowdown.

## 4. Status

Phase 1 is done (19 October): the SH-2 core replays 6.79 million instructions over 19 trace windows
with no difference, and the 68000 core about 260,000 (the sound driver), both through check_all.sh.

Phase 2: the machine runs the whole attract mode on its own (20 October): the real BIOS from power on,
the Treasure logo, the title, the story, stage 1's demo play with its mid-boss, drawing its own frames.
It runs ahead of the oracle's pace (the cores cost one cycle an instruction; the oracle's master
averages about 4 in the BIOS and 1.8 in game), so frames are compared by content, not by number.

Phase 3: VDP2's composition matches the oracle's pictures pixel for pixel on 11 of 12 frames (the
twelfth has text typed in mid-frame), rotation and interlace included (check_vdp2.sh). VDP1's
rasterizer matches the oracle's VDP1 pixel for pixel (20 October): 15 crafted probes
(tools/vdp1/probes.py) and the game's draws at frames 1200-7200; frame 6000 differs only in the game,
where the CPU rewrites VRAM while VDP1 is still drawing (our draws are instant: draw timing is not
modelled).

Phase 4 (20 October): the 68000 core matches the oracle's traces; the SCSP synthesizes (slots, the
envelope, LFOs, FM, the effect DSP, the mix): the 19 probes match sample for sample, and replaying
play1.txt's 60-second log gives 2,653,239 of 2,653,247 samples equal. The 8 left: a voice whose
LSA/LEA (and loop mode) the driver moves thousands of samples below its position while it releases
(reproduce by filtering the log to slot 18 to sample 1717900; the oracle's loop state machine there
is not yet modelled). LFOs are untested (the game has not used them so far). No port code yet.

Phase 5, the port (20 October): the scaffold works. Every function the evidence saw called is
translated to Rust but one: 2158 since the marathon's evidence (1277 in 9 overlays; 701 from play1
and the attract mode alone), the interrupt handlers, and the BIOS's
code in RAM (its interrupt dispatch and library at 0x06000C00-0x06003000, there unchanged from the
first frames). Each is checked call by call against the interpreter: over play1 and the attract
mode, 489 million calls, every one equal (18 too long to check). `--native` ends play1 (frame 3600)
and the attract mode (frame 7200) on the interpreter's pictures. Native calls run on their own
stacks (stv's fibers, corosensei): a block charges its cycles, and when the CPU's turn is spent the
call gives the system back to the machine and is resumed later, so waits work as the SH-2 code wrote
them and interrupts are taken between blocks. Left to the interpreter: the restart routine (a stack
switch, 0x06007e50), and code the evidence has not seen called. Native waits cost almost nothing:
a pass through a block that finds the CPU as the last pass left it, after the same reads and no
writes, gives the rest of the turn back, and while both CPUs wait the machine runs on to the next
event (as with the spin player). `--native` runs play1 at 11.4 ms a frame against the
interpreter's 12.4 (4.7 after the speed work below). Once the game has started the
interpreter has almost nothing left to do: from frame 1200 to 3600 of play1 it fetched 51,323 master
instructions (about 21 a frame); the 555 million before are the BIOS's boot (`--native` prints the
counts).

The rewrite (20 October): started, in `crates/rsgun/src/game/`. dma.c whole (the leftover object
names its six functions and `dma_wk`; its text sits at 0x060044a0 in this build), the objects
(`Base_obj` in a pool of 0x80-byte blocks in five `OBJ_INF` lists: obj_set, obj_size, the free-block
search, freeing, the frame's exec pass, init), the pads (`PAD_INF`), the division helpers on the
divide unit, and small system pieces (the VBlank count, credits, cache purge, the DMA wait, the
interrupt mask). Compare mode's checks by what a call does catch planted bugs (a wrong field, a
missed hardware write, a wrong result). Naming sources: dma.c's types (Base_obj, OBJ_INF, PAD_INF,
DMA_WK, PLAY_DATA, the game-mode enum STG00-05 ...), the leftover source's vocabulary (obj_set,
MAIN_WK mw, ATARI_WK aw, dousa_func ...), the leftover map (from an older build: only `_main` and
`_credit_ct` land on this build's functions). The game's main loop is f_06040000 (.control's first
function, rewritten as game/main_loop.rs: SGL's frame set-up, the pads, the objects' exec pass, the
colours, the file loads, the DMA queue, slSynch; it returns when GAME_END is set). `main`
(0x06004000) clears the bss and SGL's work area and jumps through the restart routine (the stack
switch, 0x06007e50) to 0x06004070, the game's top level: a switch on the game mode with a
0x200-byte frame, which calls the main loop for each mode. Reached by a jump, it is not
translated: it is the ~21 instructions a frame still interpreted. Compare mode cannot check the
main loop or the top level whole (one call lasts a game mode); check_native.sh covers them.

Then (20 October) the sound (the driver's command slots, Sega's sound library's commands, the sound
manager object: music, fades, se_set with dma.c's SE names), the rest of the files (whole-file
reads, load sizes, starts, ends), the backup RAM (settings from the BIOS, the score tables in
ENTRY_DATA, its format and save), memset/memcpy, and small system pieces. Then the game's
director (game.rs's director.rs: a SYS_OBJ that runs the game's states from a table at 0x060723f8,
keeps the game's speed (16.16; a game frame when it adds up to one: slow motion, and the debug
pause with a frame step), the players' positions for the last 32 frames, the coins through the
BIOS), the first two states (states.rs: the reset, and a game mode's load: each mode's file
"TITLE.BIN", "RANK.BIN", "OPENING.BIN", "STG00-05.BIN" into the overlay window at 256 KB a frame,
then the overlay's start at 0x06080000), the game mode (GAME_MODE 0x0600ebb0, dma.c's enum), the
screen (screen.rs: the screen object, VDP2's colour offsets, the fades and their channels, the
VBlank's copy queues), the text (text.rs: the font, the players' lines and credit display), and
the status lines (hud.rs: the score with the continues in its last digit, lives, weapon levels in
turn, the gauge). PLAY_DATA (dma.c's) is at 0x0600ebe0, two of them.

Then (20 October) the director's other states (states.rs): the screens (name entry, result,
opening: Start skips the opening on free play), the title (each player's line by their coin state
from the BIOS (COINS byte +3: 0 nothing, 1 coins short of a credit, 2 can start); Start takes a
credit through the BIOS and gives the lives of the second setting (so that setting is the lives);
without a player who can start it times out after 600 frames to the opening or a demo by turns:
STG01-STG03 in turn, STG04 every sixteenth), the stage (each player's state: out, joined, playing,
counting down to continue (shared count, A/B/C hurry it below 8, coins restart it), game over; the
continue screen with both out; demos play their file's input for 0xf88 frames, without sound effects
unless a BIOS bit allows), the stage's end (after STG00 the players' scores are filed in a per-stage
table in PLAY_DATA +0x48 [6 stages][5 routes], rounded to tens, and the game starts anew), the
clearing up as a mode ends (every object freed but six), and the typewriter text objects.

Then (20 October) the hits (hits.rs: the lists are the players' bodies 0, the enemies' 1, a third
side's 2, the players' shots 3, the enemies' 4, the third side's 5, the enemies' small bullets as
points 6; each box is registered by its actor each frame, its offset turned with the actor and
culled off screen; the hit functions by kind pairs from two tables; a players' shot's hit gives its
weapon (types 7-16: A, B, C's two, A+B, A+C, B+C, ..., A+B+C's two) points, A/B/C experience and
hit counts; a kill scores with the colour chains: three kills of a colour in a row give 10000,
every third kill of a chain its score (up to 10000) times its thirds, up to 100000, and the longest
chain by colour is kept in PLAY_DATA; the sword's sparks score 3 a contact, one contact in four
makes a spark while the sparks' frames are under 600; a type-15 striker absorbing a bullet charges
its player's gauge by 1000 up to 25000), the actors (actors.rs: the generic object frame:
behaviour at +0x38, flash while hit, boxes, the white colour 0xe0 from the first character while
flashing, the draw), aiming (aim.rs: the nearer player, or where it was 1-32 frames ago from the
director's history), the sprite and model draws through SGL (sprites.rs), the camera (camera.rs:
eased a sixteenth a frame), the random numbers (seed*5 + 0x3573; demos seed 0), values scaled by
the difficulty (the first setting: (difficulty - 2) * x / 2), the players' play data anew and a
game's start (play.rs), and the game's state at the start (main_loop.rs init_game).

The marathon script (tools/port/marathon.py) plays the whole game natively: through every stage to
the last boss by frame 150000, continuing at each game over.

Save states and the boot state (20 October): `stv-check state` stays equal from states saved at
frames 1, 300 and 1300 to frame 1500. A run from the boot state ends play1 (native) on the cold
run's picture, from frame 1159 at 9.0 ms a frame. Put back into the boot state, another backup RAM
gives the cold run with that backup RAM but for two bytes (the BIOS's count of the time it has been
on, which misses the boot's frames); work RAM is equal at frame 4000.

Speed (21 October): a headless run prints what the machine's other parts took beside its ms a frame
(`Sys::costs`: VDP2's composition, VDP1's drawing, the sound CPU with the SCSP). Warm native play1
(frame 1159 to 3600, the default save directory copied fresh, 8 cores, nothing else running) went
from 9.35 ms a frame (main 898fd01) to 4.69: VDP2 1.03, VDP1 1.31, sound 1.45, the CPUs and the rest
about 0.9. Every picture, framebuffer and sample stayed the same (check_vdp2.sh's dumps, VDP1's game
draws and probes, the SCSP probes, play1's picture and sound byte for byte). VDP2: the settings are
taken once a frame, a layer's pixel is packed so that the one shown is the greater of two (priority,
then the earlier layer), a character's pattern name is read once per character line, RBG0 has no
divisions, and `Machine::render_threads` renders bands of lines at once (the player and headless
runs use up to 4: 2.85 ms a frame on one). SCSP: slots keyed off are not decoded, and the DSP stops
at its program's last step that is not all zeros (a zero step writes nothing). VDP1: a line's texel
columns and minor-axis steps are kept for the next line (a sprite's lines share them).
`STV_BENCH=N stv-check vdp1|vdp2 DIR` times the drawing (STV_THREADS=T for VDP2's bands).

The player (20 October): `cargo run --release -p rsgun` plays the game in a window; a scripted coin and
start reach the Trial Ride tutorial with the ship moving and firing. Gameplay frames cost about 5.5 ms
(wait loops played, not executed; VDP2 about half of what is left), the BIOS's about 12.

## 5. Quirks that cost time

- Ghidra's C for the game's assembly helpers is wrong where they keep their own register
  conventions: 0x0600815c-0x06008174 are libgcc's `__ashiftrt_r4_N`, a chain of `shar r4` whose
  result is r4 (Ghidra shows them empty); se_set's song is `sequence >> 8` through one. Read the
  disassembly (tools/rom/dis.py) for anything that calls into the 0x06006a58-0x06008174 helpers.
- A rewrite's polling loop must yield (`Cpu::wait`) on each pass, or nothing else in the machine
  runs (the sound CPU never takes the command it waits for).
- A rewrite takes no machine time but its waits, and the game mostly does not care. Where the
  hardware relies on how long the SH-2 code takes, it does: slInitSystem starts the slave (SSHON,
  not waited for) and signals it 37 lines later, after the VDP set-up; by then the slave must
  have been reset (the reset clears the signal) and be polling. The rewrite took none of that
  time, the signal came first, and the slave never took its command loop: the screen black from
  the boot on, every compare equal (compare mode checks each call, not the time between). Such a
  routine is generated `asm2sm.py --timed`, or written with `Cpu::spend` at the end of each of the
  SH-2 code's blocks (an instruction a cycle, as the translations count them; asm2sm's --timed
  output gives the counts). slInitSystem itself and its first frame are so too: with them taking
  no time, the SMPC's first requests and frames came early, and a native attract mode drifted
  from the interpreter's by frame 7200 (a boss's flash and an explosion a frame apart); a bisect
  by variants (out/bisect/, binaries out/port/bis*) put it there. Between the CPUs, native code
  could signal the other twice within one turn: SGL's master resets its command area and queues
  an empty frame again, the end mark back where the slave sleeps; it wakes, sees nothing new,
  and the frame is never drawn. So a native CPU's write to MINIT/SINIT ends its turn there, no
  time charged, and the other CPU catches up first (stv's FiberBus::after_write and
  Machine::advance; charging the rest of the slice instead changed nothing seen). Run the native
  check on every batch: b6zj's and b6zk's compares were all equal, and only the native run showed
  the boot black.
- A rewrite that hands a callee a buffer on the stack writes it at `Cpu::stack()` (the bottom of
  its frame, as its prologue makes it), where the SH-2 code had it; a rewrite that calls one that
  does must call it with `nested!` (else the buffer lands in the caller's frame, above where the
  SH-2 code's was: compare mode reports the bytes as memory left otherwise).
- Repro of a divergence the whole-game check finds at frame F: `rsgun --headless --script
  out/evidence/marathon-input.txt --load-state <nearest marathon state> --frames F-50 --states F-50
  --dump-dir DIR --save DIR/save` (a state just before), then from it with `--compare` and
  `STV_COMPARE_DUMP=ENTRY STV_COMPARE_DUMP_DIR=DIR` to F+10: seconds instead of the segment's
  half hour. The dump's `P` lines show the SH-2 code's path; `O` what each work-RAM write overwrote.
- Cart reads are hardware in compare mode's model (in order, as many): read the cart as often as
  the SH-2 code did (backup's default settings).

- Mednafen's SH-2 is pipelined. The debugger's PC lags for the first steps of a trace window (the
  checker trusts it only once it agrees with the core's, and checks instruction words until then). Its
  pseudo-ops run through the same step loop: 0xFE (stalled for a DMA burst: after a write that starts
  an SCU DMA, the CPU waits, many records long), 0x7E (standby), 0x7F/0xFF (an exception pending: takes
  it, begins a stall, or runs the instruction in the same step). The register dump's PC is the next
  instruction + 4, or the branch target + 2 between a delayed branch and its slot; a window that opens
  in a slot starts from that target.
- `mac.l @Rm+,@Rn+` reads @Rm first on the oracle (the manual's pseudo-code reads @Rn first): the oracle
  wins. `mac.w` is assumed the same until a trace shows one.
- `tas.b` reads the bus directly in Mednafen, not through the access macros: it needed its own hook.
- The BIOS's reset code tells master from slave by BCR1's MASTER bit (the slave's reads 0x83F0); with
  both reading 0, the slave ran the master's path and issued SYSRES.
- The BIOS changes the dot clock by CKCHG and SLEEP in standby; the SMPC wakes the master with an NMI.
- On the ST-V the sound CPU is started through the SMPC's port 2 (bits 3-4 low), not SNDON; the
  settings EEPROM is on port 1 (CS, SK, DI) and port 2 bit 0 (DO). Mednafen formats the EEPROM at power
  on: `tools/oracle/eeprom.py` copies its image for the machine.
- The game waits on the sound driver's handshake in sound RAM (0x25A004E0): with no 68000 it crawled
  through its timeouts.
- Mednafen's frames end on the top blanking's last line but one (254 with 224 display lines, 262 with
  240): the machine ends its frames there so the hash lines align. Its SMPC clock comes from the host
  unless `-ss.smpc.autortc 0` (run.py sets it; the clock starts at 1994-01-01 00:00).
- Oracle code included into `ss.cpp` sits inside its namespace: headers it needs go at the top of
  `ss.cpp` (the patch adds the 68000's).
- The game checks the cart's protection chip at start (a 512-byte pattern from 0x04FFFFFC after
  arming 0x04FFFFF0) and, if it is wrong, clears "PLAY.BIN" from its file list: the attract mode then
  stalls after the title's ten seconds (it waits for 13 background loads that never start). Found by
  comparing the two machines: dumps of the title's state block (0x0600EE54), the oracle's trace of the
  load queue (0x0600CA94), the machine's own trace (STV_CHECK_TRACE) of the code that cleared the name.
- VDP1's frame change comes at VBlank-OUT on the oracle, so the framebuffer on display at a VBlank-OUT
  is the one VDP1 has just finished.
- A colour-calculation ratio r weighs the layer below (r + 1)/32 and the top (31 - r)/32; blended
  pixels keep 8 bits (compare pictures on every bit).
- Wait loops: both SH-2s spend 96-99% of a frame polling (the master a byte in work RAM through a
  do-nothing callback, the slave its FRT's FTCSR for the master's MINIT). A loop is only replayed
  when its turn is pure; an FRT event during the recorded turn (after the loop read FTCSR) once made
  the replay stale: the check compares FTCSR at the turn's start and end.
- perf is not allowed here (perf_event_paranoid 3): profile with pprof-rs (SIGPROF) in a scratch
  crate that depends on `stv` by path.
- Probes must actually draw: an untextured command without SPD is invisible when VRAM's last dot is
  0 (the probes' VRAM is), so a "match" of two blank framebuffers proves nothing.
- With the desktop locked, a vsynced present blocks about a second on Wayland and X11 alike: the
  player then catches up 4 frames a present; test pacing with `SDL_VIDEODRIVER=offscreen`.
- `tools/rom/dis.py` shadows the standard `dis` module: run it with `python -I`.
- SCSP details the oracle has and MAME does not (each found by a probe; slot.rs and dsp.rs list
  them): key on/off acts after the sample's envelope clock; the envelope clock restarts with an SCSP
  reset; a release ends at 0x3C0 after one more sample; DL is met, not passed (and DL 0 still takes
  one decay-1 step); key scaling is max(0, 2(KRS + OCT)); the high envelope rates' larger step falls
  on even counts; send level and pan are one rounding; the pitch register's bit 10 is added in; the
  DSP reads its sends a sample late, its ring buffer is always float, its read lands two steps later,
  MEMS is written after INPUTS reads it, its counter wraps at the ring's current length, EFREG holds;
  0x7C0-0x7FF is not a MADRS mirror (the game writes zeros there right after its MADRS); most common
  registers read as 0. Compare the machine with the oracle on sound by replaying logs, not by
  running both: the 68000 driver's timing follows the SH-2s', whose pace differs.

- An interlaced frame (the title, 704×480) gives widths only for the current field's lines: the
  screenshot takes the widest line.
- `fetch_refs.sh` piped into `tail` hides a failed build's status; read `ref/mednafen-build.log`.
- The map fragment in the padding is from an older build (section offsets differ): use its names
  as hints only.
- Mednafen wants ST-V games as a ZIP of MAME-named chips with nothing else in it; the torrentzipped
  MAME set works as it is.
- PyGhidra: pass `java.io.File` to `ProgramLoader.source` (Python bytes pick the `String` overload);
  the loader's base address is `addLoaderArg("-loader-baseAddr", "0x...")`.

- An RTS is not always a function's return: 06036dd8 (the geometry transform) sets PR into itself
  (`mova 06037074,r0; lds r0,pr`) and jumps into routines whose RTS lands there. The translation
  returns only when the RTS goes to the PR the function was entered with; return targets the
  evidence saw within the function are blocks, and anything else (also a computed jump with no
  evidence) is interpreted until the function returns (`stv::native::goto`). Treating an unknown
  computed jump as a tail call was wrong: mid-function, PR is stale.
- pprof's default unwinder crashes on the fibers' stacks (a sample during a switch): profile with its
  `frame-pointer` feature and `RUSTFLAGS="-C force-frame-pointers=yes"`; inlined code then shows
  under its caller (VDP2's composition under `run_frame`).
- Compare mode needs the spin player off (it skips the accesses the log must hold): the machine turns
  it off itself when `compare` is set, as for evidence.

## 6. Next steps, in order

1. The overlays ("the overlay loop"; docs/AGENTS.md for how batches are split and merged).
   Overlays (21 October): the second way, which commits nothing taken from the ROM. stv's
   registry asks the client to name each fingerprint the window shows (`Natives::namer`, asked
   once a fingerprint), and a rewrite registered for a name (`rewrites! { n; overlay "STG01"; ...
   }`, `Natives::register_in`) runs for any fingerprint with that name, before a translation
   registered by fingerprint. rsgun's namer (states::overlay_name) reads the game mode and the
   mode's file name in the game's own table (0x06072310): TITLE, RANK, OPENING, STG00-STG05.
   Checked on the evidence's window dumps (each found in the cart at its file's place in the
   game's FILE_INF): a native attract run and a run from each marathon state name all nine
   fingerprints as their files. TITLE.BIN is 2 KB but the fingerprint covers 4 KB, so TITLE has
   two fingerprints (the rest of the window is what the file before it left): both named TITLE.
   If the owner would rather commit the fingerprints, `register_as` with them still works.
   Overlay rewrites so far: TITLE (logos.rs: the boot logos, the FBI's warning where the BIOS's
   setting is 4, so that setting is the region, 4 the US's as far as can be told), OPENING
   (opening.rs: the story's pictures and typed text), STG00 (stg00.rs: the stage before the
   first battle, its count and "PRESS START TO SKIP"), each with its copies of SGL's routines
   (sgl.rs: NBG3's setters, the sprites' colour calculation, rotation parameter B and the
   coefficient table). Code the mode files share goes in modes.rs and states.rs
   (keep_mode_settings, stage_part). Compare reports now name each function by the native that
   ran (overlay code has one at an entry for each of its codes). Not seen run, left to the SH-2
   code by `cpu.call`: OPENING's text where the BIOS's setting is not 1 (0x0608080c), STG00's
   row hooks (0x06080328, 0x06080424); TITLE's warning steps (2-6) are written but not seen run.
   STG01 (stg01.rs; stage 3, the first stage played, 384 functions in the evidence): its mode
   and 33 more committed (7dd890c on wip-b6): the scenery and debris, the riser fortress's
   pieces (the risers and their parts, beams, turrets, lasers and laser cannons, the spinner, the
   electric arcs, the burst seeds, the tethered drones, ring parts, glows, link beams, laser
   heads and their launchers); d4ba91e: 21 more (the boss and its set-up, the enemy rows
   (0x060878d4, STG00's ground_row with every maker) and their hooks, the route fliers, the
   depth-tinted ground, the sinking craft with its parts and guns, the bomber and its bombs, the
   rail claw). The gunship's group (0x06089450-0x0608aadc) is written and being compared. To
   pick a compare's segment, grep the function's address in the last whole-game sweep's
   reports (out/port/marathon-898fd01/fNNNNNN.txt: STG01 runs in f000000-f060000 and f200000)
   and run that 10000-frame segment (from out/evidence/marathon-states; f000000 from power on);
   for the riser fortress, 3000-frame compares from out/port/stg01-states (f051000-f069000).
   `rsgun --reach ADDR` gives the first frame a function runs, but is slow over long spans. Ghidra's drafts (build/ghidra/c/f_ADDR_FINGERPRINT.c) list
   the same 384; the disassembly is the reference (the drafts drop arguments and misread
   delay slots).
   **The overlay loop** (since 21 October; about 25 functions an hour). In address order, a
   function at a time: its disassembly from `build/dis/FINGERPRINT.dis` (tools/setup_debian.sh
   makes it: `tools/rom/dis.py out/evidence/marathon-end.overlays/FP.bin 0 06080000 75776`; skip the
   `nop` lines but mind delay slots), Ghidra's draft beside it, tables (jump tables, step
   tables) read from the overlay image with a few lines of Python (the image is the window from
   0x06080000). The list of an overlay's functions: `grep -o "fn f_[0-9a-f]*_FP"
   build/port/generated.rs`; a function's callers: `grep "= ADDR" build/dis/FP.dis` (a function
   no code calls is in a table: search the image for its address). Write, register, build; then
   pick the segments it runs in: `grep -l "^ADDR " out/port/marathon-898fd01/f*.txt` (the last
   whole-game check's reports: STG01 runs in f000000-f020000, the riser fortress f050000-f060000,
   and again in f200000), copy the binary aside (`mkdir -p out/port/NAME; cp target/release/rsgun
   out/port/NAME/`) and run each segment, several at once:
   `out/port/NAME/rsgun --headless --load-state out/evidence/marathon-states/fNNNNNN.state --script
   out/evidence/marathon-input.txt --frames $((NNNNNN + 10000)) --compare out/port/NAME/fNNNNNN.txt`
   (f000000 without --load-state: from power on; in bash write the start as `$((10#$n))`, a
   leading 0 is octal). The report's first line counts calls equal and diverged; each function
   has a line `ADDR name calls N equal N diverged N too long N`, and a divergence a line `ADDR at
   frame F: ...` naming what differed. A segment takes 16-40 minutes (16 cores, 8 jobs
   at once in all); write the next functions meanwhile. A batch is committed when every new
   function it ran is equal (hold back, uncommitted, the ones no segment ran).
   Where STG01 stands (22 October): done (see the top). Its last part, the stage boss
   (0x0609bcc4-0x0609f028, frames 40000-50000): the boss object (its parts' numbers +0x70-+0x75,
   its script +0xb4 of 20-byte entries, its move +0x6d from 0x060a429c, its burst +0x6f, its
   beam's size +0xbc), four arms (numbered objects, steps from 0x060a4314, given the boss in r5),
   two guns, the beam's segments and the head's glow. One stale-stack read was matched there:
   targets_frame holds the SH-2 code's r8-r13 around a rotation matrix (a hatch's set-up reads
   r13's push back).
2. (done: stale stack reads are matched by holding registers, section 3. Compare mode's other
   way, answering a native read of a pushed byte with the SH-2 code's read, was not needed and
   would hide a real difference in the native game.)
3. (done: the draw dispatcher, merged in 910a3e1.)
4. The rewrite, by calls in the marathon (the evidence's `t ... c FROM TO COUNT` lines summed by
   TO, less the entries in `rewrites!`; `calls.py` for one function's callers). On wip-b6: the
   game's part of .control (0x06040000-0x06066000) is done but for the interrupt layer
   (0x06064f68-0x06065200 and the vector stub 0x0606521e, left on purpose); the main image's game
   code is done (0x06003000-0x06033000: GCC's shift helpers too, `clib::ashiftrt_r4`, which keep
   their value in r4); below that is the BIOS's code in RAM, which is the machine's, not the
   game's (a question for later: the BIOS's services would want their own implementation, in the
   runtime, if the SH-2 is ever to go). Left: SGL (0x06030000-0x06040000 its core and the slave's
   pipeline; 0x06066000-0x06080000 its library), the overlays (waiting on the question in step 1),
   and the top level (0x06004070, behind the stack switch).
   SGL so far (sgl.rs). Its library (0x06066000-0x06080000) is done: the matrix stack and
   transforms, the turns, slPutPolygon, slPutPolygonX (the lit put, its Gouraud tables made from
   the light turned into the data's frame), slPutSprite and its variants, the DMA polygon put, the
   master-side polygon put (put_polygons, the ribbons': points turned into the points room, each
   polygon culled, depth-sorted by its attribute and put in the polygon buffer), slConvert, slAtan,
   slTan, slLookAt, the roots, the matrix inverse, the rotation scroll's parameters and coefficient
   table (rotation_parameters: 512 quotients a call on the divide unit), every VDP2 setter
   (character, page, plane, map, RAMCTL's banks for RBG0, zoom, back colour, transparency, colour
   calculation and offsets, priorities, the window controls, BGON, slScrAutoDisp's cycle patterns),
   slPerspective, slZdspLevel, the TV mode and frame mode, slTVOn/Off, the Gouraud set-up, colour
   and VBlank copy (with SGL's SCU DMA copy), slLight. Only two internal routines are left there,
   and nothing calls them now (0x0607170c, put_check inline; 0x0606e0a2, inline in slScrAutoDisp).
   Its core (0x06033000-0x06038000): slSynch (the wait for the VBlank calls SGL's wait function,
   or `Cpu::wait` for its empty one), slRandom, slInitMatrix, slUnitMatrix, slIntFunction,
   slTransferEntry, slDMACopy/slDMAXCopy, the sound flush and PCM step, the interrupt handlers
   (VBlank-in and -out, the SCU DMA ends, the SMPC: they save what they use, and the prologue
   table now takes GBR, MACH, MACL and SR pushes), the SMPC layer (its lock, port settings, mode
   bits, command table, slRequestCommand and its handlers, the command issue, INTBACK's
   parameters, the peripheral data's parse into SGL's records with the pad and none devices, the
   VBlank-in and -out SMPC work), the sync and SMPC set-up (slInitSystem's), and the slave's small
   command handlers (48 call, 52 near level, 12 perspective, 36 light, 60/64/68 settings, 56
   window, 44 frame end) and its sprite (28) and ready-made command (40) handlers. Then (b6zj, b6zk) the
   rest: the PCM stream state machine 0x06033658 (only its idle path ever runs), slInitSystem
   (0x06007ef0, in the main image, called from the top level) with its VDP2 and VRAM set-up
   0x06035540 and helpers, the slave's start 0x06035090, main loop 0x060350b0 and command loop
   0x06036110 (both never return: compare mode cannot check them, their handlers it can; the
   native check runs them), the slave's handlers 4 (0x06036ab0), 8 (0x06036292), 24 (the turned
   sprite 0x06037f74) and 20 (the polygon pipeline 0x06036dd8, about 2000 instructions), the big
   ones state machines from asm2sm.py. What a native run still interprets: `--native` now prints
   the calls native code passed to the interpreter, by target, with the instructions they took
   (`--native --evidence` shows only what the machine's own loop interpreted, not what ran
   inside a native call). d2162fe, play1 without `--cold` (from the frame before the handover, 1159, to
   3600): 430,539 master instructions by frame 1160 (the BIOS's last frame before it hands over),
   then only the BIOS's services in ROM and RAM (0x00000efc, 0x000044fc, 0x0600143e, 0x06001464 and
   a few more: about 52,000 instructions over 2,400 frames); 1173 on the slave (the BIOS's start
   after SSHON); 8-9 ms a frame on this machine. With `--cold` the BIOS's boot comes
   first (its program in RAM at 0x06010000-0x06026000, a loop at 0x060154a8 most of it: billions
   of instructions at `--cpu-speed 16`): the machine's, not the port's. Two runs of one binary
   from the warm start can end a few bytes apart (a 7x7 cell of play1's last picture): a warm
   run reads the save directory's backup RAM and EEPROM, which other runs write back. Compare
   warm runs each from its own copy of one save directory (`--save COPY`).
5. Rules learned: use `nested!` exactly where the SH-2 code calls the function (not where it has
   the same code inline: hit_once has hit's body inline, put_large_string the character's,
   init_loads the clear; a nested call there made a frame the SH-2 code never had); run
   direct_calls.py and nested_calls.py after each batch; a rewrite that builds a struct on the stack
   writes only the fields the SH-2 code writes.
   SGL's code is assembly that hands registers on: what an SGL function leaves in r1-r7,
   MACH/MACL and T, its C callers carry on untouched to the next call, and some SGL functions read
   a register before they set it (0x060714dc, the ribbon's polygon put, reads r3 as slPushMatrix
   left it: a byte of the slave's buffer came out one off on f040000). Compare mode's ABI check
   does not look at those registers (the rewrite's own call is equal); only what reads them later
   shows it. So an SGL rewrite leaves them as SGL does (`Cpu::leave`, `leave_mac`, `leave_t`, and
   its pushes below the stack pointer): sgl.rs. An SGL rewrite that calls SGL code calls it with
   `cpu.call`, which loads r4-r7 with the arguments as the SH-2 caller does (`nested!` does not
   touch them, and a callee's rewrite leaves only what its SH-2 code changes: the rest must be
   what the caller handed it); slLookAt does so. Where the SH-2 code returns past its caller
   (0x0607170c pops the put's PR to return 0 from it), the callers take that code inline.
   Where the SH-2 code's own steps decide registers it hands on (the C library's division by
   div1: r3, SR's Q and M), run the steps on the CPU's registers (`Sh2::div1`). A prologue the
   reader cannot take from the code (a push after the entry's pushes, popped again before
   return; PR pushed first; GBR, MACH, MACL or SR pushed, as the interrupt handlers do; a frame
   taken by a subtraction) goes in `PROLOGUES` in game/mod.rs with its locals' size.
   The slave's command handlers take the command's first word in r0 and the place after it in
   r4, and leave r4 past what they used (the loop goes on from there): compare mode does not
   check r4, so count the words a handler takes against the master's command for it. Some of them
   use r8-r14 without saving them (the loop does not care); compare mode checks those, so leave
   them as the SH-2 code does. Write an address as the SH-2 code does: the slave reads the window
   settings through the cache (0x060ffefc), the master around it (0x260ffefc); the two are
   different addresses to compare mode. An interrupt handler's rewrite saves and gives back what
   its SH-2 code saves (r0-r7, PR, GBR, MACH, MACL, SR) and runs with SR as it sets it (0: other
   interrupts taken at its waits).
6. Speed (section 4, "Speed"): 4.7 ms a frame in play. What is left: the sound CPU (an idle-loop
   player for the 68000, as spin.rs is for the SH-2s, if its driver polls), VDP1 (drawing on a
   thread of its own while the next frame runs: the draw buffer is shown only after the next
   swap; or fewer reads per texel), VDP2's threads kept between frames. Keep every check exact.
7. Pacing, as far as it helps checking (and VDP1's draw time, which the game races at frame 6000): a
   cost model nearer the oracle's, or comparisons aligned on the game's frame counter (0x06038076
   counts frames on both). Exact lag frames are not a goal.
8. The SCSP's loop corner above, and probes for the LFOs once the game is seen using them.
9. Finish the survey's open questions.
