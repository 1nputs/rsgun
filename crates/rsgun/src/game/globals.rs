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
/// A word of the BIOS's: unless its bits 12-15 are 1, the pads' bit 9 is cleared and the first
/// score table starts empty (what the setting is, is not known yet).
pub const BIOS_SETTING: u32 = 0x0600_0248;

/// Whether the BIOS's setting above is 1.
pub fn bios_setting_is_1(cpu: &mut super::Cpu) -> bool {
    (cpu.read32(BIOS_SETTING) >> 12) & 0xF == 1
}

/// The game mode (a byte; `mode`'s values).
pub const GAME_MODE: u32 = 0x0600_ebb0;

/// The game modes (dma.c's enum, all of it): the logo, the title, the options, the name entry, the
/// result, the opening, the stages STG00-STG05, the demos.
#[allow(dead_code)]
pub mod mode {
    pub const GAME_LOGO: u8 = 0;
    pub const GAME_TITLE: u8 = 1;
    pub const GAME_OPTION: u8 = 2;
    pub const GAME_NAME_ENTRY: u8 = 3;
    pub const GAME_RESULT: u8 = 4;
    pub const GAME_OPENING: u8 = 5;
    pub const STG00: u8 = 6;
    pub const STG01: u8 = 7;
    pub const STG05: u8 = 11;
    pub const DEMO10: u8 = 12;
    pub const STGEND: u8 = 16;
}

/// The players' `PLAY_DATA` (dma.c's types; 200 bytes each): what each has broken and chained,
/// the times played with each weapon, the route, the score (+0x40), the scores by stage, the
/// continues used (+0xc4).
pub const PLAY_DATA: u32 = 0x0600_ebe0;
pub const PLAY_DATA_SIZE: u32 = 0xc8;
pub const SCORE: u32 = 0x40;
pub const CONTINUES: u32 = 0xc4;
