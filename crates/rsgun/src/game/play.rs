//! The players' play data (PLAY_DATA, dma.c's: two of them, 200 bytes each) made anew, and a
//! game's start.

use super::nested;
use super::globals::{CONTINUES, PLAY_DATA, PLAY_DATA_SIZE, SCORE};
use super::{hits, hud, Cpu};

/// The stage scores in PLAY_DATA (states.rs), each player's weapon experience (hits.rs), the
/// numbered objects (the players first).
const STAGE_SCORES: u32 = 0x48;
const EXPERIENCE: u32 = 0x0607_4070;
const PLAYERS: u32 = 0x0607_40f0;

/// PLAY_DATA's fields (by what they are reset to and how they are used): five words (not known
/// yet), each colour's longest chain (hits.rs), a byte, nine longs (not known yet), a byte set to
/// -1 (not known yet), the score and the long after it, the stage scores (6 x 5 longs by route,
/// states.rs), a long, the continues.
const WORDS_0: u32 = 0x00;
const LONGEST_CHAINS: u32 = 0x0a;
const BYTE_10: u32 = 0x10;
const LONGS_18: u32 = 0x18;
const BYTE_3C: u32 = 0x3c;
const SCORE_2: u32 = 0x44;
const LONG_C0: u32 = 0xc0;
/// Each player's weapon levels (three bytes: A, B, C) and experience (three longs), and the levels
/// and experience a player starts with.
const WEAPON_LEVELS: u32 = 0x0607_4088;
const START_LEVELS: u32 = 0x0600_ed78;
const START_EXPERIENCE: u32 = 0x0600_eda0;
/// A player object's field cleared as its player starts anew (a word, not known yet), and its
/// sword's gauge (hud.rs).
const PLAYER_90: u32 = 0x90;
const PLAYER_GAUGE: u32 = 0x50;
const GAUGE_START: u32 = 10000;

/// Player `p`'s play data anew: the score and stage scores, the chains' records and the counts
/// cleared, the weapons back to their starting levels and experience, the gauge full, the status
/// line reset.
pub fn player_reset(cpu: &mut Cpu, p: u32) {
    let data = PLAY_DATA + PLAY_DATA_SIZE * p;
    cpu.write8(data + BYTE_3C, 0xff);
    cpu.write32(data + SCORE, 0);
    cpu.write32(data + SCORE_2, 0);
    for i in 0..6 {
        for j in 0..5 {
            cpu.write32(data + STAGE_SCORES + 20 * i + 4 * j, 0);
        }
    }
    cpu.write32(data + LONG_C0, 0);
    for k in 0..3 {
        let v = cpu.read8(START_LEVELS + 3 * p + k);
        cpu.write8(WEAPON_LEVELS + 3 * p + k, v);
    }
    for k in 0..3 {
        let v = cpu.read32(START_EXPERIENCE + 12 * p + 4 * k);
        cpu.write32(EXPERIENCE + 12 * p + 4 * k, v);
    }
    for k in 0..5 {
        cpu.write16(data + WORDS_0 + 2 * k, 0);
    }
    for k in 0..3 {
        cpu.write16(data + LONGEST_CHAINS + 2 * k, 0);
    }
    cpu.write8(data + BYTE_10, 0);
    for k in 0..9 {
        cpu.write32(data + LONGS_18 + 4 * k, 0);
    }
    let player = cpu.read32(PLAYERS + 4 * p);
    if player != 0 {
        cpu.write16(player + PLAYER_90, 0);
        cpu.write32(player + PLAYER_GAUGE, GAUGE_START);
    }
    nested!(cpu, STATUS_RESET, hud::status_reset(cpu, p));
}

/// The rewrites' SH-2 entries, for the calls between them (mod.rs's nested!).
const STATUS_RESET: u32 = 0x0606_0354;
const PLAYER_RESET: u32 = 0x0604_114c;
const HITS_RESET: u32 = 0x0604_50d4;

/// Not rewritten yet: the play screen set up (VDP2, as far as can be told).
const PLAY_SCREEN_SET: u32 = 0x0605_e27c;

/// A game starts: both players' play data anew, neither continuing, the play screen set up, the
/// hits reset.
pub fn game_start(cpu: &mut Cpu) {
    nested!(cpu, PLAYER_RESET, player_reset(cpu, 0));
    nested!(cpu, PLAYER_RESET, player_reset(cpu, 1));
    cpu.write8(PLAY_DATA + PLAY_DATA_SIZE + CONTINUES, 0xff);
    cpu.write8(PLAY_DATA + CONTINUES, 0xff);
    cpu.call(PLAY_SCREEN_SET, &[]);
    nested!(cpu, HITS_RESET, hits::hits_reset(cpu));
}
