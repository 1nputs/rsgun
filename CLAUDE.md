# Working on rsgun

Radiant Silvergun (Sega ST-V) rewritten as readable Rust, every rewrite checked call by call
against the game's own SH-2 code in compare mode. Read `docs/HANDOFF.md` first: its top says where
the work is and what comes next; `docs/PLAN.md` has the owner's decisions.

## Rules

- Nothing from the ROM is committed: not the ROM, not images, dumps, evidence, save states,
  translations (`build/port`), Ghidra's output, the cart's leftovers, or data tables copied out of
  it (a rewrite reads them where the game keeps them).
  They live in `roms/`, `build/` and `out/` (git-ignored). Game knowledge (names, field offsets,
  addresses, constants the code uses) is fine in `crates/rsgun`; none in `crates/stv`.
- The oracle (Mednafen, GPL) is a development tool: never shipped, never linked, never translated.
- One agent at a time: no subagents.
- Commit after each finished, checked batch and go straight on; stop only for a decision the owner
  must make. Commits are made as `git -c user.name=1nputs -c user.email=1nputs@protonmail.com
  commit`, end with the Co-Authored-By line Claude Code gives, and carry no session links and no
  person's or machine's name.
- A rewrite is committed only when compare mode has run it with every call equal (`docs/HANDOFF.md`
  section 6, step 1, says how to pick the run); `cargo clippy --release -p rsgun` clean first.

## The loop for an overlay function

1. Read its disassembly (`build/dis/FINGERPRINT.dis`; delay slots: the instruction after a
   delayed branch or call runs before it lands) and Ghidra's draft
   (`build/ghidra/c/f_ADDR_FINGERPRINT.c`: a guide to its shape only, it drops arguments and
   misreads delay slots). Tables it reads come from the overlay image
   (`out/evidence/*.overlays/FINGERPRINT.bin`, the window from 0x06080000).
2. Write it in its overlay's file (`crates/rsgun/src/game/stg01.rs` and so on) as the code around it
   is written: named fields and constants with a line of doc each, `nested!(cpu, ENTRY, f(...))`
   where the SH-2 code calls a rewritten function, `cpu.call(ENTRY, &[...])` for one not yet
   rewritten, `cpu.nested_with_stack` past four arguments, structs on the stack at `cpu.stack()`
   with only the fields the SH-2 code writes.
3. Register it in `crates/rsgun/src/game/mod.rs` (`rewrites! { n; overlay "STG01"; ... }`, in
   address order), build, then compare on a segment it runs in and commit when all is equal.
