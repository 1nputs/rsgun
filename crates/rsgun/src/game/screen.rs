//! The screen: an object (screen_set) that keeps VDP2's colour offsets, the copies to video memory
//! queued for the VBlank, and the background's scroll (a function each mode), with a VBlank
//! callback that sends them; and the fades, objects of their own that move a colour (the offset
//! the screen adds, as a rule) toward a target. Three fade channels let one fade at a time touch
//! each colour.

use super::nested;
use super::globals::{GAME_FLAGS, HELD};
use super::{objects, vblank, Cpu};

/// The screen object (its fields as far as known below).
pub const SCREEN: u32 = 0x0607_4740;
const SCREEN_SIZE: u32 = 0xa0;
const SCREEN_EXEC: u32 = 0x0605_e49c;
/// Off (nothing is sent) while set.
const SCREEN_OFF: u32 = 0x50;
/// The colour offset fades move (r, g, b; added to both of VDP2's offsets).
const FADE_OFFSET: u32 = 0x5c;

/// A fade: its start colour, its target (r, g, b each), its phase (0 waiting for its channel, 1
/// fading), its channel, the colour it moves, its speed and its progress (16.16 of the way).
const FADE_SIZE: u32 = 0x30;
const FADE_EXEC: u32 = 0x0605_e73c;
const FROM: u32 = 0x10;
const TO: u32 = 0x16;
const PHASE: u32 = 0x1c;
const CHANNEL: u32 = 0x20;
const COLOUR: u32 = 0x24;
const SPEED: u32 = 0x28;
const PROGRESS: u32 = 0x2c;
const ONE: i32 = 0x10000;
/// Each channel taken by a fade (three).
const CHANNELS: u32 = 0x0607_3c78;
const CHANNEL_COUNT: u32 = 3;

/// The object mode (Base_obj's): running, to stop, stopping.
const MODE: u32 = 0x03;
const RUNNING: u8 = 1;
const TO_STOP: u8 = b'p';
const STOPPING: u8 = b's';

/// SGL's slMulFX (16.16).
const SL_MUL_FX: u32 = 0x0606_e26c;
/// The screen's set up: VDP2's scroll screens (not rewritten yet), and the VBlank callback.
const SCROLL_SETUP: u32 = 0x0605_ea18;
const SCREEN_VBLANK: u32 = 0x0605_ea98;
/// Cleared as the screen is set up: the VBlank's copy queue, its long copies, and another queue
/// (the counts).
const QUEUE_COUNT: u32 = 0x0607_3c90;
const LONG_COUNT: u32 = 0x0607_3c94;
const LONG_ON: u32 = 0x0607_3c98;
const SCROLL_COUNT: u32 = 0x0607_3c9c;

fn screen(cpu: &mut Cpu) -> u32 {
    cpu.read32(SCREEN)
}

/// The colour offset fades move, set at once.
pub fn set_fade_offset(cpu: &mut Cpu, r: u32, g: u32, b: u32) {
    let s = screen(cpu);
    cpu.write16(s + FADE_OFFSET, r as u16);
    cpu.write16(s + FADE_OFFSET + 2, g as u16);
    cpu.write16(s + FADE_OFFSET + 4, b as u16);
}

/// A fade toward (r, g, b) at `speed` (16.16 of the way a frame), not yet told what to move: it,
/// or 0.
pub fn fade_new(cpu: &mut Cpu, r: u32, g: u32, b: u32, speed: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, FADE_SIZE));
    let fade = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 0, blocks as i8 as i32 as u32, FADE_EXEC));
    if fade != 0 {
        cpu.write16(fade + TO, r as u16);
        cpu.write16(fade + TO + 2, g as u16);
        cpu.write16(fade + TO + 4, b as u16);
        cpu.write32(fade + SPEED, speed & 0xFFFF);
    }
    fade
}

/// Fades the screen's colour offset toward (r, g, b) at `speed`, on channel 0.
pub fn fade_to(cpu: &mut Cpu, r: u32, g: u32, b: u32, speed: u32) {
    let fade = nested!(cpu, 0x0605_e8dc, fade_new(cpu, r as i16 as i32 as u32, g as i16 as i32 as u32, b as i16 as i32 as u32, speed));
    if fade != 0 {
        let s = screen(cpu);
        cpu.write32(fade + COLOUR, s + FADE_OFFSET);
        cpu.write32(fade + CHANNEL, 0);
    }
}

/// Stops every fade (object list 0's), the channels freed.
pub fn stop_fades(cpu: &mut Cpu) {
    let mut obj = cpu.read32(objects::OBJ_INF);
    while obj != 0 {
        if cpu.read32(obj + objects::EXEC) == FADE_EXEC {
            cpu.write8(obj + MODE, TO_STOP);
        }
        obj = cpu.read32(obj + objects::NEXT);
    }
    for c in (0..CHANNEL_COUNT).rev() {
        cpu.write32(CHANNELS + 4 * c, 0);
    }
}

/// A fade's frame: once its channel is free it takes it and the colour's start; then it moves the
/// colour its speed's way each game frame, and at the target frees the channel and stops.
pub fn fade_exec(cpu: &mut Cpu, fade: u32) {
    if cpu.read32(GAME_FLAGS) & HELD != 0 {
        return;
    }
    match cpu.read8(fade + MODE) {
        0 => {
            cpu.write32(fade + PROGRESS, 0);
            cpu.write32(fade + PHASE, 0);
            cpu.write8(fade + MODE, RUNNING);
        }
        RUNNING => {}
        TO_STOP => {
            cpu.write8(fade + MODE, STOPPING);
            return;
        }
        STOPPING => {
            nested!(cpu, 0x0600_579c, objects::obj_free(cpu, fade));
            return;
        }
        _ => return,
    }
    let channel = CHANNELS + 4 * cpu.read32(fade + CHANNEL);
    match cpu.read32(fade + PHASE) {
        0 => {
            if cpu.read32(channel) == 0 {
                cpu.write32(fade + PHASE, 1);
                for i in 0..3 {
                    let colour = cpu.read32(fade + COLOUR);
                    let c = cpu.read16(colour + 2 * i);
                    cpu.write16(fade + FROM + 2 * i, c);
                }
                cpu.write32(channel, 1);
            }
        }
        1 => {
            let progress = cpu.read32(fade + PROGRESS).wrapping_add(cpu.read32(fade + SPEED));
            cpu.write32(fade + PROGRESS, progress);
            if (progress as i32) < ONE {
                let mut mixed = [0u32; 6];
                // each channel: start × (1 - progress) + target × progress
                for i in 0..3 {
                    let p = cpu.read32(fade + PROGRESS);
                    let from = (cpu.read16(fade + FROM + 2 * i) as i16 as i32) << 16;
                    let to = (cpu.read16(fade + TO + 2 * i) as i16 as i32) << 16;
                    mixed[2 * i as usize] = cpu.call(SL_MUL_FX, &[from as u32, (ONE as u32).wrapping_sub(p)]);
                    mixed[2 * i as usize + 1] = cpu.call(SL_MUL_FX, &[to as u32, p]);
                }
                for i in 0..3 {
                    let colour = cpu.read32(fade + COLOUR);
                    let c = mixed[2 * i].wrapping_add(mixed[2 * i + 1]) >> 16;
                    cpu.write16(colour + 2 * i as u32, c as u16);
                }
            } else {
                for i in 0..3 {
                    let colour = cpu.read32(fade + COLOUR);
                    let c = cpu.read16(fade + TO + 2 * i);
                    cpu.write16(colour + 2 * i, c);
                }
                cpu.write32(channel, 0);
                cpu.write8(fade + MODE, TO_STOP);
            }
        }
        _ => {}
    }
}

/// screen_set: the screen made (object list 0; off until told), its VBlank callback set, the
/// queues and the fade channels empty.
pub fn screen_set(cpu: &mut Cpu) {
    cpu.call(SCROLL_SETUP, &[]);
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SCREEN_SIZE));
    let s = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 0, blocks as i8 as i32 as u32, SCREEN_EXEC));
    cpu.write32(SCREEN, s);
    cpu.write8(s + SCREEN_OFF, 1);
    nested!(cpu, 0x0605_e2dc, screen_fields(cpu));
    nested!(cpu, 0x0605_e6c0, offsets_clear(cpu));
    for count in [QUEUE_COUNT, LONG_COUNT, SCROLL_COUNT, LONG_ON] {
        cpu.write32(count, 0);
    }
    nested!(cpu, 0x0600_43d4, vblank::add_callback(cpu, SCREEN_VBLANK));
    for c in (0..CHANNEL_COUNT).rev() {
        cpu.write32(CHANNELS + 4 * c, 0);
    }
}

/// The screen off, the mode's scroll stopped (not rewritten yet), every fade stopped.
pub fn screen_off(cpu: &mut Cpu) {
    let s = screen(cpu);
    cpu.write8(s + SCREEN_OFF, 1);
    cpu.call(0x0606_3294, &[]);
    nested!(cpu, 0x0605_e9d8, stop_fades(cpu));
}

/// The screen's hook while game frames are held (a function, or 0), the mode's scroll (its index
/// in the table of scroll functions), and VDP2's two colour offsets as the mode sets them (r, g,
/// b each; the fade offset is added to both).
const HELD_HOOK: u32 = 0x58;
const SCROLL_INDEX: u32 = 0x51;
const OFFSET_A: u32 = 0x62;
const OFFSET_B: u32 = 0x68;
/// The scroll functions' table (its address; 8 bytes a mode, the function at +4).
const SCROLL_TABLE: u32 = 0x0607_47cc;
/// A request for the next frame (set by code not rewritten yet), passed on as this frame's flag.
const REQUEST: u32 = 0x0607_3c84;
const REQUESTED: u32 = 0x0607_4804;
/// With the director's flag bit 3 (director.rs) the screen is handed to SCREEN_ALT instead (not
/// rewritten yet).
const SCREEN_ALT: u32 = 0x0605_e630;

/// SGL's: colour offsets A and B (r, g, b).
const SL_COLOR_OFFSET_A: u32 = 0x0606_98ec;
const SL_COLOR_OFFSET_B: u32 = 0x0606_98fc;

/// The screen's frame: the mode's scroll, the colour offsets to VDP2.
pub fn screen_exec(cpu: &mut Cpu, s: u32) {
    if cpu.read8(s + SCREEN_OFF) != 0 {
        return;
    }
    if nested!(cpu, 0x0604_0cb4, super::director::director_flags(cpu)) & 8 != 0 {
        cpu.call(SCREEN_ALT, &[]);
        return;
    }
    let work = s + 0x10;
    if cpu.read32(GAME_FLAGS) & HELD != 0 {
        let at = screen(cpu);
        let hook = cpu.read32(at + HELD_HOOK);
        if hook != 0 {
            cpu.call(hook, &[work]);
        }
        return;
    }
    let requested = cpu.read32(REQUEST) != 0;
    cpu.write32(REQUESTED, requested as u32);
    if requested {
        cpu.write32(REQUEST, 0);
    }
    nested!(cpu, 0x0605_fdc0, take_plane_shared(cpu));
    nested!(cpu, 0x0605_ef44, clear_scroll_word(cpu));
    let at = screen(cpu);
    let index = cpu.read8(at + SCROLL_INDEX) as i8 as i32;
    let table = cpu.read32(SCROLL_TABLE);
    let scroll = cpu.read32(table.wrapping_add((8 * index + 4) as u32));
    cpu.call(scroll, &[work]);
    for (offset, send) in [(OFFSET_A, SL_COLOR_OFFSET_A), (OFFSET_B, SL_COLOR_OFFSET_B)] {
        let at = screen(cpu);
        let mut rgb = [0u32; 3];
        for (i, c) in rgb.iter_mut().enumerate() {
            let i = 2 * i as u32;
            let v = cpu.read16(at + offset + i).wrapping_add(cpu.read16(at + FADE_OFFSET + i)) as i16 as i32;
            *c = v.clamp(-0xFF, 0xFF) as u32;
        }
        cpu.call(send, &rgb);
    }
}

/// The text plane's shared flag (text.rs's PLANE_SHARED) as this frame takes it, and as set for
/// the next (-1: the text's own).
const PLANE_SHARED: u32 = 0x0607_474c;
const PLANE_SHARED_NEXT: u32 = 0x0607_4748;
/// A request for the VBlank to clear the text plane.
const CLEAR_TEXT_PLANE: u32 = 0x0607_3c88;
/// The text plane (VDP2 VRAM): its rows (64 tiles each), and the part cleared (44 tiles by 28 rows).
const TEXT_PLANE: u32 = 0x25e7_e000;
const TEXT_ROW: u32 = 0x80;
const TEXT_ROWS: u32 = 28;
const TEXT_ROW_SHOWN: u32 = 0x54;
/// SGL's: VDP2's colour calculation settings (cleared with 0x73, 0).
const SL_COLOR_CALC: u32 = 0x0606_e25c;
/// A word cleared each frame before the scroll (not known yet).
const SCROLL_WORD: u32 = 0x0607_47c8;

/// The screen's fields to their start: no hook, the first scroll, the offsets zero, the text plane
/// the text's own.
pub fn screen_fields(cpu: &mut Cpu) {
    let s = screen(cpu);
    cpu.write32(CLEAR_TEXT_PLANE, 0);
    cpu.write32(PLANE_SHARED, OWN);
    cpu.write32(PLANE_SHARED_NEXT, OWN);
    cpu.write32(s + HELD_HOOK, 0);
    cpu.write8(s + SCROLL_INDEX, 0);
    cpu.write8(s + SCROLL_INDEX + 1, 0);
    for offset in [FADE_OFFSET, OFFSET_A, OFFSET_B] {
        for i in 0..3 {
            cpu.write16(s + offset + 2 * i, 0);
        }
    }
}
const OWN: u32 = u32::MAX;

/// VDP2's colour offsets back to none (SGL's colour calculation).
pub fn offsets_clear(cpu: &mut Cpu) {
    cpu.call(SL_COLOR_CALC, &[0x73, 0]);
}

/// The text plane's shared flag moved on a frame.
pub fn take_plane_shared(cpu: &mut Cpu) {
    let next = cpu.read32(PLANE_SHARED_NEXT);
    cpu.write32(PLANE_SHARED, next);
    cpu.write32(PLANE_SHARED_NEXT, OWN);
}

/// Asks the VBlank to clear the text plane.
pub fn request_clear_text_plane(cpu: &mut Cpu) {
    cpu.write32(CLEAR_TEXT_PLANE, 1);
}

/// Clears the text plane's shown part.
pub fn clear_text_plane(cpu: &mut Cpu) {
    for row in 0..TEXT_ROWS {
        let at = TEXT_PLANE + TEXT_ROW * row;
        for i in (0..TEXT_ROW_SHOWN + 1).step_by(4) {
            cpu.write32(at + i, 0);
        }
    }
}

/// Clears the word cleared each frame before the scroll.
pub fn clear_scroll_word(cpu: &mut Cpu) {
    cpu.write32(SCROLL_WORD, 0);
}

/// Copies `bytes` from `src` to `dst` by 32-bit words when both are aligned to them, else by
/// 16-bit words (video memory takes no bytes).
pub fn copy_words(cpu: &mut Cpu, src: u32, dst: u32, bytes: u32) {
    if src & 3 == 0 && dst & 3 == 0 {
        for i in 0..bytes >> 2 {
            let v = cpu.read32(src + 4 * i);
            cpu.write32(dst + 4 * i, v);
        }
    } else {
        for i in 0..bytes >> 1 {
            let v = cpu.read16(src + 2 * i);
            cpu.write16(dst + 2 * i, v);
        }
    }
}

/// Whether the VBlank's long copies are on.
pub fn long_copies_on(cpu: &mut Cpu) -> u32 {
    cpu.read32(LONG_ON)
}

/// Adds `delta` to each of `n` addresses at `list` (but -1, none).
pub fn offset_list(cpu: &mut Cpu, delta: u32, list: u32, n: u32) {
    for i in 0..(n as i32).max(0) as u32 {
        let v = cpu.read32(list + 4 * i);
        if v != u32::MAX {
            cpu.write32(list + 4 * i, v.wrapping_add(delta));
        }
    }
}

/// The VBlank's copies queued by the frame (16 bytes each: by slDMACopy (0) or slDMAXCopy (1),
/// source, destination, size).
const QUEUE: u32 = 0x0607_3ca0;
const QUEUE_ENTRY: u32 = 16;
/// The long copy under way (source, destination, size, a piece's size) and how far it is, then
/// the long copies queued behind it (the same 16 bytes each).
const LONG: u32 = 0x0607_3da0;
const LONG_DONE: u32 = 0x0607_3db0;
const LONG_QUEUE: u32 = 0x0607_3db8;
/// The scroll's work at the VBlank, queued by the frame (20 bytes each: two longs, three words, a
/// long; for SCROLL_WORK, not rewritten yet).
const SCROLL_QUEUE: u32 = 0x0607_3eb8;
const SCROLL_ENTRY: u32 = 20;
const SCROLL_WORK: u32 = 0x0605_ee3c;
/// Cleared at each VBlank (not known yet).
const VBLANK_CLEARED: u32 = 0x0607_48c0;
/// The game's flags (globals::GAME_FLAGS) bit 14, and a flag beside it (not known yet): the
/// background's VBlank work (SCROLL_VBLANK, not rewritten yet) is told the text plane's shared flag,
/// or 0.
const FLAG_14: u32 = 0x4000;
const SCROLL_FLAG: u32 = 0x0607_3c8c;
const SCROLL_VBLANK: u32 = 0x0605_fcdc;
/// SGL's copies.
const SL_DMA_COPY: u32 = 0x0603_3a10;
const SL_DMAX_COPY: u32 = 0x0603_3aa0;
const DMAX_MODE: u32 = 0x11;

/// The screen's VBlank callback: the text plane cleared if asked, the text lines hidden since
/// cleared; then, while the screen is on, the copies queued, a piece of the long copy, the
/// background's work, the scroll's work; the text lines sent.
pub fn screen_vblank(cpu: &mut Cpu) {
    cpu.write32(VBLANK_CLEARED, 0);
    if cpu.read32(CLEAR_TEXT_PLANE) != 0 {
        nested!(cpu, 0x0605_fdfc, clear_text_plane(cpu));
        cpu.write32(CLEAR_TEXT_PLANE, 0);
    }
    nested!(cpu, 0x0606_0824, super::text::clear_hidden_lines(cpu));
    let s = screen(cpu);
    if cpu.read8(s + SCREEN_OFF) == 0 {
        let mut i = 0;
        while (i as i32) < cpu.read32(QUEUE_COUNT) as i32 {
            let e = QUEUE + QUEUE_ENTRY * i;
            let (src, dst, size) = (cpu.read32(e + 4), cpu.read32(e + 8), cpu.read32(e + 12));
            match cpu.read32(e) {
                0 => {
                    cpu.call(SL_DMA_COPY, &[src, dst, size]);
                }
                1 => {
                    cpu.call(SL_DMAX_COPY, &[src, dst, size, DMAX_MODE]);
                }
                _ => {}
            }
            i += 1;
        }
        cpu.write32(QUEUE_COUNT, 0);
        long_copy_piece(cpu);
        let flags = cpu.read32(GAME_FLAGS);
        if flags & FLAG_14 == 0 {
            let shared = cpu.read32(PLANE_SHARED);
            cpu.call(SCROLL_VBLANK, &[shared]);
        } else if cpu.read32(SCROLL_FLAG) != 0 {
            cpu.call(SCROLL_VBLANK, &[0]);
        }
        let mut i = 0;
        while (i as i32) < cpu.read32(SCROLL_COUNT) as i32 {
            let e = SCROLL_QUEUE + SCROLL_ENTRY * i;
            let args = [cpu.read32(e), cpu.read32(e + 4), cpu.read16(e + 8) as u32, cpu.read16(e + 10) as u32];
            let stack = [cpu.read16(e + 12) as u32, cpu.read32(e + 16)];
            cpu.call_with_stack(SCROLL_WORK, &args, &stack);
            i += 1;
        }
        cpu.write32(SCROLL_COUNT, 0);
    }
    nested!(cpu, 0x0606_08bc, super::text::send_lines(cpu));
}

/// The long copy's next piece (the rest, when that is no more than a piece: then the next long
/// copy queued starts, or the copies end).
fn long_copy_piece(cpu: &mut Cpu) {
    let piece = cpu.read32(LONG + 12);
    if cpu.read32(LONG_ON) == 0 {
        return;
    }
    let (src, dst, size, done) = (cpu.read32(LONG), cpu.read32(LONG + 4), cpu.read32(LONG + 8), cpu.read32(LONG_DONE));
    if size < done.wrapping_add(piece) {
        cpu.call(SL_DMA_COPY, &[src.wrapping_add(done), dst.wrapping_add(done), size.wrapping_sub(done)]);
        if cpu.read32(LONG_COUNT) == 0 {
            cpu.write32(LONG_ON, 0);
        } else {
            for w in 0..4 {
                let v = cpu.read32(LONG_QUEUE + 4 * w);
                cpu.write32(LONG + 4 * w, v);
            }
            cpu.write32(LONG_DONE, 0);
            let left = cpu.read32(LONG_COUNT).wrapping_sub(1);
            cpu.write32(LONG_COUNT, left);
            for i in 0..(left as i32).max(0) as u32 {
                for w in 0..4 {
                    let v = cpu.read32(LONG_QUEUE + QUEUE_ENTRY * (i + 1) + 4 * w);
                    cpu.write32(LONG_QUEUE + QUEUE_ENTRY * i + 4 * w, v);
                }
            }
        }
    } else {
        cpu.call(SL_DMA_COPY, &[src.wrapping_add(done), dst.wrapping_add(done), piece]);
        cpu.write32(LONG_DONE, done.wrapping_add(piece));
    }
}
