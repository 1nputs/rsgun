# rsgun: Radiant Silvergun reimplementation plan

## Goal

Radiant Silvergun (Sega ST-V, MAME's `rsgun`, JUET 980523 V1.000) running natively in Rust, verified
against the oracle (Mednafen) frame by frame, the way `md-recomp` did NHL '94 and Gunstar Heroes
(md-recomp's `playbook/PLAYBOOK.md`). No SH-2 code left in a release.

## The owner's decisions (19 October 2026)

- **The SH-2 interpreter scaffold.** The runtime gets an SH-2 interpreter (development only) so the
  original program runs on our hardware model early; functions are replaced by Rust one at a time,
  each checked against the interpreted original on every call (in-process differential testing) and
  by the oracle's records. The interpreter is compiled out of releases once nothing calls it.
  Fallback: if machine accuracy becomes a swamp, the per-function harness alone (the pure playbook)
  carries on with nothing lost.
- **Rust.** One Cargo workspace: `crates/stv` (the runtime: no game knowledge), `crates/rsgun` (the
  game and its native client). Tools stay Python.
- **MIT**, the runtime written from scratch. Mednafen (GPL) is the oracle and the judge of behaviour,
  never a source to translate. Permissive code may be ported with credit (MAME's BSD-3 SH-2 and
  SCSP). The specification is Sega's hardware manuals plus what the oracle shows.
- **Smooth 60.** A logic frame every vertical blank; the arcade's slowdown is not reproduced (later
  a toggle, with a cost model). Lockstep resyncs after the oracle's lag frames.
- **One agent** at a time beside the lead. (21 October: two, each on its own overlay in its own
  worktree; CLAUDE.md has the rules.)
- **Names**: the repository and game are `rsgun`; the runtime crate is `stv` for now.
- **The BIOS** (23 October). Its services the game calls after the handover (its code in RAM at
  0x06000c00-0x06003000 and the few ROM routines: the operating-time count, the empty hook, the
  backup-RAM helpers) are rewritten as readable Rust in `crates/rsgun` (bios.rs: this BIOS's code
  as the game sees it), checked in compare mode like the game. A release starts with a **native
  boot**: the handover state (the game loaded from the cart, the BIOS's RAM area and service table
  from the BIOS ROM, the settings from the EEPROM, the machine's registers as the BIOS leaves them)
  built in Rust and checked byte for byte against the interpreted boot's state; the BIOS's intro
  is skipped. The BIOS ROM stays required (read at run time, never committed). The BIOS's
  test/service menu: decided later (kept interpreted meanwhile).

## The architecture

- **Machine**: two SH-2s (the interpreter, with the on-chip divider, timers, DMA and interrupt
  controller), SCU (DMA, interrupts, timers; the DSP if the survey finds it used), SMPC, the ST-V
  I/O, the cart and its protection stub, VDP1, VDP2, SCSP with a 68000 for Sega's standard sound
  driver (needed in any design). One scheduler drives them; the client owns the clock, as in
  pulseman.
- **Native code** runs behind the same bus: a ported function is registered at its address; when
  the interpreter reaches the address it calls the Rust function instead (registers in, registers
  out), and a Rust function calls a not-yet-ported one through the interpreter.
- **Checking**: in compare mode the interpreter runs the original and logs each ported function's
  call (its reads and writes in order, interrupt handlers' apart); when the call returns, the Rust
  runs from the same registers against the log (reads answered from it, writes checked against it,
  registers compared at the end) and touches nothing. Every play session tests every ported
  function. The oracle stays the judge of the machine: traces, lockstep, vectors.
- **Waits**: native calls run on their own stacks (fibers): code charges its cycles as it goes, and
  when the CPU's turn is spent the call gives the machine back until its next turn, so a ported wait
  waits as the original did, and interrupts are taken between its blocks.
- **State**: work RAM as native structs with a generated machine view (ninety-four's design); pointers
  in RAM as typed 32-bit handles, so the interpreter can read and write native state through the
  byte map during the scaffold years. The debug types recovered from the cart (`dma.c`'s) seed the
  structs.

## Phases

1. **The SH-2 core**, checked against the oracle: the oracle records a frame of each CPU (the
   addresses it executed, its data reads and writes in order, the interrupts it took); the core
   replays the frame with its loads answered from the record and must produce the same addresses
   and writes. Then unit tests for what play does not reach. *Done 19 October: 19 windows, 6.79
   million instructions, no difference (HANDOFF.md, status).*
2. **The machine boots the game**: bus, SCU, SMPC, I/O, timers, interrupts, both CPUs, the sound CPU;
   RAM hashes against the oracle at each frame. *It runs the whole attract mode from power on through
   the real BIOS (20 October); it runs ahead of the oracle's pace.*
3. **Pictures**: VDP2, then VDP1, each against the oracle's register and memory traces (vectors),
   then the game's own frames. *VDP2 exact on 11 of 12 frames; VDP1 pixel for pixel with the
   oracle's (20 October).*
4. **Sound**: the 68000 and the SCSP; the driver's output against the oracle's. *Done 20 October:
   all but 8 of 2.65 million samples equal over a minute of play.*
5. **The port**: a mechanical SH-2 to Rust translation of every function first (exact, checked by
   compare mode), then each rewritten by hand into readable Rust with native structs, Ghidra's C as
   the guide to names and types, leaf first by area: SGL, the object system, the player and weapons,
   enemies, stages and bosses, the front end. *20 October: 534 of the main program's 540 functions
   translated, every call equal over play1 and the attract mode (488 million calls); the overlays
   and interrupt handlers next.*
6. **Native**: the interpreter compiled out; golden runs; lockstep; states; the front end; release.
