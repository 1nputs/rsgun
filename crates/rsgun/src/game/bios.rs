//! The ST-V BIOS's services as the game calls them after the BIOS hands over (BIOS EPR-20091):
//! its code the BIOS copies into work RAM-H at 0x06000c00-0x06003000 (the service table at
//! 0x06000200-0x06000800 points into it: interrupts, the SCU's mask, the vertical blank's hook,
//! coins and credits, continues, the backup RAM) and the few routines the game reaches in the
//! BIOS ROM (the operating time's count, an empty hook, the backup RAM's helpers). The machine's
//! code, not the game's: read where the BIOS puts it, never copied into the source.

#[allow(unused_imports)]
use super::{nested, Cpu};
