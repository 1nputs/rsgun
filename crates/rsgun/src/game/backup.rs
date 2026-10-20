//! The backup RAM: the game's settings (from the BIOS) and its score tables, kept in the game's
//! area of the cartridge slot's backup RAM behind a signature. The game reaches the backup RAM
//! through the BIOS (its area, its 32-bit reads and writes) and reads and writes its bytes itself
//! (the odd bytes of each 16-bit word: the RAM is 8 bits wide).

use super::globals;
use super::Cpu;

/// The BIOS's functions (their addresses, in its table in RAM): the game's four settings (into a
/// buffer), the game's area of backup RAM, a 32-bit value read (1) or written (3) there, and the
/// area cleared.
const BIOS_SETTINGS: u32 = 0x0600_0634;
const BIOS_AREA: u32 = 0x0600_063c;
const BIOS_BACKUP_IO: u32 = 0x0600_0644;
const BIOS_CLEAR: u32 = 0x0600_0674;
const READ_LONG: u32 = 1;
const WRITE_LONG: u32 = 3;

/// The area's start, and where the game's data starts (past the signature).
const AREA: u32 = 0x0603_3270;
const DATA: u32 = 0x0603_3274;
/// The area's size in bytes, and the signature (zero-terminated): "Radiant SilverGun Ver 1...".
const AREA_BYTES: u32 = 0x778;
const SIGNATURE: u32 = 0x0600_9f80;

/// The settings as the game keeps them: the first (the difficulty, 0-4: values scale by it around
/// 2, math.rs, which the demos take; two copies: a demo sets the current one, a game takes the
/// kept one), the second (the lives a player starts with: one of the values listed at
/// SECOND_VALUES), the third (0-7), the fourth (0-1). What the others set is not known yet; the
/// cart holds their defaults.
const SETTING_0: [u32; 2] = [0x0600_ed84, 0x0600_edc5];
const SETTING_1: u32 = 0x0600_ebb2;
const SETTING_2: u32 = 0x0600_cc60;
const SETTING_3: u32 = 0x0600_edc4;
const SECOND_VALUES: u32 = 0x0600_9fb0;
const DEFAULT_SETTINGS: u32 = 0x2220_0f48;
/// Cleared with the settings read (not known yet).
const SETTINGS_CLEARED: u32 = 0x0600_edbc;
/// Set while the backup RAM is being written (by code not rewritten yet); cleared here.
const WRITING: u32 = 0x0600_cb54;

/// The score tables, `ENTRY_DATA` (dma.c's types: a name of 3 letters, the route, the score): six
/// tables of five, and a table of a hundred; their defaults (in the same order).
const SCORES: u32 = 0x0603_3180;
const TABLES: u32 = 6;
const TABLE: u32 = 5;
const MORE_SCORES: u32 = 0x0603_2e50;
const MORE: u32 = 100;
const DEFAULT_SCORES: u32 = 0x0600_9ff8;
const ENTRY: u32 = 8;

/// Why the backup is made anew: its settings were out of range (1), or its signature was not there
/// (2).
const BAD_SETTINGS: u32 = 1;
const NO_SIGNATURE: u32 = 2;

fn bios(cpu: &mut Cpu, function: u32, args: &[u32]) -> u32 {
    let f = cpu.read32(function);
    cpu.call(f, args)
}

/// Byte `i` of the game's data (signed).
pub fn backup_byte(cpu: &mut Cpu, i: u32) -> u32 {
    let data = cpu.read32(DATA);
    cpu.read8(data.wrapping_add(2 * i + 1)) as i8 as i32 as u32
}

/// The 32-bit value from byte `i` of the game's data.
pub fn backup_long(cpu: &mut Cpu, i: u32) -> u32 {
    let data = cpu.read32(DATA);
    bios(cpu, BIOS_BACKUP_IO, &[READ_LONG, data.wrapping_add(2 * i), 0])
}

/// Sets byte `i` of the game's data.
pub fn set_backup_byte(cpu: &mut Cpu, i: u32, v: u32) {
    let data = cpu.read32(DATA);
    cpu.write8(data.wrapping_add(2 * i + 1), v as u8);
}

/// Writes the 32-bit value at byte `i` of the game's data: what the BIOS returned.
pub fn set_backup_long(cpu: &mut Cpu, i: u32, v: u32) -> u32 {
    let data = cpu.read32(DATA);
    bios(cpu, BIOS_BACKUP_IO, &[WRITE_LONG, data.wrapping_add(2 * i), v])
}

/// Every score entry's address, in the order they are kept.
fn entries() -> impl Iterator<Item = u32> {
    let tables = (0..TABLES).flat_map(|t| (0..TABLE).map(move |e| SCORES + t * TABLE * ENTRY + e * ENTRY));
    tables.chain((0..MORE).map(|e| MORE_SCORES + e * ENTRY))
}

/// At the start: the settings from the BIOS, then the scores from the backup RAM; the backup made
/// anew if the settings are out of range or the signature is not there. The last value read.
pub fn load_backup(cpu: &mut Cpu) -> u32 {
    let settings = cpu.stack();
    bios(cpu, BIOS_SETTINGS, &[settings]);
    let mut bad = 0;
    let first = cpu.read8(settings);
    cpu.write8(SETTING_0[0], first);
    if first > 4 {
        bad = BAD_SETTINGS;
    }
    cpu.write8(SETTING_0[1], first);
    let second = cpu.read8(settings + 1);
    cpu.write8(SETTING_1, second);
    // one of the values listed (the list ends with a zero)
    let mut at = SECOND_VALUES;
    loop {
        let v = cpu.read8(at);
        if v == second {
            break;
        }
        if v == 0 {
            bad = BAD_SETTINGS;
            break;
        }
        at += 1;
    }
    let third = cpu.read8(settings + 2);
    cpu.write8(SETTING_2, third);
    if third as i8 > 7 {
        bad = BAD_SETTINGS;
    }
    let fourth = cpu.read8(settings + 3);
    cpu.write8(SETTING_3, fourth);
    if fourth as i8 > 1 {
        bad = BAD_SETTINGS;
    }
    cpu.write8(SETTINGS_CLEARED, 0);
    let area = bios(cpu, BIOS_AREA, &[]);
    cpu.write32(AREA, area);
    cpu.write32(DATA, area);
    if bad == 0 && !skip_signature(cpu) {
        bad = NO_SIGNATURE;
    }
    let last = if bad != 0 {
        format_backup(cpu, bad);
        save_scores(cpu)
    } else {
        let mut last = 0;
        for (i, entry) in entries().enumerate() {
            let i = i as u32 * ENTRY;
            for b in 0..4 {
                let v = backup_byte(cpu, i + b);
                cpu.write8(entry + b, v as u8);
            }
            last = backup_long(cpu, i + 4);
            cpu.write32(entry + 4, last);
        }
        last
    };
    cpu.write8(WRITING, 0);
    last
}

/// Whether the game's data starts with the signature: the data pointer is moved past each
/// character that matches (its terminator too).
fn skip_signature(cpu: &mut Cpu) -> bool {
    let mut i = 0;
    loop {
        let data = cpu.read32(DATA);
        if cpu.read8(data + 1) != cpu.read8(SIGNATURE + i) {
            return false;
        }
        cpu.write32(DATA, data + 2);
        if cpu.read8(SIGNATURE + i) == 0 {
            return true;
        }
        i += 1;
    }
}

/// The backup made anew (`why`: BAD_SETTINGS also takes the settings back to the cart's
/// defaults): the area cleared, the signature, the default scores (the first table's empty unless
/// the BIOS's setting is 1).
pub fn format_backup(cpu: &mut Cpu, why: u32) {
    if why == 0 {
        return;
    }
    if why == BAD_SETTINGS {
        for to in SETTING_0 {
            let first = cpu.read8(DEFAULT_SETTINGS);
            cpu.write8(to, first);
        }
        for (to, from) in [(SETTING_1, 1), (SETTING_2, 2), (SETTING_3, 3)] {
            let v = cpu.read8(DEFAULT_SETTINGS + from);
            cpu.write8(to, v);
        }
        cpu.write8(SETTINGS_CLEARED, 0);
    }
    bios(cpu, BIOS_CLEAR, &[]);
    let area = bios(cpu, BIOS_AREA, &[]);
    cpu.write32(AREA, area);
    cpu.write32(DATA, area);
    for i in 0..AREA_BYTES {
        set_backup_byte(cpu, i, 0);
    }
    // the signature, the data pointer past it
    let mut i = 0;
    loop {
        let c = cpu.read8(SIGNATURE + i);
        let data = cpu.read32(DATA);
        cpu.write8(data + 1, c);
        cpu.write32(DATA, data + 2);
        if c == 0 {
            break;
        }
        i += 1;
    }
    let first_table_empty = !globals::bios_setting_is_1(cpu);
    for (k, entry) in entries().enumerate() {
        let default = DEFAULT_SCORES + k as u32 * ENTRY;
        for b in 0..4 {
            let v = cpu.read8(default + b);
            cpu.write8(entry + b, v);
        }
        let score = cpu.read32(default + 4);
        cpu.write32(entry + 4, score);
        if (k as u32) < TABLE && first_table_empty {
            cpu.write32(entry + 4, u32::MAX);
        }
    }
}

/// Writes the score tables to the backup RAM: the last value the BIOS returned.
pub fn save_scores(cpu: &mut Cpu) -> u32 {
    let mut last = 0;
    for (i, entry) in entries().enumerate() {
        let i = i as u32 * ENTRY;
        for b in 0..4 {
            let v = cpu.read8(entry + b) as i8 as i32 as u32;
            set_backup_byte(cpu, i + b, v);
        }
        let score = cpu.read32(entry + 4);
        last = set_backup_long(cpu, i + 4, score);
    }
    last
}
