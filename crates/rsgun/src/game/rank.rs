//! RANK.BIN's code (fingerprint 4635b7419f6fc1a7), which the ranking mode runs in the overlay
//! window after a game ends: the score table and the name entry.
//!
//! The mode's code (the director calls it each frame until it gives 1) loads the screen's data
//! file and sets up the scrolls, then makes the actor for the game mode it was started for: the
//! name entry (game mode 3: a new score typed in) or the result (4: what the game scored).

use super::globals::{self, mode, GAME_MODE, LOOPS};
use super::{actors, backup, bg, director, files, math, nested, objects, pads, palette, screen, sgl, sound, sprites, states, system, text, Cpu};

/// The director's slots the mode uses (states.rs's, as OPENING's mode uses them): its step, the
/// load (a local written back each time), the settings kept (the buffer at +0x6c, the room's top
/// at +0x70), the file's size (+0x80), and the mode's music (+0xa4).
const MODE_STEP: u32 = 0x64;
const LOAD: u32 = 0x68;
const KEPT: u32 = 0x6c;
const PLACE: u32 = 0x70;
const SIZE: u32 = 0x80;
const MUSIC: u32 = 0xa4;
/// The room the mode takes below the room's top (states.rs's settings), and the top itself.
const ROOM: u32 = 0x4_4040;
const ROOM_TOP: u32 = 0x0607_4050;
/// The rate the next load takes (files.rs), and the game mode that follows (a byte; 0 is the
/// first).
const NEXT_RATE: u32 = 0x0603_2e40;
const NEXT_MODE: u32 = 0x0600_ed70;
const SOUND_RATE: u32 = 0x2_0000;
/// The sound data files the mode's first steps load, in turn (steps 1 to 8; step 0 goes on to
/// step 5, so the first two are not used).
const SOUND_FILES: [u32; 4] = [5, 4, 0x19, 0x18];
/// The name of the screen's data file (in the main image's table of names).
const SCREEN_FILE: u32 = 0x0600_92e8;

/// The ranking mode's code: the settings kept, sound data files 0x19 and 0x18 loaded in turn
/// below the room's top and copied to their areas (steps 1 to 4, the same with the files 5 and 4,
/// are skipped by the first), the screen's data file loaded into the buffer, the screen set up
/// from it (rank_screen), the text plane cleared and the actor for the game mode made, the
/// settings given back; 1.
pub fn rank_mode(cpu: &mut Cpu, game: u32) -> u32 {
    let mut load = cpu.read32(game + LOAD);
    let mut done = 0;
    match cpu.read8(game + MODE_STEP) {
        0 => {
            states::keep_mode_settings(cpu, game, ROOM);
            cpu.write8(game + MODE_STEP, 5);
        }
        step @ (1 | 3 | 5 | 7) => {
            let n = SOUND_FILES[(step / 2) as usize];
            cpu.write32(NEXT_RATE, SOUND_RATE);
            let place = cpu.read32(game + PLACE);
            load = nested!(cpu, 0x0600_46d8, sound::load_sound_data(cpu, n, place));
            if load != 0 {
                let size = nested!(cpu, 0x0600_5edc, files::load_size(cpu, load));
                cpu.write32(game + SIZE, size);
                cpu.write8(game + MODE_STEP, step + 1);
            }
        }
        step @ (2 | 4 | 6 | 8) => {
            let n = SOUND_FILES[(step / 2 - 1) as usize];
            if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 {
                let (place, size) = (cpu.read32(game + PLACE), cpu.read32(game + SIZE));
                nested!(cpu, 0x0600_4adc, sound::copy_to_area(cpu, n, place, size));
                let next = if step == 8 { cpu.read32(ROOM_TOP) } else { place.wrapping_add(size) };
                cpu.write32(game + PLACE, next);
                if step == 4 {
                    cpu.write16(game + MUSIC, 1);
                }
                cpu.write8(game + MODE_STEP, step + 1);
            }
        }
        9 => {
            load = nested!(cpu, 0x0600_5e40, files::load_file(cpu, SCREEN_FILE));
            if load != 0 {
                cpu.write32(NEXT_RATE, SOUND_RATE);
                let buffer = cpu.read32(game + KEPT);
                nested!(cpu, 0x0600_603c, files::start_load(cpu, load, buffer));
                cpu.write8(game + MODE_STEP, 10);
            }
        }
        10 => {
            if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 {
                cpu.write8(game + MODE_STEP, 14);
                let buffer = cpu.read32(game + KEPT);
                // (the SH-2 code holds the director's slot of the load in r9 and the load in r10)
                cpu.holding(&[(9, game + LOAD), (10, load), (12, 0)], |cpu| nested!(cpu, 0x0608_4684, rank_screen(cpu, buffer)));
                cpu.write32(game + LOAD, load);
                return 0;
            }
        }
        13 => cpu.write8(game + MODE_STEP, 14),
        14 => {
            nested!(cpu, 0x0608_4904, text_plane_clear(cpu));
            if cpu.read8(GAME_MODE) == mode::GAME_NAME_ENTRY {
                nested!(cpu, 0x0608_3840, name_entry_set(cpu));
            }
            if cpu.read8(GAME_MODE) == mode::GAME_RESULT {
                nested!(cpu, 0x0608_45a8, result_set(cpu));
            }
            cpu.write8(NEXT_MODE, 0);
            states::give_back_mode_settings(cpu, game);
            cpu.write8(game + MODE_STEP, 15);
        }
        15 => done = 1,
        _ => {}
    }
    cpu.write32(game + LOAD, load);
    done
}

// ---- the actors the mode makes -------------------------------------------------------------------

/// An actor's sprite (its own struct, at +0x10), its behaviour (+0x38) and its step (a word at
/// +0x3c: the part in its low byte, the stage of that in the next).
const SPRITE: u32 = 0x10;
const BEHAVIOUR: u32 = 0x38;
const STEP: u32 = 0x3c;
/// actors.rs's plain_exec, which runs an actor's behaviour each frame.
const PLAIN_EXEC: u32 = 0x0604_dcb4;
/// The name entry's actor (list 0, 0x90 bytes) and the result's (list 2, 0xb50 bytes).
const NAME_ENTRY: u32 = 0x0608_07f0;
const RESULT: u32 = 0x0608_3b1c;

/// An actor of `bytes` bytes in `list` with `behaviour`, its sprite made and its step 0: the
/// actor (0 if there is no room).
fn actor_set(cpu: &mut Cpu, list: u32, bytes: u32, behaviour: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, bytes)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, list, blocks, PLAIN_EXEC));
    if o != 0 {
        nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
        cpu.write32(o + BEHAVIOUR, behaviour);
        cpu.write16(o + STEP, 0);
    }
    o
}

/// The name entry's actor made.
pub fn name_entry_set(cpu: &mut Cpu) -> u32 {
    actor_set(cpu, 0, 0x90, NAME_ENTRY)
}

/// The result's actor made.
pub fn result_set(cpu: &mut Cpu) -> u32 {
    actor_set(cpu, 2, 0xb50, RESULT)
}

// ---- the screen ----------------------------------------------------------------------------------

/// The screen's VRAM: NBG1's cells, its map and NBG0's (the text plane's, 64 cells a row) and
/// NBG0's cells; the back colour's place.
const NBG1_CELLS: u32 = 0x25e0_0000;
const NBG1_MAP: u32 = 0x25e6_0000;
const TEXT_MAP: u32 = 0x25e6_4000;
const NBG0_CELLS: u32 = 0x25e7_8000;
const BACK_COLOUR_AT: u32 = 0x25e7_fffe;
/// The screen's data file's cells (all of NBG1's, 0x12ec0 bytes), colours (index 0x200, 0x100
/// of them) and map (44 by 28 cells, 32 past the font).
const CELLS_BYTES: u32 = 0x1_2ec0;
const COLOURS_AT: u32 = 0x200;
const COLOURS: u32 = 0x100;
const MAP_W: u32 = 44;
const MAP_H: u32 = 28;
const MAP_CHAR_OFFSET: u32 = 32;
/// The screen's fields (screen.rs): the byte cleared; and the pointer set to a table.
const SCREEN_AT: u32 = 0x0607_4740;
const FIELD_CLEARED: u32 = 0x50;
const TABLE_AT: u32 = 0x0607_47cc;
const TABLE: u32 = 0x0607_35a8;
/// The planes' start: where the centre is (x, y) and their zoom.
const PLANE_PLACE: (u32, u32) = (0xb0_0000, 0x70_0000);
const ONE: u32 = 0x1_0000;
/// SGL's priority setter's entry (the SH-2 code keeps it in a register for its three calls).
const SL_PRIORITY: u32 = 0x0606_98dc;
const BLACK: u32 = -255i32 as u32;

/// The screen set up from the data file at `base` (its header: the offsets of three lists of
/// offsets, each of whose first item is taken): NBG0 for the text (its cells at 0x25e78000) and
/// NBG1 for the picture, VRAM split in two, both shown, back colour and colour calculation off,
/// sprite type 5, the priorities, the planes at their start and the data put in
/// (rank_screen_load).
pub fn rank_screen(cpu: &mut Cpu, base: u32) {
    // (the SH-2 code holds the data in r12; r9 is 0 from the first page on, r8 is NBG0's map for
    // the next three calls, the zoom, the first plane's place and the second's after them, with
    // r10 and r11 their place, then SGL's priority setter)
    cpu.holding(&[(12, base)], |cpu| {
        nested!(cpu, 0x0606_e288, sgl::sl_char_nbg0(cpu, 0, 0));
        cpu.holding(&[(9, 0)], |cpu| {
            nested!(cpu, 0x0606_e300, sgl::sl_page_nbg0(cpu, NBG0_CELLS, 0x600, 0x8000));
            nested!(cpu, 0x0607_0c3c, sgl::sl_plane_nbg0(cpu, 0));
            cpu.holding(&[(8, TEXT_MAP)], |cpu| {
                nested!(cpu, 0x0607_0c4c, sgl::sl_map_nbg0(cpu, TEXT_MAP, TEXT_MAP, TEXT_MAP, TEXT_MAP));
                nested!(cpu, 0x0605_eee4, screen::fill_cells(cpu, 0, TEXT_MAP, 64, 64));
                nested!(cpu, 0x0607_0cfc, sgl::sl_zoom_mode(cpu, 1, 0));
            });
            let planes = [(8, ONE), (10, PLANE_PLACE.1), (11, PLANE_PLACE.0)];
            cpu.holding(&planes, |cpu| {
                plane_set(cpu, bg::PLANE_A);
                nested!(cpu, 0x0605_f040, bg::plane_a_frame(cpu));
                nested!(cpu, 0x0606_dd5c, sgl::sl_scr_transparent(cpu, 2));
                nested!(cpu, 0x0606_e294, sgl::sl_char_nbg1(cpu, 0x10, 0));
                nested!(cpu, 0x0607_0d0c, sgl::sl_plane_nbg1(cpu, 0));
                nested!(cpu, 0x0606_e2e4, sgl::sl_page_nbg1(cpu, NBG1_CELLS, 0x200, 0x4000));
                nested!(cpu, 0x0607_0d1c, sgl::sl_map_nbg1(cpu, NBG1_MAP, NBG1_MAP, NBG1_MAP, NBG1_MAP));
                plane_set(cpu, bg::PLANE_B);
                nested!(cpu, 0x0605_f08c, bg::plane_b_frame(cpu));
                nested!(cpu, 0x0606_dd4c, sgl::vram_split(cpu, 2));
                nested!(cpu, 0x0606_dd6c, sgl::sl_scr_auto_disp(cpu, 7));
                nested!(cpu, 0x0607_0dec, sgl::sl_back1_col_set(cpu, BACK_COLOUR_AT, 0));
                nested!(cpu, 0x0607_0ddc, sgl::sl_color_calc(cpu, 0));
                nested!(cpu, 0x0605_e6c0, screen::offsets_clear(cpu));
                nested!(cpu, 0x0608_4924, sgl::sl_sprite_type(cpu, 5));
            });
            let rest = [(8, SL_PRIORITY), (10, PLANE_PLACE.1), (11, PLANE_PLACE.0)];
            cpu.holding(&rest, |cpu| {
                for (screen, pri) in [(1, 6), (0, 4), (-7i32 as u32, 5)] {
                    nested!(cpu, 0x0606_98dc, sgl::sl_priority(cpu, screen, pri));
                }
                nested!(cpu, 0x0605_e34c, screen::screen_reset(cpu));
                nested!(cpu, 0x0605_e2dc, screen::screen_fields(cpu));
                cpu.write32(TABLE_AT, TABLE);
                nested!(cpu, 0x0605_e9d8, screen::stop_fades(cpu));
                nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, BLACK, BLACK, BLACK));
                let fields = cpu.read32(SCREEN_AT);
                cpu.write8(fields + FIELD_CLEARED, 0);
                nested!(cpu, 0x0605_fde8, screen::request_clear_text_plane(cpu));
                nested!(cpu, 0x0608_4600, rank_screen_load(cpu, base));
            });
        });
    });
}

/// A plane at its start: the centre, no speed, the zoom one.
fn plane_set(cpu: &mut Cpu, plane: u32) {
    cpu.write32(plane + bg::X, PLANE_PLACE.0);
    cpu.write32(plane + bg::Y, PLANE_PLACE.1);
    cpu.write32(plane + bg::SPEED_Y, 0);
    cpu.write32(plane + bg::SPEED_X, 0);
    cpu.write32(plane + bg::ZOOM_Y, ONE);
    cpu.write32(plane + bg::ZOOM_X, ONE);
}

const PUT_LONG_CELLS: u32 = 0x0605_ee94;
static PUT_LONG_CELLS_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

/// The screen's data (the file at `base`, its three lists' first items): the cells copied to
/// NBG1's VRAM, the colours set, the map put on its plane.
pub fn rank_screen_load(cpu: &mut Cpu, base: u32) {
    let part = |cpu: &mut Cpu, k: u32| {
        let list = cpu.read32(base + 4 * k);
        cpu.read32(base.wrapping_add(list)).wrapping_add(base)
    };
    let cells = part(cpu, 0);
    nested!(cpu, 0x0605_edf8, screen::copy_words(cpu, cells, NBG1_CELLS, CELLS_BYTES));
    let colours = part(cpu, 1);
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, colours, COLOURS_AT, COLOURS));
    let map = part(cpu, 2);
    cpu.nested_with_stack(&PUT_LONG_CELLS_PROLOGUE, PUT_LONG_CELLS, &[MAP_CHAR_OFFSET, 0], |cpu| {
        screen::put_long_cells(cpu, map, NBG1_MAP, MAP_W, MAP_H, MAP_CHAR_OFFSET, 0)
    });
}

/// Plane A's zoom set (x, y), and its frame done.
pub fn plane_a_scale(cpu: &mut Cpu, x: u32, y: u32) {
    cpu.write32(bg::PLANE_A + bg::ZOOM_X, x);
    cpu.write32(bg::PLANE_A + bg::ZOOM_Y, y);
    nested!(cpu, 0x0605_f040, bg::plane_a_frame(cpu));
}

/// Plane A's centre set (x, y), and its frame done.
pub fn plane_a_place(cpu: &mut Cpu, x: u32, y: u32) {
    cpu.write32(bg::PLANE_A + bg::X, x);
    cpu.write32(bg::PLANE_A + bg::Y, y);
    nested!(cpu, 0x0605_f040, bg::plane_a_frame(cpu));
}

/// The text plane's cell for the place (`x`, `y`) (16.16 pixels from the screen's centre; a cell
/// is 8 pixels, a row 64 cells): its address.
pub fn map_address(_cpu: &mut Cpu, x: u32, y: u32) -> u32 {
    let column = ((x >> 16) as i16 as i32 + 0xb0) >> 3;
    let row = ((y >> 16) as i16 as i32 + 0x70) >> 3;
    ((column * 2 + row * 128) as u32 & 0x1fff) + TEXT_MAP
}

/// The text plane's map cleared (0x2000 bytes).
pub fn text_plane_clear(cpu: &mut Cpu) {
    for at in (0..0x2000).step_by(4) {
        cpu.write32(TEXT_MAP + at, 0);
    }
}

// ---- text ----------------------------------------------------------------------------------------

/// The text's buffers: the name (3 characters, 0), the rank (3), the sign (1), the score (8); the
/// character the name's text doubles (a dot's: the pair's second character follows it).
const NAME_TEXT: u32 = 0x0608_4a88;
const RANK_TEXT: u32 = 0x0608_4a80;
const SIGN_TEXT: u32 = 0x0608_4a90;
const SCORE_TEXT: u32 = 0x0608_4a94;
const DOT: u32 = 0x0608_49fd;
const DOT_AFTER: u32 = 0x0608_4a01;
/// A table of pointers to the digits' strings.
const DIGITS: u32 = 0x0600_9fb8;

/// The three characters at `name` copied to the text buffer at `buffer`, each dot with the pair's
/// second character after it; the text ended.
fn name_text_to(cpu: &mut Cpu, name: u32, buffer: u32) {
    let mut at = 0;
    for i in 0..3 {
        let c = cpu.read8(name + i);
        cpu.write8(buffer + at, c);
        if c == cpu.read8(DOT) {
            at += 1;
            let after = cpu.read8(DOT_AFTER);
            cpu.write8(buffer + at, after);
        }
        at += 1;
    }
    cpu.write8(buffer + at, 0);
}

/// The three characters at `name` copied as they are to the text buffer at `buffer`; the text
/// ended.
fn name_copy_to(cpu: &mut Cpu, name: u32, buffer: u32) {
    for i in 0..3 {
        let c = cpu.read8(name + i);
        cpu.write8(buffer + i, c);
    }
    cpu.write8(buffer + 3, 0);
}

/// The digits of `n` (on the divide unit) into the characters from `last` back to `first`, blanks
/// before the first digit.
fn digits_to(cpu: &mut Cpu, mut n: i32, first: u32, last: u32) {
    let mut at = last;
    loop {
        let c = if n == 0 {
            b' '
        } else {
            let digit = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, n as u32, 10));
            let string = cpu.read32(DIGITS.wrapping_add(digit << 2));
            n /= 10;
            cpu.read8(string)
        };
        cpu.write8(at, c);
        at = at.wrapping_sub(1);
        if at < first {
            break;
        }
    }
}

/// A rank's text (3 characters) at `buffer`: its number, one more than `n`, or TOP for 0.
fn rank_text_to(cpu: &mut Cpu, n: u32, buffer: u32) {
    if n == 0 {
        for (k, c) in b"TOP".iter().enumerate() {
            cpu.write8(buffer + k as u32, *c);
        }
        return;
    }
    digits_to(cpu, (n as i32).wrapping_add(1), buffer, buffer + 2);
}

/// A change's sign character at `buffer`: 0 is a `2`, 1 a `4`, -1 a minus, else none.
fn change_sign_to(cpu: &mut Cpu, v: u32, buffer: u32) {
    let c = match v as i8 {
        0 => b'2',
        1 => b'4',
        -1 => b'-',
        _ => return,
    };
    cpu.write8(buffer, c);
}

/// A score's text (8 characters) at `buffer`: its digits, blanks before them (a zero is all blanks).
fn score_text_to(cpu: &mut Cpu, score: u32, buffer: u32) {
    digits_to(cpu, score as i32, buffer, buffer + 7);
}

/// The result's text helpers (the buffers are the result's).
pub fn name_text(cpu: &mut Cpu, name: u32) {
    name_text_to(cpu, name, NAME_TEXT);
}

pub fn rank_text(cpu: &mut Cpu, n: u32) {
    rank_text_to(cpu, n, RANK_TEXT);
}

pub fn change_sign(cpu: &mut Cpu, v: u32) {
    change_sign_to(cpu, v, SIGN_TEXT);
}

pub fn score_text(cpu: &mut Cpu, score: u32) {
    score_text_to(cpu, score, SCORE_TEXT);
}

/// The name entry's copies: the name with its dots paired, the name as it is, the rank, the sign
/// and the score, in the name entry's buffers.
pub fn entry_name_text(cpu: &mut Cpu, name: u32) {
    name_text_to(cpu, name, E_NAME_TEXT);
}

pub fn entry_name_copy(cpu: &mut Cpu, name: u32) {
    name_copy_to(cpu, name, E_NAME_TEXT);
}

pub fn entry_change_sign(cpu: &mut Cpu, v: u32) {
    change_sign_to(cpu, v, E_SIGN_TEXT);
}

pub fn entry_rank_text(cpu: &mut Cpu, n: u32) {
    rank_text_to(cpu, n, E_RANK_TEXT);
}

pub fn entry_score_text(cpu: &mut Cpu, score: u32) {
    score_text_to(cpu, score, E_SCORE_TEXT);
}

// ---- the result --------------------------------------------------------------------------------

/// The result's actor (sprite flags +0x10, place +0x14-+0x1c, scales +0x20, +0x24): its step (the
/// part of a stage in +0x3c, the stage in +0x3d, a byte each; a word write sets the stage and
/// clears the part), the rows put so far (+0x40), a timer (+0x44), the list's speed (+0x48), its
/// limit (+0x4c: the zoom's step first, then where the scroll ends), and a byte for each cell of
/// the picture that is up (+0x50, 0xb00 of them, 44 by 64).
const FLAGS: u32 = 0x10;
const X: u32 = 0x14;
const Y: u32 = 0x18;
const Z: u32 = 0x1c;
const SCALE_X: u32 = 0x20;
const SCALE_Y: u32 = 0x24;
const PART: u32 = 0x3c;
const STAGE: u32 = 0x3d;
const ROWS: u32 = 0x40;
const TIMER: u32 = 0x44;
const SPEED: u32 = 0x48;
const LIMIT: u32 = 0x4c;
const CELLS: u32 = 0x50;
const CELL_COUNT: u32 = 0xb00;
/// The actor's mode byte, and the value that ends it.
const OBJ_MODE: u32 = 3;
const ENDING: u8 = 0x70;
/// The sprite flag that keeps a sprite from being drawn.
const NO_DRAW: u32 = 0x8000_0000;
/// The scores: 100 ranks of 8 bytes (three characters of name, a byte of change (+3), the score
/// (+4)), and the best of each of six stages' five areas (8 bytes each, a negative score is none).
const RANKS: u32 = 0x0603_2e50;
const AREA_SCORES: u32 = 0x0603_3180;
const STAGES: u32 = 6;
const AREAS: u32 = 5;
/// The texts the result puts up: its title, its heads, a space, a dash, "STAGE-", a blank row, the
/// stage list's head and the small word after the stage's place; the stage list's names (a table
/// of pointers, five to a stage).
const TITLE: u32 = 0x0608_3a98;
const HEADS: u32 = 0x0608_3aac;
const SPACE: u32 = 0x0608_3ad4;
const DASH: u32 = 0x0608_3ad8;
const STAGE_WORD: u32 = 0x0608_3adc;
const BLANK_ROW: u32 = 0x0608_3ae4;
const STAGE_HEAD: u32 = 0x0608_3afc;
const STAGE_MARK: u32 = 0x0608_3b14;
const AREA_NAMES: u32 = 0x0608_4a08;
/// The camera (camera.rs): its depth +0x1c.
const CAMERA: u32 = 0x0607_46e4;
const CAMERA_DEPTH: u32 = 0x1c;
/// The coin settings' free play bit's byte; the next mode's byte (1: the title).
const NEXT_TITLE: u8 = 1;

fn large(cpu: &mut Cpu, s: u32, x: u32, y: u32) {
    let at = nested!(cpu, 0x0608_48c4, map_address(cpu, x, y));
    nested!(cpu, 0x0606_1204, text::put_large_string(cpu, s, at, 0));
}

fn small(cpu: &mut Cpu, s: u32, x: u32, y: u32) {
    let at = nested!(cpu, 0x0608_48c4, map_address(cpu, x, y));
    nested!(cpu, 0x0606_11b8, text::put_string(cpu, s, at, 0));
}

/// A word write at the actor's step (what `mov.w` of a stage does): the stage `n`, its part 0.
fn set_stage(cpu: &mut Cpu, o: u32, n: u8) {
    cpu.write16(o + PART, n as u16);
}

/// The step's part set.
fn set_part(cpu: &mut Cpu, o: u32, part: u8) {
    cpu.write8(o + PART, part);
}

/// The result's actor each frame: a start (a credit's coin in, or start on free play) ends the
/// mode at once. Its stages: 0, the text plane cleared and the sprite hidden until the director's
/// mode runs; 1, the 100 ranks' list (rows put up as the list scrolls up, a title, the heads, a
/// picture of cells put up at random); 2, the same by stages (each stage's area bests); 3, the
/// mode done; 4, nothing. The plane A placed and zoomed by the actor's place and scales
/// through SGL's conversion; the actor ended by a button the director shows or by stage 4.
pub fn result_exec(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    let start = if !system::free_play(cpu) {
        nested!(cpu, 0x0604_0f44, system::coin_byte3(cpu, 0)) != 0 || nested!(cpu, 0x0604_0f44, system::coin_byte3(cpu, 1)) != 0
    } else {
        (pads::pressed(cpu, 0) | pads::pressed(cpu, 1)) & pads::START != 0
    };
    if start {
        cpu.write8(NEXT_MODE, NEXT_TITLE);
        nested!(cpu, 0x0604_0c64, director::mode_done(cpu));
        nested!(cpu, 0x0604_0c8c, director::mode_skipped(cpu));
    }
    match cpu.read8(o + STAGE) {
        0 => {
            if cpu.read8(o + PART) == 0 {
                nested!(cpu, 0x0608_4904, text_plane_clear(cpu));
                for field in [Z, Y, X] {
                    cpu.write32(o + field, 0);
                }
                let flags = cpu.read32(o + FLAGS) | NO_DRAW;
                cpu.write32(o + FLAGS, flags);
                set_part(cpu, o, 1);
            }
            if nested!(cpu, 0x0604_0cb4, director::director_flags(cpu)) & 1 != 0 {
                set_stage(cpu, o, 1);
            }
        }
        stage @ (1 | 2) => {
            result_part(cpu, o, stage);
            result_rows(cpu, o, stage);
        }
        3 => {
            nested!(cpu, 0x0604_0c64, director::mode_done(cpu));
            set_stage(cpu, o, 4);
        }
        _ => {}
    }
    result_place(cpu, o, f);
}

/// A part of stage 1 or 2: 0, the text plane cleared and the place zero; 1, the zoom set (2 for
/// the first stage, the part of the picture; the limit the zoom's step); 2 (stage 2 only), on;
/// 10, the title; 11, the zoom in; 12, a wait, then where the list scrolls to; 13, the list
/// scrolls up a pixel a frame to there; 14, a wait, then the picture's cells cleared; 15, the
/// picture's cells put up at random, 30 a frame; 16, the next stage.
fn result_part(cpu: &mut Cpu, o: u32, stage: u8) {
    match cpu.read8(o + PART) {
        0 => {
            nested!(cpu, 0x0608_4904, text_plane_clear(cpu));
            for field in [Z, Y, X] {
                cpu.write32(o + field, 0);
            }
            set_part(cpu, o, 1);
        }
        1 => {
            cpu.write32(o + ROWS, 0);
            cpu.write32(o + SCALE_Y, 0x20_0000);
            cpu.write32(o + SCALE_X, 0x20_0000);
            cpu.write32(o + LIMIT, 0x8000);
            set_part(cpu, o, if stage == 1 { 10 } else { 2 });
        }
        2 if stage == 2 => set_part(cpu, o, 10),
        10 => {
            if stage == 1 {
                large(cpu, TITLE, 0xff80_0000, 0xffa0_0000);
                small(cpu, HEADS, 0xff60_0000, 0xffb8_0000);
            } else {
                large(cpu, STAGE_HEAD, 0xff60_0000, 0xffa0_0000);
            }
            set_part(cpu, o, 11);
        }
        11 => {
            let step = cpu.read32(o + LIMIT).wrapping_add(0x100);
            cpu.write32(o + LIMIT, step);
            let from = cpu.read32(o + SCALE_X);
            let scale = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, 0x1_0000, step));
            cpu.write32(o + SCALE_X, scale);
            cpu.write32(o + SCALE_Y, scale);
            if cpu.read32(o + SCALE_X) == 0x1_0000 {
                cpu.write32(o + TIMER, 120);
                set_part(cpu, o, 12);
            }
        }
        12 => {
            let timer = cpu.read32(o + TIMER).wrapping_sub(1);
            cpu.write32(o + TIMER, timer);
            if timer as i32 <= 0 {
                cpu.write32(o + LIMIT, 0xb0_0000);
                if stage == 1 {
                    for _ in 0..101 {
                        let limit = cpu.read32(o + LIMIT).wrapping_sub(0x10_0000);
                        cpu.write32(o + LIMIT, limit);
                    }
                } else {
                    for s in 0..STAGES {
                        let mut count = 0;
                        for k in 0..AREAS {
                            if cpu.read32(AREA_SCORES + 0x28 * s + 8 * k + 4) as i32 >= 0 {
                                count += 1;
                                let limit = cpu.read32(o + LIMIT).wrapping_sub(0x10_0000);
                                cpu.write32(o + LIMIT, limit);
                            }
                        }
                        if count != 0 {
                            let limit = cpu.read32(o + LIMIT).wrapping_sub(0x8_0000);
                            cpu.write32(o + LIMIT, limit);
                        }
                    }
                }
                set_part(cpu, o, 13);
            }
        }
        13 => {
            cpu.write32(o + SPEED, 0xffff_0000);
            let y = cpu.read32(o + Y).wrapping_add(0xffff_0000);
            cpu.write32(o + Y, y);
            let limit = cpu.read32(o + LIMIT);
            if y as i32 <= limit as i32 {
                cpu.write32(o + Y, limit);
                cpu.write32(o + TIMER, 120);
                set_part(cpu, o, 14);
            }
        }
        14 => {
            let timer = cpu.read32(o + TIMER).wrapping_sub(1);
            cpu.write32(o + TIMER, timer);
            if timer as i32 <= 0 {
                for i in 0..CELL_COUNT {
                    cpu.write8(o + CELLS + i, 0);
                }
                cpu.write32(o + TIMER, 0);
                set_part(cpu, o, 15);
            }
        }
        15 => {
            for _ in 0..30 {
                let (cell, at) = loop {
                    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, CELL_COUNT)) as i32;
                    let r = r.wrapping_abs() as u32;
                    if cpu.read8(o + CELLS + r) == 0 {
                        break (r, o + CELLS + r);
                    }
                };
                let rem = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, cell, 44));
                let x = 0xff60_0000u32.wrapping_add(rem.wrapping_mul(0x8_0000));
                let quot = nested!(cpu, 0x0600_6a78, math::quotient(cpu, cell, 44));
                let y = 0xff90_0000u32.wrapping_add(quot.wrapping_mul(0x8_0000));
                small(cpu, SPACE, x, y);
                cpu.write8(at, 1);
                let count = cpu.read32(o + TIMER).wrapping_add(1);
                cpu.write32(o + TIMER, count);
                if count as i32 > 0xaff {
                    set_part(cpu, o, 16);
                    break;
                }
            }
        }
        16 => set_stage(cpu, o, stage + 1),
        _ => {}
    }
}

/// The rows put up as they come into view (parts 10 to 14), from the first not up while the
/// list's place lets them be seen: stage 1's, the ranks, each with its number, its name, its score
/// and its change; stage 2's, each stage's areas' bests with a name and a score (the first stage
/// by its areas' names, the others by the table's).
fn result_rows(cpu: &mut Cpu, o: u32, stage: u8) {
    if cpu.read8(o + PART).wrapping_sub(10) > 4 {
        return;
    }
    let n = cpu.read32(o + ROWS) as i32;
    if stage == 1 {
        if n > 99 {
            return;
        }
        let mut y = 0xffc0_0000u32.wrapping_add((n as u32) << 20);
        for rank in n as u32..100 {
            if cpu.read32(o + Y).wrapping_add(y) as i32 > 0x80_0000 {
                return;
            }
            nested!(cpu, 0x0608_3958, rank_text(cpu, rank));
            large(cpu, RANK_TEXT, 0xff60_0000, y);
            large(cpu, DASH, 0xff90_0000, y);
            let entry = RANKS + 8 * rank;
            nested!(cpu, 0x0608_3908, name_text(cpu, entry));
            large(cpu, NAME_TEXT, 0xffa0_0000, y);
            let score = cpu.read32(entry + 4);
            nested!(cpu, 0x0608_3a28, score_text(cpu, score));
            large(cpu, SCORE_TEXT, 0xffe0_0000, y);
            let below = y.wrapping_add(0x8_0000);
            small(cpu, STAGE_WORD, 0x68_0000, below);
            let change = cpu.read8(entry + 3) as i8 as i32 as u32;
            nested!(cpu, 0x0608_39e4, change_sign(cpu, change));
            small(cpu, SIGN_TEXT, 0x98_0000, below);
            y = y.wrapping_add(0x10_0000);
            let rows = cpu.read32(o + ROWS).wrapping_add(1);
            cpu.write32(o + ROWS, rows);
            large(cpu, BLANK_ROW, 0xff60_0000, y);
            if rank == 99 {
                y = y.wrapping_add(0x10_0000);
                large(cpu, BLANK_ROW, 0xff60_0000, y);
            }
        }
    } else {
        let mut y = 0xffc0_0000u32;
        for stage in 0..n.max(0) as u32 {
            let mut any = false;
            for area in 0..AREAS {
                if cpu.read32(AREA_SCORES + 0x28 * stage + 8 * area + 4) as i32 >= 0 {
                    y = y.wrapping_add(0x10_0000);
                    any = true;
                }
            }
            if any {
                y = y.wrapping_add(0x8_0000);
            }
        }
        if n > 5 {
            return;
        }
        for stage in n as u32..STAGES {
            if cpu.read32(o + Y).wrapping_add(y) as i32 > 0x70_0000 {
                return;
            }
            let mut any = false;
            for area in 0..AREAS {
                let entry = AREA_SCORES + 0x28 * stage + 8 * area;
                if cpu.read32(entry + 4) as i32 >= 0 {
                    let marked_at = if stage == 0 {
                        let string = cpu.read32(AREA_NAMES + 4 * area);
                        small(cpu, string, 0xff70_0000, y);
                        y.wrapping_add(0x8_0000)
                    } else {
                        let string = cpu.read32(AREA_NAMES + 4 * (stage * 5 + area));
                        large(cpu, string, 0xff98_0000, y);
                        y
                    };
                    any = true;
                    small(cpu, STAGE_MARK, 0xff70_0000, marked_at);
                    nested!(cpu, 0x0608_3908, name_text(cpu, entry));
                    large(cpu, NAME_TEXT, 0xffd0_0000, y);
                    let score = cpu.read32(entry + 4);
                    nested!(cpu, 0x0608_3a28, score_text(cpu, score));
                    large(cpu, SCORE_TEXT, 0x10_0000, y);
                    y = y.wrapping_add(0x10_0000);
                    large(cpu, BLANK_ROW, 0xff60_0000, y);
                }
            }
            if any {
                y = y.wrapping_add(0x8_0000);
            }
            large(cpu, BLANK_ROW, 0xff60_0000, y);
            let rows = cpu.read32(o + ROWS).wrapping_add(1);
            cpu.write32(o + ROWS, rows);
            if stage == 5 {
                y = y.wrapping_add(0x10_0000);
                large(cpu, BLANK_ROW, 0xff60_0000, y);
            }
        }
    }
}

/// The plane A put where the actor's place and scales say, through SGL's conversion (a place
/// with no depth is taken as it is, and the depth the camera's): the place and scales put on the
/// stack at `inp`, converted to `out` (both six and five words), the plane's centre and zoom set
/// from them.
fn place_plane(cpu: &mut Cpu, o: u32, inp: u32, out: u32) {
    let (x, y, z) = (cpu.read32(o + X), cpu.read32(o + Y), cpu.read32(o + Z));
    let (sx, sy) = (cpu.read32(o + SCALE_X), cpu.read32(o + SCALE_Y));
    for (k, v) in [x, y, z, sx, sy, z].into_iter().enumerate() {
        cpu.write32(inp + 4 * k as u32, v);
    }
    if z == 0 {
        cpu.write32(out, x);
        cpu.write32(out + 4, y);
        let camera = cpu.read32(CAMERA);
        let depth = cpu.read32(camera + CAMERA_DEPTH).wrapping_neg();
        cpu.write32(out + 8, depth);
        cpu.write32(out + 12, sx);
        cpu.write32(out + 16, sy);
    } else {
        let depth = nested!(cpu, 0x0606_750c, sgl::sl_convert(cpu, inp, out));
        cpu.write32(out + 8, depth);
        nested!(cpu, 0x0606_750c, sgl::sl_convert(cpu, inp + 12, out + 12));
    }
    let x = cpu.read32(out).wrapping_add(0xb0_0000);
    cpu.write32(out, x);
    let y = cpu.read32(out + 4).wrapping_add(0xff90_0000);
    cpu.write32(out + 4, y);
    nested!(cpu, 0x0608_48a4, plane_a_place(cpu, x, y.wrapping_neg()));
    let sx = cpu.read32(out + 12);
    let sx = nested!(cpu, 0x0600_6a8e, math::fixed_div(cpu, 0x1_0000, sx));
    cpu.write32(out + 12, sx);
    let sy = cpu.read32(out + 16);
    let sy = nested!(cpu, 0x0600_6a8e, math::fixed_div(cpu, 0x1_0000, sy));
    cpu.write32(out + 16, sy);
    nested!(cpu, 0x0608_4858, plane_a_scale(cpu, sx, sy));
}

/// The result's plane placed (place_plane), and the actor ended by a button the director shows or
/// by stage 4.
fn result_place(cpu: &mut Cpu, o: u32, f: u32) {
    place_plane(cpu, o, f, f + 64);
    let flags = nested!(cpu, 0x0604_0cb4, director::director_flags(cpu));
    if flags & 8 != 0 || cpu.read8(o + STAGE) == 4 {
        cpu.write8(o + OBJ_MODE, ENDING);
    }
}

// ---- the name entry ------------------------------------------------------------------------------

/// The name entry's actor (the result's fields above, and): the player who types (+0x3e: 0, 1 or
/// -1 for none), the rank the score takes (+0x3f; 100 for none), the name (+0x40, three
/// characters), the route (+0x43), the score rounded down to tens (+0x44), a byte for each area
/// (+0x48, five to a stage) that the score beat, how many that is (+0x66), the cursor's place in
/// the name (+0x67), the character picked (+0x68; on the row of BS, SPC and END, which of them),
/// whether the cursor is on that row (+0x69), the time left (+0x6a, a word, frames), a count
/// (+0x70), the cursor's speed (+0x74), the list's speed (+0x78), a limit (+0x7c), and the
/// cursor's corners (+0x80, 16 bytes).
mod ne {
    pub const PLAYER: u32 = 0x3e;
    pub const RANK: u32 = 0x3f;
    pub const NAME: u32 = 0x40;
    pub const ROUTE: u32 = 0x43;
    pub const SCORE: u32 = 0x44;
    pub const AREA_FLAGS: u32 = 0x48;
    pub const AREAS_BEATEN: u32 = 0x66;
    pub const CURSOR: u32 = 0x67;
    pub const PICK: u32 = 0x68;
    pub const ON_ROW: u32 = 0x69;
    pub const TIMEOUT: u32 = 0x6a;
    pub const COUNT: u32 = 0x70;
    pub const CURSOR_SPEED: u32 = 0x74;
    pub const LIST_SPEED: u32 = 0x78;
    pub const LIMIT: u32 = 0x7c;
    pub const CORNERS: u32 = 0x80;
    /// The sprite's colour word.
    pub const COLOUR: u32 = 0x28;
}

/// The players' play data (globals.rs): 0xc8 bytes each, the continues (+0xc4), the route (+0x3c),
/// the score (+0x40), and the scores by area (+0x48, five to a stage).
const PLAY_DATA: u32 = 0x0600_ebe0;
const PLAY_DATA_SIZE: u32 = 0xc8;
const PLAY_CONTINUES: u32 = 0xc4;
const PLAY_ROUTE: u32 = 0x3c;
const PLAY_SCORE: u32 = 0x40;
const PLAY_AREAS: u32 = 0x48;
/// The pads (pads.rs): each 8 bytes: held (+0), pressed (+4). The bits: right, left, down, up,
/// start, a, c, b.
const PAD_INF: u32 = 0x0602_ee10;
const RIGHT: u16 = 0x8000;
const LEFT: u16 = 0x4000;
const DOWN: u16 = 0x2000;
const UP: u16 = 0x1000;
const A_OR_C: u16 = 0x600;
const B: u16 = 0x100;
/// The characters the name is typed from (a table of 40 in the overlay's data); the blank (a
/// character the data keeps, as the dot's pair does).
const CHARS: u32 = 0x0608_49d8;
const BLANK: u32 = 0x0608_4a00;
/// The countdown's digits (two characters, in the overlay's data).
const COUNTDOWN: u32 = 0x0608_4a04;

/// The text of the name entry's screens (the overlay's data): the title, the congratulations,
/// the heads, "YOU GET TOP SCORE", "TOP SCORE", "TOP-", "STAGE-", "BS SPC END", "RANKING", "-",
/// the blank row, "YOU GET AREA TOP SCORE", "PRACTICE", the stages' lists (five), "NAME-", three
/// blanks, "TOP", the cells' two blanks.
const T_TITLE: u32 = 0x0608_0678;
const T_CONGRATS: u32 = 0x0608_0684;
const T_HEADS: u32 = 0x0608_0694;
const T_YOU_GET_TOP: u32 = 0x0608_06bc;
const T_TOP_SCORE: u32 = 0x0608_06d0;
const T_TOP_DASH: u32 = 0x0608_06dc;
const T_STAGE_DASH: u32 = 0x0608_06e4;
const T_KEYS: u32 = 0x0608_06ec;
const T_TWO_BLANKS: u32 = 0x0608_0704;
const T_RANKING: u32 = 0x0608_0708;
const T_DASH: u32 = 0x0608_0710;
const T_BLANK_ROW: u32 = 0x0608_0714;
const T_YOU_GET_AREA: u32 = 0x0608_072c;
const T_PRACTICE: u32 = 0x0608_0744;
const T_STAGE_LISTS: [u32; 5] = [0x0608_0750, 0x0608_0768, 0x0608_0780, 0x0608_0798, 0x0608_07b0];
const T_NAME_DASH: u32 = 0x0608_07c8;
const T_THREE_BLANKS: u32 = 0x0608_07d0;
const T_AREA_TOP_SCORE: u32 = 0x0608_07d4;
const T_TOP_WORD: u32 = 0x0608_07e4;
const T_SCORE_WORD: u32 = 0x0608_07e8;
/// A table of pointers to the stages' areas' names (five to a stage).
const AREA_TITLES: u32 = 0x0608_4940;
/// The texts the helpers fill (the overlay's buffers): the rank (3 characters, then a dash), the
/// name, the sign and the score (the name entry's copies of the result's).
const E_RANK_TEXT: u32 = 0x0608_49b8;
const E_NAME_TEXT: u32 = 0x0608_49c0;
const E_SIGN_TEXT: u32 = 0x0608_49c8;
const E_SCORE_TEXT: u32 = 0x0608_49cc;
/// The main image's code the name entry calls (not rewritten): the name finished into the table
/// of ranks, the name kept; the name put in an area's table.
const NAME_FINISH_1: u32 = 0x0600_6154;
const NAME_FINISH_2: u32 = 0x0600_627c;
const AREA_NAME_SET: u32 = 0x0600_61d8;
/// The area names' table's first entry's place, and the tables of the ranks (name, route and
/// score: 8 bytes each) and the areas (the same, 0x28 a stage).
const RANK_TABLE: u32 = 0x0603_2e50;

/// The sprite on the stack (sprites.rs's): its flags, its place (x, y, z), its colour (a word),
/// its corners' address (a polygon's).
const SP_FLAGS: u32 = 0;
const SP_X: u32 = 4;
const SP_Y: u32 = 8;
const SP_Z: u32 = 0xc;
const SP_SCALE_X: u32 = 0x10;
const SP_SCALE_Y: u32 = 0x14;
const SP_COLOUR: u32 = 0x18;
const SP_CORNERS: u32 = 0x24;
/// A polygon's flags: a quad (2), or one with its own corners (1).
const POLY_2: u32 = 2;
const POLY_1: u32 = 1;

fn large_attr(cpu: &mut Cpu, s: u32, x: u32, y: u32, attr: u32) {
    let at = nested!(cpu, 0x0608_48c4, map_address(cpu, x, y));
    nested!(cpu, 0x0606_1204, text::put_large_string(cpu, s, at, attr));
}

/// The pad a player has (a byte, signed, 0 or 1) held and pressed.
fn pad_of(cpu: &mut Cpu, o: u32) -> u32 {
    let p = cpu.read8(o + ne::PLAYER) as i8 as i32 as u32;
    PAD_INF.wrapping_add(p.wrapping_mul(8))
}

fn pressed(cpu: &mut Cpu, o: u32) -> u16 {
    let pad = pad_of(cpu, o);
    cpu.read16(pad + 4)
}

fn held(cpu: &mut Cpu, o: u32) -> u16 {
    let pad = pad_of(cpu, o);
    cpu.read16(pad)
}

/// The cursor's colour (a word): SGL's cosine of the loops turned `shift`, `amp` times, plus
/// `base` and the actor's own.
fn pulse(cpu: &mut Cpu, o: u32, shift: u32, amp: u32, base: u32) -> u16 {
    let angle = (cpu.read32(LOOPS) << shift) as i16 as i32 as u32;
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, angle));
    let m = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, amp, cos));
    (m as u16).wrapping_add(base as u16).wrapping_add(cpu.read16(o + ne::COLOUR))
}

/// The sprite at `s` (on the stack) made (sprite_init) and drawn at (`x`, `y`, `z`) by
/// polygon `flags` with the corners at `corners`.
fn stack_poly(cpu: &mut Cpu, s: u32, flags: u32) {
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, s));
    cpu.write32(s + SP_FLAGS, flags);
}

fn draw(cpu: &mut Cpu, s: u32) {
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, s));
}

fn set_corners(cpu: &mut Cpu, at: u32, corners: [u16; 8]) {
    for (k, c) in corners.into_iter().enumerate() {
        cpu.write16(at + 2 * k as u32, c);
    }
}

/// The name entry's actor each frame (the director's mode runs it; it ends when the screen's work
/// is done: the table saved at the end).
pub fn name_entry_exec(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    match cpu.read8(o + STAGE) {
        0 => name_first(cpu, o),
        1 => name_scores(cpu, o),
        2 => name_typing(cpu, o, f, &TOP),
        3 => name_top_list(cpu, o, f),
        4 => {
            if cpu.read8(o + ne::AREAS_BEATEN) == 0 {
                cpu.write16(o + PART, 8);
            } else {
                nested!(cpu, 0x0608_4904, text_plane_clear(cpu));
                cpu.write16(o + PART, 5);
            }
        }
        5 => name_typing(cpu, o, f, &AREA),
        6 => name_area_screen(cpu, o, f),
        7 => name_final(cpu, o, f),
        8 => {
            cpu.write8(GAME_MODE, mode::GAME_RESULT);
            nested!(cpu, 0x0608_45a8, result_set(cpu));
            cpu.write16(o + PART, 10);
        }
        9 => {
            nested!(cpu, 0x0604_0c64, director::mode_done(cpu));
            cpu.write16(o + PART, 10);
        }
        _ => {}
    }
    let flags = nested!(cpu, 0x0604_0cb4, director::director_flags(cpu));
    if flags & 8 != 0 || cpu.read8(o + STAGE) == 10 {
        nested!(cpu, 0x0600_65fc, backup::save_scores(cpu));
        cpu.write8(o + OBJ_MODE, ENDING);
    }
}

/// Stage 0: the text plane cleared, the sprite hidden and the plane A's zoom one, until the
/// director's mode runs; then the player who types the name: the one with no continues (player 1
/// if both have none: neither, if both have none and...), and none if a player used a continue
/// (the game's `none` is -1; stage 8, the result, then).
fn name_first(cpu: &mut Cpu, o: u32) {
    if cpu.read8(o + PART) == 0 {
        nested!(cpu, 0x0608_4904, text_plane_clear(cpu));
        let flags = cpu.read32(o + FLAGS) | NO_DRAW;
        cpu.write32(o + FLAGS, flags);
        nested!(cpu, 0x0608_4858, plane_a_scale(cpu, ONE, ONE));
        cpu.write8(o + ne::PICK, 0);
        cpu.write8(o + ne::CURSOR, 0);
        cpu.write8(o + ne::ON_ROW, 0);
        cpu.write8(o + PART, 1);
    }
    if nested!(cpu, 0x0604_0cb4, director::director_flags(cpu)) & 1 == 0 {
        return;
    }
    cpu.write16(o + PART, 1);
    let continues = |cpu: &mut Cpu, p: u32| cpu.read8(PLAY_DATA + PLAY_DATA_SIZE * p + PLAY_CONTINUES) as i8;
    let (c0, c1) = (continues(cpu, 0), continues(cpu, 1));
    let mut player = -1i8;
    if c0 == 0 {
        player = 0;
    }
    if c1 == 0 {
        player = 1;
    }
    if c0 == 0 && c1 == 0 {
        player = -1;
    }
    if c0 > 0 {
        player = -1;
    }
    if c1 > 0 {
        player = -1;
    }
    cpu.write8(o + ne::PLAYER, player as u8);
    if player == -1 {
        cpu.write16(o + PART, 8);
    }
}

/// Stage 1: the name made blank, which areas the score beat (the area's score a player's own,
/// better than the table's), the sprite's colour for the player, the route and the score (to tens)
/// kept, the rank the score takes in the table of 100 (stage 2 for the typing, or 4 where it takes
/// none).
fn name_scores(cpu: &mut Cpu, o: u32) {
    for k in 0..3 {
        cpu.write8(o + ne::NAME + k, b' ');
    }
    cpu.write8(o + ne::AREAS_BEATEN, 0);
    let player = cpu.read8(o + ne::PLAYER) as i8 as i32 as u32;
    for stage in 0..STAGES {
        for area in 0..AREAS {
            let flag = o + ne::AREA_FLAGS + 5 * stage + area;
            cpu.write8(flag, 0);
            let table = cpu.read32(AREA_SCORES + 0x28 * stage + 8 * area + 4) as i32;
            if table >= 0 {
                let own_at = PLAY_DATA.wrapping_add(PLAY_DATA_SIZE.wrapping_mul(player)).wrapping_add(PLAY_AREAS + 4 * (5 * stage + area));
                let own = cpu.read32(own_at) as i32;
                if table < own {
                    cpu.write8(flag, 1);
                    let n = cpu.read8(o + ne::AREAS_BEATEN) + 1;
                    cpu.write8(o + ne::AREAS_BEATEN, n);
                }
            }
        }
    }
    let base = cpu.read16(0x0607_41fc);
    match player as i8 {
        0 => cpu.write16(o + ne::COLOUR, base.wrapping_add(0x20)),
        1 => cpu.write16(o + ne::COLOUR, base.wrapping_add(0x10)),
        _ => {}
    }
    cpu.write16(o + PART, 2);
    let at = PLAY_DATA.wrapping_add(PLAY_DATA_SIZE.wrapping_mul(player));
    let route = cpu.read8(at + PLAY_ROUTE);
    cpu.write8(o + ne::ROUTE, route);
    let score = cpu.read32(at + PLAY_SCORE) as i32;
    cpu.write32(o + ne::SCORE, (score / 10 * 10) as u32);
    cpu.write8(o + ne::RANK, 0);
    let mine = cpu.read32(o + ne::SCORE) as i32;
    loop {
        let rank = cpu.read8(o + ne::RANK) as i8;
        let table = cpu.read32(RANK_TABLE + 4 + 8 * rank as i32 as u32) as i32;
        if table < mine {
            break;
        }
        cpu.write8(o + ne::RANK, (rank + 1) as u8);
        if rank + 1 > 99 {
            break;
        }
    }
    if cpu.read8(o + ne::RANK) as i8 > 99 {
        cpu.write16(o + PART, 4);
    }
}

/// What the two typing stages (2: the name for a place in the table of ranks, 5: for the areas'
/// tables) differ in: the stack place of the cursor's sprite, where the underline goes, where the name shows, the stage after, and
/// whether the name is finished into the rank table first.
struct Typing {
    cursor: u32,
    underline: (u32, u32),
    name_at: (u32, u32),
    next: u8,
    area: bool,
}

const TOP: Typing = Typing { cursor: 0, underline: (0xffa0_0000, 0xfff0_0000), name_at: (0xffa0_0000, 0xfff0_0000), next: 3, area: false };
const AREA: Typing = Typing { cursor: 0xbc, underline: (0x10_0000, 0x20_0000), name_at: (0x10_0000, 0x20_0000), next: 6, area: true };
/// The corners the underline and the area marks share (on the stack).
const SHARED_CORNERS: u32 = 0x2c;

/// A typing stage's frame: the screen's text and the cursor put up first (part 0, then part 1);
/// the pad moves the cursor along the characters (left and right: a speed that builds, the
/// cursor following the character it is over), a or c puts the character in the name, b takes one
/// back, down puts the cursor on the row of BS, SPC and END (where left and right pick, a or c
/// acts and up goes back); then the cursor drawn (pulsing) with the underline at the place in the
/// name, the time counted down (the name is taken as it is when it runs out), the countdown and the
/// name shown; a name finished (END, or no time) goes on to the next stage.
fn name_typing(cpu: &mut Cpu, o: u32, f: u32, v: &Typing) {
    if cpu.read8(o + PART) == 0 {
        if v.area {
            area_screen_put(cpu, o);
        } else {
            top_screen_put(cpu, o);
        }
    }
    if cpu.read8(o + PART) == 1 {
        typing_input(cpu, o, v);
    }
    let a = f + v.cursor;
    stack_poly(cpu, a, 0);
    let flags = cpu.read32(a + SP_FLAGS) | POLY_2;
    cpu.write32(a + SP_FLAGS, flags);
    cpu.write32(a + SP_CORNERS, o + ne::CORNERS);
    let x = (cpu.read32(o + X) as i32).clamp(-0xa0_0000, 0x98_0000) as u32;
    cpu.write32(a + SP_X, x);
    let (y, z) = (cpu.read32(o + Y), cpu.read32(o + Z));
    cpu.write32(a + SP_Y, y);
    cpu.write32(a + SP_Z, z);
    let colour = pulse(cpu, o, 11, 7, 8);
    cpu.write16(a + SP_COLOUR, colour);
    let box_x = cpu.read16(o + ne::CORNERS + 4);
    cpu.write16(o + ne::CORNERS + 8, box_x);
    draw(cpu, a);
    let cursor = cpu.read8(o + ne::CURSOR) as i8;
    if cursor < 3 {
        cpu.write32(a + SP_FLAGS, POLY_1);
        let at = f + SHARED_CORNERS;
        cpu.write32(a + SP_CORNERS, at);
        set_corners(cpu, at, [0xffff, 8, 0x10, 8, 0x10, 0xe, 0xffff, 0xe]);
        cpu.write32(a + SP_X, (cursor as i32 as u32).wrapping_mul(0x10_0000).wrapping_add(v.underline.0));
        cpu.write32(a + SP_Y, v.underline.1);
        draw(cpu, a);
    }
    let left = (cpu.read16(o + ne::TIMEOUT) as i16).wrapping_sub(1);
    cpu.write16(o + ne::TIMEOUT, left as u16);
    if left < 1 {
        cpu.write8(o + ne::CURSOR, 3);
        name_finish(cpu, o, v);
        cpu.write8(o + PART, 2);
    }
    let seconds = nested!(cpu, 0x0600_6a78, math::quotient(cpu, left as i32 as u32, 60));
    digits_to(cpu, seconds as i32, COUNTDOWN, COUNTDOWN + 1);
    large(cpu, COUNTDOWN, 0x60_0000, 0x50_0000);
    nested!(cpu, 0x0608_04c4, name_text_to(cpu, o + ne::NAME, E_NAME_TEXT));
    large(cpu, E_NAME_TEXT, v.name_at.0, v.name_at.1);
    if v.area {
        area_marks(cpu, o, f + 0xe8, f + SHARED_CORNERS, f + 0x114);
    }
    if cpu.read8(o + PART) == 2 {
        cpu.write32(o + ne::COUNT, 0);
        for field in [Z, Y, X] {
            cpu.write32(o + field, 0);
        }
        cpu.write32(o + SCALE_X, ONE);
        cpu.write32(o + SCALE_Y, ONE);
        cpu.write8(COUNTDOWN, b' ');
        cpu.write8(COUNTDOWN + 1, b' ');
        large(cpu, COUNTDOWN, 0x60_0000, 0x50_0000);
        cpu.write16(o + PART, v.next as u16);
    }
}

/// The character at the cursor taken back (the cursor one place back, that place blank): when
/// there is one.
fn name_back(cpu: &mut Cpu, o: u32) {
    let cursor = cpu.read8(o + ne::CURSOR) as i8;
    if cursor > 0 {
        let cursor = cursor - 1;
        cpu.write8(o + ne::CURSOR, cursor as u8);
        let blank = cpu.read8(BLANK);
        cpu.write8(o + ne::NAME + cursor as i32 as u32, blank);
    }
}

/// The blank put at the cursor and the cursor on; past the name (3 characters) it goes to the row.
fn name_blank(cpu: &mut Cpu, o: u32) {
    let cursor = cpu.read8(o + ne::CURSOR);
    let blank = cpu.read8(BLANK);
    cpu.write8(o + ne::NAME + cursor as i8 as i32 as u32, blank);
    cpu.write8(o + ne::CURSOR, cursor.wrapping_add(1));
    if (cursor.wrapping_add(1) as i8) > 2 {
        cpu.write8(o + ne::PICK, 2);
        cpu.write8(o + ne::ON_ROW, 1);
    }
}

/// The name finished (END pressed, or the time out): the main image's code puts it in the table
/// (the area stage first puts it in the rank table's).
fn name_finish(cpu: &mut Cpu, o: u32, v: &Typing) {
    if v.area {
        cpu.call(NAME_FINISH_1, &[o + ne::NAME]);
    }
    cpu.call(NAME_FINISH_2, &[o + ne::NAME]);
}

/// The cursor's character (the picks along the 40 characters: x of the cursor 0x80000 each, from
/// -0xa00000) and the corner's pick from where the cursor is.
fn pick_at(cpu: &mut Cpu, o: u32) -> u8 {
    let t = (cpu.read32(o + X) as i32).wrapping_add(0xa0_0000);
    let t = if t < 0 { t.wrapping_add(0x7_ffff) } else { t };
    ((t >> 16) as i16 >> 3) as u8
}

fn typing_input(cpu: &mut Cpu, o: u32, v: &Typing) {
    if cpu.read8(o + ne::ON_ROW) == 0 {
        let held = held(cpu, o);
        if held & RIGHT == 0 {
            if held & LEFT == 0 {
                cpu.write32(o + ne::CURSOR_SPEED, 0);
                let pick = cpu.read8(o + ne::PICK) as i8 as i32;
                let to = (pick << 19).wrapping_sub((cpu.read32(o + X) as i32).wrapping_add(0xa0_0000));
                let to = (if to < 0 { to.wrapping_add(3) } else { to }) >> 2;
                let step = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, to as u32, 0x2_0000));
                let x = cpu.read32(o + X).wrapping_add(step);
                cpu.write32(o + X, x);
            } else {
                let from = cpu.read32(o + ne::CURSOR_SPEED);
                let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, 0x2_0000, 0x4000));
                cpu.write32(o + ne::CURSOR_SPEED, speed);
                let x = cpu.read32(o + X).wrapping_sub(speed);
                cpu.write32(o + X, x);
                let pick = pick_at(cpu, o);
                cpu.write8(o + ne::PICK, pick);
                if (cpu.read32(o + X) as i32) < -0xa0_0000 {
                    cpu.write32(o + X, 0xa0_0000);
                    cpu.write8(o + ne::PICK, 0x27);
                }
            }
        } else {
            let from = cpu.read32(o + ne::CURSOR_SPEED);
            let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, 0x2_0000, 0x4000));
            cpu.write32(o + ne::CURSOR_SPEED, speed);
            let x = cpu.read32(o + X).wrapping_add(speed);
            cpu.write32(o + X, x);
            let pick = pick_at(cpu, o).wrapping_add(1);
            cpu.write8(o + ne::PICK, pick);
            if cpu.read32(o + X) as i32 > 0x98_0000 {
                cpu.write32(o + X, 0xff50_0000);
                cpu.write8(o + ne::PICK, 0);
            }
        }
        let pick = cpu.read8(o + ne::PICK) as i8;
        if pick < 0 {
            cpu.write8(o + ne::PICK, 0);
        }
        if cpu.read8(o + ne::PICK) as i8 > 0x27 {
            cpu.write8(o + ne::PICK, 0x27);
        }
        let pr = pressed(cpu, o);
        if pr & A_OR_C != 0 {
            let pick = cpu.read8(o + ne::PICK) as i8 as i32 as u32;
            let cursor = cpu.read8(o + ne::CURSOR);
            let c = cpu.read8(CHARS.wrapping_add(pick));
            cpu.write8(o + ne::NAME + cursor as i8 as i32 as u32, c);
            cpu.write8(o + ne::CURSOR, cursor.wrapping_add(1));
            if (cursor.wrapping_add(1) as i8) > 2 {
                cpu.write8(o + ne::PICK, 2);
                cpu.write8(o + ne::ON_ROW, 1);
            }
        }
        let pr = pressed(cpu, o);
        if pr & B != 0 {
            name_back(cpu, o);
        }
        let pr = pressed(cpu, o);
        if pr & DOWN != 0 {
            cpu.write8(o + ne::PICK, 2);
            let x = cpu.read32(o + X) as i32;
            if x <= 0x1f_ffff {
                cpu.write8(o + ne::PICK, 1);
            }
            if x < -0x20_0000 {
                cpu.write8(o + ne::PICK, 0);
            }
            cpu.write8(o + ne::ON_ROW, 1);
        }
        cpu.write32(o + Y, 0x40_0000);
        return;
    }
    let pr = pressed(cpu, o);
    if pr & RIGHT != 0 {
        let old = cpu.read8(o + ne::PICK) as i8;
        let new = old.wrapping_add(1);
        cpu.write8(o + ne::PICK, new as u8);
        if new == 1 && cpu.read8(o + ne::CURSOR) as i8 > 2 {
            cpu.write8(o + ne::PICK, old.wrapping_add(2) as u8);
        }
        if cpu.read8(o + ne::PICK) as i8 > 2 {
            cpu.write8(o + ne::PICK, 0);
        }
    }
    let pr = pressed(cpu, o);
    if pr & LEFT != 0 {
        let old = cpu.read8(o + ne::PICK) as i8;
        let new = old.wrapping_sub(1);
        cpu.write8(o + ne::PICK, new as u8);
        if new == 1 && cpu.read8(o + ne::CURSOR) as i8 > 2 {
            cpu.write8(o + ne::PICK, old.wrapping_sub(2) as u8);
        }
        if (cpu.read8(o + ne::PICK) as i8) < 0 {
            cpu.write8(o + ne::PICK, 2);
        }
    }
    let (x, shape) = match cpu.read8(o + ne::PICK) as i8 {
        0 => {
            let pr = pressed(cpu, o);
            if pr & (A_OR_C | B) != 0 {
                name_back(cpu, o);
            }
            (0xffb0_0000, 0x10)
        }
        1 => {
            if pressed(cpu, o) & A_OR_C != 0 {
                name_blank(cpu, o);
            }
            if pressed(cpu, o) & B != 0 {
                name_back(cpu, o);
            }
            (0xfff0_0000, 0x18)
        }
        2 => {
            if pressed(cpu, o) & B != 0 {
                name_back(cpu, o);
            }
            if pressed(cpu, o) & A_OR_C != 0 {
                cpu.write8(o + ne::CURSOR, 3);
                name_finish(cpu, o, v);
                cpu.write8(o + PART, 2);
            }
            (0x38_0000, 0x18)
        }
        _ => (0, 0),
    };
    if shape != 0 {
        cpu.write32(o + X, x);
        cpu.write16(o + ne::CORNERS + 4, shape);
    }
    if cpu.read8(o + ne::CURSOR) as i8 <= 2 && pressed(cpu, o) & UP != 0 {
        let pick = pick_at(cpu, o);
        cpu.write8(o + ne::PICK, pick);
        cpu.write16(o + ne::CORNERS + 4, 8);
        cpu.write8(o + ne::ON_ROW, 0);
    }
    cpu.write32(o + Y, 0x48_0000);
}

/// The typing stage's cursor set at the start (a place, its corners (a box 10 by 10), the time
/// left (0x708 frames), the speed), and the stage's part 1.
fn cursor_start(cpu: &mut Cpu, o: u32, y: u32) {
    cpu.write32(o + X, 0xff60_0000);
    cpu.write32(o + Y, y);
    cpu.write32(o + Z, 0);
    for (at, v) in [(0x8c, 0xfffe), (0x80, 0xfffe), (0x84, 8), (0x86, 0xfffe), (0x82, 0xfffe), (0x8e, 8), (0x8a, 8)] {
        cpu.write16(o + at, v);
    }
    cpu.write16(o + ne::TIMEOUT, 0x708);
    cpu.write32(o + ne::CURSOR_SPEED, 0);
    cpu.write8(o + PART, 1);
}

/// The name entry's screen for a place in the table of ranks (stage 2, part 0): its title, the
/// congratulations, the heads, a top score's praise or the table's first place and this score
/// and route, and the characters to pick from.
fn top_screen_put(cpu: &mut Cpu, o: u32) {
    large(cpu, T_TITLE, 0xffb8_0000, 0xffa0_0000);
    large(cpu, T_CONGRATS, 0xff88_0000, 0xffb8_0000);
    small(cpu, T_HEADS, 0xff60_0000, 0xffe0_0000);
    if cpu.read8(o + ne::RANK) == 0 {
        large(cpu, T_YOU_GET_TOP, 0xff78_0000, 0x10_0000);
    } else {
        small(cpu, T_TOP_SCORE, 0xff60_0000, 0x10_0000);
        small(cpu, T_TOP_DASH, 0xff60_0000, 0x20_0000);
        nested!(cpu, 0x0608_0514, name_copy_to(cpu, RANK_TABLE, E_NAME_TEXT));
        small(cpu, E_NAME_TEXT, 0xff80_0000, 0x20_0000);
        let score = cpu.read32(RANK_TABLE + 4);
        nested!(cpu, 0x0608_0608, score_text_to(cpu, score, E_SCORE_TEXT));
        small(cpu, E_SCORE_TEXT, 0xffb0_0000, 0x20_0000);
        small(cpu, T_STAGE_DASH, 0, 0x20_0000);
        let change = cpu.read8(RANK_TABLE + 3) as i8 as i32 as u32;
        nested!(cpu, 0x0608_0538, change_sign_to(cpu, change, E_SIGN_TEXT));
        small(cpu, E_SIGN_TEXT, 0x30_0000, 0x20_0000);
    }
    let rank = cpu.read8(o + ne::RANK) as i8 as i32 as u32;
    nested!(cpu, 0x0608_057c, rank_text_to(cpu, rank, E_RANK_TEXT));
    large(cpu, E_RANK_TEXT, 0xff60_0000, 0xfff0_0000);
    let score = cpu.read32(o + ne::SCORE);
    nested!(cpu, 0x0608_0608, score_text_to(cpu, score, E_SCORE_TEXT));
    large(cpu, E_SCORE_TEXT, 0xffe0_0000, 0xfff0_0000);
    small(cpu, T_STAGE_DASH, 0x68_0000, 0xfff8_0000);
    let route = cpu.read8(o + ne::ROUTE) as i8 as i32 as u32;
    nested!(cpu, 0x0608_0538, change_sign_to(cpu, route, E_SIGN_TEXT));
    small(cpu, E_SIGN_TEXT, 0x98_0000, 0xfff8_0000);
    small(cpu, CHARS, 0xff60_0000, 0x40_0000);
    small(cpu, T_KEYS, 0xffb0_0000, 0x48_0000);
    cursor_start(cpu, o, 0x40_0000);
}

/// The name entry's screen for an area's table (stage 5, part 0): the title, the praise, "PRACTICE"
/// where the BIOS's setting is 1, the stages' lists, the characters.
fn area_screen_put(cpu: &mut Cpu, o: u32) {
    large(cpu, T_TITLE, 0xffb8_0000, 0xffa0_0000);
    small(cpu, T_YOU_GET_AREA, 0xffb0_0000, 0xffb0_0000);
    if globals::bios_setting_is_1(cpu) {
        large(cpu, T_PRACTICE, 0xff60_0000, 0xffc0_0000);
    }
    for (k, list) in T_STAGE_LISTS.into_iter().enumerate() {
        large(cpu, list, 0xff60_0000, [0xffd0_0000u32, 0xffe0_0000, 0xfff0_0000, 0, 0x10_0000][k]);
    }
    large(cpu, T_NAME_DASH, 0xffc0_0000, 0x20_0000);
    small(cpu, CHARS, 0xff60_0000, 0x40_0000);
    small(cpu, T_KEYS, 0xffb0_0000, 0x48_0000);
    cursor_start(cpu, o, 0x50_0000);
}

/// The cursor box's stack place for stage 3's draw and the conversions' places (stage 3: the
/// sprite, the place in, the place out).
const TOP_LIST_SPRITE: u32 = 0x3c;
const TOP_LIST_PLACE: (u32, u32) = (0x7c, 0x68);

/// A sprite's corners as the stages 5 and 6 keep them (on the stack): a narrow box 32 by 16, and
/// a wide one 128 by 16.
const NARROW: [u16; 8] = [0, 0xffff, 0x20, 0xffff, 0x20, 0xf, 0, 0xf];
const WIDE: [u16; 8] = [0, 0, 0x80, 0, 0x80, 0xf, 0, 0xf];

/// The marks over the areas' names (stages 5 and 6): one sprite (at `s` on the stack) drawn at each
/// area the score beat, six rows of five, the first row's boxes wide; the areas with no score are
/// left a blank for the other rows (the stages' lists put up by stage 5's screen).
fn area_marks(cpu: &mut Cpu, o: u32, s: u32, narrow: u32, wide: u32) {
    stack_poly(cpu, s, POLY_1);
    set_corners(cpu, narrow, NARROW);
    set_corners(cpu, wide, WIDE);
    let colour = pulse(cpu, o, 12, 3, 0xc);
    cpu.write16(s + SP_COLOUR, colour);
    cpu.write32(s + SP_Z, 0);
    cpu.write32(s + SP_SCALE_Y, ONE);
    cpu.write32(s + SP_SCALE_X, ONE);
    cpu.write32(s + SP_Y, 0xffc0_0000);
    for stage in 0..STAGES {
        if stage == 0 {
            cpu.write32(s + SP_CORNERS, wide);
            cpu.write32(s + SP_X, 0xff60_0000);
        } else {
            cpu.write32(s + SP_CORNERS, narrow);
            cpu.write32(s + SP_X, 0xffc0_0000);
        }
        for area in 0..AREAS {
            if cpu.read32(AREA_SCORES + 0x28 * stage + 8 * area + 4) as i32 >= 0 {
                if cpu.read8(o + ne::AREA_FLAGS + 5 * stage + area) != 0 {
                    draw(cpu, s);
                }
            } else if stage != 0 {
                let (x, y) = (cpu.read32(s + SP_X).wrapping_sub(0x10_0000), cpu.read32(s + SP_Y));
                large(cpu, T_THREE_BLANKS, x, y);
            }
            let x = cpu.read32(s + SP_X).wrapping_add(0x30_0000);
            cpu.write32(s + SP_X, x);
        }
        let y = cpu.read32(s + SP_Y).wrapping_add(0x10_0000);
        cpu.write32(s + SP_Y, y);
    }
}

/// Stage 3 (the name now in the list of ranks): the name blinks 60 frames where it will stand, the
/// list scrolls up, the ten ranks around the new one are put up with a box over the new one, the
/// plane zooms in; the box pulses till 300 frames or start; then stage 7.
fn name_top_list(cpu: &mut Cpu, o: u32, f: u32) {
    match cpu.read8(o + PART) {
        0 => {
            let n = cpu.read32(o + ne::COUNT) as i32;
            let (x, y) = if n < 0x1e { (0xff60_0000u32.wrapping_add((n as u32) << 20), 0x40_0000) } else { (0xffb0_0000u32.wrapping_add(((n - 0x1e) as u32) << 20), 0x48_0000) };
            small(cpu, T_TWO_BLANKS, x, y);
            if n & 1 == 0 {
                large(cpu, E_NAME_TEXT, 0xffa0_0000, 0xfff0_0000);
            } else {
                large_attr(cpu, E_NAME_TEXT, 0xffa0_0000, 0xfff0_0000, 0x1000);
            }
            let n = n + 1;
            cpu.write32(o + ne::COUNT, n as u32);
            if n > 0x3b {
                cpu.write32(o + ne::LIST_SPEED, 0xfffc_0000);
                cpu.write8(o + PART, 1);
            }
        }
        1 => {
            let y = cpu.read32(o + Y).wrapping_add(cpu.read32(o + ne::LIST_SPEED));
            cpu.write32(o + Y, y);
            if y as i32 <= -0xc0_0000 {
                for (at, v) in [(0x80, 0xff90), (0x8c, 0xff90), (0x84, 0x60), (0x88, 0x60), (0x82, 0xffff), (0x86, 0xffff), (0x8a, 0x10), (0x8e, 0x10)] {
                    cpu.write16(o + at, v);
                }
                nested!(cpu, 0x0608_4904, text_plane_clear(cpu));
                cpu.write32(o + Y, 0);
                cpu.write32(o + Z, 0);
                cpu.write32(o + SCALE_Y, 0x20_0000);
                cpu.write32(o + SCALE_X, 0x20_0000);
                cpu.write32(o + ne::LIMIT, 0x8000);
                cpu.write8(o + PART, 2);
            }
        }
        2 => {
            large(cpu, T_RANKING, 0xffc8_0000, 0xffa0_0000);
            let rank = cpu.read8(o + ne::RANK) as i8 as i32;
            let mut base = rank - 5;
            if rank <= 5 {
                base = 0;
            }
            if rank > 95 {
                base = 90;
            }
            let mut y = 0xffb0_0000u32;
            for k in 0..10u32 {
                if base == cpu.read8(o + ne::RANK) as i8 as i32 {
                    cpu.write8(o + ne::CURSOR, k as u8);
                    if base == 0 {
                        cpu.write16(o + ne::CORNERS, 0xff60);
                        cpu.write16(o + ne::CORNERS + 0xc, 0xff60);
                    } else {
                        let mut digits = base + 1;
                        while digits > 0 {
                            for at in [ne::CORNERS, ne::CORNERS + 0xc] {
                                let v = cpu.read16(o + at).wrapping_sub(0x10);
                                cpu.write16(o + at, v);
                            }
                            digits /= 10;
                        }
                    }
                }
                nested!(cpu, 0x0608_057c, rank_text_to(cpu, base as u32, E_RANK_TEXT));
                large(cpu, E_RANK_TEXT, 0xff60_0000, y);
                large(cpu, T_DASH, 0xff90_0000, y);
                let entry = RANK_TABLE + 8 * base as u32;
                nested!(cpu, 0x0608_04c4, name_text_to(cpu, entry, E_NAME_TEXT));
                large(cpu, E_NAME_TEXT, 0xffa0_0000, y);
                let score = cpu.read32(entry + 4);
                nested!(cpu, 0x0608_0608, score_text_to(cpu, score, E_SCORE_TEXT));
                large(cpu, E_SCORE_TEXT, 0xffe0_0000, y);
                small(cpu, T_STAGE_DASH, 0x68_0000, y.wrapping_add(0x8_0000));
                let change = cpu.read8(entry + 3) as i8 as i32 as u32;
                nested!(cpu, 0x0608_0538, change_sign_to(cpu, change, E_SIGN_TEXT));
                small(cpu, E_SIGN_TEXT, 0x98_0000, y.wrapping_add(0x8_0000));
                y = y.wrapping_add(0x10_0000);
                large(cpu, T_BLANK_ROW, 0xff60_0000, y);
                base += 1;
            }
            cpu.write8(o + PART, 3);
        }
        3 => zoom_in(cpu, o, 4),
        4 => {
            let s = f + TOP_LIST_SPRITE;
            stack_poly(cpu, s, POLY_1);
            cpu.write32(s + SP_CORNERS, o + ne::CORNERS);
            cpu.write32(s + SP_X, 0);
            let y = (cpu.read8(o + ne::CURSOR) as i8 as i32 as u32).wrapping_mul(0x10_0000).wrapping_add(0xffb0_0000);
            cpu.write32(s + SP_Y, y);
            cpu.write32(s + SP_Z, 0);
            let colour = pulse(cpu, o, 12, 3, 0xc);
            cpu.write16(s + SP_COLOUR, colour);
            draw(cpu, s);
            let left = cpu.read32(o + ne::COUNT).wrapping_sub(1);
            cpu.write32(o + ne::COUNT, left);
            if (pads::pressed(cpu, 0) | pads::pressed(cpu, 1)) & pads::START != 0 {
                cpu.write32(o + ne::COUNT, 0);
            }
            if cpu.read32(o + ne::COUNT) as i32 <= 0 {
                cpu.write16(o + PART, 7);
            }
        }
        _ => {}
    }
    place_plane(cpu, o, f + TOP_LIST_PLACE.0, f + TOP_LIST_PLACE.1);
}

/// The zoom in a stage's part: the step grows by 0x100 a frame, the zoom comes to one by it; once
/// there, 300 frames' count, and part `next`.
fn zoom_in(cpu: &mut Cpu, o: u32, next: u8) {
    let step = cpu.read32(o + ne::LIMIT).wrapping_add(0x100);
    cpu.write32(o + ne::LIMIT, step);
    let from = cpu.read32(o + SCALE_X);
    let scale = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, ONE, step));
    cpu.write32(o + SCALE_X, scale);
    cpu.write32(o + SCALE_Y, scale);
    if cpu.read32(o + SCALE_X) == ONE {
        cpu.write32(o + ne::COUNT, 300);
        cpu.write8(o + PART, next);
    }
}

/// Stage 6 (the name for an area's table taken): the marks over the areas, the name blinking 60
/// frames, the list scrolling away, then stage 7.
fn name_area_screen(cpu: &mut Cpu, o: u32, f: u32) {
    match cpu.read8(o + PART) {
        1 => {
            let y = cpu.read32(o + Y).wrapping_add(cpu.read32(o + ne::LIST_SPEED));
            cpu.write32(o + Y, y);
            if y as i32 <= -0xc0_0000 {
                cpu.write8(o + PART, 10);
            }
        }
        0 => {
            area_marks(cpu, o, f + 0x124, f + 0x150, f + 0x160);
            let n = cpu.read32(o + ne::COUNT) as i32;
            let (x, y) = if n < 0x1e { (0xff60_0000u32.wrapping_add((n as u32) << 20), 0x40_0000) } else { (0xffb0_0000u32.wrapping_add(((n - 0x1e) as u32) << 20), 0x48_0000) };
            small(cpu, T_TWO_BLANKS, x, y);
            let attr = if n & 1 == 0 { 0 } else { 0x1000 };
            large_attr(cpu, E_NAME_TEXT, 0x10_0000, 0x20_0000, attr);
            let n = n + 1;
            cpu.write32(o + ne::COUNT, n as u32);
            if n > 0x3b {
                cpu.write32(o + ne::LIST_SPEED, 0xfffc_0000);
                cpu.write8(o + PART, 1);
            }
        }
        10 => cpu.write16(o + PART, 7),
        _ => {}
    }
    place_plane(cpu, o, f + 0x184, f + 0x170);
}

/// The final list's sprite and conversions' places (on the stack).
const FINAL_SPRITE: u32 = 0x218;
const FINAL_CORNERS: u32 = 0x244;
const FINAL_PLACE: (u32, u32) = (0x1d8, 0x1c4);

/// Whether either player pressed start this frame, as the final list takes it (a credit, or free
/// play, first).
fn final_start(cpu: &mut Cpu) -> bool {
    (pads::pressed(cpu, 0) | pads::pressed(cpu, 1)) & pads::START != 0
}

/// Stage 7, the areas' list with this score's: a start (with a credit) goes to part 15; the parts:
/// 0, the text plane cleared; 1, the zoom set; 2, on; 10, the name, the score and the heads put up;
/// 11, the zoom in; 12, 300 frames (or start), then where the list scrolls to; 13, it scrolls up;
/// 14, 300 frames (or start); 15, stage 9. While the list scrolls (parts 10 to 14) its rows are put
/// up as they come into view; the plane placed; and, from part 12 on, a box drawn over each area
/// the score beat.
fn name_final(cpu: &mut Cpu, o: u32, f: u32) {
    let credited = system::free_play(cpu)
        || nested!(cpu, 0x0604_0f44, system::coin_byte3(cpu, 0)) != 0
        || nested!(cpu, 0x0604_0f44, system::coin_byte3(cpu, 1)) != 0;
    if credited && final_start(cpu) {
        cpu.write8(o + PART, 15);
    }
    match cpu.read8(o + PART) {
        0 => {
            nested!(cpu, 0x0608_4904, text_plane_clear(cpu));
            cpu.write32(o + ne::COUNT, 0);
            for field in [Z, Y, X] {
                cpu.write32(o + field, 0);
            }
            cpu.write8(o + PART, 1);
        }
        1 => {
            cpu.write32(o + 0x6c, 0);
            cpu.write32(o + SCALE_Y, 0x20_0000);
            cpu.write32(o + SCALE_X, 0x20_0000);
            cpu.write32(o + ne::LIMIT, 0x8000);
            cpu.write8(o + PART, 2);
        }
        2 => cpu.write8(o + PART, 10),
        10 => {
            large(cpu, T_AREA_TOP_SCORE, 0xff98_0000, 0xffa0_0000);
            nested!(cpu, 0x0608_04c4, name_text_to(cpu, o + ne::NAME, E_NAME_TEXT));
            large_attr(cpu, E_NAME_TEXT, 0xff70_0000, 0xffb0_0000, 0x1000);
            large(cpu, T_TOP_WORD, 0x8_0000, 0xffb0_0000);
            small(cpu, T_SCORE_WORD, 0xffa8_0000, 0xffb8_0000);
            small(cpu, T_SCORE_WORD, 0x40_0000, 0xffb8_0000);
            cpu.write8(o + PART, 11);
        }
        11 => zoom_in(cpu, o, 12),
        12 => {
            let left = cpu.read32(o + ne::COUNT).wrapping_sub(1);
            cpu.write32(o + ne::COUNT, left);
            if final_start(cpu) {
                cpu.write32(o + ne::COUNT, 0);
            }
            if cpu.read32(o + ne::COUNT) as i32 <= 0 {
                cpu.write32(o + ne::LIMIT, 0xa8_0000);
                for stage in 0..STAGES {
                    let mut count = 0;
                    for area in 0..AREAS {
                        if cpu.read32(AREA_SCORES + 0x28 * stage + 8 * area + 4) as i32 >= 0 {
                            let limit = cpu.read32(o + ne::LIMIT).wrapping_add(0xffe0_0000);
                            cpu.write32(o + ne::LIMIT, limit);
                            count += 1;
                        }
                    }
                    if count != 0 {
                        let limit = cpu.read32(o + ne::LIMIT).wrapping_add(0xfff0_0000);
                        cpu.write32(o + ne::LIMIT, limit);
                    }
                }
                cpu.write8(o + PART, 13);
            }
        }
        13 => {
            cpu.write32(o + ne::LIST_SPEED, 0xffff_0000);
            let y = cpu.read32(o + Y).wrapping_add(0xffff_0000);
            cpu.write32(o + Y, y);
            let limit = cpu.read32(o + ne::LIMIT);
            if y as i32 <= limit as i32 {
                cpu.write32(o + Y, limit);
                cpu.write32(o + ne::COUNT, 300);
                cpu.write8(o + PART, 14);
            }
        }
        14 => {
            let left = cpu.read32(o + ne::COUNT).wrapping_sub(1);
            cpu.write32(o + ne::COUNT, left);
            if final_start(cpu) {
                cpu.write32(o + ne::COUNT, 0);
            }
            if cpu.read32(o + ne::COUNT) as i32 <= 0 {
                cpu.write8(o + PART, 15);
            }
        }
        15 => cpu.write16(o + PART, 9),
        _ => {}
    }
    final_rows(cpu, o);
    place_plane(cpu, o, f + FINAL_PLACE.0, f + FINAL_PLACE.1);
    if cpu.read8(o + PART) as i8 > 11 {
        final_marks(cpu, o, f);
    }
}

/// The final list's rows (parts 10 to 14): from the stage after the ones put up (the count at
/// +0x6c), each stage's areas with a score: this player's own score for the area (kept at +0x44),
/// the area's name marking a new best, the name, the score, and blank rows after.
fn final_rows(cpu: &mut Cpu, o: u32) {
    if cpu.read8(o + PART).wrapping_sub(10) > 4 {
        return;
    }
    let n = cpu.read32(o + 0x6c) as i32;
    let mut y = 0xffc0_0000u32;
    for stage in 0..n.max(0) as u32 {
        let mut count = 0;
        for area in 0..AREAS {
            if cpu.read32(AREA_SCORES + 0x28 * stage + 8 * area + 4) as i32 >= 0 {
                y = y.wrapping_add(0x20_0000);
                count += 1;
            }
        }
        if count != 0 {
            y = y.wrapping_add(0x10_0000);
        }
    }
    let mut stage = cpu.read32(o + 0x6c) as i32;
    if stage > 5 {
        return;
    }
    let player = cpu.read8(o + ne::PLAYER) as i8 as i32 as u32;
    loop {
        if cpu.read32(o + Y).wrapping_add(y) as i32 > 0x70_0000 {
            return;
        }
        let mut count = 0;
        for area in 0..AREAS {
            let entry = AREA_SCORES + 0x28 * stage as u32 + 8 * area;
            if cpu.read32(entry + 4) as i32 >= 0 {
                let own_at = PLAY_DATA.wrapping_add(PLAY_DATA_SIZE.wrapping_mul(player)).wrapping_add(PLAY_AREAS + 4 * (5 * stage as u32 + area));
                let own = cpu.read32(own_at);
                cpu.write32(o + ne::SCORE, own);
                let title = cpu.read32(AREA_TITLES + 4 * (5 * stage as u32 + area));
                if own != 0 {
                    small(cpu, title, 0xff60_0000, y.wrapping_add(0x8_0000));
                }
                small(cpu, title, 0x8_0000, y.wrapping_add(0x8_0000));
                y = y.wrapping_add(0x10_0000);
                cpu.call(AREA_NAME_SET, &[stage as i8 as i32 as u32, area as i8 as i32 as u32, o + ne::NAME]);
                nested!(cpu, 0x0608_0514, name_copy_to(cpu, entry, E_NAME_TEXT));
                small(cpu, E_NAME_TEXT, 0x8_0000, y.wrapping_add(0x8_0000));
                let score = cpu.read32(entry + 4);
                nested!(cpu, 0x0608_0608, score_text_to(cpu, score, E_SCORE_TEXT));
                large(cpu, E_SCORE_TEXT, 0x20_0000, y);
                let own = cpu.read32(o + ne::SCORE);
                if own != 0 {
                    nested!(cpu, 0x0608_0608, score_text_to(cpu, own, E_SCORE_TEXT));
                    large(cpu, E_SCORE_TEXT, 0xff60_0000, y);
                }
                y = y.wrapping_add(0x10_0000);
                large(cpu, T_BLANK_ROW, 0xff60_0000, y);
                large(cpu, T_BLANK_ROW, 0xff60_0000, y.wrapping_add(0x10_0000));
                large(cpu, T_BLANK_ROW, 0xff60_0000, y.wrapping_add(0x20_0000));
                count += 1;
            }
        }
        if count != 0 {
            y = y.wrapping_add(0x10_0000);
        }
        let rows = cpu.read32(o + 0x6c).wrapping_add(1);
        cpu.write32(o + 0x6c, rows);
        stage += 1;
        if stage > 5 {
            return;
        }
    }
}

/// A box drawn over each area the score beat (stage 7, from part 12): the sprite at `FINAL_SPRITE`
/// with its corners, its place at the list's.
fn final_marks(cpu: &mut Cpu, o: u32, f: u32) {
    let s = f + FINAL_SPRITE;
    stack_poly(cpu, s, POLY_1);
    let corners = f + FINAL_CORNERS;
    cpu.write32(s + SP_CORNERS, corners);
    set_corners(cpu, corners, [0xffff, 0xffff, 0x18, 0xffff, 0x18, 8, 0xffff, 8]);
    cpu.write32(s + SP_X, 0x8_0000);
    let y = cpu.read32(o + Y).wrapping_add(0xffd8_0000);
    cpu.write32(s + SP_Y, y);
    let z = cpu.read32(o + Z);
    cpu.write32(s + SP_Z, z);
    let colour = pulse(cpu, o, 12, 3, 0xc);
    cpu.write16(s + SP_COLOUR, colour);
    for stage in 0..STAGES {
        let mut count = 0;
        for area in 0..AREAS {
            if cpu.read32(AREA_SCORES + 0x28 * stage + 8 * area + 4) as i32 >= 0 {
                if cpu.read8(o + ne::AREA_FLAGS + 5 * stage + area) != 0 {
                    draw(cpu, s);
                }
                let y = cpu.read32(s + SP_Y).wrapping_add(0x20_0000);
                cpu.write32(s + SP_Y, y);
                count += 1;
            }
        }
        if count != 0 {
            let y = cpu.read32(s + SP_Y).wrapping_add(0x10_0000);
            cpu.write32(s + SP_Y, y);
        }
    }
}

