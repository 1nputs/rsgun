//! The game's start: crt0 (the BIOS jumps to the game's entry), GCC's __main, and the top level,
//! which sets the machine up (SGL, the ST-V's I/O, the A-bus, the loads, the sound, the VBlank,
//! the backup), reads the cart's protection device and checks what it gives, then loads the
//! game's code and runs it, over and over. None of it returns.

use super::globals::{mode, GAME_FLAGS, GAME_FRAMES, GAME_MODE, LOOPS, VBLANKS};
use super::{backup, clib, files, main_loop, nested, pads, sgl, sound, vblank, Cpu};

/// crt0: GCC's __main, the BSS and SGL's work area cleared a byte at a time, then the top level,
/// on SGL's stack, through SGL's restart. The clears take the SH-2 loops' time (`Cpu::spend`, an
/// instruction a cycle): the game's first frame comes when the SH-2 code's does.
const BSS_START: u32 = 0x0600_a9a0;
const BSS_END: u32 = 0x0603_3284;
const SGL_WORK: u32 = 0x060f_fc00;
const SGL_WORK_SIZE: u32 = 0x400;
const TOP_LEVEL: u32 = 0x0600_4070;

pub fn start(cpu: &mut Cpu) {
    nested!(cpu, 0x0600_8100, gcc_main(cpu));
    for at in BSS_START..BSS_END {
        cpu.write8(at, 0);
        cpu.spend(0x0600_4014, 4);
    }
    for i in 0..SGL_WORK_SIZE {
        cpu.write8(SGL_WORK + i, 0);
        cpu.spend(0x0600_4024, 5);
    }
    nested!(cpu, 0x0600_7e50, restart(cpu, TOP_LEVEL));
}

/// SGL's restart: the stack SGL keeps for the master (the long at 0x060089b4), every interrupt
/// masked, the cache purged and on (CCR), GBR SGL's work area, then a jump to `f` (the top level,
/// with the return address as it was: it never returns).
const MASTER_STACK: u32 = 0x0600_89b4;
const MASKED: u32 = 0xf0;
const CCR: u32 = 0xffff_fe92;
const CACHE_PURGED_ON: u8 = 0x11;

pub fn restart(cpu: &mut Cpu, f: u32) {
    cpu.sh2.sr = MASKED;
    cpu.sh2.r[15] = cpu.read32(MASTER_STACK);
    cpu.write8(CCR, CACHE_PURGED_ON);
    cpu.sh2.gbr = SGL_WORK;
    let pr = cpu.pr();
    cpu.call_returning(pr, f, &[]);
}

/// The top level (SGL's restart jumps to it; it never returns): the machine set up, the cart's
/// protection checked (its loops taking the SH-2 code's time), then the game's code (GAME.BIN)
/// loaded at 0x06040000 and run until it asks to start again, and all of it again. (The SH-2 code is a switch on a mode: 0 the set-up,
/// 2 the run, 1 and the rest a wait for ever; it goes 0, 2, 0, 2.) A 0x200-byte buffer in its
/// frame takes the protection device's data. The SH-2 code's r8-r10 held across its calls.
///
/// The set-up: slInitSystem (TV mode 4, SGL's texture table, a sync a frame), then with every
/// interrupt masked for each, the ST-V's I/O byte at 0x00400007 given bits 2 and 3, the one at
/// 0x00400011 0x77, the first one's high nibble set; the A-bus's timing for both cart areas
/// (ASR0, ASR1), each after a read of the cart; the loads freed, the sound driver loaded, the
/// VBlank's callbacks, the game's state cleared, the backup read, the counters cleared, the
/// protection device's data read. Its first 0x100 bytes must be each place's high nibble, the
/// next each place's low nibble (PROTECTION 3, 4 otherwise); when either is wrong, the names of
/// PLAY.BIN and the stages' files are blanked (no stage loads).
const SR_MASK_OFF: u32 = 0xffff_ff0f;
const TV_MODE: u32 = 4;
/// SGL's texture table (the characters' sizes, CHAR_SIZES elsewhere).
const TEXTURES: u32 = 0x0600_cc70;
const IO_D: u32 = 0x2040_0007;
const IO_H: u32 = 0x2040_0011;
const IO_D_SET: u8 = 12;
const IO_H_VALUE: u8 = 0x77;
const CART: u32 = 0x2200_0000;
const ASR: [u32; 2] = [0x25fe_00b0, 0x25fe_00b4];
const A_BUS_TIMING: u32 = 0x1430_1430;
/// The protection check's verdict (0 good, 3 or 4 wrong): player.rs's GROUND_SAFE (the player
/// does not die while it is set), backup.rs's SETTINGS_CLEARED (cleared as the backup is read).
const PROTECTION: u32 = 0x0600_edbc;
const PROTECTION_SIZE: u32 = 0x200;
const BLANKED: [u32; 6] = [0x0600_8c6c, 0x0600_8bb8, 0x0600_8bc4, 0x0600_8bd0, 0x0600_8bdc, 0x0600_8be8];
const GAME_FILE: u32 = 0x0600_8b7c; // "GAME.BIN"
const CONTROL: u32 = 0x0604_0000;

pub fn top_level(cpu: &mut Cpu) {
    let buffer = cpu.stack();
    let mut held = [(8, SR_MASK_OFF), (9, PROTECTION), (10, cpu.reg(10))];
    loop {
        cpu.holding(&held, |cpu| nested!(cpu, 0x0600_7ef0, sgl::sl_init_system(cpu, TV_MODE, TEXTURES, u32::MAX)));
        held[2].1 = MASKED;
        let io_d = |cpu: &mut Cpu, f: fn(u8) -> u8| {
            let mask = cpu.sh2.sr & MASKED;
            cpu.sh2.sr = (cpu.sh2.sr & SR_MASK_OFF) | MASKED;
            let v = cpu.read8(IO_D);
            cpu.write8(IO_D, f(v));
            cpu.sh2.sr = (cpu.sh2.sr & SR_MASK_OFF) | mask;
        };
        io_d(cpu, |v| v | IO_D_SET);
        cpu.write8(IO_H, IO_H_VALUE);
        io_d(cpu, |v| v & 15 | 0xf0);
        for asr in ASR {
            cpu.read32(CART);
            cpu.write32(asr, A_BUS_TIMING);
        }
        cpu.holding(&held, |cpu| {
            nested!(cpu, 0x0600_5e10, files::init_loads(cpu));
            nested!(cpu, 0x0600_49e8, sound::sound_load(cpu));
            nested!(cpu, 0x0600_4468, vblank::init_vblank(cpu));
            nested!(cpu, 0x0600_4288, clear_state(cpu));
            nested!(cpu, 0x0600_63e4, backup::load_backup(cpu));
            nested!(cpu, 0x0600_4304, reset_counters(cpu));
            nested!(cpu, 0x0600_6978, protection_read(cpu, buffer));
        });
        for i in 0..PROTECTION_SIZE / 2 {
            if cpu.read8(buffer + i) != (i & 0xf0) as u8 {
                cpu.write8(PROTECTION, 3);
            }
            cpu.spend(0x0600_4150, 9);
        }
        for i in PROTECTION_SIZE / 2..PROTECTION_SIZE {
            if cpu.read8(buffer + i) != (i & 15) as u8 {
                cpu.write8(PROTECTION, 4);
            }
            cpu.spend(0x0600_4170, 8);
        }
        if cpu.read8(PROTECTION) != 0 {
            for name in BLANKED {
                let mut at = name;
                while cpu.read8(at) != 0 {
                    cpu.write8(at, 0);
                    at += 1;
                }
            }
        }
        cpu.holding(&held, |cpu| {
            nested!(cpu, 0x0600_5f10, files::read_file(cpu, GAME_FILE, CONTROL));
            nested!(cpu, 0x0600_4340, run_game(cpu));
        });
    }
}

/// The game's state at the start: the debug flags, the sound lock, the full-speed and display
/// flags, the game's flags, the levels and experience a player starts with cleared; the mode the
/// title.
const DEBUG_FLAGS: u32 = 0x0600_ed8c;
const FULL_SPEED: u32 = 0x0600_ed90;
const DISPLAY_FLAGS: [u32; 2] = [0x0600_cc61, 0x0600_edcc];
const START_LEVELS: u32 = 0x0600_ed78;
const START_EXPERIENCE: u32 = 0x0600_eda0;

pub fn clear_state(cpu: &mut Cpu) {
    cpu.write32(DEBUG_FLAGS, 0);
    cpu.write32(sound::SE_LOCK, 0);
    cpu.write8(FULL_SPEED, 0);
    cpu.write32(GAME_FLAGS, 0);
    for at in DISPLAY_FLAGS {
        cpu.write8(at, 0);
    }
    cpu.write8(GAME_MODE, mode::GAME_TITLE);
    for player in [0, 3] {
        for i in [2, 1, 0] {
            cpu.write8(START_LEVELS + player + i, 0);
        }
        for i in [2, 1, 0] {
            cpu.write32(START_EXPERIENCE + 4 * (player + i), 0);
        }
    }
}

/// The game's frames, the main loop's passes and the VBlanks counted cleared, the mode the title,
/// the pads cleared.
pub fn reset_counters(cpu: &mut Cpu) {
    cpu.write32(GAME_FRAMES, 0);
    cpu.write32(LOOPS, 0);
    cpu.write32(VBLANKS, 0);
    cpu.write8(GAME_MODE, mode::GAME_TITLE);
    nested!(cpu, 0x0600_584c, pads::pad_init(cpu));
}

/// The game's main loop run again while it ends with anything but 1 (start again).
const MAIN_LOOP: u32 = 0x0604_0000;

pub fn run_game(cpu: &mut Cpu) {
    cpu.holding(&[(8, MAIN_LOOP)], |cpu| while nested!(cpu, 0x0604_0000, main_loop::main_loop(cpu)) as i16 != 1 {});
}

/// The cart's protection device (the ST-V's, at 0x04fffff0): 0x200 bytes of its data into `buf`
/// a long at a time, after its set-up (0x0800, 0x0010 and 0x7777 into its registers 8, 10 and 12,
/// with it off; then on), and off again.
const PROT_ON: u32 = 0x04ff_fff1;
const PROT_SET_UP: [(u32, u16); 3] = [(0x04ff_fff8, 0x0800), (0x04ff_fffa, 0x0010), (0x04ff_fffc, 0x7777)];
const PROT_DATA: u32 = 0x24ff_fffc;

pub fn protection_read(cpu: &mut Cpu, buf: u32) {
    cpu.write8(PROT_ON, 0);
    for (at, v) in PROT_SET_UP {
        cpu.write16(at, v);
    }
    cpu.write8(PROT_ON, 1);
    for i in (0..PROTECTION_SIZE).step_by(4) {
        let v = cpu.read32(PROT_DATA);
        cpu.write32(buf + i, v);
    }
    cpu.write8(PROT_ON, 0);
}

/// GCC's __main: the global constructors, once (its flag lies in the BSS, which crt0 clears only
/// after; the constructors are not seen run).
const CONSTRUCTED: u32 = 0x0600_cb88;

pub fn gcc_main(cpu: &mut Cpu) {
    if cpu.read32(CONSTRUCTED) == 0 {
        cpu.write32(CONSTRUCTED, 1);
        nested!(cpu, 0x0600_80bc, global_constructors(cpu));
    }
}

/// GCC's __do_global_ctors: the constructors run from the last (none: the list is empty), and the
/// destructors' runner given to atexit (not seen run).
const CONSTRUCTORS: (u32, u32) = (0x0603_83e0, 0x0603_83e0);
const GLOBAL_DESTRUCTORS: u32 = 0x0600_8088;

pub fn global_constructors(cpu: &mut Cpu) {
    let (first, mut at) = CONSTRUCTORS;
    while at > first {
        at -= 4;
        let f = cpu.read32(at);
        cpu.call(f, &[]);
    }
    nested!(cpu, 0x0600_826c, clib::atexit(cpu, GLOBAL_DESTRUCTORS));
}
