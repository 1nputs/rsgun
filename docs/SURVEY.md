# rsgun: day-zero survey

What the game asks of the hardware, measured on the oracle (Mednafen 1.32.1 with
`tools/patches/mednafen`). Started 19 October 2026 with a static look at the ROM and one
7,200-frame attract-mode run with no input (`tools/oracle/run.py --frames 7200`). Open questions are
at the end; each answer goes in here with the run that showed it.

## The ROM set

- MAME's `rsgun`: "Radiant Silvergun (JUET 980523 V1.000)". All five chips match MAME's SHA-1s
  (`tools/rom/mkcart.py` checks them). 18 MB of ROM in a 48 MB cart window at $02000000.
- The chips are stored as MAME loads them (16-bit words swapped). `mpr20958.7` goes at cart
  +$200000 and starts with the header "SEGA ST-V(TITAN)", "CART-A", "JUET", "RADIANT.S.GUN".
- The board carries the 315-5881 protection chip ("game start protection", per MAME). The game
  reads a counter-based stream at $04FFFFFC; Mednafen models it with a few lines
  (`src/ss/cart/stv.cpp`, `rsg_thingy`), and the game runs. Not a blocker.
- BIOS: the Japanese ST-V BIOS `epr-20091.ic8` (Mednafen picks the region from its database).

## How it was built

- Cygnus GCC 2.7-96q3 (the Saturn SDK's DOS-hosted GCC); standard frame-pointer prologues
  (`mov.l r14,@-r15; mov r15,r14`), literal pools. On SGL, Sega's graphics library (slDMACopy,
  slSetAmbient, slDivFX, slSpecialCCalcModeNbg0 ...), with libgcc helpers (`___mulsi3`).
- Ghidra 12.1.4 (SuperH:BE:32:SH-2) decompiles it cleanly: `dma_set` came out field for field as
  the debug info below describes it.

## What the cart's padding holds

The cart image was written from a development disk without clearing unused space. Extracted to
`build/leftovers/` by the tools in `tools/rom/` (never committed):

- A whole COFF object, `dma.c` (cart $288000), with full debug information: 42 structs, 3 unions,
  42 enums, 71 typedefs, with Treasure's names and member offsets. Among them the object system
  (`Base_obj`, $80 bytes: size, area, post, mode, `exec` callback, `before`/`next` links, $70 bytes of
  work), `PLAY_DATA` (chain counters per colour, time with each weapon, score per stage), `PAD_INF`,
  `DMA_WK`, `LOAD_FILE_INF`, the weapon list, object areas and sizes, the game modes, every sound
  effect and tune by name.
- About 4,400 lines of the game's C source (cart $2C8800-$2E8E00): a boss (`bos12`, about 2,700
  lines), the stage 2 enemy script (`STG02`), 3D hit detection (`HIT3D`); includes `bios.h`, `ryo.h`,
  `game.h`, `nami.h`, `scroll.h`, `ang_edt.c`, `mot_edt.c`.
- 136 lines of a linker map. They are from an older build: 2 of the 48 `.control` addresses fall on
  a function in the shipped code. The section layout is the same (`.control` at $06040000, `_main`
  at $06004000), so the names are hints, not addresses.
- At least one older build's code (two copies of `dma.c` linked at different addresses). Only what
  the BIOS loads and what runs counts.

## The load map (attract mode, frame 7,200; `tools/survey/loadmap.py`)

| Work RAM-H | From the cart | What |
|---|---|---|
| $06003000-$0600A9A4 | $0227F000- | the main program's code and data; `.bss` follows ($0600A9B8 is `dma_wk`), where the cart file holds disk leftovers |
| $0603327C-$0603C000 | $022AF27C- (the same file, the same offset) | more of the main image: both CPUs' idle loops are here (master $06034D02, slave $06036156, the addresses MAME's driver flags); what lies between is work RAM the game overwrote |
| $06040000-$060735E8 | $02300000- | `.control`: the game's code and SGL (about 210 KB) |
| $06080000-$060A5000 | $02342000- in the stage; $0233A800- at the title | an overlay window: code or data per mode and stage |

Work RAM-L ($00200000) holds data copied from many places (graphics, tables). The VBRs are
$06000000 (master) and $06000400 (slave).

## Measured so far

- Both SH-2s run the game (the slave's PC is in the game's code at frame 7,200).
- Display: 352×240 in play and attract; the title is 704×480 interlaced (high resolution).
- Sound: the 68000 runs Sega's standard Saturn sound driver ("Ver-2.2.0 << SEGASATURN >> Fri Jul 11
  1997" in sound RAM), so the game talks to it through the driver's standard command interface.
- The oracle runs headless at about three times real time with full SH-2 cache emulation.

## Open questions (the next runs)

| Question | How to see it | Decides |
|---|---|---|
| What does each overlay hold, and when is it loaded? | dumps at each stage, the title, the menus; the loader's calls | function identity (overlay, address); the Ghidra project's memory blocks |
| What does the slave SH-2 do? | evidence per CPU; the master's signals to it | the native model of the second CPU |
| Which interrupts are used, and what runs in each handler? | the vector tables at the two VBRs; a count per vector | the frame driver |
| Which VDP1 commands and VDP2 features? | command tables in VDP1 VRAM per frame; VDP2 register writes | what the runtime renders first |
| Is the SCU DSP used? | writes to the DSP's program port | a DSP port or interpreter |
| How much code runs, per CPU and overlay? | per-PC evidence | the porting estimate |
| What does the game read that depends on time? | timer (FRT) reads, VDP status, the HV counter | injectable sources for lockstep |
| Slowdown | frames where the logic misses a vertical blank | the owner's decision on pacing |
