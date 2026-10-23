# Working with agents: the orchestrator and its batch agents

How the overlay rewrite was run from 22 October 2026 on, and what carries over to the next ST-V or
Saturn game done the same way. HANDOFF.md section 6 has the per-function loop; this file has the
way the work is split, briefed, checked and merged.

## The arrangement (the owner's decision, 22 October)

- **One orchestrator** (the main session, the strongest model available). It writes no overlay
  functions itself once batch agents are running. It scaffolds, briefs, reviews, merges, keeps
  the docs, and fixes what a batch could not.
- **Two coding agents** at a time (Sonnet 5.5), one per overlay, each a fresh subagent for **one
  batch** (a third only while another is mostly waiting on its compare runs: three at once spent
  20% of the owner's usage window in 45 minutes, 22 October): about
  20-25 functions of one overlay in address order, written, checked in compare mode, committed on
  its branch, reported, then the agent ends. The next batch is a new agent with a new brief, so
  no agent's context grows and usage stays even (long-lived agents hit the hourly limit; batch
  agents finish inside it).
- Each overlay has its own worktree and branch, `git worktree add ../rsgun-stgNN -b wip-stgNN main`,
  with `out/` and `build/` symlinked from the main checkout (`ln -s <main>/out <wt>/out`, same for
  build). One agent per worktree at a time.
- Compare jobs: a batch's time is mostly waiting on compare segments (16-40 minutes each), so
  spare cores buy another agent, not more jobs per agent. With the machine to itself, leave two
  cores for builds: on 16 cores, 14 jobs (4 per coding agent, 2 for the orchestrator); a job takes
  about 100 MB. On a shared machine, half the cores.

Measured (22 October, 7 Sonnet batches): about 25 functions, 55 minutes and 300k agent tokens a
batch; two agents together about twice the rate of the orchestrator writing alone, with its
context kept for review.

## Scaffolding (orchestrator, before an overlay's first batch)

1. Confirm the overlay's name and fingerprint at run time (`states::overlay_name`, game mode ->
   file name). Do not trust a list's order: STG04 and STG05's fingerprints were swapped in the
   first brief, and the first batch's rewrites never ran (a block registered under the wrong
   overlay name is silently never called; its report shows only `f_ADDR_FP` names).
2. Find the segments that run it: count `_FINGERPRINT` lines per marathon report.
3. Create `crates/rsgun/src/game/stgNN.rs` (module doc, imports), `mod stgNN;` in mod.rs, and a
   marker comment where its `rewrites! { n; overlay "STGNN"; ... }` block goes (an empty
   `rewrites!` block does not compile; the first agent creates it with its first entry). Commit.

## The brief (one per batch; the agent knows only this)

Every brief has: the worktree and branch (and "rebase on main first" if main moved), "touch no
other worktree, do not merge, no subagents"; what to read (CLAUDE.md; HANDOFF section 6 step 1
and section 3's stale-stack paragraph; the overlay's own file and stg01.rs as style models); the
overlay's fingerprint, disassembly, Ghidra drafts, image; the start address and the next few
addresses; the segments and the grep that finds which functions run; the held-back rule; known
gotchas from the last report; the build/copy/run commands with the job cap and the safe `pgrep`
pattern; "commit only what ran equal, exactly the source checked"; the commit identity, a blank
line after the subject, the Co-Authored-By line naming the agent's model; and "report in under 250
words: commits, range done, held back, uncommitted and why, gotchas". The reports' gotchas go
into the next brief for that overlay.

## Review and merge (orchestrator, when an agent reports)

1. `git log --oneline main..wip-X`, `git diff --stat main...wip-X`: only the overlay's file and
   mod.rs (a shared module change is its own commit); commit messages (subject, blank line,
   body, Co-Authored-By; no session links or names).
2. The reports the agent names: every new function present as `overlay::name` with calls > 0 and
   no `leaves memory otherwise` line (`grep -c 'stgNN::'`, `grep -c otherwise`) in the LAST run of
   each segment: list every report of the batch's runs, not only the ones the agent quotes. A fix
   checked only by a short repro is rerun on the full segment before merging (an STG02 batch was
   merged with its f090000 report still diverging; the full rerun confirmed the fix afterwards).
   Every segment that runs a new function must be among them: grep the new addresses in all the
   overlay's segments' marathon reports (an STG05 batch ran f170000-f190000 but one function also
   ran in f200000; the orchestrator ran it before merging).
   `tools/port/check.sh` (play1, the attract mode) also runs after a batch of an overlay those
   runs load (STG01: a leaver's step compared its turn unshifted; only play1 and the attract
   mode reached that step, a merged batch later).
3. No ROM data copied in: small constants the code uses are fine (literal-pool values, a few
   immediates); tables are read where the game keeps them. `grep "^const .*: \["` and look.
4. Read a function or two for style and sense (docs a reader understands, fields named).
5. No job or wait loop of the agent's left running (`pgrep -af "rsgun --[h]eadless"`; a stale
   `until [ -f ... ]` loop is killed by its pid).
6. Uncommitted held-back code in the worktree: `git stash`, `git rebase main` (resolve mod.rs:
   both `mod` lines, both blocks), `cargo clippy --release -p rsgun` on the committed tree, fast-
   forward main from the main checkout (`git merge --ff-only wip-X`), `git stash pop`.
7. Start the next batch with a fresh agent.

8. An overlay is done only when a whole-game check on main (`check_marathon.sh`) lists none of its
   functions as `f_ADDR_FP` with calls: STG02's batches ran f060000-f140000, but its code also runs
   at the start of f150000 (before STG04 loads), and two "never run" groups ran there and in
   f140000.

- **Partly run functions are registered.** A function counts when every call that ran was equal,
  as with any rewrite's branches not reached; holding one back because only its first stage ran
  leaves running SH-2 code (the RANK batch held back its result frame, 4 equal calls; the
  orchestrator registered it again). Extra scenarios (patched states, scripted inputs) are worth
  it only when asked: that batch spent 640k tokens, twice a normal batch, building them.

## What worked

- Compare mode as the arbiter: Sonnet batches came back with real bugs found and fixed by the
  agents themselves (a missed `jsr` in a branch, a `mov.w` constant not sign-extended, a ribbon's
  flags, a boss event that was not the shared one). Nothing is merged on review alone.
- The repro recipe (HANDOFF section 3: a state just before frame F, then `--compare` with
  `STV_COMPARE_DUMP=ADDR` plain hex, `STV_COMPARE_DUMP_DIR`) turns a 40-minute segment into
  seconds; the dump's `W`/`R`/`P` lines show which push wrote a stale slot.
- Committing exactly the checked source: snapshot the files when building, commit the snapshot
  (`git hash-object -w` + `git update-index --cacheinfo`) while new work goes on in the tree.
- Address order within an overlay, with never-run functions held back and called through
  `cpu.call`; switch a call site to `nested!` when its callee is rewritten.

## Pitfalls (SH-2, ST-V, the tools)

- **Stale stack.** The game reads struct fields its caller never set; their bytes are whatever an
  earlier callee's prologue pushed. A rewrite that calls the same code with other values in
  r8-r13 (or PR) leaves other bytes. Symptom: a caller "leaves memory otherwise" while every
  callee is equal, often in an object or a freed block. Fix: `cpu.holding(&[(n, v)...], ..)` with
  the SH-2 code's register values at that call (read them off the disassembly: loop counters,
  table pointers, the object), or `cpu.call_returning(PR, ..)`. Found in STG01 (targets' rotation
  matrix, the arms' aim), STG02 (thruster_set's obj_size).
- **A new recording finds stale-stack reads in merged code.** The stage 4 route (STG03, never
  in the marathon) made obj_exec diverge in eight segments before any STG03 rewrite: STG03's code
  reads stack slots its callers never set, and merged rewrites left other bytes there (a nested
  call's pushed return address: `nested!(cpu, ENTRY, returning PR, f(...))` pushes the SH-2
  code's; loop variables the SH-2 code keeps in its frame: the rewrite writes their final values).
  Six fixes (shared modules) went in after check.sh and the whole marathon passed on their branch.
- **Delay slots** run before the branch lands; a `bt/s`'s slot runs both ways; a compare in a delay
  slot sets T after the branch decided. Recount every `jsr` in a path.
- **Sign extension**: `mov.b`/`mov.w` loads and `mov.w` literal-pool constants (0x8000 and up are
  negative); `extu` only where the code has it. 16-bit angle arithmetic wraps; |d| of a
  sign-extended -0x8000 is 0x8000 (use i32, not i16's abs).
- **Return values**: a rewrite registered `-> ()` leaves r0 undefined; a caller (a timeline
  event, a table dispatch) may read it. Check callers, register `-> u32` where read (an event
  whose r0 is a leftover loop offset still means "done").
- **Arguments in r5 a caller loads in a delay slot** (the STG01 arms' step table got the boss in
  r5): pass them in the `cpu.call`.
- **Stack arguments past the fourth**: `[a, b, c, d; e, f]` in the registration; callers that push
  and do not pop between calls are mirrored with `cpu.push`/`cpu.pop`.
- **Scratch pushes read as a prologue**: compare mode reads a function's opening pushes as its
  prologue; a function that pushes and pops scratch words as it goes (STG05's matrix multiply)
  "leaves memory otherwise" on every call until it gets a `PROLOGUES` entry in mod.rs with no
  pushes.
- **Ghidra's offsets** can be shifted by its idea of the argument (STG05's draw: +0x10, the
  argument being `o + 0x10`); take offsets from the disassembly.
- **Overlay images end before their tables**: read tables at run time from memory, not from the
  evidence's .bin, when they lie past the image.
- **Report labels**: a marathon report names an address by one overlay's fingerprint; in a
  segment where several overlays run, `f_ADDR_otherFP` may be this overlay's function. Check your
  own compare runs' reports.
- **Shell**: never `pkill -f`/`pgrep -f` with a pattern your own shell's command line matches
  (`rsgun --[h]eadless`); no foreground `sleep`; wait with a background `until` loop.
- **Usage limits**: long-lived agents hit them; batch agents with short reports do not. If an
  agent stops on a limit, message it to resume rather than starting over.

## For the next game

The same order works: the oracle and evidence first (the marathon's input script and states),
the main image's code, SGL/library code (recognise copies linked into each overlay: STG01 had its
own SGL rotation routines), then overlays by fingerprint, one worktree each, batches as above.
Keep the per-overlay files' style uniform (one completed overlay as the model), and write the
held-back list and gotchas into HANDOFF as each overlay finishes.
