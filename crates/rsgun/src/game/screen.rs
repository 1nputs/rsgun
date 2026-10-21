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
/// Cleared as the screen is set up: the VBlank's copy queue, its long copies, and its cell
/// writes (the counts).
const QUEUE_COUNT: u32 = 0x0607_3c90;
const LONG_COUNT: u32 = 0x0607_3c94;
const LONG_ON: u32 = 0x0607_3c98;
const CELLS_COUNT: u32 = 0x0607_3c9c;

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
    for count in [QUEUE_COUNT, LONG_COUNT, CELLS_COUNT, LONG_ON] {
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
    nested!(cpu, 0x0605_ef44, super::bg::map_hit_off(cpu));
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
/// The cell writes for the VBlank (put_cells), queued by the frame (20 bytes each: source,
/// destination, width, height and the character offset as words, the value added).
const CELLS_QUEUE: u32 = 0x0607_3eb8;
const CELLS_ENTRY: u32 = 20;
/// Each queue's room.
const QUEUE_ROOM: u32 = 16;
/// Cleared at each VBlank (not known yet).
const VBLANK_CLEARED: u32 = 0x0607_48c0;
/// The game's flags (globals::GAME_FLAGS) bit 14: while it is clear the VBlank puts the picture
/// the text plane is shared with into it (plane_picture); while set, it clears the last one shown.
const FLAG_14: u32 = 0x4000;
const PLANE_PICTURE: u32 = 0x0605_fcdc;
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
            nested!(cpu, PLANE_PICTURE, plane_picture(cpu, shared));
        } else if cpu.read32(PICTURE_SHOWN) != 0 {
            nested!(cpu, PLANE_PICTURE, plane_picture(cpu, 0));
        }
        let mut i = 0;
        while (i as i32) < cpu.read32(CELLS_COUNT) as i32 {
            let e = CELLS_QUEUE + CELLS_ENTRY * i;
            let (src, dst, w, h) = (cpu.read32(e), cpu.read32(e + 4), cpu.read16(e + 8) as u32, cpu.read16(e + 10) as u32);
            let (char_offset, add) = (cpu.read16(e + 12) as u32, cpu.read32(e + 16));
            cpu.nested_with_stack(&PUT_CELLS_PROLOGUE, PUT_CELLS, &[char_offset, add], |cpu| put_cells(cpu, src, dst, w, h, char_offset, add));
            i += 1;
        }
        cpu.write32(CELLS_COUNT, 0);
    }
    nested!(cpu, 0x0606_08bc, super::text::send_lines(cpu));
}

const PUT_CELLS: u32 = 0x0605_ee3c;
static PUT_CELLS_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

/// Queues a fill for the VBlank (slDMAXCopy, its source fixed): `size` bytes at `dst` with the
/// word at `src`. -1 when the queue is full, else 0.
pub fn queue_fill(cpu: &mut Cpu, src: u32, dst: u32, size: u32) -> u32 {
    queue(cpu, 1, src, dst, size)
}

/// Queues a copy for the VBlank (slDMACopy). -1 when the queue is full, else 0.
pub fn queue_copy(cpu: &mut Cpu, src: u32, dst: u32, size: u32) -> u32 {
    queue(cpu, 0, src, dst, size)
}

fn queue(cpu: &mut Cpu, kind: u32, src: u32, dst: u32, size: u32) -> u32 {
    let n = cpu.read32(QUEUE_COUNT);
    if n == QUEUE_ROOM {
        return u32::MAX;
    }
    let e = QUEUE + QUEUE_ENTRY * n;
    cpu.write32(e, kind);
    cpu.write32(e + 4, src);
    cpu.write32(e + 8, dst);
    cpu.write32(e + 12, size);
    cpu.write32(QUEUE_COUNT, n.wrapping_add(1));
    0
}

/// Starts a long copy (sent `piece` bytes a VBlank), or queues it behind the one under way
/// (no limit): 0 when started, else the long copies queued.
pub fn queue_long_copy(cpu: &mut Cpu, src: u32, dst: u32, size: u32, piece: u32) -> u32 {
    if cpu.read32(LONG_ON) == 0 {
        cpu.write32(LONG_ON, 1);
        for (i, v) in [src, dst, size, piece].into_iter().enumerate() {
            cpu.write32(LONG + 4 * i as u32, v);
        }
        cpu.write32(LONG_DONE, 0);
        return 0;
    }
    let n = cpu.read32(LONG_COUNT);
    let e = LONG_QUEUE + QUEUE_ENTRY * n;
    for (i, v) in [src, dst, size, piece].into_iter().enumerate() {
        cpu.write32(e + 4 * i as u32, v);
    }
    let n = n.wrapping_add(1);
    cpu.write32(LONG_COUNT, n);
    n
}

/// Queues put_cells for the VBlank. -1 when the queue is full, else 0.
pub fn queue_cells(cpu: &mut Cpu, src: u32, dst: u32, w: u32, h: u32, char_offset: u32, add: u32) -> u32 {
    let n = cpu.read32(CELLS_COUNT);
    if n == QUEUE_ROOM {
        return u32::MAX;
    }
    let e = CELLS_QUEUE + CELLS_ENTRY * n;
    cpu.write32(e, src);
    cpu.write32(e + 4, dst);
    cpu.write16(e + 8, w as u16);
    cpu.write16(e + 10, h as u16);
    cpu.write16(e + 12, char_offset as u16);
    cpu.write32(e + 16, add);
    cpu.write32(CELLS_COUNT, n.wrapping_add(1));
    0
}

/// A plane's row: 64 cells.
const PLANE_ROW: u32 = 64;

/// Writes `w` by `h` cells (16-bit words) from `src` into a plane at `dst`, each plus
/// `char_offset` << 12 (its palette, as a rule) and `add`.
pub fn put_cells(cpu: &mut Cpu, src: u32, dst: u32, w: u32, h: u32, char_offset: u32, add: u32) {
    let (w, h) = (w & 0xffff, h & 0xffff);
    let offset = (char_offset & 0xffff) << 12 & 0xffff;
    let (mut src, mut dst) = (src, dst);
    for _ in 0..h {
        for _ in 0..w {
            let v = cpu.read16(src) as u32;
            cpu.write16(dst, v.wrapping_add(offset).wrapping_add(add) as u16);
            src += 2;
            dst += 2;
        }
        dst = dst.wrapping_add((PLANE_ROW - w).wrapping_mul(2));
    }
}

/// put_cells for 32-bit cells, each plus `char_offset` << 16 and `add`.
pub fn put_long_cells(cpu: &mut Cpu, src: u32, dst: u32, w: u32, h: u32, char_offset: u32, add: u32) {
    let (w, h) = (w & 0xffff, h & 0xffff);
    let offset = char_offset << 16;
    let (mut src, mut dst) = (src, dst);
    for _ in 0..h {
        for _ in 0..w {
            let v = cpu.read32(src);
            cpu.write32(dst, v.wrapping_add(offset).wrapping_add(add));
            src += 4;
            dst += 4;
        }
        dst = dst.wrapping_add((PLANE_ROW - w).wrapping_mul(4));
    }
}

/// Fills `w` by `h` cells of a plane at `dst` with `cell`.
pub fn fill_cells(cpu: &mut Cpu, cell: u32, dst: u32, w: u32, h: u32) {
    let (w, h) = (w & 0xffff, h & 0xffff);
    let mut dst = dst;
    for _ in 0..h {
        for _ in 0..w {
            cpu.write16(dst, cell as u16);
            dst += 2;
        }
        dst = dst.wrapping_add((PLANE_ROW - w).wrapping_mul(2));
    }
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

/// The pictures the text plane is shared with (their cells at PICTURES_AT's address, palette 7
/// plus 0xa8, VDP2's colour offsets then on screens 7, or 6 for the first: as far as can be told):
/// each its offset into the cells, where in the text plane, its size in cells, and whether the
/// plane is cleared first. 0 clears it; -1 leaves it be.
const PICTURE_SHOWN: u32 = 0x0607_3c8c;
const PICTURES_AT: u32 = 0x0607_3ffc;
const PICTURES: [(u32, bool, u32, u32, u32, u32); 4] = [
    (6, false, 0, 0x25e7_e000, 44, 28),
    (7, false, 0x9a0, 0x25e7_e000, 44, 28),
    (7, true, 0x1340, 0x25e7_e618, 21, 4),
    (7, true, 0x13e8, 0x25e7_e718, 18, 2),
];
const PICTURE_PALETTE: u32 = 7;
const PICTURE_ADD: u32 = 0xa8;
const SL_COLOR_OFFSET_USE: u32 = 0x0606_98dc;
const QUEUE_CELLS: u32 = 0x0605_ed94;
static QUEUE_CELLS_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

/// Puts picture `picture` into the text plane (by put_cells at the VBlank: queued), or clears it.
pub fn plane_picture(cpu: &mut Cpu, picture: u32) {
    if picture == u32::MAX {
        return;
    }
    cpu.write32(PICTURE_SHOWN, picture);
    if picture > 4 {
        return;
    }
    if picture == 0 {
        cpu.call(SL_COLOR_OFFSET_USE, &[3, 7]);
        nested!(cpu, 0x0605_fdfc, clear_text_plane(cpu));
        return;
    }
    let (screens, clear, offset, dst, w, h) = PICTURES[picture as usize - 1];
    cpu.call(SL_COLOR_OFFSET_USE, &[3, screens]);
    if clear {
        nested!(cpu, 0x0605_fdfc, clear_text_plane(cpu));
    }
    let src = cpu.read32(PICTURES_AT).wrapping_add(offset);
    cpu.nested_with_stack(&QUEUE_CELLS_PROLOGUE, QUEUE_CELLS, &[PICTURE_PALETTE, PICTURE_ADD], |cpu| queue_cells(cpu, src, dst, w, h, PICTURE_PALETTE, PICTURE_ADD));
}


/// VDP2's colour offset A as the mode sets it (r, g, b).
pub fn set_offset_a(cpu: &mut Cpu, r: u32, g: u32, b: u32) {
    let s = cpu.read32(SCREEN);
    for (k, v) in [r, g, b].into_iter().enumerate() {
        cpu.write16(s + OFFSET_A + 2 * k as u32, v as u16);
    }
}

/// VDP2's colour offset B as the mode sets it (r, g, b).
pub fn set_offset_b(cpu: &mut Cpu, r: u32, g: u32, b: u32) {
    let s = cpu.read32(SCREEN);
    for (k, v) in [r, g, b].into_iter().enumerate() {
        cpu.write16(s + OFFSET_B + 2 * k as u32, v as u16);
    }
}

/// Sets PLANE_SHARED_NEXT (for the stages' code).
pub fn set_plane_shared_next(cpu: &mut Cpu, v: u32) {
    cpu.write32(PLANE_SHARED_NEXT, v);
}

/// The next scroll: its start (the scroll table's +0, given the screen's work), unless there is
/// none (the index stays). Reaching the next of the screen's marks (a list of scroll indices,
/// +0x54) moves on to the one after, and past the first asks for the next frame's request.
const SCROLL_MARK: u32 = 0x52;
const SCROLL_MARKS: u32 = 0x54;

pub fn scroll_next(cpu: &mut Cpu) {
    let s = screen(cpu);
    let i = cpu.read8(s + SCROLL_INDEX).wrapping_add(1);
    cpu.write8(s + SCROLL_INDEX, i);
    let s = screen(cpu);
    let i = cpu.read8(s + SCROLL_INDEX) as i8 as i32;
    let table = cpu.read32(SCROLL_TABLE);
    let start = cpu.read32(table.wrapping_add((8 * i) as u32));
    if start == 0 {
        cpu.write8(s + SCROLL_INDEX, (i - 1) as u8);
    } else {
        cpu.call(start, &[s + 0x10]);
    }
    let s = screen(cpu);
    let n = cpu.read8(s + SCROLL_MARK) as i8;
    let marks = cpu.read32(s + SCROLL_MARKS);
    let mark = cpu.read8(marks.wrapping_add(n as i32 as u32));
    if cpu.read8(s + SCROLL_INDEX) == mark {
        cpu.write8(s + SCROLL_MARK, (n as u8).wrapping_add(1));
        if n > 0 {
            cpu.write32(REQUEST, 1);
        }
    }
}
