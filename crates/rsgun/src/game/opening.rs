//! OPENING.BIN's code, which the opening mode runs in the overlay window: the story told before
//! the attract mode's play. The mode's code loads four sound data files below the room's top and
//! starts the screen on the opening's scroll table: its set-up, then (once the mode runs) the
//! pictures, each loaded in turn into whichever of NBG0 and NBG1 is not showing and cross-faded
//! over it while the planes drift, and over them on NBG3 the story's text typed in line by line
//! (where the BIOS's setting is 1; elsewhere code not seen run). A player with coins in (or, on
//! free play, pressing start) skips it. The overlay carries SGL's NBG3 and sprite colour
//! calculation setters (sgl.rs).

use super::director;
use super::globals::{BIOS_SETTING, GAME_FLAGS, HELD};
use super::{bg, files, nested, objects, palette, pads, player, screen, sgl, sound, states, system, text, Cpu};

/// The play's room settings (states.rs's): the first, a buffer's address (the pictures' files
/// load there); the room's top.
const BUFFER: u32 = 0x0607_40e4;
const ROOM_TOP: u32 = 0x0607_4050;
/// The rate the next load takes (files.rs), and the game mode a screen's state goes to when it
/// ends (states.rs: back to the first), the stage's sound effects' table (sound.rs).
const NEXT_RATE: u32 = 0x0603_2e40;
const NEXT_MODE: u32 = 0x0600_ed70;
const STAGE_SE_TABLE: u32 = 0x0600_edd4;

/// The director's slots the mode uses (states.rs's): its step (the first slot's byte), the load
/// (the second, a local the C code writes back each time), the settings kept
/// (states::keep_mode_settings: the buffer at +0x6c, the room's top at +0x70, the place the next
/// file goes), the file's size (+0x80), and the mode's music (+0xa4).
const MODE_STEP: u32 = 0x64;
const LOAD: u32 = 0x68;
const KEPT: u32 = 0x6c;
const PLACE: u32 = 0x70;
const SIZE: u32 = 0x80;
const MUSIC: u32 = 0xa4;
/// The room the opening takes below the top.
const ROOM: u32 = 0x4_4040;
const SOUND_RATE: u32 = 0x2_0000;
/// The sound data files loaded, in turn.
const SOUND_FILES: [u32; 4] = [5, 4, 0x19, 0x18];
/// The opening's scroll table (screen.rs), and its stage sound effects' table.
const SCROLLS: u32 = 0x0608_6544;
const SOUND_EFFECTS: u32 = 0x0608_6564;

/// The opening mode's code (the director calls it each frame until it gives 1): the settings
/// kept and the room's top moved down (states::keep_mode_settings); the four sound data files loaded there in turn, each
/// copied to its sound area once in (the place past it, but after the last the room's top); the
/// screen started on the opening's scrolls; the settings given back; 1.
pub fn opening_mode(cpu: &mut Cpu, game: u32) -> u32 {
    let mut load = cpu.read32(game + LOAD);
    let mut done = 0;
    match cpu.read8(game + MODE_STEP) {
        0 => {
            states::keep_mode_settings(cpu, game, ROOM);
            cpu.write8(game + MODE_STEP, 1);
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
                let next = if step == 8 { cpu.read32(ROOM_TOP) } else { cpu.read32(game + PLACE).wrapping_add(cpu.read32(game + SIZE)) };
                cpu.write32(game + PLACE, next);
                if step == 4 {
                    cpu.write16(game + MUSIC, 1);
                }
                cpu.write8(game + MODE_STEP, step + 1);
            }
        }
        9 => {
            let (kept, place) = (cpu.read32(game + KEPT), cpu.read32(game + PLACE));
            nested!(cpu, 0x0605_e3e0, screen::screen_start(cpu, SCROLLS, kept, place));
            cpu.write8(game + MODE_STEP, 10);
        }
        10 => cpu.write8(game + MODE_STEP, 11),
        11 => {
            cpu.write32(STAGE_SE_TABLE, SOUND_EFFECTS);
            cpu.write8(NEXT_MODE, 0);
            states::give_back_mode_settings(cpu, game);
            cpu.write8(game + MODE_STEP, 12);
        }
        12 => done = 1,
        _ => {}
    }
    cpu.write32(game + LOAD, load);
    done
}

/// The screen's place for its pictures (the scroll start's second argument), and a count of the
/// opening's starts kept in its own data.
const SCREEN_ROOM: u32 = 0x0607_47a0;
const STARTS: u32 = 0x0608_12c0;
/// SGL's display bits the set-up takes (NBG2 shown, NBG0 and NBG1 hidden), the sprites' colour
/// calculation (its condition and number), and the screen's scroll marks (screen.rs's: a list of
/// scroll indices) set to the opening's.
const START_PLANES: u32 = 0x1b_0004;
const SPRITE_CC_CONDITION: u32 = 1;
const SPRITE_CC_NUMBER: u32 = 4;
const SCROLL_MARKS: u32 = 0x54;
const MARKS: u32 = 0x0608_12c4;

/// The opening's first scroll's start (screen.rs's scroll table, given the buffer and the place):
/// the stage data set from the buffer, the screen's planes and windows to their start.
pub fn opening_start(cpu: &mut Cpu, data: u32, room: u32) {
    cpu.write32(SCREEN_ROOM, room);
    let n = cpu.read32(STARTS).wrapping_add(1);
    cpu.write32(STARTS, n);
    nested!(cpu, 0x0605_e17c, states::stage_data_set(cpu, data));
    nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, START_PLANES));
    nested!(cpu, 0x0608_1190, sgl::sl_sprite_cc_condition(cpu, SPRITE_CC_CONDITION));
    nested!(cpu, 0x0608_11b0, sgl::sl_sprite_cc_number(cpu, SPRITE_CC_NUMBER));
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 0));
    nested!(cpu, 0x0606_990c, sgl::sl_zdsp_level(cpu, 1));
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 1));
    nested!(cpu, 0x0605_fde8, screen::request_clear_text_plane(cpu));
    let s = cpu.read32(screen::SCREEN);
    cpu.write32(s + SCROLL_MARKS, MARKS);
}

/// The NBG3 map's word for the cell at (`x`, `y`) (its page from 0x25e68000, 64 cells a row).
const TEXT_MAP: u32 = 0x25e6_8000;

pub fn text_cell(_cpu: &mut Cpu, x: u32, y: u32) -> u32 {
    ((2 * x + 0x80 * y) & 0x1fff) + TEXT_MAP
}

/// A line typed in (an object): its text (the cells of its characters, two rows: the top halves,
/// then the bottom), its width (cells a row), its place on NBG3's map, how far it has typed, what
/// it does (typing, then the cursor blinking) and the cursor's count of frames.
const MODE: u32 = 0x03;
const STARTING: u8 = 0;
const RUNNING: u8 = 1;
const TO_STOP: u8 = b'p';
const STOPPING: u8 = b's';
const PHASE: u32 = 0x10;
const WIDTH: u32 = 0x12;
const AT: u32 = 0x14;
const TYPED: u32 = 0x18;
const BLINKS: u32 = 0x1c;
const TEXT: u32 = 0x20;
const TYPING: u8 = 0;
const BLINKING: u8 = 1;
/// The pattern names the text's cells take (its characters after the font's, palette 7); the
/// cursor, and a space.
const TEXT_CHARS: u16 = 0xa8;
const TEXT_PALETTE: u16 = 0x7000;
const CURSOR: u32 = b'_' as u32;
const SPACE: u32 = b' ' as u32;
const ROW: u32 = 0x80;

fn text_name(cpu: &mut Cpu, text: u32, i: u32) -> u16 {
    cpu.read16(text.wrapping_add(2 * i)).wrapping_add(TEXT_CHARS) | TEXT_PALETTE
}

/// A line's frame (unless game frames are held): typing, a character (2x2 cells) a frame with
/// the cursor after it, until the width; then the cursor blinking (shown while its count's bit 2
/// is set) until the count has run out.
pub fn typer_exec(cpu: &mut Cpu, obj: u32) {
    if cpu.read32(GAME_FLAGS) & HELD != 0 {
        return;
    }
    match cpu.read8(obj + MODE) {
        RUNNING => {}
        STARTING => {
            cpu.write32(obj + TYPED, 0);
            cpu.write8(obj + PHASE, TYPING);
            cpu.write8(obj + MODE, RUNNING);
        }
        TO_STOP => {
            cpu.write8(obj + MODE, STOPPING);
            return;
        }
        STOPPING => {
            nested!(cpu, 0x0600_579c, objects::obj_free(cpu, obj));
            return;
        }
        _ => return,
    }
    match cpu.read8(obj + PHASE) {
        TYPING => {
            let (text, at, i, width) = (cpu.read32(obj + TEXT), cpu.read32(obj + AT), cpu.read32(obj + TYPED), cpu.read16(obj + WIDTH) as u32);
            for (place, k) in [(0, i), (2, i + 1), (ROW, i + width), (ROW + 2, i + width + 1)] {
                let name = text_name(cpu, text, k);
                cpu.write16(at + place, name);
            }
            cpu.write32(obj + AT, at + 4);
            cpu.write32(obj + TYPED, i + 2);
            nested!(cpu, 0x0606_111c, text::put_large_char(cpu, CURSOR, at + 4, 0));
            if cpu.read32(obj + TYPED) == cpu.read16(obj + WIDTH) as u32 {
                let p = cpu.read8(obj + PHASE);
                cpu.write8(obj + PHASE, p + 1);
            }
        }
        BLINKING => {
            let c = if cpu.read32(obj + BLINKS) & 4 != 0 { CURSOR } else { SPACE };
            let at = cpu.read32(obj + AT);
            nested!(cpu, 0x0606_111c, text::put_large_char(cpu, c, at, 0));
            let count = cpu.read32(obj + BLINKS);
            cpu.write32(obj + BLINKS, count.wrapping_sub(1));
            if count == 0 {
                cpu.write8(obj + MODE, TO_STOP);
            }
        }
        _ => {}
    }
}

/// A line made (list 0): its text, its place, its width, its cursor's count.
const TYPER_EXEC: u32 = 0x0608_083c;

pub fn typer_set(cpu: &mut Cpu, text: u32, at: u32, width: u32, blinks: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x24)) as i8 as i32 as u32;
    let obj = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 0, blocks, TYPER_EXEC));
    if obj != 0 {
        cpu.write32(obj + TEXT, text);
        cpu.write32(obj + AT, at);
        cpu.write32(obj + BLINKS, blinks);
        cpu.write16(obj + WIDTH, width as u16);
    }
}

/// The story (an object): its text's font cells and colours to VRAM (and a long at +0x10 cleared,
/// not read after); then each frame a frame on
/// (and NBG3 half a dot down), each of its lines (a table: the frame it starts at, its text, its
/// width; a line with no text ends it) made at its frame two rows below the frame's sixteenth,
/// and every sixteenth frame the row five below that cleared.
const UNUSED: u32 = 0x10;
const FRAME: u32 = 0x14;
const LINE: u32 = 0x18;
const LINES: u32 = 0x0608_1394;
const LINE_SIZE: u32 = 12;
const FONT: u32 = 0x0608_16c4;
const FONT_AT: u32 = 0x25e7_9500;
const FONT_BYTES: u32 = 0x45a0;
const FONT_COLOURS: u32 = 0x0608_5c64;
const FONT_COLOUR_INDEX: u32 = 0x370;
const BLANK_ROW: u32 = 0x0608_09f4;
const DRIFT: u32 = 0x8000;

pub fn story_exec(cpu: &mut Cpu, obj: u32) {
    match cpu.read8(obj + MODE) {
        STARTING => {
            nested!(cpu, 0x0605_edf8, screen::copy_words(cpu, FONT, FONT_AT, FONT_BYTES));
            nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, FONT_COLOURS, FONT_COLOUR_INDEX, 16));
            for at in [LINE, FRAME, UNUSED] {
                cpu.write32(obj + at, 0);
            }
            cpu.write8(obj + MODE, RUNNING);
        }
        RUNNING => {
            let line = LINES + LINE_SIZE * cpu.read32(obj + LINE);
            let mut frame = cpu.read32(obj + FRAME);
            if frame == cpu.read32(line) {
                let text = cpu.read32(line + 4);
                if text == 0 {
                    cpu.write8(obj + MODE, TO_STOP);
                    return;
                }
                let width = cpu.read16(line + 8) as u32;
                let at = nested!(cpu, 0x0608_03e0, text_cell(cpu, 2, ((frame as i32 >> 4) + 24) as u32));
                nested!(cpu, 0x0608_099c, typer_set(cpu, text, at, width, 0));
                let n = cpu.read32(obj + LINE);
                cpu.write32(obj + LINE, n + 1);
                frame = cpu.read32(obj + FRAME);
            }
            frame = frame.wrapping_add(1);
            cpu.write32(obj + FRAME, frame);
            if frame & 15 == 0 {
                let at = nested!(cpu, 0x0608_03e0, text_cell(cpu, 2, ((frame as i32 >> 4) + 29) as u32));
                nested!(cpu, 0x0606_11b8, text::put_string(cpu, BLANK_ROW, at, 0));
            }
            nested!(cpu, 0x0608_11c0, sgl::sl_scr_move_nbg3(cpu, 0, DRIFT));
        }
        TO_STOP => cpu.write8(obj + MODE, STOPPING),
        STOPPING => nested!(cpu, 0x0600_579c, objects::obj_free(cpu, obj)),
        _ => {}
    }
}

/// The story made (list 0).
const STORY_EXEC: u32 = 0x0608_0a20;

pub fn story_set(cpu: &mut Cpu) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x20)) as i8 as i32 as u32;
    nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 0, blocks, STORY_EXEC));
}

/// The pictures' work (the screen's, given to its scroll): its step (a byte), whether the last
/// picture is up, which plane shows (0: NBG0, the next goes to NBG1), the load, the next
/// picture, a count of frames (the cross-fade's), and how long the picture holds.
const STEP: u32 = 0x08;
const LAST: u32 = 0x09;
const SHOWING: u32 = 0x20;
const PICTURE_LOAD: u32 = 0x24;
const PICTURE: u32 = 0x28;
const COUNT: u32 = 0x2c;
const HOLD: u32 = 0x30;
/// The pictures (a table of 36 bytes each: frames held, the cells' size, the colours' count
/// (+10, a word), the map's width and height (+14, +18, words), the plane's place and speed
/// (+20-+32)), and their files' names (to a 0).
const PICTURES: u32 = 0x0608_14cc;
const PICTURE_SIZE: u32 = 36;
const PICTURE_FILES: u32 = 0x0608_1490;
const PICTURE_RATE: u32 = 0x800;
/// What the set-up and the cross-fades show (SGL's display bits).
const BOTH_PLANES: u32 = 0x3_000c;
const PLANES_ON: u32 = 0xf;
const NBG0_SHOWN: u32 = 0x2_000d;
const NBG1_SHOWN: u32 = 0x1_000e;
const FADE_IN: u32 = 0x400;
const FADE_FRAMES: u32 = 64;
const BLACK: u32 = -255i32 as u32;
/// Each plane's VRAM: its cells, its map, its colours' index, the pattern names' addition.
const PLANE_VRAM: [(u32, u32, u32, u32); 2] = [(0x25e4_0000, 0x25e6_0000, 0x100, 0x2000), (0x25e0_0000, 0x25e6_4000, 0x200, 0)];
const COPY_PIECE: u32 = 0x2000;

/// The pictures' set-up (the second scroll's start, given its work): NBG0, NBG1 and NBG3's
/// cells, maps and places, VRAM split, the screens shown and their priorities, colour offset on
/// the planes, black, both planes at the pictures' place, the work cleared; the story (or, where
/// the BIOS's setting is not 1, code not seen run); the scene on.
const PLANE_PLACE: (u32, u32) = (0xb0_0000, 0x70_0000);
const ONE: u32 = 0x1_0000;
const OTHER_TEXT: u32 = 0x0608_080c;

pub fn pictures_start(cpu: &mut Cpu, work: u32) {
    nested!(cpu, 0x0606_e288, sgl::sl_char_nbg0(cpu, 0x10, 0));
    nested!(cpu, 0x0606_e300, sgl::sl_page_nbg0(cpu, 0x25e4_0000, 0x100, 0x4000));
    nested!(cpu, 0x0607_0c4c, sgl::sl_map_nbg0(cpu, 0x25e6_0000, 0x25e6_0000, 0x25e6_0000, 0x25e6_0000));
    nested!(cpu, 0x0607_0cfc, sgl::sl_zoom_mode(cpu, 1, 0));
    nested!(cpu, 0x0606_e294, sgl::sl_char_nbg1(cpu, 0x10, 0));
    nested!(cpu, 0x0606_e2e4, sgl::sl_page_nbg1(cpu, 0x25e0_0000, 0x200, 0x4000));
    nested!(cpu, 0x0607_0d1c, sgl::sl_map_nbg1(cpu, 0x25e6_4000, 0x25e6_4000, 0x25e6_4000, 0x25e6_4000));
    nested!(cpu, 0x0606_e2b6, sgl::sl_char_nbg3(cpu, 0, 0));
    nested!(cpu, 0x0606_e2f4, sgl::sl_page_nbg3(cpu, 0x25e7_8000, 0x600, 0x8000));
    nested!(cpu, 0x0608_11e0, sgl::sl_plane_nbg3(cpu, 0));
    nested!(cpu, 0x0608_11f0, sgl::sl_map_nbg3(cpu, TEXT_MAP, TEXT_MAP, TEXT_MAP, TEXT_MAP));
    nested!(cpu, 0x0605_eee4, screen::fill_cells(cpu, 0, TEXT_MAP, 64, 64));
    nested!(cpu, 0x0608_12b0, sgl::sl_scr_pos_nbg3(cpu, 0, 0));
    nested!(cpu, 0x0606_dd4c, sgl::vram_split(cpu, 2));
    nested!(cpu, 0x0606_dd6c, sgl::sl_scr_auto_disp(cpu, PLANES_ON));
    nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, BOTH_PLANES));
    nested!(cpu, 0x0606_dd5c, sgl::sl_scr_transparent(cpu, 3));
    nested!(cpu, 0x0607_0ddc, sgl::sl_color_calc(cpu, 0));
    for (screen, pri) in [(1, 5), (0, 4), (2, 7)] {
        nested!(cpu, 0x0606_98dc, sgl::sl_priority(cpu, screen, pri));
    }
    nested!(cpu, 0x0606_e25c, sgl::sl_col_offset_on(cpu, 0x53, 0));
    nested!(cpu, 0x0605_e9d8, screen::stop_fades(cpu));
    nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, BLACK, BLACK, BLACK));
    plane_place(cpu, bg::PLANE_A);
    nested!(cpu, 0x0605_f040, bg::plane_a_frame(cpu));
    plane_place(cpu, bg::PLANE_B);
    nested!(cpu, 0x0605_f08c, bg::plane_b_frame(cpu));
    cpu.write8(work + STEP, 0);
    cpu.write32(work + PICTURE, 0);
    cpu.write32(work + SHOWING, 0);
    cpu.write8(work + LAST, 0);
    if (cpu.read32(BIOS_SETTING) >> 12) & 0xF == 1 {
        nested!(cpu, 0x0608_0b60, story_set(cpu));
    } else {
        cpu.call(OTHER_TEXT, &[]);
    }
    nested!(cpu, 0x0606_1738, player::scene_on(cpu));
}

fn plane_place(cpu: &mut Cpu, plane: u32) {
    cpu.write32(plane + bg::X, PLANE_PLACE.0);
    cpu.write32(plane + bg::Y, PLANE_PLACE.1);
    cpu.write32(plane + bg::SPEED_Y, 0);
    cpu.write32(plane + bg::SPEED_X, 0);
    cpu.write32(plane + bg::ZOOM_Y, ONE);
    cpu.write32(plane + bg::ZOOM_X, ONE);
}

/// The picture whose file is at `p` put on the plane that is not showing: its cells to that
/// plane's VRAM by long copies, its colours, its map (the pattern names past that plane's colours
/// and cells), sizes from the picture's entry.
pub fn picture_put(cpu: &mut Cpu, work: u32, p: u32) {
    let (cells_at, map_at, colours, add) = PLANE_VRAM[(cpu.read32(work + SHOWING) != 0) as usize];
    let entry = |cpu: &mut Cpu| PICTURES + PICTURE_SIZE * cpu.read32(work + PICTURE);
    let (cells, colour_data, map) = (screen::picture_part(cpu, p, 0), screen::picture_part(cpu, p, 1), screen::picture_part(cpu, p, 2));
    let e = entry(cpu);
    let size = cpu.read32(e + 4);
    nested!(cpu, 0x0605_ed2c, screen::queue_long_copy(cpu, cells, cells_at, size, COPY_PIECE));
    let e = entry(cpu);
    let count = cpu.read16(e + 10) as u32;
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, colour_data, colours, count));
    let e = entry(cpu);
    let (w, h) = (cpu.read16(e + 14) as u32, cpu.read16(e + 18) as u32);
    let char_offset = colours >> 4;
    cpu.nested_with_stack(&PUT_LONG_CELLS_PROLOGUE, PUT_LONG_CELLS, &[char_offset, add], |cpu| screen::put_long_cells(cpu, map, map_at, w, h, char_offset, add));
}

const PUT_LONG_CELLS: u32 = 0x0605_ee94;
static PUT_LONG_CELLS_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

/// The pictures' frame (the second scroll; skipped, the mode done and skipped first). Its
/// steps: 0, the next picture's file loading into the buffer; 1, the first put up and faded in;
/// 2, the next put up; 3, once its long copies are done; 4, the plane at the picture's place and
/// speed and its hold, then (unless it was the last) the next file; 5, the cross-fade to it over
/// 64 frames (the first picture waits 64 frames first); 6, its hold, then (after the last) the
/// next scroll. Then the planes moved and the ships kept still and hidden.
pub fn pictures_exec(cpu: &mut Cpu, work: u32) {
    let skip = if !system::free_play(cpu) {
        nested!(cpu, 0x0604_0f44, system::coin_byte3(cpu, 0)) != 0 || nested!(cpu, 0x0604_0f44, system::coin_byte3(cpu, 1)) != 0
    } else {
        (pads::pressed(cpu, 0) | pads::pressed(cpu, 1)) & pads::START != 0
    };
    if skip {
        nested!(cpu, 0x0604_0c64, director::mode_done(cpu));
        nested!(cpu, 0x0604_0c8c, director::mode_skipped(cpu));
    }
    let next_step = |cpu: &mut Cpu| {
        let s = cpu.read8(work + STEP);
        cpu.write8(work + STEP, s + 1);
    };
    match cpu.read8(work + STEP) {
        step @ (0 | 4) => {
            if step == 4 {
                let plane = if cpu.read32(work + SHOWING) == 0 { bg::PLANE_A } else { bg::PLANE_B };
                let e = PICTURES + PICTURE_SIZE * cpu.read32(work + PICTURE);
                for (k, field) in [bg::X, bg::Y, bg::SPEED_X, bg::SPEED_Y].into_iter().enumerate() {
                    let v = cpu.read32(e + 20 + 4 * k as u32);
                    cpu.write32(plane + field, v);
                }
                let hold = cpu.read32(e);
                cpu.write32(work + HOLD, hold);
                let n = cpu.read32(work + PICTURE) + 1;
                cpu.write32(work + PICTURE, n);
                let name = cpu.read32(PICTURE_FILES + 4 * n);
                if name == 0 {
                    cpu.write32(work + COUNT, 0);
                    cpu.write8(work + LAST, 1);
                    next_step(cpu);
                    return pictures_tail(cpu);
                }
            }
            let n = cpu.read32(work + PICTURE);
            let name = cpu.read32(PICTURE_FILES + 4 * n);
            let load = nested!(cpu, 0x0600_5e40, files::load_file(cpu, name));
            if load != 0 {
                cpu.write32(NEXT_RATE, PICTURE_RATE);
                let buffer = cpu.read32(BUFFER);
                nested!(cpu, 0x0600_603c, files::start_load(cpu, load, buffer));
                cpu.write32(work + PICTURE_LOAD, load);
                cpu.write32(work + COUNT, 0);
                next_step(cpu);
            }
        }
        1 => {
            let load = cpu.read32(work + PICTURE_LOAD);
            if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 {
                let buffer = cpu.read32(BUFFER);
                nested!(cpu, 0x0608_0d90, picture_put(cpu, work, buffer));
                nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, NBG0_SHOWN));
                nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, 0, 0, 0, FADE_IN));
                cpu.write8(work + STEP, 4);
            }
        }
        2 => {
            let load = cpu.read32(work + PICTURE_LOAD);
            if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 {
                let buffer = cpu.read32(BUFFER);
                nested!(cpu, 0x0608_0d90, picture_put(cpu, work, buffer));
                next_step(cpu);
            }
        }
        3 => {
            if nested!(cpu, 0x0605_ed84, screen::long_copies_on(cpu)) == 0 {
                next_step(cpu);
            }
        }
        5 => {
            let count = cpu.read32(work + COUNT);
            if cpu.read32(work + PICTURE) == 1 && count != FADE_FRAMES {
                cpu.write32(work + COUNT, count + 1);
            } else if count == FADE_FRAMES {
                let showing = cpu.read32(work + SHOWING);
                nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, if showing == 0 { NBG0_SHOWN } else { NBG1_SHOWN }));
                cpu.write32(work + COUNT, 0);
                let showing = cpu.read32(work + SHOWING) ^ 1;
                cpu.write32(work + SHOWING, showing);
                next_step(cpu);
            } else {
                if count == 0 {
                    // the new picture's plane under the old one, colour calculation by its rate
                    nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, PLANES_ON));
                    let (priorities, ratio) = if cpu.read32(work + SHOWING) == 0 { ([(1, 4), (0, 5)], 2) } else { ([(1, 5), (0, 4)], 1) };
                    for (screen, pri) in priorities {
                        nested!(cpu, 0x0606_98dc, sgl::sl_priority(cpu, screen, pri));
                    }
                    nested!(cpu, 0x0607_0ddc, sgl::sl_color_calc(cpu, ratio));
                }
                let showing = cpu.read32(work + SHOWING);
                let count = cpu.read32(work + COUNT);
                nested!(cpu, 0x0606_e45c, sgl::sl_col_rate(cpu, (showing != 0) as u32, (count as i32 >> 1) as u32 & 0xffff));
                let count = cpu.read32(work + COUNT) + 1;
                cpu.write32(work + COUNT, count);
            }
        }
        6 => {
            let hold = cpu.read32(work + HOLD).wrapping_sub(1);
            cpu.write32(work + HOLD, hold);
            if hold == 0 {
                if cpu.read8(work + LAST) != 0 {
                    nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
                    return;
                }
                cpu.write8(work + STEP, 2);
            }
        }
        _ => {}
    }
    pictures_tail(cpu);
}

fn pictures_tail(cpu: &mut Cpu) {
    nested!(cpu, 0x0605_f040, bg::plane_a_frame(cpu));
    nested!(cpu, 0x0605_f08c, bg::plane_b_frame(cpu));
    nested!(cpu, 0x0606_16b8, player::ships_still(cpu));
    nested!(cpu, 0x0606_1704, player::ships_hidden(cpu));
}

/// The third scroll's start: the first display flag (states.rs's DISPLAY_FLAGS) set, the mode
/// done.
const DISPLAY_FLAG: u32 = 0x0600_edcc;

pub fn opening_end(cpu: &mut Cpu) {
    cpu.write8(DISPLAY_FLAG, 1);
    nested!(cpu, 0x0604_0c64, director::mode_done(cpu));
}

/// The third scroll's frame: nothing.
pub fn opening_idle(_cpu: &mut Cpu) {}
