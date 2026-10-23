# rsgun (Radiant Silvergun, ST-V): working guide and handoff

Read this first. `PLAN.md` beside it has the owner's decisions and the phases, `SURVEY.md` what the
game asks of the hardware; the method is md-recomp's playbook (`md-recomp/playbook/PLAYBOOK.md`, a
sibling project: not needed to carry on). Paths are from the repository's root.

**The native boot (23 October):** built, checkable and wired in as `--native-boot` (opt-in, the
default unchanged); see "The native boot, built" (section before 6). Next: make it the default
(under boot.rs's place, once the owner agrees), the BIOS's repair path for an EEPROM it does not
accept, the BIOS-menu path.

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
- RANK (4635b7419f6fc1a7, the ranking after a game ends, f200000; ../rsgun-rank, wip-rank):
  every function that runs is rewritten (out/port/orch-rank/f200000.txt); its scenario scripts
  (patched states for the name entry) are in out/port/rank-scripts.
- STG03 (game mode 9) is stage 4: STG01's end asks "SELECT NEXT STAGE" (stage 2 or 4; up or down
  turns it, a button or 300 frames takes it) and the marathon took stage 2 (its weaving turned it
  at random). out/evidence/route4-input.txt is the marathon's input with the controls let go from
  65500 and down pressed at 66900 (stage 4 taken, STG03 loaded at 67050); its run from the
  f060000 state (evidence out/evidence/route4.txt, states out/evidence/route4-states every 10000
  frames from 70000; extended to 210000 as route4-end.txt: STG03 ends at 176670) is STG03's
  evidence (fingerprint fd6475daa2eebeb4, 461 functions translated, all 453 the baseline saw run).
  `tools/port/check_route4.sh` compares the route's segments (r060000, f070000-f200000); its
  baseline on main (out/port/route4-base) had obj_exec diverging in eight segments before any
  STG03 rewrite: stale-stack reads, fixed in shared modules (the "objects:" commits). Left:
  three in f170000 (frames 176282, 176354, 176467): STG03's code at 0x0608052e-0x06080594 builds
  an effect's struct on the stack without its +0x1d, and the slave CPU's pushes reach that slot
  (060ff0e0); compare mode keeps the frame private, so it is a two-CPU timing difference to look
  at when that code is rewritten (the rewrite still matches the SH-2 code, unset field and all). STG03 is batched on
  ../rsgun-stg03 (wip-stg03) below 0x06094000 and ../rsgun-stg03b (wip-stg03b) from it (markers in
  stg03.rs and its rewrites! block split the two). It is also the attract mode's third demo (mode 14).
The whole-game check on main 96b4bf4 (out/port/marathon-96b4bf4): all 21 segments every call equal;
still run unrewritten there: RANK's 16 (since done) and STG02's 0x06097610 (f140000) and 0x060a19d0-0x060a2504
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
The BIOS (23 October; PLAN.md has the decision): after the handover a native run interprets only
the BIOS ROM's 0x0efc (each frame: the operating-time count at 0x06000758), 0x44fc (an empty hook,
three times a frame, also through the RAM stubs 0x0600143e/0x06001464) and, rarely, 0x372c (area),
0x4dc0 (settings), 0x3842 (players), 0x3a9c (backup clear); its RAM code (0x06000c00-0x06003000,
the service table at 0x06000200-0x06000800 points into it: interrupts, SCU mask, vblank hook,
coins, credits, continue, backup I/O) is 41 functions called, 40 of them mechanical translations.
Next: rewrite those in bios.rs (compare mode, as the overlays), then the native boot (the handover
state is mapped in "The native boot: the handover state", before section 6; native_boot.rs builds
work RAM, the CPUs, the devices and the sound side equal to it, `--native-boot-check`; `--native-boot`
starts a run from it). Left for the boot: see "The native boot, built" (what is not equal, other
EEPROMs, making it the default).
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

## The native boot: the handover state

(23 October; PLAN.md has the decision.) What the machine is at the instant the master first executes
0x06004000, and where each part comes from, so that it can be built in Rust without running the
BIOS. Tools: `tools/boot/` (all read the images from `build/rom`, nothing from them is written in
the repository). The reference state is made by `tools/boot/reach/build.sh WORK` (a private copy
of `crates/stv` with a patch: a snapshot at a breakpoint, a last-writer map of work RAM and sound
RAM, an EEPROM log; nothing in `crates/` changes), then
`WORK/target/release/driver . OUT 0x06004000` (about 17 s: dumps in OUT/dump, OUT/entry.state,
OUT/lastw.bin; env EEPROM, BACKUP, RTC, AREA, EELOG vary the inputs).

**The instant matters.** boot.rs's boot state is the end of frame 1159, before the BIOS's last
frame; the BIOS finishes in the next frame (the VDP2 fill loop at ROM 0x4018 is where
frame 1159 ends, and the cart is copied after it), so that state has no game in it (0x06004000 is
0xFF fill) and the game's image is in none of its dumps. The handover instant is mid-frame (frame
counter 1159, line 228). Everything below is that instant (`entry.state`).

### Work RAM-H (0x06000000, 1 MB) and RAM-L (0x00200000, 1 MB)

`tools/boot/recipe.py OUT` builds work RAM-H from the images alone and compares it with the real
one: **all but 898 bytes (0x382, in 16 extents) are equal**. Work RAM-L is the fill only.

| region | source | how |
|---|---|---|
| all of RAM-H and RAM-L | the BIOS's fill (ROM routine 0x0d04) | "SEGA" repeated, S at addresses divisible by 4 |
| 0x06000000-0x06000800 | BIOS ROM 0x000600-0x000e00 (ROM routine 0x0d1a copies) | memcpy; the vector table (VBR = 0x06000000) and the service table 0x06000200-0x06000800 |
| 0x06000800-0x06000c00 | zeros (ROM 0x305c-0x3062) | memset, then a few variables (below) |
| 0x06000c00-0x06003000 | BIOS ROM 0x030000-0x032400 | memcpy: the BIOS's RAM code (the 41 service functions); 0x06002864-0x0600287c and 0x06002ca0-0x06002cd4 are variables inside it, set at boot (pointers into the SCSP/VDP areas) |
| 0x06003000-0x06004000 | zeros | |
| 0x06004000-0x0603c000 | cart file 0x280000-0x2b8000 | **the game**, 0x38000 bytes, copied by ROM 0x4526 (long words) from the boot table in the cart header (below); this overwrites the BIOS's boot program (RAM 0x06010000-0x0603340c, from ROM 0x008000-0x02b40c), whose work areas are all inside it |
| 0x0603c000-0x060f0000 | fill | the game loads its other files later (.control at 0x06040000 and the overlays at 0x06080000 come from the cart by the game's own file loads) |
| 0x060f0000-0x060fa000 | BIOS ROM 0x066000-0x070000 | memcpy by the boot program (data, 0xFF-padded); never read in play1 (below) |
| 0x060fa000-0x060fb600 | zeros | boot program |
| 0x06000780-0x06000800 | the EEPROM image (128 bytes, as the BIOS reads all 64 words once) | memcpy; two copies of the settings (word 4 = 0x9015 is some checksum I could not reproduce) |
| 0x06000a00-0x06000b00 | pointer table: ROM 0x44fc (empty hook) and RAM code 0x06000c34, 0x060153de ... | written by ROM 0x44f4 and the RAM code |
| 0x060006c0-0x06000700 | the title string of the cart header (cart 0x200060 + 0x10 x region index: "RADIANT.S.GUN   ") | copied by the boot program (ROM 0xb040-0xb076), region from 0x06000248 bits 12-15 |
| 0x06000234-0x06000350, 0x06000604-0x06000678 | pointers into the BIOS ROM's routines (0x4114, 0x424c ... 0x4880, 0x34c4 ...) and a few settings | stored by ROM 0x02b0 (literals in the code: read them from the code, or run it), 0x320a-0x32d8, 0x3798, 0x4718-0x475a, 0x43e2, 0x339a (settings) |
| 0x06000650-0x0600065b, 0x060006b4-0x060006c0 | settings/area bytes (0x06000656 = area 1) | from the EEPROM; stored by ROM 0x3798, 0x4d54, 0x32be, 0x459c, 0x11ee and the boot program (0x06012762 ...) |
| 0x06000700-0x06000780, 0x06000807, 0x06000821-0x06000828 | zeros, then counters | ROM 0x3088-0x3096 (memsets) and RAM code; **0x06000740-0x06000747 depend on the backup RAM** (formatted: 03 00 01 02 00 00 00 02; blank: 00 00 01 00 00 00 00 00; RAM code 0x0600162c-0x06001664); **0x06000758 is the operating-time count** (run dependent) |
| 0x06041aa0-0x06041b28 | boot program variables (0x88 bytes) | never read (below) |
| 0x060fff34-0x06100000 | the boot program's stack residue | never read (below); the master's r15 is 0x06100000 |

The residue (898 bytes) is what the BIOS computes: pointers that are literals of its code, the
EEPROM-derived settings, zeros of its variable area, and the run-dependent counters. They are
listed by `tools/boot/variables.py OUT` and `recipe.py OUT --writers`, each extent with the ROM
or RAM address of the instruction that wrote it last.

### The cart's boot table

The cart header (ST-V, "SEGA ST-V(TITAN)" at file offset 0x200000) holds, at 0x200f10, one record
per region index (16 bytes: source, destination, count in long words, entry): for index 0 and 1
`0x22280000 -> 0x06004000, 0xe000, entry 0x06004000` (source 0x22280000 is cart file offset
0x280000), index 2 `0x222c0000, 0xc000`. ROM 0x4500 picks the record by the byte at 0x06000656,
copies (0x4526, the 4-byte loop at 0x454e), loads r15 from ROM word 0x0510 (0x06100000) and jumps to
the entry with `jmp @r7` after a `bsr` (so pr = 0x4516).

### The CPUs

Master at the handover: pc = 0x06004000, pr = 0x00004516, sr = 0x00000001 (T set, interrupt mask 0),
gbr = 0xfffffe00, vbr = 0x06000000, mach = macl = 0, r0 = 0x510, r1 = 0x22200000, r2 = 0x060040ec,
r3 = 0x06000254, r4 = 0x0603c000, r5 = 0x222b8000, r6 = 0, r7 = 0x06004000, r8-r14 = 0,
r15 = 0x06100000; no delayed branch, not sleeping. Registers r0-r5 are the loader's leftovers
(unlikely to matter; `main` clears the bss and sets its own stack).

Slave: not started. `slave_on` is false, its registers are zero, its on-chip registers the
power-on values (BCR1 = 0x83f0 ...). The game starts it (SSHON) later.

Master on-chip registers the BIOS set (all else power-on): BCR1 0x3f1, BCR2 0xfc, WCR 0x5555,
MCR 0x78, RTCSR 0x8, RTCNT 0, RTCOR 0x36 (the `bsc` array), IPRB 0x0100, VCRA 0x6061, VCRB 0x6263,
VCRC 0x6465, VCRD 0x6600, VCRWDT 0x6869, VCRDIV 0x6e, DMA vectors 0x6d and 0x6c, ICR 1, CCR 1
(cache on, the cache arrays hold zeros), WTCSR 0x1e, FRT: TIER 0x81, TOCR 0xe0, FTCSR 0x0e,
OCRA = OCRB = 0xffff. Run dependent: FRC (0x9cb0), the FRT's sub-tick count, and FTCSR's flags.

### The devices

SCU (set by the BIOS from its power-on values): IMS 0x3ffe (only VBlank-IN unmasked), DMA mode
registers 7 (three levels), T0C 0x3ff, T1S 0x1ff, AIACK 1, ASR0 = ASR1 = 0x1ff01ff0, AREF 0x1f,
RSEL 1. IST 0x2006 (pending bits 1, 2, 13) and the T0 line count 0xe5 are the instant's.

SMPC: last command 0x19 (RESENAB; `reset_enabled`), SNDON done (`sound_on` true), CKCHG352 done
(`clock352`, `dotsel` true), port data directions 0x3f and 0x18 with `direct` and `exle` set, the
I/O chip's registers 0x07 = 0x0c and 0x11 = 0x77. The area code is in `smpc.area` and changes
nothing (the region is taken from the EEPROM). The RTC is not read at all (a year later changed no
byte of RAM). Run dependent: `rtc_seconds` (19), `rtc_cycles`, the SCSP's sample clock
(`sound_owed`, `sound_in_sample`, `sound_steps`: 34 million 68000 steps).

VDP1: TVMR 0, FBCR 0, PTMR 2, EWRR 0x58e0, EDSR 3 (the others 0). Its VRAM holds the BIOS's RAM
test pattern (0xAA/0x55 bytes, 505,000 of 512 KB) with some data at 0x300fc-0x3025c: nothing in it
is the game's. VDP2: the display registers the BIOS set (TVMD 0x0001, RAMCTL 0x1323; non-zero
registers 0x00 0x0e 0x10-0x1e 0x20 0x36 0x4c 0x4e 0x78 0x7c 0x88 0x8c 0xbc 0xe0 0xe4
0xf8-0xfc 0x110-0x118), 3,317 non-zero bytes of VRAM (the BIOS screens' leftovers; 0x5e635fc-0x63650
is a ROM copy of 0x8ac-0x900) and 1,305 of CRAM (0x000-0x400 is BIOS ROM 0x55a24-0x55e44, 0x420-0x500
ROM 0x65c60, 0x820-0x900 ROM 0x2b2e0; the rest computed).

SCSP and the sound CPU: running (`sound_on`); sound RAM holds the BIOS's sound driver (68000
program "ver1.28 94/12/29" at 0x1000: **LZ-decompressed** by the ROM routine at 0x1764 from the
BIOS ROM, not a plain copy), tables copied from ROM 0x34f70 (to 0x10000, 0x125d bytes) and 0x361cc
(to 0x40000, 0xfb14 bytes), 0x6060 words (the fill loop at ROM 0x4018) over 0x20000-0x3ffff, and the
68000's own writes. The backup RAM is the saved file's (a blank one gets the BIOS's 22-byte
settings blocks at 0x3d00 and 0x3e80 and a header at 4-7; the header differs with the content).
The EEPROM: read-only during the boot when valid (the 64 words in `smpc.eeprom`).

### What does not matter (leftovers)

Each tested by editing a copy of `entry.state` (`tools/boot/stateedit.py`) and playing
`tools/oracle/play1.txt` from it to frame 4000 (`--load-state`), then comparing the end state
(`tools/boot/cmpstates.py`), and the picture and sound (`--shot`, `--wav`, byte for byte):

| edit | result |
|---|---|
| zero 0x060f0000-0x060fb600, 0x060fff00-0x06100000 and 0x06041aa0-0x06041b28 | the end state is equal but for the zeroed block itself (the game never touches it); picture and sound equal |
| zero the stack residue alone | end state identical |
| zero VDP2 VRAM and CRAM | end state identical; picture and sound equal |
| zero VDP1 VRAM | only VDP1 VRAM differs (the pattern where the game wrote nothing); picture and sound equal |
| zero sound RAM | only the unwritten part of sound RAM and the 68000's step count differ; sound equal |
| zero the fill in RAM-L or RAM-H above 0x06080000 | the game copies uninitialised bytes into places (VDP1 command tables, the slave's r8, parts of RAM-L, sound RAM); no register but those, picture and sound equal |

So the leftovers can be left out (zeros or the fill), and **the fill should be built anyway**:
it is cheap and keeps the state byte-for-byte equal to the interpreted boot's. Not tested: the
BIOS's test/service menu (kept interpreted), which may read the 0x060f0000 block.

### Depends on the run

The operating-time count (0x06000758) and the BIOS variables counting frames, FRC and the other
timers' counts (FRT sub-ticks, SCU T0 line count, SMPC RTC cycles, the SCSP clock), IST's pending
bits, the stack residue, the registers r0-r5, `line` (228) and `frame` (1159). No random seeds were found
(nothing but the counters above changes between runs of the same inputs). A native boot picks fixed values; boot.rs
already shows that two bytes of the operating-time count are all play1 sees of it.

### The native boot, built (23 October)

`crates/rsgun/src/native_boot.rs` builds this state from the images (`build(&Inputs)`: the BIOS,
the cart, the EEPROM and backup RAM files): work RAM-H and -L, the backup RAM's boot changes, both
CPUs' registers and the master's on-chip registers, the SCU, SMPC, VDP1/VDP2 registers, VDP2's
colour RAM, sound RAM with the sound CPU and the SCSP where the BIOS's boot leaves them, the
machine's line time. **`rsgun --native-boot`** (windowed or headless, with `--script`, `--compare`,
`--load-state` not) starts from it: no BIOS run, no boot.rs state (the saves' backup RAM and EEPROM
are read as usual). The default is unchanged (boot.rs's state, or the BIOS with `--cold`).

**Check it:** `tools/boot/reach/build.sh WORK` once (its patch to a private copy of crates/stv also
logs EEPROM reads and writes with env EELOG), then
`WORK/target/release/driver . out/port/boot-ref/default 0x06004000` (17 s; env EEPROM, BACKUP for
the inputs), then `target/release/rsgun --native-boot-check out/port/boot-ref/default
[--boot-eeprom FILE] [--boot-backup FILE]` (the same files the driver had; the default is the
images' EEPROM and a blank backup RAM). It builds the native state, loads `entry.state` into a second
machine, and prints each part: memories by extents (native / reference), the CPUs, on-chip
registers, SCU, SMPC by field, the SCSP's engine, the 68000, the line time. The exit code is nonzero
while any part differs.

**Equal now (default EEPROM, blank backup RAM):** work RAM-H and RAM-L byte for byte but for two
ranges listed as *ignored* (below), the backup RAM after the boot, both CPUs and on-chip registers,
the SCU, the SMPC, VDP1's registers and framebuffers, VDP2's registers and colour RAM, **sound RAM,
the SCSP's registers and engine (slots, DSP, counters) and the 68000, byte for byte**, `line` 228,
`frame` 1159, the cycles each CPU has run into line 228 (75 and 64). Not equal: VDP1 VRAM (0xc5be
bytes: the BIOS's pictures over the RAM test pattern) and VDP2 VRAM (0x6f5 bytes: the BIOS screens'
leftovers), both computed by its drawing code and never used by the game (a zeroed copy plays
identically, section above), and `sound_steps` (a statistic of 68000 steps, not compared).
*Ignored* by the check, listed with their number of differing bytes: the boot program's variables
(0x06041aa0-0x06041b28, inside the game's .bss, cleared by its `main`) and the stack residue below
the master's r15 (0x060fff34-0x06100000): never read (play1 from a state with them changed gave the
same pictures and sound, section above); the native boot no longer builds them.

**How work RAM is made** (each function names its ROM routine): the fills and copies come from the
tables the reset code reads (ROM 0x494-0x4e0 and the pointer table 0x530/0x564), read from the BIOS
image; the settings word, the device bytes 0x06000740-0x06000751 and the slot bytes come from the
EEPROM image's words and the backup RAM's settings block (the device scan RAM code 0x0600150e-0x060016ae
is ported for the case the EEPROM byte at 0x06000795 has bit 2 clear; the per-device scan of the
backup RAM is not); the title string from the cart header's title for the region in the settings
word; the pointers into the BIOS's own routines and small variables are literals of its code;
the slot scan's results (0x0600064c, 0x06000658), the area byte 0x06000656 (1) and a few counters
are the values found at the handover.

**How the sound is made** (`sound_ram`, `sound_driver_start`): sound RAM is the LZ-decompressed
driver (ROM 0x32000 into 0, the decoder at ROM 0x1760 ported: `lz_decode`), the copies from ROM
(0xa000, 0x10000, 0x40000, 0x50000) and the 0x6000 fill of 0x20000-0x3ffff. The BIOS loads the
driver three times in its boot (frames 5, 1149 and 1156; the last one is what is left); a
write trace of the main CPU to sound RAM below 0x1000 and the SCSP registers (the reach driver
with a patch printing them) shows what it says to the driver after the 68000 is released (SNDON,
frame 1157 line 173): the mailbox at sound RAM 0x700 (ROM 0x149a, 0x1502, 0x12f2-0x1330,
0x141c-0x142a: command words 0x0c00, 0x0d00, 0x0800 twice, 0x8700 with two argument bytes) and
three entries of the command queue (0x504, 0x514, 0x51c = 0x8001, ROM 0x3ffc-0x400c), at lines 0,
101, 199, 218 and 239 after the release. The native boot does the same: the 68000 is reset, the
SCSP's sample clock is set to the phase it had then (`Sys::set_sound_phase`), and the machine's
own sound step (`Sys::run_sound`, one line of cycles) runs 581 lines, the main CPU's writes made
between lines at the lines the BIOS made them; that gives the 68000's registers and the whole
sound RAM equal. The SCSP's counters (samples since its reset 851,200, the noise register, the sound
stack position and the DSP's down-counter) are the BIOS boot's at the handover, set as constants:
the envelope clock's parity and the noise decide how the first sound the game starts sounds
(without them the wav differed from the first sound effect on). The sound output made on the way
is thrown away.

**Checked (play1 to frame 4000, `tools/oracle/play1.txt`, own save dirs; out/port/boot-3):** from
the reference entry state (`--load-state` of OUT/entry.state) and from `--native-boot`: **the same
picture (`--shot`), the same sound (`--wav`, byte for byte) and the same end state** but for VDP1
VRAM (unused bytes) and `sound_steps`; also the same picture as the default run from power on.
`--native-boot --compare out/port/boot-3/play1.txt` (headless, `--script play1.txt --frames
4000`, 3.5 minutes): **658 functions, 151120432 calls, 151120417 equal, 0 diverged**, 15 too long to
check (the same 15 as the default run). Note the frame numbering: a native run starts at frame 1159,
line 228, as the reference does, so a script's frames and the reports' frame numbers agree with a
run that loaded `entry.state` (a run from boot.rs's state or the BIOS also has 1159 at the handover).
A run with a used save directory starts with its backup RAM (the previous run's scores): use a
fresh `--save` dir for comparisons.

**Other EEPROMs.** The BIOS accepts an EEPROM when it starts with "SEGA" and each of the two copies
(checksum word at bytes 8 and 68, the 56 bytes after its 4 bytes) has a matching checksum: a CRC-16
(polynomial 0x1021, in a 32-bit register starting at 0xdebdeb00, the BIOS's RAM code 0x060013bc,
called from ROM 0x49c4; `native_boot::settings_checksum`). The default's 0x9015 is that, so the
earlier "not a sum" puzzle is solved. Checked with 28 single-byte variants (valid checksums,
out/port/boot-4/sweep, made with the reach driver; the references are deleted, the EEPROM files kept): 23 give a native state equal in work RAM, backup
RAM, devices and sound (the FRT's sub-tick count is one or two ticks off in 10 of them: run dependent, the
boot's cycle count differs with the settings); **b28 and b29 (EEPROM word 14) and b38, b39 (word 19)
take the BIOS's repair path** (the value is out of range: it resets the settings block to its
defaults, 0x5c80 as the checksum, and rewrites the EEPROM and its RAM image and the backup RAM's
copy of a setting), which the native boot does not make (`build` says so when the checksum is wrong;
out-of-range values are not detected); **b26 never reaches the handover** (the BIOS's menu/error
path). To match these: port the range checks (ROM 0x4996 onwards and the settings routines at
0x4dc0, 0x372c) and the default block. eeprom.rs's write decoding is fixed (crates/stv, a unit
test): the repair now writes what the BIOS meant.

**Not done:** the BIOS-menu/error path (b26-like), the repair path, windowed `--native-boot` (the
code is the same as the headless one; only the headless run was tried), VDP1/VDP2 VRAM (never used),
and making it the default.

### Open questions

- The EEPROM's range checks and the repair path (see "Other EEPROMs" above); the checksum is known.
  (The eeprom.rs bug noted here before is fixed.)
- What in the 898 residue bytes reads the EEPROM (0x06000248 settings word, 0x06000650-0x0600065b,
  0x060006b4-0x060006c0): needs the code at ROM 0x339a, 0x3798, 0x4d54 and the boot program read.
- The 68000 driver in sound RAM and the BIOS's commands to it: solved ("How the sound is made").
- The VDP2 registers/CRAM computed bytes, and VDP1's: whether the game depends on any (nothing seen
  by frame 4000).
- Timing: the native boot starts at the instant's frame 1159, line 228, with the cycles into the line
  of the instant, so a run from it matches a run that loaded `entry.state`; the run-dependent items
  above (FRC, the FRT sub-tick count, the operating-time count) are the instant's values.

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
   makes it: `tools/rom/dis.py out/evidence/marathon-end.overlays/FP.bin 0 06080000 75776`; keep the
   `nop` lines: a jsr's delay slot is often a nop, and the next instruction shown then runs after
   the call), Ghidra's draft beside it, tables (jump tables, step
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
