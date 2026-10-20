//! The game's variables that more than one part of it uses, by address in work RAM.

/// VBlanks counted since the game started (system::count_vblank, a VBlank callback).
pub const VBLANKS: u32 = 0x0600_ebb4;
/// The VBlank count the main loop's last pass began at, and the VBlanks that pass took (at least
/// one; read by a timer that must keep time through slowdown).
pub const LAST_VBLANKS: u32 = 0x0600_edb8;
pub const ELAPSED: u32 = 0x0600_edc8;
/// The main loop's passes, and the game's frames (the passes not paused).
pub const LOOPS: u32 = 0x0600_ed88;
pub const GAME_FRAMES: u32 = 0x0600_ebd4;
/// The game's flags: bit 31 holds the game still (no game frames counted).
pub const GAME_FLAGS: u32 = 0x0600_ed80;
pub const HELD: u32 = 1 << 31;
/// Set to end the main loop: its result.
pub const GAME_END: u32 = 0x0607_40dc;
