//! The objects: everything that acts in the game (enemies, shots, effects, the player, the game's
//! own processes) is an object, a `Base_obj` header (dma.c's debug types) in a pool of 0x80-byte
//! blocks, in one of five lists ("areas"; the leftover source's ENM_OBJ_AREA is one), each list
//! with its own range of the pool. Each frame every object's `exec` runs, list by list (obj_exec).
//! obj_set and obj_size are the leftover source's names.

use super::nested;
use super::Cpu;
use super::math;

/// `Base_obj`: how many blocks it takes, its list, a flag cleared before each exec, its mode, its
/// function (called with the object), the objects before and after it in its list.
const SIZE: u32 = 0x00; // char
const AREA: u32 = 0x01; // char
const POST: u32 = 0x02; // unsigned char
const MODE: u32 = 0x03; // char
pub const EXEC: u32 = 0x04; // void (*)()
const BEFORE: u32 = 0x08; // Base_obj *
pub const NEXT: u32 = 0x0c; // Base_obj *

/// The pool and its blocks.
const POOL: u32 = 0x0600_ee10;
const BLOCK: u32 = 0x80;
const BLOCKS: u32 = 0x400;
/// The lists, `OBJ_INF[5]` (dma.c's types): each its first and last object (top_adr, end_adr).
pub const OBJ_INF: u32 = 0x0600_ede0;
pub const AREAS: u32 = 5;
/// Each list's range of the pool's blocks (first, end), as shorts.
const AREA_BLOCKS: u32 = 0x0600_8b60;

fn head(area: u32) -> u32 {
    OBJ_INF + 8 * area
}

fn tail(area: u32) -> u32 {
    OBJ_INF + 8 * area + 4
}

/// obj_size: the blocks an object of `bytes` takes (at least one).
pub fn obj_size(cpu: &mut Cpu, bytes: u32) -> u32 {
    let bytes = bytes as i16 as i32 as u32;
    if bytes == 0 {
        return 1;
    }
    let blocks = if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, bytes, BLOCK)) == 0 {
        nested!(cpu, 0x0600_6a78, math::quotient(cpu, bytes, BLOCK))
    } else {
        nested!(cpu, 0x0600_6a78, math::quotient(cpu, bytes, BLOCK)).wrapping_add(1)
    };
    blocks as i8 as i32 as u32
}

/// The first run of `blocks` free blocks in `area`'s range of the pool, or 0. An object's first
/// block holds its size; the blocks after it are skipped.
pub fn find_free(cpu: &mut Cpu, area: u32, blocks: u32) -> u32 {
    let range = AREA_BLOCKS + 4 * area;
    let (mut at, end) = (cpu.read16(range) as i16 as i32, cpu.read16(range + 2) as i16 as i32);
    let blocks = blocks as i32;
    'search: while at + blocks < end {
        let first = POOL.wrapping_add((at as i16 as i32 * BLOCK as i32) as u32);
        let mut block = first;
        let mut left = blocks;
        loop {
            let size = cpu.read8(block + SIZE) as i8 as i32;
            at += size;
            if size != 0 {
                continue 'search;
            }
            left -= 1;
            if left <= 0 {
                return first;
            }
            at += 1;
            block += BLOCK;
        }
    }
    0
}

/// obj_set: a new object of `blocks` blocks at the end of `area`'s list, with `exec` its function;
/// 0 if its range of the pool has no room.
pub fn obj_set(cpu: &mut Cpu, area: u32, blocks: u32, exec: u32) -> u32 {
    let area = area as i16 as i32 as u32;
    let obj = nested!(cpu, 0x0600_6aac, find_free(cpu, area, blocks));
    if obj == 0 {
        return 0;
    }
    let last = cpu.read32(tail(area));
    cpu.write8(obj + SIZE, blocks as u8);
    cpu.write8(obj + AREA, area as u8);
    cpu.write8(obj + POST, 0);
    cpu.write8(obj + MODE, 0);
    cpu.write32(obj + EXEC, exec);
    cpu.write32(obj + BEFORE, last);
    cpu.write32(obj + NEXT, 0);
    if cpu.read32(head(area)) == 0 {
        cpu.write32(head(area), obj);
    }
    cpu.write32(tail(area), obj);
    if last != 0 {
        cpu.write32(last + NEXT, obj);
    }
    obj
}

/// obj_set as it links the object in after `after` (in its list) rather than at the end.
pub fn obj_set_after(cpu: &mut Cpu, area: u32, blocks: u32, exec: u32, after: u32) -> u32 {
    let (area, blocks) = (area as i8 as i32 as u32, blocks as i8 as i32 as u32);
    let obj = nested!(cpu, 0x0600_6aac, find_free(cpu, area, blocks));
    if obj == 0 {
        return 0;
    }
    let next = cpu.read32(after + NEXT);
    cpu.write8(obj + SIZE, blocks as u8);
    cpu.write8(obj + AREA, area as u8);
    cpu.write8(obj + POST, 0);
    cpu.write8(obj + MODE, 0);
    cpu.write32(obj + EXEC, exec);
    cpu.write32(obj + BEFORE, after);
    cpu.write32(obj + NEXT, next);
    if cpu.read32(head(area)) == 0 {
        cpu.write32(head(area), obj);
    }
    if cpu.read32(obj + NEXT) == 0 {
        cpu.write32(tail(area), obj);
    }
    cpu.write32(after + NEXT, obj);
    if next != 0 {
        cpu.write32(next + BEFORE, obj);
    }
    obj
}

/// Takes `obj` out of its list and frees its blocks.
pub fn obj_free(cpu: &mut Cpu, obj: u32) {
    let (before, next) = (cpu.read32(obj + BEFORE), cpu.read32(obj + NEXT));
    let area = |cpu: &mut Cpu| cpu.read8(obj + AREA) as i8 as i32 as u32;
    match before {
        0 => {
            let area = area(cpu);
            cpu.write32(head(area), next);
        }
        _ => cpu.write32(before + NEXT, next),
    }
    match next {
        0 => {
            let area = area(cpu);
            cpu.write32(tail(area), before);
        }
        _ => cpu.write32(next + BEFORE, before),
    }
    let blocks = cpu.read8(obj + SIZE) as i8;
    for i in 0..blocks.max(0) as u32 {
        cpu.write8(obj + i * BLOCK + SIZE, 0);
    }
}

/// Each frame: every object's function, list by list (`post` cleared first). An object may free
/// itself: the next is taken after (its header stays as it was).
pub fn obj_exec(cpu: &mut Cpu) {
    for area in 0..AREAS {
        let mut obj = cpu.read32(head(area));
        while obj != 0 {
            cpu.write8(obj + POST, 0);
            let exec = cpu.read32(obj + EXEC);
            cpu.call(exec, &[obj]);
            obj = cpu.read32(obj + NEXT);
        }
    }
}

/// Empties the lists and the pool.
pub fn obj_init(cpu: &mut Cpu) {
    for area in 0..AREAS {
        cpu.write32(head(area), 0);
        cpu.write32(tail(area), 0);
    }
    for i in 0..BLOCKS {
        let block = POOL + i * BLOCK;
        for field in [SIZE, AREA, POST, MODE] {
            cpu.write8(block + field, 0);
        }
        for field in [EXEC, BEFORE, NEXT] {
            cpu.write32(block + field, 0);
        }
    }
}
