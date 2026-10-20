//! The game's main loop (`.control`'s first function): SGL's screen set up, then a pass a frame
//! until something ends it, each pass the pads, the objects, the colours, the file loads, the
//! queued copies, and SGL's frame sync (which waits for the VBlank).

use super::nested;
use super::globals::{ELAPSED, GAME_END, GAME_FLAGS, GAME_FRAMES, HELD, LAST_VBLANKS, LOOPS, VBLANKS};
use super::{camera, director, dma, files, hits, objects, pads, palette, screen, sound, vblank, Cpu};

/// SGL's: the display off and on (VDP2's TVMD, the next VBlank), slSynch (the frame's end: waits
/// for the VBlank, then the slave draws), the matrix stack back to one unit matrix.
const SL_TV_OFF: u32 = 0x0606_7126;
const SL_TV_ON: u32 = 0x0606_711c;
const SL_SYNCH: u32 = 0x0603_4cca;
const SL_INIT_MATRIX: u32 = 0x0603_5110;
/// SGL's, not yet named: two settings sent to the slave (0x3c, 0x44), the screen's centre and
/// size, a VDP2 register bit, and the window a frame is drawn through.
const SL_0606705C: u32 = 0x0606_705c;
const SL_06067088: u32 = 0x0606_7088;
const SL_060670AC: u32 = 0x0606_70ac;
const SL_060670FC: u32 = 0x0606_70fc;
const SL_0606713C: u32 = 0x0606_713c;
/// A word whose bit 31 lasts one game frame.
const ONE_FRAME: u32 = 0x0607_405c;

/// The SH-2 main loop's registers as it keeps them, for its callees' pushes (the game reads stack
/// it never set, and gets what was pushed there): r8 0 from its second call, SL_SYNCH's address
/// once it calls it; r9 GAME_END's address after the first pass; r10-r13 ONE_FRAME's, ELAPSED's,
/// LAST_VBLANKS's and VBLANKS's addresses through the loop. Kept for the caller, as the SH-2 code
/// keeps them.
fn hold(cpu: &mut Cpu, r: usize, v: u32) {
    cpu.sh2.r[r] = v;
}

/// Runs the game until something sets GAME_END: what it was set to.
pub fn main_loop(cpu: &mut Cpu) -> u32 {
    let kept: [u32; 6] = cpu.sh2.r[8..14].try_into().unwrap_or([0; 6]);
    cpu.call(SL_0606705C, &[1]);
    hold(cpu, 8, 0);
    cpu.call(SL_06067088, &[0x80]);
    cpu.call(SL_060670AC, &[0x200, 0x200]);
    cpu.call(SL_060670FC, &[1]);
    cpu.call(SL_TV_OFF, &[]);
    nested!(cpu, 0x0600_4468, vblank::init_vblank(cpu));
    nested!(cpu, 0x0600_46a0, dma::dma_control_init(cpu));
    nested!(cpu, 0x0604_016c, init_game(cpu));
    cpu.write32(sound::SE_LOCK, 0);
    cpu.call(SL_TV_ON, &[]);
    cpu.write32(GAME_END, 0);
    for (r, v) in [(13, VBLANKS), (12, LAST_VBLANKS), (11, ELAPSED), (10, ONE_FRAME)] {
        hold(cpu, r, v);
    }
    loop {
        cpu.call(SL_INIT_MATRIX, &[]);
        cpu.call(SL_0606713C, &[1]);
        // the VBlanks this pass took (a pass that took none counts one)
        let (now, last) = (cpu.read32(VBLANKS), cpu.read32(LAST_VBLANKS));
        cpu.write32(ELAPSED, if now > last { now - last } else { 1 });
        let now = cpu.read32(VBLANKS);
        cpu.write32(LAST_VBLANKS, now);
        nested!(cpu, 0x0600_58a8, pads::pad_read(cpu));
        nested!(cpu, 0x0600_6b64, objects::obj_exec(cpu));
        if cpu.read32(GAME_FLAGS) & HELD == 0 {
            let w = cpu.read32(ONE_FRAME);
            cpu.write32(ONE_FRAME, if w & HELD != 0 { w & !HELD } else { 0 });
            let n = cpu.read32(GAME_FRAMES);
            cpu.write32(GAME_FRAMES, n.wrapping_add(1));
        }
        let n = cpu.read32(LOOPS);
        cpu.write32(LOOPS, n.wrapping_add(1));
        nested!(cpu, 0x0600_5a70, palette::send_colours(cpu));
        nested!(cpu, 0x0600_5fb0, files::load_files(cpu));
        nested!(cpu, 0x0600_4580, dma::dma_control(cpu));
        hold(cpu, 8, SL_SYNCH);
        cpu.call(SL_SYNCH, &[]);
        hold(cpu, 9, GAME_END);
        if cpu.read32(GAME_END) != 0 {
            break;
        }
    }
    cpu.call(SL_SYNCH, &[]);
    cpu.call(SL_SYNCH, &[]);
    let end = cpu.read32(GAME_END);
    cpu.sh2.r[8..14].copy_from_slice(&kept);
    end
}

/// The kinds of objects (79, 16 bytes each): their sprites (+4), first character and attributes
/// (words +8, +0xa), a word (+0xc), and the points they are worth (+0xe, from the cart's table of
/// 8-byte entries).
const KINDS: u32 = 0x0607_41f0;
const KIND_SIZE: u32 = 16;
const KIND_POINTS: u32 = 0x0607_2478;
const KIND_POINTS_LAST: u32 = 0x0607_26e8;
/// Each player's lives (bytes), the numbered objects (64 longs; the players first).
const LIVES: u32 = 0x0607_4062;
const PLAYERS: u32 = 0x0607_40f0;
/// The load buffers in work RAM-L, a size, and three words: each kept at its default and current
/// (states.rs copies them as a mode loads).
const BUFFER: (u32, u32) = (0x0607_40d8, 0x0607_40e4);
const BUFFER_2: (u32, u32) = (0x0607_4090, 0x0607_4050);
const BUFFER_SIZE: (u32, u32) = (0x0607_4094, 0x0607_4058);
const WORD_E0: (u32, u32) = (0x0607_40e0, 0x0607_40ea);
const WORD_EC: (u32, u32) = (0x0607_40ec, 0x0607_40e8);
const WORD_60: (u32, u32) = (0x0607_4060, 0x0607_4054);
/// The sound file load's SH-2 entry (for nested!).
const LOAD_SOUND_FILE: u32 = 0x0600_4b18;
/// The sound files loaded at the start (system's and two more, not known yet).
const SOUND_SYSTEM: u32 = 0;
const SOUND_23: u32 = 23;
const SOUND_22: u32 = 22;

/// The game's state at the start: the objects, the numbered objects and kinds cleared (the
/// points from the cart), no lives, the buffers' defaults, the sound files loaded, then the
/// director, the camera, the sound manager, the screen and the hit manager made.
pub fn init_game(cpu: &mut Cpu) {
    nested!(cpu, 0x0600_5634, objects::obj_init(cpu));
    cpu.write32(ONE_FRAME, 0);
    for i in (0..0x100).step_by(4) {
        cpu.write32(PLAYERS + i, 0);
    }
    let mut kind = KINDS;
    let mut src = KIND_POINTS;
    loop {
        cpu.write32(kind + 4, 0);
        for k in 0..3 {
            cpu.write16(kind + 8 + 2 * k, 0);
        }
        let points = cpu.read16(src + 4);
        cpu.write16(kind + 0xe, points);
        src += 8;
        kind += KIND_SIZE;
        if src > KIND_POINTS_LAST {
            break;
        }
    }
    cpu.write8(LIVES + 1, 0);
    cpu.write8(LIVES, 0);
    for ((default, current), v) in [(BUFFER, 0x20_0000), (BUFFER_2, 0x30_0000), (BUFFER_SIZE, 0xf000)] {
        cpu.write32(default, v);
        cpu.write32(current, v);
    }
    for ((default, current), v) in [(WORD_E0, 0), (WORD_EC, 0x400), (WORD_60, 0)] {
        cpu.write16(default, v);
        cpu.write16(current, v);
    }
    nested!(cpu, LOAD_SOUND_FILE, sound::load_sound_file(cpu, SOUND_SYSTEM, 0x20_0000));
    nested!(cpu, 0x0600_49cc, sound::sound_setup_default(cpu));
    let buffer = cpu.read32(BUFFER.0);
    nested!(cpu, LOAD_SOUND_FILE, sound::load_sound_file(cpu, SOUND_23, buffer));
    let buffer = cpu.read32(BUFFER.0);
    nested!(cpu, LOAD_SOUND_FILE, sound::load_sound_file(cpu, SOUND_22, buffer));
    nested!(cpu, 0x0604_2db8, director::game_set(cpu));
    nested!(cpu, 0x0604_2ff8, camera::camera_set(cpu));
    nested!(cpu, 0x0600_5598, sound::sound_manager_set(cpu));
    nested!(cpu, 0x0605_e1b0, screen::screen_set(cpu));
    nested!(cpu, 0x0604_5154, hits::hits_set(cpu));
}
