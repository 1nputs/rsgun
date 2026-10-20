//! The game's main loop (`.control`'s first function): SGL's screen set up, then a pass a frame
//! until something ends it, each pass the pads, the objects, the colours, the file loads, the
//! queued copies, and SGL's frame sync (which waits for the VBlank).

use super::globals::{ELAPSED, GAME_END, GAME_FLAGS, GAME_FRAMES, HELD, LAST_VBLANKS, LOOPS, VBLANKS};
use super::{dma, files, objects, pads, palette, vblank, Cpu};

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
/// The game's: its state at the start (the object lists among it), not yet rewritten.
const INIT_GAME: u32 = 0x0604_016c;
/// Cleared at the start (not known yet); and a word whose bit 31 lasts one game frame.
const START_CLEARED: u32 = 0x0600_edd8;
const ONE_FRAME: u32 = 0x0607_405c;

/// Runs the game until something sets GAME_END: what it was set to.
pub fn main_loop(cpu: &mut Cpu) -> u32 {
    cpu.call(SL_0606705C, &[1]);
    cpu.call(SL_06067088, &[0x80]);
    cpu.call(SL_060670AC, &[0x200, 0x200]);
    cpu.call(SL_060670FC, &[1]);
    cpu.call(SL_TV_OFF, &[]);
    vblank::init_vblank(cpu);
    dma::dma_control_init(cpu);
    cpu.call(INIT_GAME, &[]);
    cpu.write32(START_CLEARED, 0);
    cpu.call(SL_TV_ON, &[]);
    cpu.write32(GAME_END, 0);
    loop {
        cpu.call(SL_INIT_MATRIX, &[]);
        cpu.call(SL_0606713C, &[1]);
        // the VBlanks this pass took (a pass that took none counts one)
        let (now, last) = (cpu.read32(VBLANKS), cpu.read32(LAST_VBLANKS));
        cpu.write32(ELAPSED, if now > last { now - last } else { 1 });
        let now = cpu.read32(VBLANKS);
        cpu.write32(LAST_VBLANKS, now);
        pads::pad_read(cpu);
        objects::obj_exec(cpu);
        if cpu.read32(GAME_FLAGS) & HELD == 0 {
            let w = cpu.read32(ONE_FRAME);
            cpu.write32(ONE_FRAME, if w & HELD != 0 { w & !HELD } else { 0 });
            let n = cpu.read32(GAME_FRAMES);
            cpu.write32(GAME_FRAMES, n.wrapping_add(1));
        }
        let n = cpu.read32(LOOPS);
        cpu.write32(LOOPS, n.wrapping_add(1));
        palette::send_colours(cpu);
        files::load_files(cpu);
        dma::dma_control(cpu);
        cpu.call(SL_SYNCH, &[]);
        if cpu.read32(GAME_END) != 0 {
            break;
        }
    }
    cpu.call(SL_SYNCH, &[]);
    cpu.call(SL_SYNCH, &[]);
    cpu.read32(GAME_END)
}
