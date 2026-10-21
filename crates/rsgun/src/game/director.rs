//! The game's director: an object made as play starts (game_set) that runs the game's current
//! state each frame (a table of functions), keeps the game's speed (a game frame passes only when
//! the speed has added up to one: slow motion, and pauses), keeps the players' positions for the
//! last 32 game frames (enemies aim at where a player was), and keeps the coins and credits
//! (through the BIOS, with their sounds). A `SYS_OBJ` (dma.c's types): its `SYS_WK` holds the
//! point enemies aim at when no player is there.

use super::nested;
use super::globals::{ELAPSED, GAME_FLAGS, GAME_FRAMES, HELD, LAST_VBLANKS, LOOPS, VBLANKS};
use super::{objects, sound, system, Cpu};

/// The director (0 when there is none).
pub const GAME: u32 = 0x0607_46e0;
const GAME_SIZE: u32 = 0x2a8;
const GAME_EXEC: u32 = 0x0604_2b14;

/// Its `Base_obj` mode: 0 starting, 1 running, 'p' to stop, 's' stopping (freed).
const MODE: u32 = 0x03;
const STARTING: u8 = 0;
const RUNNING: u8 = 1;
const TO_STOP: u8 = b'p';
const STOPPING: u8 = b's';
/// Its `SYS_WK` (initialised by SYS_WK_INIT): x, y, z.
const MW: u32 = 0x10;
const X: u32 = 0x14;
const Y: u32 = 0x18;
const Z: u32 = 0x1c;
/// The game's state (a word: its step, then its index in the table of states), a byte of flags
/// (bit 1 set by code not rewritten yet), the coins counted (+1), and a coin or credit sound to
/// play.
const STATE: u32 = 0x3c;
const STATE_INDEX: u32 = 0x3d;
const COUNTED: u32 = 0x3f;
const COIN_SOUND: u32 = 0x40;
/// Not known yet (cleared at the start).
const SIDE: u32 = 0x32;
/// The game's speed: what it has added up to (16.16; a game frame each time it passes one), what
/// it adds each VBlank, and what a held button adds while paused (debug).
const SPEED_SUM: u32 = 0x58;
const SPEED: u32 = 0x5c;
const STEP: u32 = 0x60;
const ONE: u32 = 0x10000;
/// The players' positions (x, y for each), the last 32 game frames, the newest first.
const HISTORY: u32 = 0xa8;
const HISTORY_LEN: u32 = 32;
const HISTORY_ENTRY: u32 = 16;

/// The table of the game's states (a function each, called with the director).
const STATES: u32 = 0x0607_23f8;
/// SYS_WK's initialisation (sprites.rs's sprite_init).
const SYS_WK_INIT: u32 = 0x0604_6264;

/// The game's flags (globals::GAME_FLAGS): paused by player 1, by player 2, otherwise held (no
/// speed adds up).
const PAUSED_1: u32 = 0x1000;
const PAUSED_2: u32 = 0x2000;
const STOPPED: u32 = 0x7000;
/// Debug flags (cleared at the start; how they are set is not known yet): with bit 3 set and bit 2
/// clear, a pausing player's pad bit 3 steps the game on.
const DEBUG_FLAGS: u32 = 0x0600_ed8c;
const PAD_1: u32 = 0x0602_ee10;
const PAD_2: u32 = 0x0602_ee18;
/// While set, the game runs a game frame every VBlank whatever its speed (not known yet).
const FULL_SPEED: u32 = 0x0600_ed90;

/// The BIOS's functions (in its table): the coin and credit state brought up to date (into
/// COINS), the coins counted, the credits to sound.
const BIOS_COINS: u32 = 0x0600_0660;
const BIOS_COUNTED: u32 = 0x0600_0670;
const BIOS_CREDITS: u32 = 0x0600_0668;
const COINS: u32 = 0x0600_ebc0;
/// The fourth setting (backup.rs) on: a button on the I/O board (port 0x20400005 bit 7) is read,
/// its edges kept, and a press handed to code not rewritten yet (as 0x800).
const SETTING_3: u32 = 0x0600_edc4;
const EXTRA_PORT: u32 = 0x2040_0005;
const EXTRA_HELD: u32 = 0x0607_3c70;
const EXTRA_CHANGED: u32 = 0x0607_3c71;
const EXTRA_PRESSED: u32 = 0x0607_3c72;
const EXTRA_BUTTON: u32 = 0x0604_0e30;

const SE_COIN: u32 = 48;
const SE_CREDIT: u32 = 49;

/// game_set: the director made (object list 0), the game's counts from zero: it, or 0.
pub fn game_set(cpu: &mut Cpu) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, GAME_SIZE));
    let game = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 0, blocks as i8 as i32 as u32, GAME_EXEC));
    cpu.write32(GAME, game);
    if game == 0 {
        return 0;
    }
    for counter in [LOOPS, GAME_FRAMES, LAST_VBLANKS, VBLANKS] {
        cpu.write32(counter, 0);
    }
    cpu.write32(ELAPSED, 1);
    nested!(cpu, SYS_WK_INIT, super::sprites::sprite_init(cpu, game + MW));
    let game = cpu.read32(GAME);
    cpu.write16(game + STATE, 0);
    cpu.write32(game + X, 0);
    cpu.write32(game + Y, 0x40_0000);
    cpu.write32(game + Z, 0);
    cpu.write8(game + SIDE, 0);
    let game = cpu.read32(GAME);
    cpu.write32(game + SPEED_SUM, 0);
    cpu.write32(game + SPEED, ONE);
    cpu.write32(game + STEP, 0x4000);
    cpu.read32(GAME)
}

/// The director's frame.
pub fn game_exec(cpu: &mut Cpu, game: u32) {
    match cpu.read8(game + MODE) {
        STARTING => {
            system::bios(cpu, BIOS_COINS, &[COINS]);
            for b in [EXTRA_PRESSED, EXTRA_CHANGED, EXTRA_HELD] {
                cpu.write8(b, 0);
            }
            cpu.write8(game + MODE, RUNNING);
            run(cpu, game);
        }
        RUNNING => run(cpu, game),
        TO_STOP => cpu.write8(game + MODE, STOPPING),
        STOPPING => {
            cpu.write32(GAME, 0);
            nested!(cpu, 0x0600_579c, objects::obj_free(cpu, game));
        }
        _ => {}
    }
    coins(cpu, game);
}

fn run(cpu: &mut Cpu, game: u32) {
    if cpu.read8(SETTING_3) != 0 {
        let held = !cpu.read8(EXTRA_PORT) & 0x80;
        let changed = cpu.read8(EXTRA_HELD) ^ held;
        cpu.write8(EXTRA_HELD, held);
        cpu.write8(EXTRA_CHANGED, changed);
        cpu.write8(EXTRA_PRESSED, changed & held);
        let press = if changed & held != 0 { 0x800 } else { 0 };
        cpu.call(EXTRA_BUTTON, &[press, 0]);
    }
    if cpu.read32(GAME_FLAGS) & STOPPED == 0 {
        let sum = cpu.read32(game + SPEED_SUM);
        let add = if cpu.read8(FULL_SPEED) == 0 {
            cpu.read32(game + SPEED).wrapping_mul(cpu.read32(ELAPSED))
        } else {
            ONE
        };
        cpu.write32(game + SPEED_SUM, sum.wrapping_add(add));
    }
    let moved = cpu.read32(GAME_FLAGS) & HELD == 0;
    if moved {
        // the history moves down a frame (the newest entry is the state's to fill)
        for i in (1..HISTORY_LEN).rev() {
            for w in 0..4 {
                let v = cpu.read32(game + HISTORY + (i - 1) * HISTORY_ENTRY + 4 * w);
                cpu.write32(game + HISTORY + i * HISTORY_ENTRY + 4 * w, v);
            }
        }
    }
    let state = cpu.read8(game + STATE_INDEX) as i8 as i32;
    let f = cpu.read32(STATES.wrapping_add((4 * state) as u32));
    // the SH-2 code holds the game in r8, and in r9 the history loop's pointer (the game at its
    // end; the history held, the mode byte's address): the state's prologue pushes them (read
    // later as an unset field)
    let r9 = if moved { game } else { game + 3 };
    cpu.holding(&[(8, game), (9, r9)], |cpu| cpu.call(f, &[game]));
    // paused, a debug step
    let debug = cpu.read32(DEBUG_FLAGS);
    if debug & 8 != 0 && debug & 4 == 0 {
        let mut stepping = 0;
        if cpu.read32(GAME_FLAGS) & PAUSED_1 != 0 && cpu.read16(PAD_1) & 8 != 0 {
            stepping = 1;
        }
        if cpu.read32(GAME_FLAGS) & PAUSED_2 != 0 && cpu.read16(PAD_2) & 8 != 0 {
            stepping += 2;
        }
        if stepping != 0 {
            let (sum, step) = (cpu.read32(game + SPEED_SUM), cpu.read32(game + STEP));
            cpu.write32(game + SPEED_SUM, sum.wrapping_add(step));
        }
    }
    // a game frame when the speed has added up to one
    let sum = cpu.read32(game + SPEED_SUM);
    let flags = cpu.read32(GAME_FLAGS);
    if sum > 0xFFFF {
        cpu.write32(game + SPEED_SUM, sum & 0xFFFF);
        cpu.write32(GAME_FLAGS, flags & !HELD);
    } else {
        cpu.write32(GAME_FLAGS, flags | HELD);
    }
}

/// The coins counted, a coin or credit sound for credits the BIOS says came in, the coin state
/// brought up to date.
fn coins(cpu: &mut Cpu, game: u32) {
    let counted = system::bios(cpu, BIOS_COUNTED, &[]);
    cpu.write8(game + COUNTED, counted.wrapping_add(1) as u8);
    let credits = system::bios(cpu, BIOS_CREDITS, &[]) as i16 as i32;
    if credits > cpu.read8(game + COIN_SOUND) as i32 {
        cpu.write8(game + COIN_SOUND, credits as u8);
    }
    let pending = cpu.read8(game + COIN_SOUND);
    if pending != 0 {
        let se = if pending == 1 { SE_COIN } else { SE_CREDIT };
        if nested!(cpu, 0x0600_50d4, sound::se_set(cpu, se)) as i8 != 0 {
            cpu.write8(game + COIN_SOUND, 0);
        }
    }
    system::bios(cpu, BIOS_COINS, &[COINS]);
}

/// The director's flags byte (states.rs: bit 0 a mode runs, bit 1 the mode is done).
const FLAGS: u32 = 0x3e;
const MODE_DONE: u8 = 2;

/// The current mode is done (its code says so): the director moves on. With no director the game
/// stops here (a trap left in the game).
pub fn mode_done(cpu: &mut Cpu) {
    let game = cpu.read32(GAME);
    if game != 0 {
        let f = cpu.read8(game + FLAGS);
        cpu.write8(game + FLAGS, f | MODE_DONE);
        return;
    }
    loop {
        cpu.wait(0x0604_0c72, 1);
    }
}

/// The current mode is skipped (its code says so). With no director the game stops here.
const MODE_SKIPPED: u8 = 4;

pub fn mode_skipped(cpu: &mut Cpu) {
    let game = cpu.read32(GAME);
    if game != 0 {
        let f = cpu.read8(game + FLAGS);
        cpu.write8(game + FLAGS, f | MODE_SKIPPED);
        return;
    }
    loop {
        cpu.wait(0x0604_0c9a, 1);
    }
}

/// The director's flags.
pub fn director_flags(cpu: &mut Cpu) -> u32 {
    let game = cpu.read32(GAME);
    cpu.read8(game + FLAGS) as u32
}
