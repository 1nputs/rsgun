//! The files: the game's data is a set of files in the cart, as it would be on a Saturn disc
//! ("GAME.BIN", "TITLE.BIN", "STG00.BIN" ...), listed in a table of `FILE_INF` (name, where in the
//! cart, size; dma.c's types). A load (`LOAD_FILE_INF`) copies a file into work RAM a piece a frame.

use super::Cpu;

/// `FILE_INF files[206]`: the name, its address in the cart, its size. The first is not a file the
/// game loads (a lookup that finds nothing gives 0).
const FILE_INF: u32 = 0x0600_95d0;
const FILES: u32 = 0xce;
const FILE: u32 = 12;
const CHAR_FNAME: u32 = 0;
const ROM_ADR: u32 = 4;
const FILE_SIZE: u32 = 8;
/// How far a name is compared (beyond, it is taken as matching).
const NAME_MAX: u32 = 13;
/// The end of a name, ".BIN" with its terminator: a name matches once its characters have run
/// through these, in order.
const BIN: u32 = 0x0600_9f78;
const BIN_LEN: u32 = 5;

/// `LOAD_FILE_INF loads[12]`: the file (0: the load is free), where it goes, how much of it is
/// there (-1: not started), how much goes each frame.
const LOAD_FILE_INF: u32 = 0x0600_ca94;
const LOADS: u32 = 12;
const LOAD: u32 = 16;
const FILE_NO: u32 = 0;
const LOAD_ADR: u32 = 4;
const LOADED: u32 = 8;
const TRANS_SIZE: u32 = 12;

/// Set with the loads emptied, at the game's start (what it is, is not known yet).
const LOAD_SETTING: u32 = 0x0603_2e40;

/// The file named `name` (its index in the table), or 0.
pub fn file_no(cpu: &mut Cpu, name: u32) -> u32 {
    for file in 0..FILES {
        let fname = cpu.read32(FILE_INF + FILE * file + CHAR_FNAME);
        let mut ended = 0;
        for i in 0..NAME_MAX {
            let c = cpu.read8(name + i);
            if c != cpu.read8(fname + i) {
                break;
            }
            if c == cpu.read8(BIN + ended) {
                ended += 1;
                if ended == BIN_LEN {
                    break;
                }
            }
        }
        if ended == BIN_LEN {
            return file;
        }
    }
    0
}

/// A load of the file named `name`, not started (its destination and rate are the caller's to
/// set): the load, or 0 if all are taken or there is no such file.
pub fn load_file(cpu: &mut Cpu, name: u32) -> u32 {
    let Some(load) = (0..LOADS).map(|i| LOAD_FILE_INF + LOAD * i).find(|&l| cpu.read32(l + FILE_NO) == 0) else {
        return 0;
    };
    let file = file_no(cpu, name);
    if file == 0 {
        return 0;
    }
    cpu.write32(load + FILE_NO, file);
    cpu.write32(load + LOADED, u32::MAX);
    load
}

/// Each frame: each load started goes on by its rate (or to its end, if that is less than half as
/// much again).
pub fn load_files(cpu: &mut Cpu) {
    for i in 0..LOADS {
        let load = LOAD_FILE_INF + LOAD * i;
        let file = cpu.read32(load + FILE_NO);
        let from = cpu.read32(load + LOADED) as i32;
        if file == 0 || from < 0 {
            continue;
        }
        let entry = FILE_INF + FILE * file;
        let rate = cpu.read32(load + TRANS_SIZE) as i32;
        let mut to = cpu.read32(entry + FILE_SIZE) as i32;
        if to - from > rate + rate / 2 {
            to = from + rate;
        }
        for at in from..to {
            let (rom, dst) = (cpu.read32(entry + ROM_ADR), cpu.read32(load + LOAD_ADR));
            let b = cpu.read8(rom.wrapping_add(at as u32));
            cpu.write8(dst.wrapping_add(at as u32), b);
        }
        cpu.write32(load + LOADED, to as u32);
    }
}

/// Frees every load.
pub fn clear_loads(cpu: &mut Cpu) {
    for i in 0..LOADS {
        cpu.write32(LOAD_FILE_INF + LOAD * i + FILE_NO, 0);
    }
}

/// At the game's start: frees every load.
pub fn init_loads(cpu: &mut Cpu) -> u32 {
    clear_loads(cpu);
    cpu.write32(LOAD_SETTING, 0x600);
    1
}
