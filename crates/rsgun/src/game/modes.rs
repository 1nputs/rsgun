//! Code the mode files (the overlays) each carry a copy of, the same in form.

use super::director::GAME;
use super::{nested, screen, Cpu};

/// The director's flags byte: bit 0, a mode runs.
const FLAGS: u32 = 0x3e;
const MODE_RUNS: u8 = 1;

/// A mode's first scroll's frame: on to the next scroll once the mode runs.
pub fn mode_wait(cpu: &mut Cpu) {
    let game = cpu.read32(GAME);
    if cpu.read8(game + FLAGS) & MODE_RUNS != 0 {
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
    }
}
