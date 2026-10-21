//! The game's states: the director (director.rs) runs the current one each frame. Its step is the
//! director's byte +0x3c and the state's number +0x3d; a state moves on by setting the step, or
//! another state at its step 0 (the two as a word).

use super::nested;
use super::globals::mode::{GAME_LOGO, GAME_NAME_ENTRY, GAME_OPENING, GAME_OPTION, GAME_TITLE};
use super::globals::{GAME_FLAGS, GAME_MODE, LOOPS};
use super::{dma, files, math, objects, screen, sound, system, text, Cpu};

const STEP: u32 = 0x3c;
/// The director's flags (bit 0 a mode is running; bit 1 the mode is done, set by the mode's code;
/// bit 2 it was skipped; bit 3 the screen is handed over, screen.rs), the coins counted, the coin
/// sound (director.rs), and others the states use (not known yet).
const FLAGS: u32 = 0x3e;
const RUNNING: u8 = 1;
const DONE: u8 = 2;
const SKIPPED: u8 = 4;
const HANDED_OVER: u8 = 8;
/// The director's word of flags (bit 15: a game is on; bits 20 and 28 cleared as a mode starts).
const GAME_FLAGS_44: u32 = 0x44;
const MODE_START_CLEARS: u32 = 0x1010_0000;
const COUNTED: u32 = 0x3f;
const COIN_SOUND: u32 = 0x40;
const FIELD_44: u32 = 0x44;
const FIELD_4C: u32 = 0x4c;
const FIELD_4D: u32 = 0x4d;
/// Sixteen words the states use (the first, a load in progress or a count of frames), and the
/// mode's music (a word: -1 as a mode starts, for none; the mode's code sets it).
const SLOTS: u32 = 0x64;
const LOAD: u32 = 0x64;
const COUNT: u32 = 0x64;
const MUSIC: u32 = 0xa4;

/// Each game mode's file (its code and data, loaded into the overlay window: the title's, the
/// rankings', the opening's, each stage's).
const MODE_FILES: u32 = 0x0607_2310;
const OVERLAY: u32 = 0x0608_0000;

/// The mode files whose code the overlay rewrites are registered for: each file's name less
/// ".BIN".
pub const OVERLAYS: [&str; 9] = ["TITLE", "RANK", "OPENING", "STG00", "STG01", "STG02", "STG03", "STG04", "STG05"];

/// Which mode file's code the overlay window holds (`wram_h`: work RAM-H): the game mode's file,
/// by its name in the game's table (none for a mode without one).
pub fn overlay_name(wram_h: &[u8]) -> Option<&'static str> {
    let off = |a: u32| (a & 0xF_FFFF) as usize;
    let mode = wram_h[off(GAME_MODE)] as u32;
    let name = stv::mem::rd32(wram_h, off(MODE_FILES + 4 * mode));
    if name & 0xFFF0_0000 != 0x0600_0000 {
        return None;
    }
    let text = &wram_h[off(name)..];
    OVERLAYS.into_iter().find(|o| text.strip_prefix(o.as_bytes()).is_some_and(|rest| rest.starts_with(b".BIN\0")))
}

/// A mode's file loads at 256 KB a frame.
const MODE_RATE: u32 = 0x4_0000;
/// The rate the next load takes (files.rs).
const NEXT_RATE: u32 = 0x0603_2e40;

/// The play's settings (scroll and camera, as far as can be told; not known yet): each copied from
/// its default (set at the start) as a mode loads. (default, current, as words or longs)
const SETTINGS_LONG: [(u32, u32); 3] = [(0x0607_40d8, 0x0607_40e4), (0x0607_4090, 0x0607_4050), (0x0607_4094, 0x0607_4058)];
const SETTINGS_WORD: [(u32, u32); 3] = [(0x0607_40e0, 0x0607_40ea), (0x0607_40ec, 0x0607_40e8), (0x0607_4060, 0x0607_4054)];
/// Cleared as a mode loads: the route within a stage (as the stage score table's index tells).
const MODE_SUB: u32 = 0x0600_ebb1;
const STAGE_SE_TABLE: u32 = 0x0600_edd4;
const FULL_SPEED: u32 = 0x0600_ed90;
const DISPLAY_FLAGS: [u32; 2] = [0x0600_edcc, 0x0600_cc61];

/// SGL's: whether DMA channel 0 is copying.
const SL_DMA_BUSY: u32 = 0x0606_98cc;

fn go_to(cpu: &mut Cpu, game: u32, state: u16) {
    cpu.write16(game + STEP, state);
}

fn set_step(cpu: &mut Cpu, game: u32, step: u8) {
    cpu.write8(game + STEP, step);
}

/// Starts loading game mode `mode`'s file into the overlay window: the load.
pub fn load_mode_file(cpu: &mut Cpu, mode: u32) -> u32 {
    let name = cpu.read32(MODE_FILES + 4 * mode);
    let load = nested!(cpu, 0x0600_5e40, files::load_file(cpu, name));
    cpu.write32(NEXT_RATE, MODE_RATE);
    nested!(cpu, 0x0600_603c, files::start_load(cpu, load, OVERLAY));
    load
}

/// State 0: the director's counts cleared, the attract mode from the logo (or the title, with
/// credits to start), the credits shown; then state 1.
pub fn state_reset(cpu: &mut Cpu, game: u32) {
    match cpu.read8(game + STEP) {
        0 => {
            for field in [FLAGS, COUNTED, COIN_SOUND] {
                cpu.write8(game + field, 0);
            }
            cpu.write32(game + FIELD_44, 0);
            let f = if super::globals::bios_setting_is_1(cpu) { 6 } else { 7 };
            cpu.write8(game + FIELD_4C, f);
            cpu.write8(game + FIELD_4D, 0);
            cpu.write8(GAME_MODE, GAME_LOGO);
            if !system::free_play(cpu) && (nested!(cpu, 0x0604_0f44, system::coin_byte3(cpu, 0)) != 0 || nested!(cpu, 0x0604_0f44, system::coin_byte3(cpu, 1)) != 0) {
                cpu.write8(GAME_MODE, GAME_TITLE);
            }
            nested!(cpu, 0x0606_0c08, text::credits_shown(cpu));
            nested!(cpu, 0x0606_0894, text::set_mode(cpu, 1));
            set_step(cpu, game, 1);
        }
        1 => {}
        _ => return,
    }
    go_to(cpu, game, 1);
}

/// State 1: the game mode loaded: the play's settings from their defaults, the mode's file into
/// the overlay window (not the title's: it is there), the overlay's start run until it is done,
/// the copies done; then the mode's state.
pub fn state_load_mode(cpu: &mut Cpu, game: u32) {
    match cpu.read8(game + STEP) {
        0 => {
            for (default, current) in SETTINGS_LONG {
                let v = cpu.read32(default);
                cpu.write32(current, v);
            }
            for (default, current) in SETTINGS_WORD {
                let v = cpu.read16(default);
                cpu.write16(current, v);
            }
            for i in 0..16 {
                cpu.write32(game + SLOTS + 4 * i, 0);
            }
            cpu.write32(STAGE_SE_TABLE, 0);
            cpu.write8(game + FLAGS, 0);
            cpu.write8(MODE_SUB, 0);
            set_step(cpu, game, 1);
            if cpu.read8(GAME_MODE) == GAME_TITLE {
                set_step(cpu, game, 5);
            }
        }
        1 => {
            let mode = cpu.read8(GAME_MODE) as i8 as i32 as u32;
            let load = nested!(cpu, 0x0604_02c4, load_mode_file(cpu, mode));
            cpu.write32(game + LOAD, load);
            set_step(cpu, game, 2);
        }
        2 => {
            let load = cpu.read32(game + LOAD);
            if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 {
                cpu.write32(game + LOAD, 0);
                cpu.write16(game + MUSIC, 0xFFFF);
                set_step(cpu, game, 3);
            }
        }
        3 => {
            if cpu.call(OVERLAY, &[game]) != 0 {
                set_step(cpu, game, 4);
            }
        }
        4 => {
            if nested!(cpu, 0x0600_46b8, dma::dma_check(cpu)) == 0 && cpu.call(SL_DMA_BUSY, &[]) == 0 {
                set_step(cpu, game, 5);
            }
        }
        5 => {
            cpu.write8(FULL_SPEED, 0);
            for f in DISPLAY_FLAGS {
                cpu.write8(f, 0);
            }
            let next = match cpu.read8(GAME_MODE) {
                GAME_LOGO | GAME_OPTION => 5,
                GAME_TITLE => 6,
                GAME_NAME_ENTRY..=GAME_OPENING => 2,
                _ => 7,
            };
            go_to(cpu, game, next);
        }
        _ => {}
    }
}

/// Black, the screen's fade offset; the fade to none at the screens' speed.
const BLACK: u32 = -0xFFi32 as u32;
const FADE_IN_SPEED: u32 = 0x888;

/// State 5 (the logo, the options): the screen faded in from black, the players' lines hidden;
/// when the mode is done, the fades stopped and state 6.
pub fn state_run_mode(cpu: &mut Cpu, game: u32) {
    match cpu.read8(game + STEP) {
        0 => {
            nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, BLACK, BLACK, BLACK));
            nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, 0, 0, 0, FADE_IN_SPEED));
            let f = cpu.read32(game + GAME_FLAGS_44);
            cpu.write32(game + GAME_FLAGS_44, f & !MODE_START_CLEARS);
            let flags = cpu.read8(game + FLAGS);
            cpu.write8(game + FLAGS, flags | RUNNING);
            nested!(cpu, 0x0606_0b34, text::hide_line(cpu, 0));
            nested!(cpu, 0x0606_0b34, text::hide_line(cpu, 1));
            set_step(cpu, game, 1);
        }
        1 if cpu.read8(game + FLAGS) & DONE != 0 => {
            cpu.write8(game + FLAGS, 0);
            nested!(cpu, 0x0605_e9d8, screen::stop_fades(cpu));
            nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, 0, 0, 0));
            go_to(cpu, game, 6);
        }
        _ => {}
    }
}

/// The game mode a screen's state goes to when it ends (a byte; the title when the opening is
/// skipped).
const NEXT_MODE: u32 = 0x0600_ed70;
/// The pads' presses (pads.rs: each pad's trigger word), and Start's bit.
const PAD_TRIG: [u32; 2] = [0x0602_ee14, 0x0602_ee1c];
const START: u16 = 1 << 11;
/// The players' lines' messages (text.rs; what they say is not known yet, only their use): for a
/// player who can start, and by turns with it on free play; for a player with nothing to start
/// with; by turns with that one when the coin settings' bit 1 is set; for a player with coins short
/// of a credit.
const MESSAGE_START: u32 = 10;
const MESSAGE_FREE: u32 = 15;
const MESSAGE_COIN: u32 = 11;
const MESSAGE_SECOND: u32 = 14;
const MESSAGE_SHORT: u32 = 12;
/// The fade to black's speed as a screen ends.
const FADE_OUT_SPEED: u32 = 0x8d3;
/// Sets bit 2 of the director's flags (director.rs's mode_skipped).
const SET_SKIPPED: u32 = 0x0604_0c8c;

/// States 2-4 (the name entry, the result, the opening): the screen faded in from black, the
/// players' lines hidden; during the opening, the lines blink their messages and on free play
/// Start skips to the title. When the mode is done: a fade to black with the music faded out
/// (cut at once when skipped), the screen handed over, then the clearing up and the next mode
/// loaded (state 1).
pub fn state_screen(cpu: &mut Cpu, game: u32) {
    match cpu.read8(game + STEP) {
        0 => {
            nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, BLACK, BLACK, BLACK));
            nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, 0, 0, 0, FADE_IN_SPEED));
            nested!(cpu, 0x0606_0b34, text::hide_line(cpu, 0));
            nested!(cpu, 0x0606_0b34, text::hide_line(cpu, 1));
            let flags = cpu.read8(game + FLAGS);
            cpu.write8(game + FLAGS, flags | RUNNING);
            set_step(cpu, game, 1);
        }
        1 => {
            if cpu.read8(GAME_MODE) == GAME_OPENING {
                let loops = cpu.read32(LOOPS);
                let second = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, loops, 60)) as i32;
                if system::free_play(cpu) {
                    let message = if second > 29 { MESSAGE_START } else { MESSAGE_FREE };
                    nested!(cpu, 0x0606_0b78, text::show_message(cpu, 0, message));
                    nested!(cpu, 0x0606_0b78, text::show_message(cpu, 1, message));
                    let pressed = cpu.read16(PAD_TRIG[0]) | cpu.read16(PAD_TRIG[1]);
                    if pressed & START != 0 {
                        nested!(cpu, 0x0606_0b34, text::hide_line(cpu, 0));
                        nested!(cpu, 0x0606_0b34, text::hide_line(cpu, 1));
                        cpu.write8(NEXT_MODE, 1);
                        nested!(cpu, SET_SKIPPED, super::director::mode_skipped(cpu));
                        go_to(cpu, game, 9);
                    }
                } else if second > 39 {
                    nested!(cpu, 0x0606_0b34, text::hide_line(cpu, 0));
                    nested!(cpu, 0x0606_0b34, text::hide_line(cpu, 1));
                } else {
                    nested!(cpu, 0x0606_0b78, text::show_message(cpu, 0, MESSAGE_COIN));
                    nested!(cpu, 0x0606_0b78, text::show_message(cpu, 1, MESSAGE_COIN));
                }
            }
            if cpu.read8(game + FLAGS) & DONE != 0 {
                nested!(cpu, 0x0606_0b34, text::hide_line(cpu, 0));
                nested!(cpu, 0x0606_0b34, text::hide_line(cpu, 1));
                set_step(cpu, game, 2);
            }
        }
        2 => {
            cpu.write32(GAME_FLAGS, 0);
            nested!(cpu, 0x0605_e9d8, screen::stop_fades(cpu));
            if cpu.read8(game + FLAGS) & SKIPPED != 0 {
                nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, BLACK, BLACK, BLACK));
                cpu.write32(game + COUNT, 0);
                nested!(cpu, 0x0600_4f0c, sound::music_fade_out(cpu, 0));
            } else {
                nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, BLACK, BLACK, BLACK, FADE_OUT_SPEED));
                cpu.write32(game + COUNT, 30);
                nested!(cpu, 0x0600_4f0c, sound::music_fade_out(cpu, 20));
            }
            set_step(cpu, game, 3);
        }
        3 => {
            let n = cpu.read32(game + COUNT).wrapping_sub(1);
            cpu.write32(game + COUNT, n);
            if n as i32 <= 0 {
                let flags = cpu.read8(game + FLAGS);
                cpu.write8(game + FLAGS, flags | HANDED_OVER);
                cpu.write32(game + COUNT, 2);
                set_step(cpu, game, 4);
            }
        }
        4 => {
            let n = cpu.read32(game + COUNT).wrapping_sub(1);
            cpu.write32(game + COUNT, n);
            if n as i32 <= 0 {
                nested!(cpu, 0x0604_1010, clear_mode(cpu, game));
                cpu.write8(game + FLAGS, 0);
                let next = cpu.read8(NEXT_MODE);
                cpu.write8(GAME_MODE, next);
                go_to(cpu, game, 1);
            }
        }
        _ => {}
    }
}

/// Each player's lives (bytes), the player objects' table (64 longs), and a word whose bit 31 lasts
/// one game frame (main_loop.rs).
const LIVES: u32 = 0x0607_4062;
const PLAYERS: u32 = 0x0607_40f0;
const PLAYERS_SIZE: u32 = 0x100;
const ONE_FRAME: u32 = 0x0607_405c;
/// The BIOS's function (by its table) that changes the SCU's interrupt mask, Timer 1's interrupt
/// bit in it, and Timer 1's mode register.
const BIOS_CHANGE_SCU_MASK: u32 = 0x0600_0344;
const TIMER_1: u32 = 1 << 3;
const T1MD: u32 = 0x25fe_0098;
/// Not known yet (called with 0x43, 0; not rewritten).
const SL_06064F68: u32 = 0x0606_4f68;
/// The objects that outlive a mode: the director, two not known yet, the sound manager, the
/// screen, the credit display (each a pointer to it).
const KEPT: [u32; 6] = [0x0607_46e0, 0x0607_46e4, 0x0600_edd0, 0x0607_4740, 0x0607_46e8, 0x0607_35e8];

/// The clearing up as a mode ends: the effects' lock, the game's flags and the lives cleared,
/// Timer 1 off, the sound effects stopped, the screen off, the loads forgotten, the player
/// objects' table cleared, every object freed but those that outlive a mode, the director's flags
/// cleared, the fades stopped at black.
pub fn clear_mode(cpu: &mut Cpu, game: u32) {
    cpu.write32(sound::SE_LOCK, 0);
    cpu.write32(GAME_FLAGS, 0);
    cpu.write8(LIVES + 1, 0);
    cpu.write8(LIVES, 0);
    let change_mask = cpu.read32(BIOS_CHANGE_SCU_MASK);
    cpu.call(change_mask, &[u32::MAX, TIMER_1]);
    cpu.write32(T1MD, 0);
    cpu.call(SL_06064F68, &[0x43, 0]);
    nested!(cpu, 0x0600_4ddc, sound::stop_effects(cpu));
    nested!(cpu, 0x0605_e24c, screen::screen_off(cpu));
    nested!(cpu, 0x0600_60c4, files::clear_loads(cpu));
    cpu.write32(ONE_FRAME, 0);
    for i in (0..PLAYERS_SIZE).step_by(4) {
        cpu.write32(PLAYERS + i, 0);
    }
    for area in 0..objects::AREAS {
        let mut obj = cpu.read32(objects::OBJ_INF + 8 * area);
        while obj != 0 {
            let mut kept = false;
            for k in KEPT {
                if cpu.read32(k) == obj {
                    kept = true;
                    break;
                }
            }
            if !kept {
                nested!(cpu, 0x0600_579c, objects::obj_free(cpu, obj));
            }
            obj = cpu.read32(obj + objects::NEXT);
        }
    }
    cpu.write8(game + FLAGS, 0);
    nested!(cpu, 0x0605_e9d8, screen::stop_fades(cpu));
    nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, BLACK, BLACK, BLACK));
}


/// The title's fields in the director: the picture put in VRAM, the frames the title has shown
/// (then, as it ends, those left), the frames left before it times out, whether it is to time out
/// (any bit but 15, set each frame while it is to) and whether it is (bit 15), the players who
/// started (a bit each), the mode to go to once started, the times it timed out (the opening and a
/// demo by turns), and the picture's data.
const PICTURE_SHOWN: u32 = 0x64;
const FRAMES: u32 = 0x68;
const TIME_LEFT: u32 = 0x6c;
const TIMING: u32 = 0x70;
const TIMING_NOW: u32 = 0x8000;
const STARTED: u32 = 0x74;
const START_MODE: u32 = 0x4c;
const TIMED_OUT: u32 = 0x4d;
const PICTURE: u32 = 0x48;
/// Bits of the director's word +0x44: the title's screen is set up already (not known how); the
/// picture is ready to put in VRAM (as far as can be told); cleared as the title ends; and the low
/// half, all set when the title may end (not known yet).
const TITLE_SET_UP: u32 = 0x2000_0000;
const PICTURE_READY: u32 = 0x1000_0000;
const TITLE_END_CLEARS: u32 = 0x3010_0000;
const LOW_ALL: u32 = 0xffff;
/// The frames the title shows before it times out, and when its fade to black starts.
const TITLE_TIME: u32 = 600;
const TITLE_FADE: u32 = 30;
/// The frames the title lasts once a player started (the fade to black's).
const STARTED_TIME: u32 = 30;
/// The coin settings' bits (as shown): MESSAGE_SECOND shown, a slot for each player.
const SHOW_SECOND: u32 = 2;
const SLOT_EACH: u32 = 4;
/// The lives a player starts with (the second setting, backup.rs).
const START_LIVES: u32 = 0x0600_ebb2;
/// The sound effect as a player starts (sound.rs's SE_KETTEI).
const SE_START: u32 = 1;
/// A string under the picture once a player started: its place (x, y in pixels, words), and its two
/// forms (shown by turns, five frames each).
const STARTED_X: u32 = 0x0607_23de;
const STARTED_Y: u32 = 0x0607_23e0;
const STARTED_STRINGS: u32 = 0x0607_23ec;
const TEXT_PLANE: u32 = 0x25e7_e000;
/// The demos' modes (STG01-STG04's): one of the first three by turns, the last every sixteenth
/// time.
const DEMO_MODE: u8 = 12;
const LAST_DEMO_MODE: u8 = 15;
/// The picture's size by the BIOS's setting (1 or else), and its palette's offset.
const PICTURE_BYTES: [u32; 2] = [0x29080, 0x28a80];
const PICTURE_PALETTE: u32 = 0x100;
/// The title screen's VDP2 set up (screen.rs's picture screen); NBG0 shown; the title's object
/// made (if there is none: kinds.rs's kinds_set); a player started (system.rs); the picture put
/// in VRAM; NBG0 and NBG1 shown.
const TITLE_SCREEN_SET: u32 = 0x0605_fe8c;
const TITLE_SCREEN_SHOW: u32 = 0x0606_00d4;
const TITLE_OBJ_SET: u32 = 0x0605_3aa0;
const PLAYER_STARTED: u32 = 0x0600_68d8;
const PICTURE_LOAD: u32 = 0x0606_001c;
const PICTURE_SHOW: u32 = 0x0606_00f0;
/// Each player's byte: counting down to continue (as far as can be told: set while the player's
/// continue count runs); and the BIOS's function that is handed them (their address).
const CONTINUING: u32 = 0x0600_edc0;
const BIOS_CONTINUING: u32 = 0x0600_066c;

/// State 6, the title: the screen set up (unless it is already), the title object made; then each
/// player's line shows what they can do, until one who can start presses Start: a credit taken, the
/// lives given, a fade to black, the game started. Without a player who can start (or on free
/// play) the title times out (TITLE_TIME) and the attract mode goes on: the opening, or a demo
/// every other time. The picture is put in VRAM once it is ready.
pub fn state_title(cpu: &mut Cpu, game: u32) {
    match cpu.read8(game + STEP) {
        0 => {
            if cpu.read32(game + GAME_FLAGS_44) & TITLE_SET_UP != 0 {
                cpu.write32(game + PICTURE_SHOWN, 1);
            } else {
                cpu.write32(game + PICTURE_SHOWN, 0);
                nested!(cpu, TITLE_SCREEN_SET, screen::picture_screen_set(cpu));
                nested!(cpu, TITLE_SCREEN_SHOW, screen::plane_a(cpu));
            }
            nested!(cpu, TITLE_OBJ_SET, super::kinds::kinds_set(cpu));
            for i in (0..4).rev() {
                cpu.write8(CONTINUING + i, 0);
            }
            system::bios(cpu, BIOS_CONTINUING, &[CONTINUING]);
            cpu.write8(game + FLAGS, 0);
            for field in [FRAMES, TIMING, STARTED] {
                cpu.write32(game + field, 0);
            }
            let step = if system::free_play(cpu) {
                4
            } else if nested!(cpu, 0x0604_0f24, system::coin_settings_shown(cpu)) & SLOT_EACH != 0 {
                3
            } else {
                2
            };
            set_step(cpu, game, step);
        }
        2..=4 => title_players(cpu, game),
        5 => {
            nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, BLACK, BLACK, BLACK, FADE_OUT_SPEED));
            cpu.write32(game + FRAMES, STARTED_TIME);
            set_step(cpu, game, 6);
        }
        6 => title_end(cpu, game),
        _ => {}
    }
    title_time(cpu, game);
}

/// The second within the title's frames.
fn title_second(cpu: &mut Cpu, game: u32) -> i32 {
    let frames = cpu.read32(game + FRAMES);
    nested!(cpu, 0x0600_6a6e, math::remainder(cpu, frames, 60)) as i32
}

/// Player `p`'s line: `message` for the first half of each second, then MESSAGE_SECOND if the coin
/// settings ask for it.
fn title_line(cpu: &mut Cpu, game: u32, p: u32, message: u32) {
    if title_second(cpu, game) <= 29 {
        nested!(cpu, 0x0606_0b78, text::show_message(cpu, p, message));
    } else if nested!(cpu, 0x0604_0f24, system::coin_settings_shown(cpu)) & SHOW_SECOND != 0 {
        nested!(cpu, 0x0606_0b78, text::show_message(cpu, p, MESSAGE_SECOND));
    } else {
        nested!(cpu, 0x0606_0b34, text::hide_line(cpu, p));
    }
}

/// Steps 2-4 (by the coin settings: shared slots, a slot each, free play): each player's line; a
/// player who can start and presses Start starts (step 5). Without a player who can start, or on
/// free play, the title is to time out.
fn title_players(cpu: &mut Cpu, game: u32) {
    let mut waiting = 0;
    for p in 0..2 {
        match nested!(cpu, 0x0604_0f44, system::coin_byte3(cpu, p)) {
            0 => {
                title_line(cpu, game, p, MESSAGE_COIN);
                waiting += 1;
            }
            1 => title_line(cpu, game, p, MESSAGE_SHORT),
            2 => {
                if title_second(cpu, game) <= 29 {
                    nested!(cpu, 0x0606_0b78, text::show_message(cpu, p, MESSAGE_START));
                } else if cpu.read8(game + STEP) == 4 {
                    nested!(cpu, 0x0606_0b78, text::show_message(cpu, p, MESSAGE_FREE));
                } else {
                    nested!(cpu, 0x0606_0b34, text::hide_line(cpu, p));
                }
                if cpu.read16(PAD_TRIG[p as usize]) & START != 0 && nested!(cpu, 0x0604_0f5c, system::use_credit(cpu, p)) != 0 {
                    let lives = cpu.read8(START_LIVES);
                    cpu.write8(LIVES + p, lives);
                    nested!(cpu, PLAYER_STARTED, system::player_started(cpu, p));
                    let bit = nested!(cpu, 0x0600_69d8, system::shift_left(cpu, 1, p));
                    let started = cpu.read32(game + STARTED);
                    cpu.write32(game + STARTED, started | bit);
                    set_step(cpu, game, 5);
                }
            }
            _ => {}
        }
    }
    if waiting > 1 {
        cpu.write32(game + TIMING, 1);
    }
    match cpu.read8(game + STEP) {
        5 => {
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SE_START));
            nested!(cpu, 0x0606_0b34, text::hide_line(cpu, 0));
            nested!(cpu, 0x0606_0b34, text::hide_line(cpu, 1));
        }
        step => {
            if step == 4 {
                cpu.write32(game + TIMING, 1);
            }
            let frames = cpu.read32(game + FRAMES);
            cpu.write32(game + FRAMES, frames.wrapping_add(1));
        }
    }
}

/// Step 6, the title ending: once a player started, a string under the picture flashes. When its
/// frames are out (and the low half of the word +0x44 is all set, and the picture ready): the game
/// started; the mode the players start in, or the opening or a demo (with one life, the text's
/// demo mode) if nobody did; then that mode loaded (state 1).
fn title_end(cpu: &mut Cpu, game: u32) {
    if cpu.read32(game + STARTED) != 0 {
        let frames = cpu.read32(game + FRAMES);
        let flash = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, frames, 10)) as i32;
        let y = cpu.read16(STARTED_Y) as i16 as i32;
        let x = cpu.read16(STARTED_X) as i16 as i32;
        let at = ((y + 0x70) / 8 * 0x80 + (x + 0xb0) / 8 * 2) as u32;
        let s = cpu.read32(STARTED_STRINGS + if flash > 4 { 0 } else { 4 });
        nested!(cpu, 0x0606_11b8, text::put_string(cpu, s, TEXT_PLANE.wrapping_add(at), 0));
    }
    let frames = cpu.read32(game + FRAMES);
    if frames as i32 > 0 {
        cpu.write32(game + FRAMES, frames - 1);
        return;
    }
    let flags = cpu.read32(game + GAME_FLAGS_44);
    if flags & LOW_ALL != LOW_ALL || flags & PICTURE_READY == 0 {
        return;
    }
    nested!(cpu, 0x0604_129c, super::play::game_start(cpu));
    if cpu.read32(game + STARTED) == 0 {
        if cpu.read8(game + TIMED_OUT) & 1 == 0 {
            cpu.write8(NEXT_MODE, GAME_OPENING);
        } else {
            cpu.write8(NEXT_MODE, DEMO_MODE);
            let n = cpu.read8(game + TIMED_OUT) >> 1;
            if n == 15 {
                cpu.write8(NEXT_MODE, LAST_DEMO_MODE);
            } else {
                let r = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, n as u32, 3));
                let m = cpu.read8(NEXT_MODE).wrapping_add(r as u8);
                cpu.write8(NEXT_MODE, m);
            }
            cpu.write8(LIVES, 1);
            nested!(cpu, 0x0606_0894, text::set_mode(cpu, 2));
        }
        let n = cpu.read8(game + TIMED_OUT).wrapping_add(1);
        cpu.write8(game + TIMED_OUT, n);
        if n >> 1 > 15 {
            cpu.write8(game + TIMED_OUT, 0);
        }
    } else {
        nested!(cpu, 0x0606_0894, text::set_mode(cpu, 0));
        nested!(cpu, 0x0606_0c2c, text::credits_hidden(cpu));
        let mode = cpu.read8(game + START_MODE);
        cpu.write8(NEXT_MODE, mode);
    }
    let flags = cpu.read32(game + GAME_FLAGS_44);
    cpu.write32(game + GAME_FLAGS_44, flags & !TITLE_END_CLEARS);
    let next = cpu.read8(NEXT_MODE);
    cpu.write8(GAME_MODE, next);
    go_to(cpu, game, 1);
}

/// Every step: while the title is to time out, its time counted down (a fade to black at the end;
/// then step 6, if the low half of the word +0x44 is all set); when it no longer is, the fade
/// stopped and the time back to full. The picture put in VRAM once it is ready.
fn title_time(cpu: &mut Cpu, game: u32) {
    let timing = cpu.read32(game + TIMING);
    if timing & !TIMING_NOW != 0 {
        cpu.write32(game + TIMING, TIMING_NOW);
        if cpu.read32(game + TIME_LEFT) == TITLE_FADE {
            nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, BLACK, BLACK, BLACK, FADE_OUT_SPEED));
        }
        let left = cpu.read32(game + TIME_LEFT);
        if left == 0 {
            if cpu.read32(game + GAME_FLAGS_44) & LOW_ALL == LOW_ALL {
                cpu.write32(game + FRAMES, 0);
                cpu.write32(game + TIMING, 0);
                set_step(cpu, game, 6);
            }
        } else {
            cpu.write32(game + TIME_LEFT, left - 1);
        }
    } else {
        if timing & TIMING_NOW != 0 {
            nested!(cpu, 0x0605_e9d8, screen::stop_fades(cpu));
            nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, 0, 0, 0));
            cpu.write32(game + TIMING, 0);
        }
        cpu.write32(game + TIME_LEFT, TITLE_TIME);
    }
    if cpu.read32(game + PICTURE_SHOWN) == 0 && cpu.read32(game + GAME_FLAGS_44) & PICTURE_READY != 0 {
        let bytes = PICTURE_BYTES[if super::globals::bios_setting_is_1(cpu) { 0 } else { 1 }];
        let picture = cpu.read32(game + PICTURE);
        nested!(cpu, PICTURE_LOAD, screen::picture_load(cpu, picture, bytes, PICTURE_PALETTE));
        nested!(cpu, PICTURE_SHOW, screen::planes_ab(cpu));
        cpu.write32(game + PICTURE_SHOWN, 1);
    }
}

/// The stage's fields in the director: the continue screen is up (both players out); its count
/// (9 to 0, shared with the players' own continue counts) and the frames toward its next second;
/// each player's state (PLAYER_*), and frames counted in it (both +4 a player); where each player
/// is (x, y: the player object's, or the director's own while there is none; +8 a player); a demo's
/// frame and its input (DATA words, then TRIG words 0x2000 bytes on).
const ALL_OUT: u32 = 0x64;
const COUNTDOWN: u32 = 0x68;
const TICKS: u32 = 0x6c;
const PLAYER_STATE: u32 = 0x70;
const PLAYER_FRAMES: u32 = 0x78;
const PLAYER_XY: u32 = 0xa8;
const DIRECTOR_XY: u32 = 0x14;
const DEMO_FRAME: u32 = 0x50;
const DEMO_INPUT: u32 = 0x54;
const DEMO_TRIG: u32 = 0x2000;
/// A demo's frames: counted up to DEMO_LAST, and it ends after DEMO_END.
const DEMO_LAST: i32 = 0xfff;
const DEMO_END: i32 = 0xf88;
/// A player's states: not in yet; out (the line shows how to join); joined (the player object is
/// being made); playing; counting down to continue; out with the continue run out (GAME_OVER_FRAMES).
const PLAYER_NONE: u32 = 0;
const PLAYER_OUT: u32 = 1;
const PLAYER_JOINED: u32 = 2;
const PLAYER_PLAYING: u32 = 3;
const PLAYER_CONTINUE: u32 = 4;
const PLAYER_OVER: u32 = 5;
/// A demo's players: not followed.
const PLAYER_IDLE: u32 = 6;
const GAME_OVER_FRAMES: i32 = 0x12b;
/// The count from which a continue runs, and that a coin starts it again at.
const COUNT_FROM: u32 = 9;
const COUNT_COIN: u32 = 10;
/// A count's second (frames), and the buttons (A, B, C) that end it at once when the count is low.
const SECOND: u32 = 60;
const HURRY_BUTTONS: u16 = 0x700;
const HURRY_BELOW: i32 = 8;
/// The players' input as the game reads it (DATA, TRIG; +8 a player): copied from the pads, or a
/// demo's.
const PLAYER_INPUT: u32 = 0x0602_ee20;
const PLAYER_TRIG: u32 = 0x0602_ee24;
/// More of the players' line messages: as a player's continue runs out; while players cannot join
/// (JOIN_HELD); and (as the count shows) the count's digits 0-9.
const MESSAGE_OVER: u32 = 13;
const MESSAGE_HELD: u32 = 16;
/// Set while players cannot join (not known yet; one of the flags cleared as a mode loads).
const JOIN_HELD: u32 = 0x0600_cc61;
/// The difficulty (the first setting, backup.rs: the current copy and the kept one), and the
/// demos' own.
const SETTING_0: u32 = 0x0600_ed84;
const SETTING_0_KEPT: u32 = 0x0600_edc5;
const DEMO_SETTING_0: u8 = 2;
/// A BIOS word whose bit 6 lets the demos make sound effects (as far as can be told).
const BIOS_SOUND: u32 = 0x0600_0798;
const DEMO_SOUNDS: u32 = 0x40;
/// The game's flags' bit while the continue screen is up.
const CONTINUE_SCREEN: u32 = 0x4000;
/// A player object made: its x and y (16.16) by player, and the player at +0x1e of the struct it is
/// handed; and the stage modes in which a life is taken as it is made (STG01-STG05).
const PLAYER_START_X: [u32; 2] = [0xffd0_0000, 0x0030_0000];
const PLAYER_START_Y: u32 = 0x0020_0000;
const PLAYER_NUMBER: u32 = 0x1e;
const FIRST_COUNTED_STAGE: u8 = 7;
const COUNTED_STAGES: u8 = 5;
/// The continue screen's text: its question, the count's digits (strings by digit), the end, a
/// blank for the digit, a blank line (places on the text plane).
const CONTINUE_TEXT: u32 = 0x0604_1db8;
const CONTINUE_AT: u32 = 0x25e7_e61c;
const DIGITS: u32 = 0x0600_9fb8;
const DIGIT_AT: u32 = 0x25e7_e72a;
const OVER_TEXT: u32 = 0x0604_1dc4;
const OVER_AT: u32 = 0x25e7_e608;
const BLANK_DIGIT: u32 = 0x0604_1dd8;
const BLANK_LINE: u32 = 0x0604_1ddc;
/// The continue screen's: the text plane (NBG2) put on top (SGL's slPriority) and the screens under
/// it darkened by colour offsets A and B (slColOffsetA, slColOffsetB), and the darkening.
const SL_PRIORITY: u32 = 0x0606_98dc;
const SL_COLOR_OFFSET_A: u32 = 0x0606_98ec;
const SL_COLOR_OFFSET_B: u32 = 0x0606_98fc;
const DARKER: u32 = -80i32 as u32;
/// A player object made (from a struct: x, y, z, the player: actors.rs's player_set), a player
/// ended (system.rs).
const PLAYER_SET: u32 = 0x0604_816c;
const PLAYER_ENDED: u32 = 0x0600_6928;

/// State 7, a stage (or a demo of one): faded in, the mode's music started (in a demo: the random
/// numbers from 0, the middle difficulty, and sound effects only if the BIOS lets them). Each frame each player is followed through their
/// states; with both out, the continue screen counts down to the game's end. A demo plays its
/// input, ends by itself (to the result) or when a player can start (to the title). When the mode is
/// done: state 8.
pub fn state_stage(cpu: &mut Cpu, game: u32) {
    match cpu.read8(game + STEP) {
        0 => {
            nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, BLACK, BLACK, BLACK));
            nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, 0, 0, 0, FADE_IN_SPEED));
            let flags = cpu.read8(game + FLAGS);
            cpu.write8(game + FLAGS, flags | RUNNING);
            for field in [ALL_OUT, PLAYER_STATE + 4, PLAYER_STATE] {
                cpu.write32(game + field, 0);
            }
            cpu.write32(sound::SE_LOCK, 0);
            if cpu.read8(GAME_MODE) as i8 > 11 {
                cpu.write32(game + PLAYER_STATE + 4, PLAYER_IDLE);
                cpu.write32(game + PLAYER_STATE, PLAYER_IDLE);
                nested!(cpu, 0x0604_5d78, math::random_seed(cpu, 0));
                cpu.write32(game + DEMO_FRAME, 0);
                cpu.write8(SETTING_0, DEMO_SETTING_0);
                if cpu.read32(BIOS_SOUND) & DEMO_SOUNDS == 0 {
                    cpu.write32(sound::SE_LOCK, 1);
                }
            } else {
                let kept = cpu.read8(SETTING_0_KEPT);
                cpu.write8(SETTING_0, kept);
                let music = cpu.read16(game + MUSIC);
                if music as i16 >= 0 {
                    nested!(cpu, 0x0600_4e98, sound::request_music(cpu, music as u32));
                }
            }
            set_step(cpu, game, 1);
        }
        1 => {
            if cpu.read8(GAME_MODE) as i8 > 11 {
                stage_demo(cpu, game);
            }
            if cpu.read8(game + FLAGS) & DONE != 0 {
                go_to(cpu, game, 8);
            }
        }
        _ => {}
    }
    for p in 0..2 {
        stage_player(cpu, game, p);
    }
    stage_continue(cpu, game);
}

/// A demo's frame: its input in place of player 0's (player 1's none), its frames counted (it ends
/// past DEMO_END, to the result); the lines tell how to play; on free play Start, else coins in (or
/// a demo without input), skip to the title.
fn stage_demo(cpu: &mut Cpu, game: u32) {
    let frame = cpu.read32(game + DEMO_FRAME);
    let input = cpu.read32(game + DEMO_INPUT);
    let data = cpu.read16(input.wrapping_add(2 * frame));
    cpu.write16(PLAYER_INPUT, data);
    let frame = cpu.read32(game + DEMO_FRAME);
    let input = cpu.read32(game + DEMO_INPUT);
    let trig = cpu.read16(input.wrapping_add(2 * frame).wrapping_add(DEMO_TRIG));
    cpu.write16(PLAYER_TRIG, trig);
    cpu.write16(PLAYER_TRIG + 8, 0);
    cpu.write16(PLAYER_INPUT + 8, 0);
    let mut frame = cpu.read32(game + DEMO_FRAME) as i32;
    if frame < DEMO_LAST {
        cpu.write32(game + DEMO_FRAME, (frame + 1) as u32);
        frame = cpu.read32(game + DEMO_FRAME) as i32;
    }
    if frame > DEMO_END {
        cpu.write8(NEXT_MODE, super::globals::mode::GAME_RESULT);
        go_to(cpu, game, 9);
    }
    let skip = if system::free_play(cpu) {
        let loops = cpu.read32(LOOPS);
        let message = if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, loops, 60)) as i32 > 29 { MESSAGE_START } else { MESSAGE_FREE };
        nested!(cpu, 0x0606_0b78, text::show_message(cpu, 0, message));
        nested!(cpu, 0x0606_0b78, text::show_message(cpu, 1, message));
        (cpu.read16(PAD_TRIG[0]) | cpu.read16(PAD_TRIG[1])) & START != 0
    } else {
        let loops = cpu.read32(LOOPS);
        if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, loops, 60)) as i32 > 39 {
            nested!(cpu, 0x0606_0b34, text::hide_line(cpu, 0));
            nested!(cpu, 0x0606_0b34, text::hide_line(cpu, 1));
        } else {
            nested!(cpu, 0x0606_0b78, text::show_message(cpu, 0, MESSAGE_COIN));
            nested!(cpu, 0x0606_0b78, text::show_message(cpu, 1, MESSAGE_COIN));
        }
        nested!(cpu, 0x0604_0f44, system::coin_byte3(cpu, 0)) != 0 || nested!(cpu, 0x0604_0f44, system::coin_byte3(cpu, 1)) != 0 || cpu.read32(game + DEMO_INPUT) == 0
    };
    if skip {
        nested!(cpu, 0x0606_0b34, text::hide_line(cpu, 0));
        nested!(cpu, 0x0606_0b34, text::hide_line(cpu, 1));
        cpu.write8(NEXT_MODE, GAME_TITLE);
        nested!(cpu, SET_SKIPPED, super::director::mode_skipped(cpu));
        go_to(cpu, game, 9);
    }
}

/// Player `p` through their states: made again from their lives while they have none of their
/// object; their place kept; joining (Start with a credit, as a new player or a continue), playing,
/// counting down to continue (their line shows the count), out.
fn stage_player(cpu: &mut Cpu, game: u32, p: u32) {
    let slot = PLAYERS + 4 * p;
    let lives = LIVES + p;
    let play_data = super::globals::PLAY_DATA + super::globals::PLAY_DATA_SIZE * p;
    let xy = game + PLAYER_XY + 8 * p;
    let obj = cpu.read32(slot);
    if obj == 0 {
        if cpu.read8(lives) != 0 {
            let start = cpu.stack();
            cpu.write32(start, PLAYER_START_X[(p != 0) as usize]);
            cpu.write32(start + 4, PLAYER_START_Y);
            cpu.write32(start + 8, 0);
            cpu.write8(start + PLAYER_NUMBER, p as u8);
            nested!(cpu, PLAYER_SET, super::actors::player_set(cpu, start));
            if cpu.read8(GAME_MODE).wrapping_sub(FIRST_COUNTED_STAGE) < COUNTED_STAGES {
                let n = cpu.read8(lives);
                cpu.write8(lives, n.wrapping_sub(1));
            }
        }
        let x = cpu.read32(game + DIRECTOR_XY);
        cpu.write32(xy, x);
        let y = cpu.read32(game + DIRECTOR_XY + 4);
        cpu.write32(xy + 4, y);
    } else {
        let x = cpu.read32(obj + DIRECTOR_XY);
        cpu.write32(xy, x);
        let obj = cpu.read32(slot);
        let y = cpu.read32(obj + DIRECTOR_XY + 4);
        cpu.write32(xy + 4, y);
    }
    let mut state = cpu.read32(game + PLAYER_STATE + 4 * p);
    let mut frames = cpu.read32(game + PLAYER_FRAMES + 4 * p);
    match state {
        PLAYER_NONE => {
            if cpu.read32(slot) != 0 {
                state = PLAYER_JOINED;
            } else {
                frames = 0;
                state = PLAYER_OUT;
            }
        }
        PLAYER_OUT | PLAYER_CONTINUE => {
            if state == PLAYER_CONTINUE && cpu.read32(game + ALL_OUT) == 0 && continue_count(cpu, game, p, &mut frames) {
                nested!(cpu, 0x0606_0b78, text::show_message(cpu, p, MESSAGE_OVER));
                state = PLAYER_OVER;
            } else {
                state = player_line(cpu, game, p, state, &mut frames, play_data);
            }
        }
        PLAYER_JOINED | PLAYER_PLAYING => {
            if state == PLAYER_JOINED && cpu.read32(slot) != 0 {
                if (cpu.read8(play_data + super::globals::CONTINUES) as i8) < 0 {
                    nested!(cpu, 0x0604_114c, super::play::player_reset(cpu, p));
                    cpu.write8(play_data + super::globals::CONTINUES, 0);
                }
                nested!(cpu, 0x0606_0a48, text::show_buffers(cpu, p));
                nested!(cpu, 0x0606_0354, super::hud::status_reset(cpu, p));
                state = PLAYER_PLAYING;
            }
            if state == PLAYER_PLAYING {
                if cpu.read32(slot) != 0 {
                    nested!(cpu, 0x0606_0a0c, super::hud::draw_status(cpu, p));
                } else if cpu.read8(lives) == 0 {
                    cpu.write32(game + COUNTDOWN, COUNT_FROM);
                    cpu.write32(game + TICKS, 0);
                    frames = 0;
                    nested!(cpu, PLAYER_ENDED, system::player_ended(cpu, p));
                    state = PLAYER_CONTINUE;
                }
            }
        }
        PLAYER_OVER => {
            frames = frames.wrapping_add(1);
            if frames as i32 > GAME_OVER_FRAMES {
                state = PLAYER_OUT;
            }
        }
        _ => {}
    }
    cpu.write32(game + PLAYER_STATE + 4 * p, state);
    cpu.write32(game + PLAYER_FRAMES + 4 * p, frames);
}

/// A player's continue count (while the continue screen is not up): its frame counted, a second
/// passed at once with A, B or C once the count is low, the count back to full with coins in.
/// Whether it ran out.
fn continue_count(cpu: &mut Cpu, game: u32, p: u32, frames: &mut u32) -> bool {
    let ticks = cpu.read32(game + TICKS);
    cpu.write32(game + TICKS, ticks.wrapping_add(1));
    if cpu.read16(PAD_TRIG[0] + 8 * p) & HURRY_BUTTONS != 0 && (cpu.read32(game + COUNTDOWN) as i32) < HURRY_BELOW {
        cpu.write32(game + TICKS, SECOND);
    }
    if cpu.read8(game + COUNTED) != 0 {
        *frames = 0;
        cpu.write32(game + TICKS, SECOND);
        cpu.write32(game + COUNTDOWN, COUNT_COIN);
    }
    if cpu.read32(game + TICKS) as i32 > SECOND as i32 - 1 {
        cpu.write32(game + TICKS, 0);
        let count = cpu.read32(game + COUNTDOWN).wrapping_sub(1);
        cpu.write32(game + COUNTDOWN, count);
        return (count as i32) < 0;
    }
    false
}

/// The line of a player who is out or counting down (the BIOS told which players count down), and
/// their joining: Start with a credit gives the lives (a continue counted, else the play data to
/// be made anew). Their next state.
fn player_line(cpu: &mut Cpu, game: u32, p: u32, state: u32, frames: &mut u32, play_data: u32) -> u32 {
    cpu.write8(CONTINUING + p, (state == PLAYER_CONTINUE) as u8);
    system::bios(cpu, BIOS_CONTINUING, &[CONTINUING]);
    let mut state = state;
    let mut alternate = false;
    let second = |cpu: &mut Cpu, frames: u32| nested!(cpu, 0x0600_6a6e, math::remainder(cpu, frames, 60)) as i32;
    match nested!(cpu, 0x0604_0f44, system::coin_byte3(cpu, p)) {
        0 | 1 => {
            if second(cpu, *frames) <= 29 {
                let message = if nested!(cpu, 0x0604_0f44, system::coin_byte3(cpu, p)) == 0 { MESSAGE_COIN } else { MESSAGE_SHORT };
                nested!(cpu, 0x0606_0b78, text::show_message(cpu, p, message));
            } else {
                alternate = true;
                if state == PLAYER_OUT && nested!(cpu, 0x0604_0f24, system::coin_settings_shown(cpu)) & SHOW_SECOND != 0 {
                    nested!(cpu, 0x0606_0b78, text::show_message(cpu, p, MESSAGE_SECOND));
                    alternate = false;
                }
            }
        }
        2 if cpu.read8(JOIN_HELD) != 0 => {
            if second(cpu, *frames) <= 29 {
                nested!(cpu, 0x0606_0b78, text::show_message(cpu, p, MESSAGE_HELD));
            } else {
                nested!(cpu, 0x0606_0b34, text::hide_line(cpu, p));
            }
            cpu.write32(game + TICKS, 0);
            cpu.write32(game + COUNTDOWN, COUNT_FROM);
        }
        2 => {
            if second(cpu, *frames) <= 29 {
                nested!(cpu, 0x0606_0b78, text::show_message(cpu, p, MESSAGE_START));
            } else {
                if system::free_play(cpu) {
                    nested!(cpu, 0x0606_0b78, text::show_message(cpu, p, MESSAGE_FREE));
                }
                alternate = true;
            }
            if cpu.read16(PLAYER_TRIG + 8 * p) & START != 0 && nested!(cpu, 0x0604_0f5c, system::use_credit(cpu, p)) != 0 {
                let n = cpu.read8(START_LIVES);
                cpu.write8(LIVES + p, n);
                let continues = play_data + super::globals::CONTINUES;
                if state == PLAYER_CONTINUE {
                    let c = cpu.read8(continues);
                    if (c as i8) < 127 {
                        cpu.write8(continues, c.wrapping_add(1));
                    }
                } else {
                    cpu.write8(continues, 0xff);
                }
                nested!(cpu, PLAYER_STARTED, system::player_started(cpu, p));
                state = PLAYER_JOINED;
            }
        }
        _ => {}
    }
    if alternate {
        if state == PLAYER_CONTINUE {
            let count = cpu.read32(game + COUNTDOWN);
            nested!(cpu, 0x0606_0b78, text::show_message(cpu, p, count));
        } else if state == PLAYER_OUT {
            nested!(cpu, 0x0606_0b34, text::hide_line(cpu, p));
        }
    }
    *frames = frames.wrapping_add(1);
    state
}

/// With both players out, the continue screen: the screens darkened, the game held, its count from
/// 9 (A, B or C passes a second once it is low; coins start it again). Run out: the end shown, the
/// name entry next (state 9). A player back in: the screen cleared, the game on.
fn stage_continue(cpu: &mut Cpu, game: u32) {
    if cpu.read32(game + ALL_OUT) == 0 {
        if cpu.read32(PLAYERS) != 0 || cpu.read32(PLAYERS + 4) != 0 {
            return;
        }
        cpu.call(SL_PRIORITY, &[3, 7]);
        cpu.write32(GAME_FLAGS, CONTINUE_SCREEN);
        cpu.write32(game + COUNTDOWN, COUNT_FROM);
        cpu.write32(game + TICKS, 0);
        cpu.call(SL_COLOR_OFFSET_A, &[DARKER, DARKER, DARKER]);
        cpu.call(SL_COLOR_OFFSET_B, &[DARKER, DARKER, DARKER]);
        nested!(cpu, 0x0606_0c08, text::credits_shown(cpu));
        cpu.write32(game + PLAYER_FRAMES + 4, 0);
        cpu.write32(game + PLAYER_FRAMES, 0);
        cpu.write32(game + ALL_OUT, 1);
        return;
    }
    cpu.call(SL_COLOR_OFFSET_A, &[DARKER, DARKER, DARKER]);
    cpu.call(SL_COLOR_OFFSET_B, &[DARKER, DARKER, DARKER]);
    nested!(cpu, 0x0606_1204, text::put_large_string(cpu, CONTINUE_TEXT, CONTINUE_AT, 0));
    let count = cpu.read32(game + COUNTDOWN);
    if count as i32 >= 0 {
        let digit = cpu.read32(DIGITS + 4 * count);
        nested!(cpu, 0x0606_1204, text::put_large_string(cpu, digit, DIGIT_AT, 0));
    }
    let ticks = cpu.read32(game + TICKS);
    cpu.write32(game + TICKS, ticks.wrapping_add(1));
    if (cpu.read16(PAD_TRIG[0]) | cpu.read16(PAD_TRIG[1])) & HURRY_BUTTONS != 0 && (cpu.read32(game + COUNTDOWN) as i32) < HURRY_BELOW {
        cpu.write32(game + TICKS, SECOND);
    }
    if cpu.read8(game + COUNTED) != 0 {
        cpu.write32(game + PLAYER_FRAMES + 4, 0);
        cpu.write32(game + PLAYER_FRAMES, 0);
        cpu.write32(game + TICKS, 0);
        cpu.write32(game + COUNTDOWN, COUNT_FROM);
    }
    if cpu.read32(game + TICKS) as i32 > SECOND as i32 - 1 {
        cpu.write32(game + TICKS, 0);
        let count = cpu.read32(game + COUNTDOWN).wrapping_sub(1);
        cpu.write32(game + COUNTDOWN, count);
        if (count as i32) < 0 {
            nested!(cpu, 0x0606_1204, text::put_large_string(cpu, OVER_TEXT, OVER_AT, 0));
            nested!(cpu, 0x0606_1204, text::put_large_string(cpu, BLANK_DIGIT, DIGIT_AT, 0));
            cpu.write8(NEXT_MODE, super::globals::mode::GAME_NAME_ENTRY);
            go_to(cpu, game, 9);
        }
    }
    if cpu.read32(PLAYERS) != 0 || cpu.read32(PLAYERS + 4) != 0 {
        nested!(cpu, 0x0606_1204, text::put_large_string(cpu, BLANK_LINE, OVER_AT, 0));
        nested!(cpu, 0x0606_1204, text::put_large_string(cpu, BLANK_DIGIT, DIGIT_AT, 0));
        let f = cpu.read32(GAME_FLAGS);
        cpu.write32(GAME_FLAGS, f & !CONTINUE_SCREEN);
        nested!(cpu, 0x0606_0c2c, text::credits_hidden(cpu));
        cpu.write32(game + ALL_OUT, 0);
    }
}

/// Each player's stage scores (in their PLAY_DATA): a long for each stage (STG00-STG05) and route.
const STAGE_SCORES: u32 = 0x48;
const ROUTES: u32 = 5;
/// A stage's end counts its frames here (where the screens' states count at +0x64).
const END_COUNT: u32 = 0x68;
/// The last stage's end: a slower fade to black, and its frames and the music's fade.
const LAST_FADE_SPEED: u32 = 0x16e;
const LAST_END_FRAMES: u32 = 180;
const LAST_MUSIC_FADE: u32 = 170;

/// State 8, a stage's end: the text's objects stopped, a fade to black with the music faded out
/// (slower after the last stage; cut at once when skipped), the screen handed over; then the next
/// mode, cleared up first unless it is a stage. After STG00 the players still in keep their score
/// as that stage's (in tens: the continues' digit dropped) and start the game anew with full lives.
pub fn state_stage_end(cpu: &mut Cpu, game: u32) {
    match cpu.read8(game + STEP) {
        step @ (0 | 1) => {
            if step == 0 {
                nested!(cpu, 0x0606_12bc, text::stop_text_objects(cpu));
                set_step(cpu, game, 1);
            }
            nested!(cpu, 0x0605_e9d8, screen::stop_fades(cpu));
            if cpu.read8(game + FLAGS) & SKIPPED != 0 {
                nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, BLACK, BLACK, BLACK));
                cpu.write32(game + END_COUNT, 0);
                nested!(cpu, 0x0600_4f0c, sound::music_fade_out(cpu, 0));
            } else if cpu.read8(GAME_MODE) == super::globals::mode::STG05 {
                nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, BLACK, BLACK, BLACK, LAST_FADE_SPEED));
                cpu.write32(game + END_COUNT, LAST_END_FRAMES);
                nested!(cpu, 0x0600_4f0c, sound::music_fade_out(cpu, LAST_MUSIC_FADE));
            } else {
                nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, BLACK, BLACK, BLACK, FADE_OUT_SPEED));
                cpu.write32(game + END_COUNT, 30);
                nested!(cpu, 0x0600_4f0c, sound::music_fade_out(cpu, 20));
            }
            set_step(cpu, game, 2);
        }
        2 => {
            let n = cpu.read32(game + END_COUNT).wrapping_sub(1);
            cpu.write32(game + END_COUNT, n);
            if n as i32 <= 0 {
                let flags = cpu.read8(game + FLAGS);
                cpu.write8(game + FLAGS, flags | HANDED_OVER);
                cpu.write32(game + END_COUNT, 2);
                set_step(cpu, game, 3);
            }
        }
        3 => {
            let n = cpu.read32(game + END_COUNT).wrapping_sub(1);
            cpu.write32(game + END_COUNT, n);
            if n as i32 > 0 {
                return;
            }
            if cpu.read8(GAME_MODE) == super::globals::mode::STG00 {
                first_stage_end(cpu, game);
            }
            let next = cpu.read8(NEXT_MODE);
            cpu.write8(GAME_MODE, next);
            if next.wrapping_sub(super::globals::mode::STG00) > 5 {
                nested!(cpu, 0x0606_0894, text::set_mode(cpu, 1));
                nested!(cpu, 0x0604_1010, clear_mode(cpu, game));
            }
            go_to(cpu, game, 1);
        }
        _ => {}
    }
}

/// After STG00: the game started anew and cleared up; each player still in gets full lives, their
/// score filed as the stage's for the route (in tens), no continues.
fn first_stage_end(cpu: &mut Cpu, game: u32) {
    let alive = [cpu.read8(LIVES) as i8, cpu.read8(LIVES + 1) as i8];
    let mut scores = [0; 2];
    for p in 0..2 {
        if alive[p] > 0 {
            let play_data = super::globals::PLAY_DATA + super::globals::PLAY_DATA_SIZE * p as u32;
            scores[p] = cpu.read32(play_data + super::globals::SCORE);
        }
    }
    nested!(cpu, 0x0604_129c, super::play::game_start(cpu));
    nested!(cpu, 0x0604_1010, clear_mode(cpu, game));
    for p in 0..2 {
        if alive[p] <= 0 {
            continue;
        }
        let play_data = super::globals::PLAY_DATA + super::globals::PLAY_DATA_SIZE * p as u32;
        let lives = cpu.read8(START_LIVES);
        cpu.write8(LIVES + p as u32, lives);
        // The entry: by the stage and the route, each read anew as the SH-2 code does.
        let entry = |cpu: &mut Cpu| {
            let stage = (cpu.read8(GAME_MODE) as i8 as i32 - super::globals::mode::STG00 as i32) as u32;
            let route = cpu.read8(MODE_SUB) as i8 as i32 as u32;
            play_data + STAGE_SCORES + 4 * (ROUTES * stage + route)
        };
        let at = entry(cpu);
        cpu.write32(at, scores[p]);
        let at = entry(cpu);
        let tens = cpu.read32(at) as i32 / 10;
        cpu.write32(at, tens as u32);
        let at = entry(cpu);
        let v = cpu.read32(at);
        cpu.write32(at, v.wrapping_mul(10));
        cpu.write8(play_data + super::globals::CONTINUES, 0);
    }
}

/// A stage's data block (its overlay's, at `p`): the block, and its three parts (offsets in its
/// first three longs) made addresses.
const STAGE_DATA: u32 = 0x0607_4800;
const STAGE_PARTS: [u32; 3] = [0x0607_47fc, 0x0607_47f8, 0x0607_47d0];

pub fn stage_data_set(cpu: &mut Cpu, p: u32) {
    cpu.write32(STAGE_DATA, p);
    for (k, at) in STAGE_PARTS.into_iter().enumerate() {
        let v = cpu.read32(p + 4 * k as u32).wrapping_add(p);
        cpu.write32(at, v);
    }
}

/// The long at index `i` of a table the stages' code reads (not known yet).
const STAGE_TABLE: u32 = 0x0607_358c;

pub fn stage_table(cpu: &mut Cpu, i: u32) -> u32 {
    cpu.read32(STAGE_TABLE.wrapping_add(i << 2))
}
