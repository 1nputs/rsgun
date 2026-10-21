//! Sound: the game talks to Sega's sound driver (on the 68000, in sound RAM) as Sega's sound
//! library does, by commands in eight 16-byte slots at sound RAM 0x700: a command's parameters
//! first, its number last (the driver clears it when it has done the command). Slot 0 takes the
//! driver's settings; slots 1-7 its sequence commands (start, stop, volume). Above this the game
//! keeps a sound manager: the background music, its fades, and the sound effects (se_set, the
//! leftover source's name), each effect played as a sequence on one of the driver's controls.

use super::nested;
use super::{clib, dma, files, Cpu};

/// Sound RAM as the SH-2 sees it, and the command slots in it.
const SOUND_RAM: u32 = 0x25a0_0000;
const SLOTS: u32 = 0x25a0_0700;
const SLOT: u32 = 16;
const SLOT_COUNT: u32 = 8;
/// The command the game builds before sending: its number, then its parameters from byte 2.
const COMMAND: u32 = 0x0600_aa84;

/// The driver's commands, as Sega's sound library numbers them.
const START_SEQ: u8 = 0x01;
const STOP_SEQ: u8 = 0x02;
const SEQ_VOLUME: u8 = 0x05;
/// Settings: five flags (bits 0-4 of its argument; what they switch is not known yet), the total
/// volume, the effect bank and mixer, and a mixer's level and pan.
const SET_FLAGS: u8 = 0x10;
const TOTAL_VOLUME: u8 = 0x83;
const MIXER: u8 = 0x87;
const MIXER_PARAM: u8 = 0x88;

/// The sound RAM map the driver loaded (its address, from sound RAM's start, at 0x408): 8-byte
/// areas (bits 4-5 the area's type, 0-3 its number, then its address in 24 bits; bit 7 of the
/// first byte ends the map; bit 7 of byte 4 marks the area loaded).
const MAP_POINTER: u32 = 0x25a0_0408;
/// The game's sound data files, by number: name, then the area's type (+4) and number (+5).
const SOUND_FILES: u32 = 0x0600_89dc;

/// A loop's pass waiting for the driver (the SH-2 code's loop, and about what a pass of it cost).
const WAIT_PASS: u32 = 40;

/// Whether the driver has done the command in `slot` (1) or not yet (0).
pub fn command_done(cpu: &mut Cpu, slot: u32) -> u32 {
    (cpu.read8(SLOTS + SLOT * slot) == 0) as u32
}

/// Puts the command at `command` (`len`: its last byte) in `slot` if the driver has done the one
/// there: 1, or 0 if not.
pub fn send_to(cpu: &mut Cpu, slot: u32, command: u32, len: u32) -> u32 {
    let at = SLOTS + SLOT * slot;
    if cpu.read8(at) != 0 {
        return 0;
    }
    for i in 2..=len as i16 as i32 {
        let b = cpu.read8(command + i as u32);
        cpu.write8(at + i as u32, b);
    }
    let number = cpu.read8(command);
    cpu.write8(at, number);
    1
}

const SEND: u32 = 0x0600_47bc;

/// Puts the command in the first sequence slot the driver has done: 1, or 0 if none.
pub fn send(cpu: &mut Cpu, command: u32, len: u32) -> u32 {
    (1..SLOT_COUNT).any(|slot| nested!(cpu, 0x0600_4718, send_to(cpu, slot, command, len)) != 0) as u32
}

/// Whether the driver has done every command (1) or not (0).
pub fn driver_idle(cpu: &mut Cpu) -> u32 {
    (0..SLOT_COUNT).all(|slot| nested!(cpu, 0x0600_475c, command_done(cpu, slot)) != 0) as u32
}

fn wait_idle(cpu: &mut Cpu, at: u32) {
    while nested!(cpu, 0x0600_4780, driver_idle(cpu)) == 0 {
        cpu.wait(at, WAIT_PASS);
    }
}

/// The command built at COMMAND, sent to the settings slot (waiting for it).
fn send_setting(cpu: &mut Cpu, len: u32, at: u32) {
    while nested!(cpu, 0x0600_4718, send_to(cpu, 0, COMMAND, len)) == 0 {
        cpu.wait(at, WAIT_PASS);
    }
}

/// The command built at COMMAND, sent to a sequence slot (waiting for one). The SH-2 code calls
/// send through r8, which send's prologue pushes (read later as an unset field, by 0x06063620).
fn send_sequence(cpu: &mut Cpu, len: u32, at: u32) {
    cpu.holding(&[(8, SEND)], |cpu| {
        while nested!(cpu, SEND, send(cpu, COMMAND, len)) == 0 {
            cpu.wait(at, WAIT_PASS);
        }
    });
}

fn build(cpu: &mut Cpu, number: u8, params: &[u8]) {
    cpu.write8(COMMAND, number);
    for (i, &p) in params.iter().enumerate() {
        cpu.write8(COMMAND + 2 + i as u32, p);
    }
}

/// The driver's flags, from bits 0-4 of `bits` (once the driver has done everything, and after).
pub fn set_flags(cpu: &mut Cpu, bits: u32) {
    wait_idle(cpu, 0x0600_4814);
    let flags: Vec<u8> = (0..5).map(|i| (bits & 0xFF) >> i & 1).map(|b| b as u8).collect();
    build(cpu, SET_FLAGS, &flags);
    send_setting(cpu, 6, 0x0600_4888);
    wait_idle(cpu, 0x0600_4896);
}

/// The total volume.
pub fn total_volume(cpu: &mut Cpu, volume: u32) {
    build(cpu, TOTAL_VOLUME, &[volume as u8]);
    send_setting(cpu, 2, 0x0600_48ce);
}

/// The effect bank and mixer.
pub fn mixer(cpu: &mut Cpu, bank: u32, mixer: u32) {
    build(cpu, MIXER, &[bank as u8, mixer as u8]);
    send_setting(cpu, 3, 0x0600_4908);
}

/// A mixer setting: `which`, and a level (0-7) with a pan (5 bits).
pub fn mixer_param(cpu: &mut Cpu, which: u32, level: u32, pan: u32) {
    build(cpu, MIXER_PARAM, &[which as u8, ((level & 7) << 5 | pan & 0x1F) as u8]);
    send_setting(cpu, 3, 0x0600_4956);
}

/// The driver set up: its flags, no total volume, effect bank and mixer 0, and the two mixer
/// settings' pans `pan1` and `pan0` at full level.
pub fn sound_setup(cpu: &mut Cpu, pan1: u32, pan0: u32) {
    nested!(cpu, 0x0600_4804, set_flags(cpu, 0x18));
    nested!(cpu, 0x0600_48b8, total_volume(cpu, 0));
    nested!(cpu, 0x0600_48ec, mixer(cpu, 0, 0));
    nested!(cpu, 0x0600_4928, mixer_param(cpu, 1, 7, pan1 as i8 as i32 as u32));
    nested!(cpu, 0x0600_4928, mixer_param(cpu, 0, 7, pan0 as i8 as i32 as u32));
}

/// The driver set up as the game starts it.
pub fn sound_setup_default(cpu: &mut Cpu) {
    nested!(cpu, 0x0600_4974, sound_setup(cpu, -15i32 as u32, 15));
}

/// Starts song `song` on the driver's sequence control `control` at `priority`.
pub fn start_seq(cpu: &mut Cpu, control: u32, song: u32, priority: u32) {
    build(cpu, START_SEQ, &[control as u8, song as u8, priority as u8]);
    send_sequence(cpu, 4, 0x0600_4ba8);
}

/// The volume of control `control`'s sequence, reached at `rate`.
pub fn seq_volume(cpu: &mut Cpu, control: u32, volume: u32, rate: u32) {
    build(cpu, SEQ_VOLUME, &[control as u8, volume as u8, rate as u8]);
    send_sequence(cpu, 4, 0x0600_4be4);
}

/// Stops control `control`'s sequence.
pub fn stop_seq(cpu: &mut Cpu, control: u32) {
    build(cpu, STOP_SEQ, &[control as u8, 0]);
    send_sequence(cpu, 3, 0x0600_4c1c);
}

/// Sound data file `n`'s area in the sound RAM map (from sound RAM's start), marked loaded; 0 if
/// the map has none.
pub fn map_area(cpu: &mut Cpu, n: u32) -> u32 {
    let file = SOUND_FILES + 8 * n;
    let mut area = SOUND_RAM.wrapping_add(cpu.read32(MAP_POINTER));
    while (cpu.read8(area) as i8) >= 0 {
        let kind = cpu.read8(area);
        if kind & 0x0F == cpu.read8(file + 5) && kind & 0x30 == cpu.read8(file + 4) {
            let address = cpu.read32(area) & 0x00FF_FFFF;
            let flags = cpu.read8(area + 4);
            cpu.write8(area + 4, flags | 0x80);
            return address;
        }
        area += 8;
    }
    0
}

/// Copies `size` bytes from `src` into sound data file `n`'s area, a piece a frame (dma.c's long
/// copies).
pub fn copy_to_area(cpu: &mut Cpu, n: u32, src: u32, size: u32) {
    let area = nested!(cpu, 0x0600_4a64, map_area(cpu, n));
    nested!(cpu, 0x0600_4530, dma::divdma_set(cpu, src, SOUND_RAM.wrapping_add(area), size));
}

/// Loads sound data file `n` into `buffer`, then into its area: its size.
pub fn load_sound_file(cpu: &mut Cpu, n: u32, buffer: u32) -> u32 {
    let name = cpu.read32(SOUND_FILES + 8 * n);
    let size = nested!(cpu, 0x0600_5f10, files::read_file(cpu, name, buffer));
    let area = SOUND_RAM.wrapping_add(nested!(cpu, 0x0600_4a64, map_area(cpu, n)));
    for i in 0..(size as i32 / 4).max(0) as u32 {
        let w = cpu.read32(buffer + 4 * i);
        cpu.write32(area + 4 * i, w);
    }
    size
}

/// The sound manager (an object, sound_manager_set): what each of the driver's sequence controls
/// plays and how it may be taken, the background music and its fade. Control 0 is the music's;
/// 1-7 the effects'.
const MANAGER: u32 = 0x0600_edd0;
const MANAGER_SIZE: u32 = 0x7c;
/// Each control's sequence (-1: none; i16[8]), its priority, the frames it may not be taken yet,
/// the frames it stays busy after a start or stop, and its age in frames (to 250).
const SEQ: u32 = 0x10;
const PRIORITY: u32 = 0x20;
const HOLD: u32 = 0x28;
const BUSY: u32 = 0x30;
const AGE: u32 = 0x38;
const AGE_MAX: u8 = 0xFA;
/// Effects started this frame (at most 16), the music playing and the music asked for (i16).
const STARTED: u32 = 0x60;
const STARTED_MAX: i16 = 16;
const PLAYING: u32 = 0x62;
const REQUESTED: u32 = 0x64;
/// The music's volume (16.16, 1.0 full), the fade's start and end, the volume last sent (u8),
/// the fade's frame and length (i16; -1: no fade).
const VOLUME: u32 = 0x68;
const FADE_FROM: u32 = 0x6c;
const FADE_TO: u32 = 0x70;
const SENT: u32 = 0x74;
const FADE_FRAME: u32 = 0x76;
const FADE_LEN: u32 = 0x78;
/// Each piece of music's full volume (bytes, by its number).
const MUSIC_VOLUMES: u32 = 0x0600_8b54;
/// While set, only the coin and credit sounds play (SE_COIN, SE_CREDIT).
pub const SE_LOCK: u32 = 0x0600_edd8;

/// The sound effects, 6 bytes each: the sequence (i16: the song in its high byte, the priority
/// the driver gets in its low), the priority on a control (+2), the controls it may take (+3: 8 any of 4-7, 9 any of
/// 2-7, 10 any of 1-7, else that one), the frames before it may be taken (+4). The common ones,
/// then the stage's own from number 71 (a table the stage gives).
const SE_TABLE: u32 = 0x0600_a4c4;
const STAGE_SE_TABLE: u32 = 0x0600_edd4;
const STAGE_SE: i32 = 71;
const SE: u32 = 6;
/// Sound effects by name (dma.c's SE enum): confirm, a coin, a credit.
const SE_KETTEI: u32 = 1;
const SE_COIN: u32 = 48;
const SE_CREDIT: u32 = 49;

/// The game's: seq_status's SH-2 entry, and the pause and continue of every sequence playing.
const SEQ_STATUS: u32 = 0x0600_79a8;
const PAUSE_ALL: u32 = 0x0600_4c98;
const CONTINUE_ALL: u32 = 0x0600_4d3c;
/// The game's flags (globals::GAME_FLAGS): paused by a player (bits 12-13; director.rs) and sound
/// paused (bit 29).
const HELD_BITS: u32 = 0x3000;
const SOUND_PAUSED: u32 = 1 << 29;

fn manager(cpu: &mut Cpu) -> u32 {
    cpu.read32(MANAGER)
}

/// Sound effect `se`'s table and entry (in no table, if the stage's is not there).
fn se_place(cpu: &mut Cpu, se: u32) -> (u32, u32) {
    let se = se as i16 as i32;
    let (table, i) = if se < STAGE_SE { (SE_TABLE, se) } else { (cpu.read32(STAGE_SE_TABLE), (se - STAGE_SE) as i16 as i32) };
    (table, table.wrapping_add((i * SE as i32) as u32))
}

/// Sound effect `se`'s entry, or None if its table is not there.
fn se_entry(cpu: &mut Cpu, se: u32) -> Option<u32> {
    let (table, entry) = se_place(cpu, se);
    (table != 0).then_some(entry)
}

/// A control's sequence (i16).
fn seq(cpu: &mut Cpu, control: u32) -> i16 {
    let m = manager(cpu);
    cpu.read16(m + SEQ + 2 * control) as i16
}

/// Forgets what `control` played.
fn clear_control(cpu: &mut Cpu, control: u32) {
    let m = manager(cpu);
    cpu.write16(m + SEQ + 2 * control, 0xFFFF);
    for field in [PRIORITY, HOLD, BUSY, AGE] {
        cpu.write8(m + field + control, 0);
    }
}

/// Stops every effect (but the confirm, coin and credit sounds) and fades the music out at once,
/// once the driver has done its commands.
pub fn stop_effects(cpu: &mut Cpu) {
    wait_idle(cpu, 0x0600_4df0);
    if manager(cpu) == 0 {
        return;
    }
    nested!(cpu, 0x0600_4f0c, music_fade_out(cpu, 0));
    let kept = [SE_COIN, SE_CREDIT, SE_KETTEI].map(|se| SE_TABLE + SE * se);
    for control in 1..8 {
        let playing = seq(cpu, control);
        if kept.iter().any(|&e| cpu.read16(e) as i16 == playing) {
            continue;
        }
        nested!(cpu, 0x0600_4c00, stop_seq(cpu, control as i8 as i32 as u32));
        clear_control(cpu, control);
    }
}

/// Asks for music `music` (none for 0) at full volume.
pub fn request_music(cpu: &mut Cpu, music: u32) {
    if music & 0xFFFF == 0 {
        return;
    }
    let m = manager(cpu);
    cpu.write16(m + REQUESTED, music as u16);
    cpu.write32(m + FADE_FROM, 0);
    cpu.write32(m + VOLUME, 0);
    cpu.write32(m + FADE_TO, 0x10000);
    cpu.write16(m + FADE_LEN, 0);
    cpu.write16(m + FADE_FRAME, 0);
}

/// Fades the music to `volume` (16.16) over `frames`.
pub fn music_fade_to(cpu: &mut Cpu, volume: u32, frames: u32) {
    let m = manager(cpu);
    let now = cpu.read32(m + VOLUME);
    cpu.write32(m + FADE_FROM, now);
    cpu.write32(m + FADE_TO, volume);
    cpu.write16(m + FADE_FRAME, 0);
    cpu.write16(m + FADE_LEN, frames as u16);
}

/// Fades the music out over `frames` (then stops it).
pub fn music_fade_out(cpu: &mut Cpu, frames: u32) {
    let m = manager(cpu);
    let now = cpu.read32(m + VOLUME);
    cpu.write32(m + FADE_FROM, now);
    cpu.write32(m + FADE_TO, 0);
    cpu.write16(m + FADE_FRAME, 0);
    cpu.write16(m + FADE_LEN, frames as u16);
}

/// The control sound effect `se` should play on: 0 if it already plays (or none suits), -3 if its
/// table is not there. A free control first (one the driver says plays nothing); else the one of
/// lowest priority, of those as low as it the oldest not held.
pub fn se_control(cpu: &mut Cpu, se: u32) -> u32 {
    let Some(entry) = se_entry(cpu, se) else {
        return -3i32 as u32;
    };
    let (first, last): (i32, i32) = match cpu.read8(entry + 3) {
        8 => (4, 7),
        9 => (2, 7),
        10 => (1, 7),
        _ => {
            let only = cpu.read8(entry + 3) as i8 as i32;
            (only, only)
        }
    };
    let m = manager(cpu);
    let song = cpu.read16(entry) as i16;
    if (first..=last).any(|c| cpu.read8(m + BUSY + c as u32) != 0 && cpu.read16(m + SEQ + 2 * c as u32) as i16 == song) {
        return 0;
    }
    let status = cpu.stack();
    for c in first..=last {
        let m = manager(cpu);
        if cpu.read8(m + BUSY + c as u32) == 0 {
            nested!(cpu, SEQ_STATUS, seq_status(cpu, status, c as u8 as u32));
            if cpu.read16(status) == 0 {
                return c as u32;
            }
        }
    }
    let mut lowest = cpu.read8(entry + 2);
    let (mut best, mut best_age) = (0, 0u8);
    for c in first..=last {
        let at = manager(cpu) + c as u32;
        if cpu.read8(at + BUSY) != 0 {
            continue;
        }
        let priority = cpu.read8(at + PRIORITY);
        if priority == lowest {
            let age = cpu.read8(at + AGE);
            if cpu.read8(at + HOLD) == 0 && age > best_age {
                (best, best_age) = (c, age);
            }
        } else if priority < lowest {
            lowest = priority;
            (best, best_age) = (c, cpu.read8(at + AGE));
        }
    }
    best as u32
}

/// se_set (the leftover source's name): plays sound effect `se`: its control, 0 if it plays
/// already, or -1 (locked: only coins and credits), -2 (no manager), -3 (no table), -4 (sixteen
/// effects started this frame).
pub fn se_set(cpu: &mut Cpu, se: u32) -> u32 {
    let n = se as i16 as i32;
    if cpu.read32(SE_LOCK) != 0 && !(n as u32 == SE_COIN || n as u32 == SE_CREDIT) {
        return -1i32 as u32;
    }
    let m = manager(cpu);
    if m == 0 {
        return -2i32 as u32;
    }
    if cpu.read16(m + STARTED) as i16 >= STARTED_MAX {
        return -4i32 as u32;
    }
    // (with no table, se_control's -3 goes on as a control, as the SH-2 code had it; the SH-2
    // code holds the effect in r8, for start_seq the entry, the control, the entry's offset and
    // the table in r8-r11: the callees' prologues push them, read later as unset fields)
    let control = cpu.holding(&[(8, n as u32)], |cpu| nested!(cpu, 0x0600_4f3c, se_control(cpu, n as u32))) as u8;
    if control == 0 {
        return 0;
    }
    let (table, entry) = se_place(cpu, n as u32);
    // the entry's sequence: the song in its high byte, the driver's priority in its low
    let sequence = cpu.read16(entry) as i16 as i32;
    let (song, driver_priority) = (sequence / 256, cpu.read8(entry + 1));
    let regs = [(8, entry), (9, control as u32), (10, entry.wrapping_sub(table)), (11, table)];
    cpu.holding(&regs, |cpu| nested!(cpu, 0x0600_4b88, start_seq(cpu, control as i8 as i32 as u32, song as i8 as i32 as u32, driver_priority as u32)));
    let m = manager(cpu);
    let c = control as u32;
    let song = cpu.read16(entry);
    cpu.write16(m + SEQ + 2 * c, song);
    let (priority, hold) = (cpu.read8(entry + 2), cpu.read8(entry + 4));
    cpu.write8(m + PRIORITY + c, priority);
    cpu.write8(m + HOLD + c, hold);
    cpu.write8(m + BUSY + c, 5);
    cpu.write8(m + AGE + c, 0);
    let started = cpu.read16(m + STARTED);
    cpu.write16(m + STARTED, started.wrapping_add(1));
    control as i8 as i32 as u32
}

/// Stops sound effect `se` on `control` if it plays there: the control, 0 if not, or -2 (no
/// manager), -4 (no such control), -3 (no table).
pub fn se_stop(cpu: &mut Cpu, se: u32, control: u32) -> u32 {
    let c = control as i8 as i32;
    if manager(cpu) == 0 {
        return -2i32 as u32;
    }
    if c <= 0 {
        return -4i32 as u32;
    }
    let Some(entry) = se_entry(cpu, se) else {
        return -3i32 as u32;
    };
    let c = c as u32;
    if seq(cpu, c) != cpu.read16(entry) as i16 {
        return 0;
    }
    nested!(cpu, 0x0600_4c00, stop_seq(cpu, c));
    let m = manager(cpu);
    cpu.write16(m + SEQ + 2 * c, 0xFFFF);
    cpu.write8(m + PRIORITY + c, 0);
    cpu.write8(m + BUSY + c, 2);
    c
}

/// The music's volume for the driver: its full volume (halved) scaled by `volume` (16.16), on top
/// of the half.
fn music_volume(cpu: &mut Cpu, m: u32, volume: u32) -> u8 {
    let music = cpu.read16(m + REQUESTED) as i16 as i32 as u32;
    let half = (cpu.read8(MUSIC_VOLUMES.wrapping_add(music)) >> 1) as u32;
    (nested!(cpu, 0x0600_6a58, super::math::fixed_mul(cpu, half, volume)).wrapping_add(half)) as u8
}

/// The sound manager's frame: the controls' counts, the music asked for started, the fade on.
pub fn sound_exec(cpu: &mut Cpu, m: u32) {
    for c in 0..8 {
        for field in [HOLD, BUSY] {
            let n = cpu.read8(m + field + c);
            if n != 0 {
                cpu.write8(m + field + c, n - 1);
            }
        }
        let age = cpu.read8(m + AGE + c);
        if age < AGE_MAX {
            cpu.write8(m + AGE + c, age + 1);
        }
    }
    cpu.write16(m + STARTED, 0);
    let flags = cpu.read32(super::globals::GAME_FLAGS);
    if flags & HELD_BITS != 0 {
        if flags & SOUND_PAUSED != 0 {
            cpu.call(PAUSE_ALL, &[]);
        }
        return;
    }
    if flags & SOUND_PAUSED != 0 {
        cpu.call(CONTINUE_ALL, &[]);
    }
    let requested = cpu.read16(m + REQUESTED);
    if cpu.read16(m + PLAYING) != requested {
        nested!(cpu, 0x0600_4b88, start_seq(cpu, 0, 0, 0));
        let requested = cpu.read16(m + REQUESTED);
        cpu.write16(m + PLAYING, requested);
    }
    let len = cpu.read16(m + FADE_LEN) as i16;
    if len < 0 {
        return;
    }
    let frame = cpu.read16(m + FADE_FRAME) as i16;
    let sent = if len > frame {
        let (from, to) = (cpu.read32(m + FADE_FROM), cpu.read32(m + FADE_TO));
        let t = nested!(cpu, 0x0600_6a8e, super::math::fixed_div(cpu, frame as i32 as u32, len as i32 as u32));
        let volume = from.wrapping_add(nested!(cpu, 0x0600_6a58, super::math::fixed_mul(cpu, to.wrapping_sub(from), t)));
        cpu.write32(m + VOLUME, volume);
        let v = music_volume(cpu, m, volume);
        if cpu.read8(m + SENT) & 0xF8 != v & 0xF8 {
            nested!(cpu, 0x0600_4bc4, seq_volume(cpu, 0, v as u32, 0));
        }
        cpu.write16(m + FADE_FRAME, (frame + 1) as u16);
        v
    } else {
        let to = cpu.read32(m + FADE_TO);
        cpu.write32(m + VOLUME, to);
        let v = if to == 0 {
            nested!(cpu, 0x0600_4c00, stop_seq(cpu, 0));
            cpu.write16(m + REQUESTED, 0);
            cpu.write16(m + PLAYING, 0);
            0
        } else {
            let v = music_volume(cpu, m, to);
            nested!(cpu, 0x0600_4bc4, seq_volume(cpu, 0, v as u32, 0));
            v
        };
        cpu.write16(m + FADE_LEN, 0xFFFF);
        v
    };
    cpu.write8(m + SENT, sent);
}

/// The sound manager made (object list 0): it, or 0 if the list had no room.
pub fn sound_manager_set(cpu: &mut Cpu) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, super::objects::obj_size(cpu, MANAGER_SIZE));
    let m = nested!(cpu, 0x0600_6b00, super::objects::obj_set(cpu, 0, blocks as i8 as i32 as u32, SOUND_EXEC));
    cpu.write32(MANAGER, m);
    if m == 0 {
        return 0;
    }
    for c in 0..8 {
        clear_control(cpu, c);
    }
    for field in [PLAYING, REQUESTED] {
        cpu.write16(m + field, 0);
    }
    for field in [FADE_LEN, FADE_FRAME] {
        cpu.write16(m + field, 0xFFFF);
    }
    m
}

/// sound_exec's SH-2 entry (the manager's exec).
const SOUND_EXEC: u32 = 0x0600_53dc;

/// The driver's program and its sound map, loaded into work RAM-L for the library's init.
const DRIVER_FILE: u32 = 0x0600_9300; // "SND_DRVR.BIN"
const MAP_FILE: u32 = 0x0600_9310; // "SND_MAP.BIN"
const DRIVER_BUFFER: u32 = 0x0020_0000;
/// Loads the sound driver and its map from the cart and starts it: no stage's effects yet.
pub fn sound_load(cpu: &mut Cpu) {
    let driver_size = nested!(cpu, 0x0600_5f10, files::read_file(cpu, DRIVER_FILE, DRIVER_BUFFER));
    // the init's struct, at the bottom of the frame
    let init = cpu.stack();
    cpu.write32(init, DRIVER_BUFFER);
    cpu.write16(init + 4, driver_size as u16);
    let map = (driver_size << 1).wrapping_add(DRIVER_BUFFER);
    let map_size = nested!(cpu, 0x0600_5f10, files::read_file(cpu, MAP_FILE, map));
    cpu.write32(init + 8, map);
    cpu.write16(init + 12, map_size as u16);
    nested!(cpu, 0x0600_6be4, snd_init(cpu, init));
    nested!(cpu, 0x0600_6d7c, snd_change_map(cpu, 0));
    nested!(cpu, 0x0600_6e0c, snd_command_82(cpu, 15));
    cpu.write32(STAGE_SE_TABLE, 0);
}

/// The driver's status words, a word each control (its address, set by SND_Init).
const STATUS_WORDS: u32 = 0x0600_cb68;

/// Sega's sound library's state, which SND_Init takes from the driver's header (at sound RAM
/// 0x400, each an offset from sound RAM's start): its area table (+0x400), the command slots
/// (+0x404; the library puts its commands in the first seven) and the slot it is at, the map
/// (+0x408), and from the slots' address the status words (+0x80) and four places more (+0x90,
/// +0x94, +0xa0, +0xb0: not known yet); a place the header gives as two words at +0x412 (not
/// known yet). While a command is being put, the library is busy.
const SND_BUSY: u32 = 0x0600_cb58;
const SND_AREAS: u32 = 0x0600_cb5c;
const SND_SLOTS: u32 = 0x0600_cb60;
const SND_MAP: u32 = 0x0600_cb64;
const SND_SLOT: u32 = 0x0600_cb7c;
const SND_HEADER_PLACE: u32 = 0x0603_3278;
const SND_SLOT_PLACES: [(u32, u32); 5] = [(STATUS_WORDS, 0x80), (0x0600_cb74, 0xa0), (0x0600_cb78, 0xb0), (0x0600_cb6c, 0x90), (0x0600_cb70, 0x94)];
const HEADER: u32 = 0x400;
const SND_SLOTS_END: u32 = 7 * SLOT;
/// The sound CPU held while the driver is copied in: the SMPC's PDR2 bit 4 (the BIOS keeps the
/// port's byte); sound RAM's 4-Mbit mode (SCSP's MEM4MB); the room cleared for the driver.
const BIOS_PDR2: u32 = 0x0600_065b;
const PDR2: u32 = 0x2010_0077;
const SOUND_HELD: u8 = 0x10;
const SCSP_MEMORY_MODE: u32 = 0x25b0_0400;
const MEM4MB: u16 = 0x0200;
const DRIVER_ROOM: u32 = 0xb000;

/// SND_Init (`init`: the driver's address, its size (a word at +4), the map's address, its size
/// in words (a word at +12)): the sound CPU held, sound RAM 4 Mbit, the driver's room cleared
/// and the driver copied to sound RAM's start (as many bytes as `init` says); the library's state
/// from the driver's header; the map copied to the place the area table's third long gives (its
/// size in bytes, rounded up past the next long); the sound CPU let go; the library not busy.
pub fn snd_init(cpu: &mut Cpu, init: u32) {
    let v = cpu.read8(BIOS_PDR2);
    cpu.write8(BIOS_PDR2, v | SOUND_HELD);
    let v = cpu.read8(BIOS_PDR2);
    cpu.write8(PDR2, v);
    cpu.write16(SCSP_MEMORY_MODE, MEM4MB);
    nested!(cpu, 0x0600_7cd4, snd_clear(cpu, SOUND_RAM, DRIVER_ROOM));
    let size = cpu.read16(init + 4) as u32;
    let driver = cpu.read32(init);
    nested!(cpu, 0x0600_7da0, snd_copy(cpu, SOUND_RAM, driver, size));
    let areas = cpu.read32(SOUND_RAM + HEADER).wrapping_add(SOUND_RAM);
    cpu.write32(SND_AREAS, areas);
    let slots = cpu.read32(SOUND_RAM + HEADER + 4).wrapping_add(SOUND_RAM);
    cpu.write32(SND_SLOTS, slots);
    let high = cpu.read16(SOUND_RAM + HEADER + 0x12) as u32;
    let low = cpu.read16(SOUND_RAM + HEADER + 0x14) as u32;
    cpu.write32(SND_HEADER_PLACE, (high << 16 | low).wrapping_add(SOUND_RAM));
    cpu.write32(SND_SLOT, slots);
    let map = cpu.read32(SOUND_RAM + HEADER + 8).wrapping_add(SOUND_RAM);
    cpu.write32(SND_MAP, map);
    for (at, offset) in SND_SLOT_PLACES {
        cpu.write32(at, slots + offset);
    }
    let n = 2 * cpu.read16(init + 12) as u32;
    let n = n + 4 - (n & 3);
    let areas = cpu.read32(SND_AREAS);
    let dst = cpu.read32(areas + 8).wrapping_add(SOUND_RAM);
    let src = cpu.read32(init + 8);
    nested!(cpu, 0x0600_7da0, snd_copy(cpu, dst, src, n));
    let v = cpu.read8(BIOS_PDR2);
    cpu.write8(BIOS_PDR2, v & !SOUND_HELD);
    let v = cpu.read8(BIOS_PDR2);
    cpu.write8(PDR2, v);
    cpu.write32(SND_BUSY, 0);
}

/// The library's memory clear and copy: memset to 0, memcpy.
pub fn snd_clear(cpu: &mut Cpu, at: u32, n: u32) {
    nested!(cpu, 0x0600_817c, clib::memset(cpu, at, 0, n));
}

pub fn snd_copy(cpu: &mut Cpu, dst: u32, src: u32, n: u32) {
    nested!(cpu, 0x0600_81d0, clib::memcpy(cpu, dst, src, n));
}

/// The library's next free command slot (its first byte 0), from the one after the slot it is at
/// when that one is taken, else from the first: 1, the library at it, or 0 when none is (the
/// library past the last).
pub fn snd_next_slot(cpu: &mut Cpu) -> u32 {
    let at = cpu.read32(SND_SLOT);
    if cpu.read8(at) != 0 {
        if at >= cpu.read32(SND_SLOTS).wrapping_add(SND_SLOTS_END) {
            return 0;
        }
        let next = cpu.read32(SND_SLOT).wrapping_add(SLOT);
        cpu.write32(SND_SLOT, next);
        if next >= cpu.read32(SND_SLOTS).wrapping_add(SND_SLOTS_END) {
            return 0;
        }
    } else {
        let first = cpu.read32(SND_SLOTS);
        cpu.write32(SND_SLOT, first);
        if first >= first.wrapping_add(SND_SLOTS_END) {
            return 0;
        }
    }
    let end = cpu.read32(SND_SLOTS).wrapping_add(SND_SLOTS_END);
    loop {
        let at = cpu.read32(SND_SLOT);
        if cpu.read8(at) == 0 {
            return 1;
        }
        let next = cpu.read32(SND_SLOT).wrapping_add(SLOT);
        cpu.write32(SND_SLOT, next);
        if next >= end {
            return 0;
        }
    }
}

/// A command (`word`: its number in the high byte) and its argument byte put in the library's
/// slot, the number last.
fn snd_put(cpu: &mut Cpu, word: u16, arg: u8) {
    let slot = cpu.read32(SND_SLOT);
    cpu.write8(slot + 2, arg);
    let slot = cpu.read32(SND_SLOT);
    cpu.write16(slot, word);
}

/// The driver taken a command: when its flag (sound RAM 0x4e1) has bit 7, 0x80 written back at
/// 0x4e0; else a wait until the command in the library's slot is gone (its first word 0).
const SND_FLAG: u32 = 0x25a0_04e1;
const SND_ACK: u32 = 0x25a0_04e0;

pub fn snd_wait(cpu: &mut Cpu) {
    if cpu.read8(SND_FLAG) & 0x80 != 0 {
        cpu.write8(SND_ACK, 0x80);
        return;
    }
    let slot = cpu.read32(SND_SLOT);
    if cpu.read16(slot) == 0 {
        return;
    }
    let slot = cpu.read32(SND_SLOT);
    while cpu.read16(slot) != 0 {
        cpu.wait(0x0600_6bca, 5);
    }
}

/// SND_ChgMap: the driver's map `map` made current (command 0x08, put twice, each waited for): 0,
/// or 1 when the library is busy or no slot is free.
const CHANGE_MAP: u16 = 0x0800;

pub fn snd_change_map(cpu: &mut Cpu, map: u32) -> u32 {
    if cpu.read32(SND_BUSY) != 0 {
        return 1;
    }
    cpu.write32(SND_BUSY, 1);
    for _ in 0..2 {
        if nested!(cpu, 0x0600_7db8, snd_next_slot(cpu)) as u8 == 0 {
            cpu.write32(SND_BUSY, 0);
            return 1;
        }
        snd_put(cpu, CHANGE_MAP, map as u8);
        nested!(cpu, 0x0600_6ba0, snd_wait(cpu));
    }
    cpu.write32(SND_BUSY, 0);
    0
}

/// Command 0x82 with `v` (not known yet; sound_load sends 15), not waited for: 0, or 1 when the
/// library is busy or no slot is free.
const COMMAND_82: u16 = 0x8200;

pub fn snd_command_82(cpu: &mut Cpu, v: u32) -> u32 {
    if cpu.read32(SND_BUSY) != 0 {
        return 1;
    }
    cpu.write32(SND_BUSY, 1);
    if nested!(cpu, 0x0600_7db8, snd_next_slot(cpu)) as u8 == 0 {
        cpu.write32(SND_BUSY, 0);
        return 1;
    }
    snd_put(cpu, COMMAND_82, v as u8);
    cpu.write32(SND_BUSY, 0);
    0
}

/// SND_GetSeqStat: control `control`'s status into `buf`: the status's high byte (0 when it plays
/// nothing) as a word, then its low byte.
pub fn seq_status(cpu: &mut Cpu, buf: u32, control: u32) {
    let words = cpu.read32(STATUS_WORDS);
    let status = cpu.read16(words.wrapping_add(2 * (control & 0xFF)));
    cpu.write16(buf, status & 0xFF00);
    cpu.write8(buf + 2, status as u8);
}

/// Starts a load of sound data file `n` into `dst`: the load (or 0).
pub fn load_sound_data(cpu: &mut Cpu, n: u32, dst: u32) -> u32 {
    let name = cpu.read32(SOUND_FILES + 8 * n);
    let load = nested!(cpu, 0x0600_5e40, files::load_file(cpu, name));
    nested!(cpu, 0x0600_603c, files::start_load(cpu, load, dst));
    load
}
