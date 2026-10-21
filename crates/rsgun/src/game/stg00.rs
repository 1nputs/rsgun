//! STG00.BIN's code, which the first stage mode runs in the overlay window: the stage before
//! the first battle (the ground's enemies by its timeline, an animated sprite, the rotation
//! scroll set up), then a count of twenty seconds with "PRESS START TO SKIP" (start held by a
//! ship that is in skips it). Its mode's code loads the stage's sound data and its file, then
//! starts its screen. The overlay carries SGL's rotation scroll set-up (sgl.rs).

use super::bullets::{S_ANGLE, S_NUMBER, S_POSITION};
use super::director;
use super::globals::GAME_FRAMES;
use super::{bg, bullets, dma, files, ground, math, nested, objects, palette, pads, player, screen, sgl, sound, sprites, states, text, timeline, Cpu};

/// The director's slots the mode uses (states.rs's, and as opening.rs has them): its step, the
/// load (a local written back each time), the buffer kept (+0x6c), the room's top (+0x70), a
/// long cleared at the end (+0x54), the mode's music (+0xa4).
const MODE_STEP: u32 = 0x64;
const LOAD: u32 = 0x68;
const KEPT: u32 = 0x6c;
const PLACE: u32 = 0x70;
const SLOT_54: u32 = 0x54;
const MUSIC: u32 = 0xa4;
const ROOM: u32 = 0x4_4040;
/// The room's top (states.rs's), the next load's rate (files.rs), the stage's file (a table of
/// mode files' names: its +24), the screen's room in the stage table (states::stage_table's 6).
const ROOM_TOP: u32 = 0x0607_4050;
const NEXT_RATE: u32 = 0x0603_2e40;
const STAGE_RATE: u32 = 0x2_0000;
const STAGE_FILE: u32 = 0x0607_35b0 + 24;
/// The sound data files loaded at once (sound::load_sound_file), the defaults' set-up after the
/// first, and the music set after the third.
const SOUND_FILES: [u32; 5] = [1, 5, 4, 0x19, 0x18];
/// The stage's scroll table, its timeline, its stage sound effects, and the mode after it.
const SCROLLS: u32 = 0x0608_1608;
const EVENTS: u32 = 0x0608_121c;
const SOUND_EFFECTS: u32 = 0x0608_1628;
const STAGE_SE_TABLE: u32 = 0x0600_edd4;
const NEXT_MODE: u32 = 0x0600_ed70;
const NEXT: u8 = 7;

/// The mode's code (the director calls it each frame until it gives 1): the settings kept, the
/// sound data loaded, the stage's file loaded into the buffer, the screen started once DMA is
/// idle (its room off the place), a few frames, the timeline made, the settings given back; 1.
pub fn stg00_mode(cpu: &mut Cpu, game: u32) -> u32 {
    let mut load = cpu.read32(game + LOAD);
    let mut done = 0;
    match cpu.read8(game + MODE_STEP) {
        0 => {
            states::keep_mode_settings(cpu, game, ROOM);
            cpu.write8(game + MODE_STEP, 1);
        }
        1 => {
            for (k, n) in SOUND_FILES.into_iter().enumerate() {
                let buffer = cpu.read32(game + KEPT);
                nested!(cpu, 0x0600_4b18, sound::load_sound_file(cpu, n, buffer));
                if k == 0 {
                    nested!(cpu, 0x0600_49cc, sound::sound_setup_default(cpu));
                }
                if k == 2 {
                    cpu.write16(game + MUSIC, 1);
                }
            }
            let top = cpu.read32(ROOM_TOP);
            cpu.write32(game + PLACE, top);
            cpu.write8(game + MODE_STEP, 9);
        }
        9 => {
            let name = cpu.read32(STAGE_FILE);
            load = nested!(cpu, 0x0600_5e40, files::load_file(cpu, name));
            if load != 0 {
                cpu.write32(NEXT_RATE, STAGE_RATE);
                let buffer = cpu.read32(game + KEPT);
                nested!(cpu, 0x0600_603c, files::start_load(cpu, load, buffer));
                cpu.write8(game + MODE_STEP, 10);
            }
        }
        10 => {
            if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 {
                cpu.write8(game + MODE_STEP, 11);
            }
        }
        11 => {
            if nested!(cpu, 0x0600_46b8, dma::dma_check(cpu)) == 0 {
                let room = nested!(cpu, 0x0605_fe78, states::stage_table(cpu, 6));
                let place = cpu.read32(game + PLACE).wrapping_sub(room);
                cpu.write32(game + PLACE, place);
                let buffer = cpu.read32(game + KEPT);
                nested!(cpu, 0x0605_e3e0, screen::screen_start(cpu, SCROLLS, buffer, place));
                cpu.write8(game + MODE_STEP, 12);
            }
        }
        step @ (12 | 13) => cpu.write8(game + MODE_STEP, step + 1),
        14 => {
            nested!(cpu, 0x0605_7354, timeline::timeline_set(cpu, EVENTS));
            cpu.write32(STAGE_SE_TABLE, SOUND_EFFECTS);
            cpu.write8(NEXT_MODE, NEXT);
            cpu.write32(game + SLOT_54, 0);
            states::give_back_mode_settings(cpu, game);
            cpu.write8(game + MODE_STEP, 15);
        }
        15 => done = 1,
        _ => {}
    }
    cpu.write32(game + LOAD, load);
    done
}

/// The animated sprite (a plain actor, actors.rs's: list 3, 0x48 bytes): its frame, cycling
/// 0x6c-0x6f each third game frame; its behaviour ends it ('p') once the director's flag bit 3
/// is set. 1.
const OBJ_MODE: u32 = 3;
const TO_STOP: u8 = b'p';
const POSITION: u32 = 0x14;
const COLOUR: u32 = 0x28;
const FRAME_FLAGS: u32 = 0x2e;
const SPRITE_FRAME: u32 = 0x2f;
const LIFT: u32 = 0x33;
const BEHAVIOUR: u32 = 0x38;
const FIRST_FRAME: u8 = 0x6c;
const LAST_FRAME: u8 = 0x6f;
const PLAIN_EXEC: u32 = 0x0604_dcb4;
const CHAR_BASE: u32 = 0x0607_41fc;
const SPRITE_COLOUR: u16 = 0xa0;
const SPRITE_PLACE: (u32, u32) = (0xfff3_0000, 0x4f_0000);
const SPRITE_LIFT: u8 = 0x7d;
const HANDED_OVER: u32 = 8;

pub fn cycling_behaviour(cpu: &mut Cpu, actor: u32) {
    if nested!(cpu, 0x0604_0cb4, director::director_flags(cpu)) & HANDED_OVER != 0 {
        cpu.write8(actor + OBJ_MODE, TO_STOP);
        return;
    }
    let frames = cpu.read32(GAME_FRAMES);
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, frames, 3)) == 0 {
        let frame = cpu.read8(actor + SPRITE_FRAME).wrapping_add(1);
        cpu.write8(actor + SPRITE_FRAME, frame);
        if frame > LAST_FRAME {
            cpu.write8(actor + SPRITE_FRAME, FIRST_FRAME);
        }
    }
}

const CYCLING_BEHAVIOUR: u32 = 0x0608_0244;

pub fn cycling_set(cpu: &mut Cpu) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x48)) as i8 as i32 as u32;
    let actor = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 3, blocks, PLAIN_EXEC));
    if actor != 0 {
        nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, actor + 0x10));
        cpu.write32(actor + BEHAVIOUR, CYCLING_BEHAVIOUR);
        cpu.write8(actor + SPRITE_FRAME, FIRST_FRAME);
        cpu.write8(actor + FRAME_FLAGS, 0);
        let colour = cpu.read16(CHAR_BASE).wrapping_add(SPRITE_COLOUR);
        cpu.write16(actor + COLOUR, colour);
        cpu.write32(actor + POSITION, SPRITE_PLACE.0);
        cpu.write32(actor + POSITION + 4, SPRITE_PLACE.1);
        cpu.write32(actor + POSITION + 8, 0);
        cpu.write8(actor + LIFT, SPRITE_LIFT);
    }
    1
}

/// A row of ground pieces (a timeline event, timeline.rs's: called each frame until it gives 1):
/// one made every `E_DELAY + 1` frames (by its kind: 0-2 ground_set's enemies, 10-13 platforms,
/// each with its script by `E_SCRIPT`), each `E_STEP_X`, `E_STEP_Y` and `E_TURN` on from the last,
/// until `E_COUNT` are made. With `E_HOOKS` code not seen run is called before and after each.
const E_KIND: u32 = 0x01;
const E_PLACE: u32 = 0x04;
const E_SCRIPT: u32 = 0x08;
const E_HOOKS: u32 = 0x09;
const E_SETTINGS: u32 = 0x0a;
const E_MIRROR: u32 = 0x0b;
const E_COUNT: u32 = 0x10;
const E_DELAY: u32 = 0x11;
const E_MADE: u32 = 0x12;
const E_WAIT: u32 = 0x13;
const E_DIRECTION: u32 = 0x14;
const E_TURN: u32 = 0x16;
const E_STEP_X: u32 = 0x18;
const E_STEP_Y: u32 = 0x19;
const E_TYPE: u32 = 0x1b;
const E_AIM: u32 = 0x1c;
const BEFORE_HOOK: u32 = 0x0608_0328;
const AFTER_HOOK: u32 = 0x0608_0424;
const GROUND_SCRIPTS: u32 = 0x0608_11cc;
const PLATFORM_SCRIPTS: u32 = 0x0608_1218;
/// A ground piece's description (ground.rs's, on the stack): its place (x, y, z), mirror (the
/// long +0x14's low byte), kind (+0x1a), type (+0x2c's word, the byte +0x2d), direction (+0x2e),
/// settings (+0x30's word, the byte +0x31); for the wounded, +0x32 and +0x34.
const D_PLACE: u32 = 0x00;
const D_MIRROR: u32 = 0x14;
const D_KIND: u32 = 0x1a;
const D_TYPE: u32 = 0x2c;
const D_DIRECTION: u32 = 0x2e;
const D_SETTINGS: u32 = 0x30;
const D_32: u32 = 0x32;
const D_34: u32 = 0x34;

/// The description's fields every maker here fills from the event: place, type, direction,
/// settings.
fn describe(cpu: &mut Cpu, event: u32, d: u32) {
    let x = (cpu.read16(event + E_PLACE) as i16 as i32 as u32) << 16;
    cpu.write32(d + D_PLACE, x);
    let y = (cpu.read16(event + E_PLACE + 2) as i16 as i32 as u32) << 16;
    cpu.write32(d + D_PLACE + 4, y);
    cpu.write32(d + D_PLACE + 8, 0);
    let t = cpu.read8(event + E_TYPE) as u16;
    cpu.write16(d + D_TYPE, t);
    let direction = cpu.read16(event + E_DIRECTION);
    cpu.write16(d + D_DIRECTION, direction);
    let settings = cpu.read8(event + E_SETTINGS) as u16;
    cpu.write16(d + D_SETTINGS, settings);
}

pub fn ground_row(cpu: &mut Cpu, event: u32) -> u32 {
    if cpu.read8(event + E_WAIT) == 0 {
        let d = cpu.stack();
        loop {
            if cpu.read8(event + E_HOOKS) != 0 {
                cpu.call(BEFORE_HOOK, &[event]);
            }
            let delay = cpu.read8(event + E_DELAY);
            cpu.write8(event + E_WAIT, delay);
            describe(cpu, event, d);
            let mirror = cpu.read8(event + E_MIRROR) as i8 as i32 as u32;
            cpu.write32(d + D_MIRROR, mirror);
            let kind = cpu.read8(event + E_KIND);
            let script = |cpu: &mut Cpu, table: u32| {
                let n = cpu.read8(event + E_SCRIPT) as u32;
                cpu.read32(table + 4 * n)
            };
            match kind {
                0..=2 => {
                    cpu.write16(d + D_KIND, kind as i8 as i16 as u16);
                    let s = script(cpu, GROUND_SCRIPTS);
                    nested!(cpu, 0x0605_a780, ground::ground_set(cpu, d, s));
                }
                10..=13 => {
                    cpu.write16(d + D_KIND, (kind as i8 as i16).wrapping_sub(10) as u16);
                    let s = script(cpu, PLATFORM_SCRIPTS);
                    nested!(cpu, 0x0605_c86c, ground::platform_set(cpu, d, s));
                }
                _ => {}
            }
            let made = cpu.read8(event + E_MADE).wrapping_add(1);
            cpu.write8(event + E_MADE, made);
            if made >= cpu.read8(event + E_COUNT) {
                return 1;
            }
            for (at, step) in [(E_PLACE, E_STEP_X), (E_PLACE + 2, E_STEP_Y)] {
                let v = cpu.read16(event + at).wrapping_add(cpu.read8(event + step) as i8 as i16 as u16);
                cpu.write16(event + at, v);
            }
            let direction = cpu.read16(event + E_DIRECTION).wrapping_add(cpu.read16(event + E_TURN));
            cpu.write16(event + E_DIRECTION, direction);
            if cpu.read8(event + E_HOOKS) != 0 {
                cpu.call(AFTER_HOOK, &[event]);
            }
            if cpu.read8(event + E_WAIT) != 0 {
                break;
            }
        }
    }
    let wait = cpu.read8(event + E_WAIT).wrapping_sub(1);
    cpu.write8(event + E_WAIT, wait);
    0
}

/// The wounded enemy's behaviour here (its object's +0x38, run by ground.rs's wounded exec):
/// while it stays put (at x 0, barely moving down) and has life above 2499, two plain shots each
/// sixteenth frame of the first 76 of each 300 (from either side of it, straight down), with
/// sound 25; then its own frame; its count on.
const SPEED_Y: u32 = 0x84;
const NUMBER: u32 = 0x32;
const LIFE: u32 = 0x50;
const COUNT: u32 = 0xa6;
const SHOT_SIDE: u32 = 0xa_0000;
const SHOT_DROP: u32 = 0xc_0000;
const SHOTS_DOWN: u16 = 0x8000;
const SHOT_SOUND: u32 = 25;

pub fn wounded_shooting(cpu: &mut Cpu, o: u32) {
    // (the SH-2 code's size: negated when negative, compared signed)
    let speed = cpu.read32(o + SPEED_Y) as i32;
    let size = if speed < 0 { speed.wrapping_neg() } else { speed };
    if cpu.read32(o + POSITION) == 0 && size <= 0x7fff {
        let count = cpu.read16(o + COUNT) as i16 as i32 as u32;
        let phase = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, 300)) as i32;
        let low = cpu.read16(o + COUNT) as i16 as i32 as u32 & 15;
        if phase <= 75 && low == 0 && cpu.read32(o + LIFE) as i32 > 0x9c3 {
            let shot = cpu.stack();
            let n = cpu.read8(o + NUMBER);
            cpu.write8(shot + S_NUMBER, n);
            let (x, y) = (cpu.read32(o + POSITION), cpu.read32(o + POSITION + 4));
            cpu.write32(shot + S_POSITION, x.wrapping_add(SHOT_SIDE));
            cpu.write32(shot + S_POSITION + 4, y.wrapping_add(SHOT_DROP));
            cpu.write32(shot + S_POSITION + 8, low);
            cpu.write16(shot + S_ANGLE, SHOTS_DOWN);
            nested!(cpu, 0x0605_3f48, bullets::bullet_set_plain(cpu, shot));
            let x = cpu.read32(o + POSITION).wrapping_sub(SHOT_SIDE);
            cpu.write32(shot + S_POSITION, x);
            nested!(cpu, 0x0605_3f48, bullets::bullet_set_plain(cpu, shot));
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SHOT_SOUND));
        }
    }
    nested!(cpu, 0x0605_b568, ground::wounded_exec(cpu, o));
    let count = cpu.read16(o + COUNT).wrapping_add(1);
    cpu.write16(o + COUNT, count);
}

/// The wounded enemy's event (a timeline event): it made (wounded_set, with its script and aim
/// by the event, its kind 2 off the centre line, mirror 1), numbered, with wounded_shooting as its
/// behaviour; made again once its number's slot is empty. Done (1) once the director's flag
/// bit 3 is set.
const STEP: u32 = 0x01;
const NUMBERED: u32 = 0x0607_40f0;
/// The event's +8 long holds the enemy's number once made (over the script's byte and those
/// after it).
const E_NUMBER: u32 = 0x08;
const WOUNDED_SCRIPTS: u32 = 0x0608_11e8;
const WOUNDED_AIMS: u32 = 0x0608_11f4;
const WOUNDED_SHOOTING: u32 = 0x0608_064c;

pub fn wounded_event(cpu: &mut Cpu, event: u32) -> u32 {
    if nested!(cpu, 0x0604_0cb4, director::director_flags(cpu)) & HANDED_OVER != 0 {
        return 1;
    }
    match cpu.read8(event + STEP) {
        0 => {
            let d = cpu.stack();
            describe(cpu, event, d);
            cpu.write32(d + D_MIRROR, 1);
            let off_centre = cpu.read32(d + D_PLACE) != 0;
            cpu.write16(d + D_KIND, if off_centre { 2 } else { 0 });
            cpu.write16(d + D_34, 0);
            cpu.write16(d + D_32, 1);
            let a = cpu.read8(event + E_AIM) as i8 as i32 as u32;
            let aim = cpu.read32(WOUNDED_AIMS.wrapping_add(4 * a));
            let s = cpu.read8(event + E_SCRIPT) as u32;
            let script = cpu.read32(WOUNDED_SCRIPTS + 4 * s);
            let o = nested!(cpu, 0x0605_bbac, ground::wounded_set(cpu, d, script, aim));
            if o != 0 {
                let step = cpu.read8(event + STEP).wrapping_add(1);
                cpu.write8(event + STEP, step);
                let n = cpu.read8(o + NUMBER) as i8 as i32 as u32;
                cpu.write32(event + E_NUMBER, n);
                cpu.write32(o + BEHAVIOUR, WOUNDED_SHOOTING);
            }
        }
        1 => {
            let n = cpu.read32(event + E_NUMBER);
            if cpu.read32(NUMBERED.wrapping_add(4 * n)) == 0 {
                cpu.write8(event + STEP, 0);
            }
        }
        _ => {}
    }
    0
}

/// The stage's first scroll's start (screen.rs's scroll table): colour offsets A and B black,
/// the stage data set; RBG0's rotation parameters and coefficient table at the end of VRAM's
/// first half, its cells, maps for both parameters, both parameters' coefficient tables (mode
/// 0x61); the back colour; NBG0 and NBG1's cells and maps; the screens shown and their
/// priorities, the sprites' type and colour calculation; NBG0's cells, maps and colours, NBG1's
/// (four maps, past its colours) from the stage data; the scroll marks.
const SCREEN_ROOM: u32 = 0x0607_47a0;
const STARTS: u32 = 0x0608_15fc;
const BLACK: u32 = -255i32 as u32;
const RPARA_TABLES: u32 = 0x25e3_fe00;
const KTABLE: u32 = 0x25e2_0000;
const KTABLE_MODE: u32 = 0x61;
const SCROLL_MARKS: u32 = 0x54;
const MARKS: u32 = 0x0608_1600;

pub fn stg00_start(cpu: &mut Cpu, data: u32, room: u32) {
    nested!(cpu, 0x0606_98ec, sgl::sl_col_offset_a(cpu, BLACK, BLACK, BLACK));
    nested!(cpu, 0x0606_98fc, sgl::sl_col_offset_b(cpu, BLACK, BLACK, BLACK));
    cpu.write32(SCREEN_ROOM, room);
    let n = cpu.read32(STARTS).wrapping_add(1);
    cpu.write32(STARTS, n);
    nested!(cpu, 0x0605_e17c, states::stage_data_set(cpu, data));
    nested!(cpu, 0x0608_0e94, sgl::sl_rpara_init_set(cpu, RPARA_TABLES));
    nested!(cpu, 0x0608_0f04, sgl::sl_make_ktable(cpu, KTABLE));
    nested!(cpu, 0x0606_e27c, sgl::sl_char_rbg0(cpu, 0, 0));
    nested!(cpu, 0x0606_e2ce, sgl::sl_page_rbg0(cpu, 0x25e0_0000, 0, 0xc000));
    nested!(cpu, 0x0608_0f84, sgl::sl_plane_ra(cpu, 0));
    nested!(cpu, 0x0607_1c4c, sgl::sl_map_rbg0(cpu, 0x25e4_0000));
    nested!(cpu, 0x0608_0f94, sgl::sl_over_ra(cpu, 0));
    nested!(cpu, 0x0608_0fa4, sgl::sl_ktable_ra(cpu, KTABLE, KTABLE_MODE));
    nested!(cpu, 0x0608_1024, sgl::sl_rpara_mode(cpu, 2));
    nested!(cpu, 0x0608_1034, sgl::sl_cur_rpara(cpu, 0));
    nested!(cpu, 0x0608_1044, sgl::sl_plane_rb(cpu, 0));
    nested!(cpu, 0x0608_1054, sgl::sl_map_rb(cpu, 0x25e4_2000));
    nested!(cpu, 0x0608_10f4, sgl::sl_over_rb(cpu, 0));
    nested!(cpu, 0x0608_1104, sgl::sl_ktable_rb(cpu, KTABLE, KTABLE_MODE));
    nested!(cpu, 0x0607_0dec, sgl::sl_back1_col_set(cpu, 0x25e3_fffe, 0));
    nested!(cpu, 0x0606_e288, sgl::sl_char_nbg0(cpu, 0, 0));
    nested!(cpu, 0x0606_e300, sgl::sl_page_nbg0(cpu, 0x25e6_0000, 0x200, 0x8000));
    nested!(cpu, 0x0607_0c3c, sgl::sl_plane_nbg0(cpu, 0));
    nested!(cpu, 0x0607_0c4c, sgl::sl_map_nbg0(cpu, 0x25e7_6000, 0x25e7_6000, 0x25e7_6000, 0x25e7_6000));
    nested!(cpu, 0x0607_0cfc, sgl::sl_zoom_mode(cpu, 1, 1));
    nested!(cpu, 0x0606_e294, sgl::sl_char_nbg1(cpu, 0, 0));
    nested!(cpu, 0x0606_e2e4, sgl::sl_page_nbg1(cpu, 0x25e6_0000, 0x400, 0x8000));
    nested!(cpu, 0x0607_0d0c, sgl::sl_plane_nbg1(cpu, 0));
    nested!(cpu, 0x0607_0d1c, sgl::sl_map_nbg1(cpu, 0x25e7_4000, 0x25e7_4000, 0x25e7_4000, 0x25e7_4000));
    nested!(cpu, 0x0607_0cfc, sgl::sl_zoom_mode(cpu, 0, 0));
    nested!(cpu, 0x0606_dd6c, sgl::sl_scr_auto_disp(cpu, 0x17));
    nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, 0x10_0007));
    for (screen, pri) in [(1, 6), (0, 4), (5, 3), (-7i32 as u32, 5), (-8i32 as u32, 4)] {
        nested!(cpu, 0x0606_98dc, sgl::sl_priority(cpu, screen, pri));
    }
    nested!(cpu, 0x0608_1164, sgl::sl_sprite_type(cpu, 5));
    nested!(cpu, 0x0607_0ddc, sgl::sl_color_calc(cpu, 1));
    nested!(cpu, 0x0606_e45c, sgl::sl_col_rate(cpu, 1, 12));
    nested!(cpu, 0x0608_1174, sgl::sl_sprite_cc_condition(cpu, 1));
    nested!(cpu, 0x0608_1194, sgl::sl_sprite_cc_number(cpu, 4));
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 0));
    nested!(cpu, 0x0606_990c, sgl::sl_zdsp_level(cpu, 7));
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 1));
    // NBG0's cells, its two maps (pushed arguments: none), its colours
    let cells = states::stage_part(cpu, states::CELLS, 0);
    nested!(cpu, 0x0605_edf8, screen::copy_words(cpu, cells, 0x25e6_0000, 0x1360));
    for (k, at) in [(0, 0x25e7_6000), (1, 0x25e7_6040)] {
        put_map(cpu, k, at, 0);
    }
    let colours = states::stage_part(cpu, states::COLOURS, 0);
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, colours, 0x100, 0x20));
    // NBG1's, its pattern names past NBG0's
    let cells = states::stage_part(cpu, states::CELLS, 1);
    nested!(cpu, 0x0605_edf8, screen::copy_words(cpu, cells, 0x25e6_1360, 0x1b60));
    for (k, at) in [(2, 0x25e7_4000), (3, 0x25e7_4040)] {
        put_map(cpu, k, at, NBG1_NAMES);
    }
    cpu.pop(32);
    for (k, at) in [(2, 0x25e7_5000), (3, 0x25e7_5040)] {
        put_map(cpu, k, at, NBG1_NAMES);
    }
    let colours = states::stage_part(cpu, states::COLOURS, 1);
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, colours, 0x200, 0xe0));
    let s = cpu.read32(screen::SCREEN);
    cpu.write32(s + SCROLL_MARKS, MARKS);
}

const NBG1_NAMES: u32 = 0x9b;
const PUT_CELLS: u32 = 0x0605_ee3c;

/// The stage data's map `k` put (32 by 32 cells) at `at` with `add` to its pattern names, its two
/// arguments past the fourth pushed as the SH-2 code pushes them (and leaves them).
fn put_map(cpu: &mut Cpu, k: u32, at: u32, add: u32) {
    let map = states::stage_part(cpu, states::MAPS, k);
    cpu.push(&[0, add]);
    nested!(cpu, PUT_CELLS, screen::put_cells(cpu, map, at, 32, 32, 0, add));
}

/// The second scroll's start (given its work): both planes at the screen's place, the count at
/// 1200 frames, the blink's count 0.
const PLANE_PLACE: (u32, u32) = (0xb0_0000, 0x70_0000);
const ONE: u32 = 0x1_0000;
const W_COUNT: u32 = 0x20;
const W_BLINK: u32 = 0x24;
const COUNT_FRAMES: u32 = 1200;

pub fn countdown_start(cpu: &mut Cpu, work: u32) {
    for (plane, frame) in [(bg::PLANE_A, true), (bg::PLANE_B, false)] {
        cpu.write32(plane + bg::X, PLANE_PLACE.0);
        cpu.write32(plane + bg::Y, PLANE_PLACE.1);
        cpu.write32(plane + bg::SPEED_Y, 0);
        cpu.write32(plane + bg::SPEED_X, 0);
        cpu.write32(plane + bg::ZOOM_Y, ONE);
        cpu.write32(plane + bg::ZOOM_X, ONE);
        if frame {
            nested!(cpu, 0x0605_f040, bg::plane_a_frame(cpu));
        } else {
            nested!(cpu, 0x0605_f08c, bg::plane_b_frame(cpu));
        }
    }
    cpu.write32(work + W_COUNT, COUNT_FRAMES);
    cpu.write32(work + W_BLINK, 0);
}

/// Plane B's frame, and its speed two dots a frame up.
const PLANE_B_SPEED: u32 = 0xfffe_0000;

pub fn plane_b_scroll(cpu: &mut Cpu) {
    nested!(cpu, 0x0605_f08c, bg::plane_b_frame(cpu));
    nested!(cpu, 0x0605_eff8, bg::plane_b_speed(cpu, 0, PLANE_B_SPEED));
}

/// The count's seconds left (rounded up) on the text plane: two large digits; one, centred,
/// below ten (the tens cleared as it reaches nine).
const TENS_AT: u32 = 0x25e7_e228;
const UNITS_AT: u32 = 0x25e7_e22c;
const UNIT_AT: u32 = 0x25e7_e22a;
const TWO_SPACES: u32 = 0x0608_0c70;

pub fn countdown_show(cpu: &mut Cpu, work: u32) {
    let seconds = (cpu.read32(work + W_COUNT) as i32).wrapping_add(60) / 60;
    let (tens, units) = (seconds / 10, seconds % 10);
    let digit = |d: i32| (d + b'0' as i32) as i8 as i32 as u32;
    if tens != 0 {
        nested!(cpu, 0x0606_111c, text::put_large_char(cpu, digit(tens), TENS_AT, 0));
        nested!(cpu, 0x0606_111c, text::put_large_char(cpu, digit(units), UNITS_AT, 0));
    } else {
        if units == 9 {
            nested!(cpu, 0x0606_1204, text::put_large_string(cpu, TWO_SPACES, TENS_AT, 0));
        }
        nested!(cpu, 0x0606_111c, text::put_large_char(cpu, digit(units), UNIT_AT, 0));
    }
}

/// "PRESS START TO SKIP", shown 60 frames of each 90 (spaces the rest).
const SKIP_AT: u32 = 0x25e7_ec9a;
const SKIP_TEXT: u32 = 0x0608_0d2c;
const NO_TEXT: u32 = 0x0608_0d40;

pub fn skip_text(cpu: &mut Cpu, work: u32) {
    let blink = cpu.read32(work + W_BLINK).wrapping_add(1);
    cpu.write32(work + W_BLINK, blink);
    if blink == 90 {
        cpu.write32(work + W_BLINK, 0);
    }
    let text = if cpu.read32(work + W_BLINK) as i32 > 59 { NO_TEXT } else { SKIP_TEXT };
    nested!(cpu, 0x0606_11b8, text::put_string(cpu, text, SKIP_AT, 0));
}

/// The second scroll's frame: a ship that is in (its state past 2) with start held skips the
/// mode and ends the count; plane B on; the count down and shown with the text; past it, the
/// next scroll.
const PLAYERS: [u32; 2] = [0x0607_40f0, 0x0607_40f4];
const SHIP_STATE: u32 = 0x6d;

pub fn countdown_exec(cpu: &mut Cpu, work: u32) {
    for (n, at) in PLAYERS.into_iter().enumerate() {
        let ship = cpu.read32(at);
        if ship != 0 && cpu.read8(ship + SHIP_STATE) as i8 > 2 && pads::held(cpu, n as u32) & pads::START != 0 {
            nested!(cpu, 0x0604_0c8c, director::mode_skipped(cpu));
            cpu.write32(work + W_COUNT, 0);
        }
    }
    nested!(cpu, 0x0608_0c48, plane_b_scroll(cpu));
    let count = cpu.read32(work + W_COUNT).wrapping_sub(1);
    cpu.write32(work + W_COUNT, count);
    nested!(cpu, 0x0608_0c74, countdown_show(cpu, work));
    nested!(cpu, 0x0608_0d54, skip_text(cpu, work));
    if (cpu.read32(work + W_COUNT) as i32) < 0 {
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
    }
}

/// The third scroll's start: the text plane cleared, the first display flag set, the mode done.
const DISPLAY_FLAG: u32 = 0x0600_edcc;

pub fn stg00_end(cpu: &mut Cpu) {
    nested!(cpu, 0x0605_fde8, screen::request_clear_text_plane(cpu));
    cpu.write8(DISPLAY_FLAG, 1);
    nested!(cpu, 0x0604_0c64, director::mode_done(cpu));
}

/// The third scroll's frame: plane B on, the ships still.
pub fn stg00_idle(cpu: &mut Cpu) {
    nested!(cpu, 0x0608_0c48, plane_b_scroll(cpu));
    nested!(cpu, 0x0606_16b8, player::ships_still(cpu));
}
