//! The stages' timelines: an object that runs a table of events (32 bytes each) by the frame
//! each starts at, counting frames while the game runs. An event's function is called each frame
//! from its start until it says it is done (or at once marked done if the difficulty is below
//! its level); an event flagged as a barrier holds the timeline there (the events after it wait),
//! and the timeline ends at a barrier once all before it are done.

use super::globals::{GAME_FLAGS, HELD};
use super::{nested, objects, Cpu};

/// The objects' mode byte: new, running, ending, ended.
const OBJ_MODE: u32 = 3;
const NEW: u8 = 0;
const RUNNING: u8 = 1;
const ENDING: u8 = 0x70;
const ENDED: u8 = 0x73;
/// The timeline: its events, the first event not done, the frame it is at.
const EVENTS: u32 = 0x10;
const CURRENT: u32 = 0x14;
const FRAME: u32 = 0x18;
/// An event: its flags (a byte), the frame it starts at (a word), its function (called with the
/// event; done when it returns other than 0).
const EVENT: u32 = 0x20;
const E_FLAGS: u32 = 0x00;
const E_START: u32 = 0x02;
const E_FUNCTION: u32 = 0x0c;
/// The flags: its difficulty level (the lowest it runs at), done, a barrier (0x20 holds, 0x80
/// holds and ends the timeline when reached).
const LEVEL: u8 = 3;
const DONE: u8 = 0x40;
const BARRIER: u8 = 0xa0;
const ENDS: u8 = 0xc0;
/// The difficulty (backup.rs's first setting).
const DIFFICULTY: u32 = 0x0600_ed84;

/// A timeline's object behaviour: started (not while the game is held) and run each frame; freed
/// a frame after it ends.
pub fn timeline_exec(cpu: &mut Cpu, t: u32) {
    match cpu.read8(t + OBJ_MODE) {
        RUNNING => {}
        NEW => {
            if cpu.read32(GAME_FLAGS) & HELD != 0 {
                return;
            }
            cpu.write8(t + OBJ_MODE, RUNNING);
        }
        ENDING => {
            cpu.write8(t + OBJ_MODE, ENDED);
            return;
        }
        ENDED => {
            nested!(cpu, 0x0600_579c, objects::obj_free(cpu, t));
            return;
        }
        _ => return,
    }
    nested!(cpu, 0x0605_7210, timeline_run(cpu, t));
}

/// One frame of the timeline `t` (not while the game is held): each event from the first not
/// done that has started is run, up to a barrier; the first not done moves on past each event
/// done in turn; the frame counted.
pub fn timeline_run(cpu: &mut Cpu, t: u32) {
    if cpu.read32(GAME_FLAGS) & HELD != 0 {
        return;
    }
    let mut i = cpu.read32(t + CURRENT);
    let mut e = cpu.read32(t + EVENTS).wrapping_add(i.wrapping_mul(EVENT));
    while cpu.read16(e + E_START) as i16 as i32 <= cpu.read32(t + FRAME) as i32 {
        let function = cpu.read32(e + E_FUNCTION);
        if function != 0 {
            let flags = cpu.read8(e + E_FLAGS);
            if (flags & LEVEL) as i8 > cpu.read8(DIFFICULTY) as i8 {
                cpu.write8(e + E_FLAGS, flags | DONE);
            }
            if cpu.read8(e + E_FLAGS) & DONE == 0 {
                let function = cpu.read32(e + E_FUNCTION);
                if cpu.call(function, &[e]) != 0 {
                    let flags = cpu.read8(e + E_FLAGS) | DONE;
                    cpu.write8(e + E_FLAGS, flags);
                }
            }
            let flags = cpu.read8(e + E_FLAGS);
            if flags & DONE != 0 && cpu.read32(t + CURRENT) == i && flags & BARRIER == 0 {
                cpu.write32(t + CURRENT, i.wrapping_add(1));
            }
        }
        let flags = cpu.read8(e + E_FLAGS);
        if flags & BARRIER != 0 {
            if cpu.read32(t + CURRENT) == i && flags & ENDS != 0 {
                cpu.write8(t + OBJ_MODE, ENDING);
            }
            break;
        }
        i = i.wrapping_add(1);
        e = cpu.read32(t + EVENTS).wrapping_add(i.wrapping_mul(EVENT));
    }
    let f = cpu.read32(t + FRAME).wrapping_add(1);
    cpu.write32(t + FRAME, f);
}

/// Makes a timeline (list 0) running the events at `events` from its start. It, or 0 (also for
/// no events).
const TIMELINE_SIZE: u32 = 0x20;
const TIMELINE_LIST: u32 = 0;
const TIMELINE_EXEC: u32 = 0x0605_72ec;

pub fn timeline_set(cpu: &mut Cpu, events: u32) -> u32 {
    if events == 0 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, TIMELINE_SIZE)) as i8 as i32 as u32;
    let t = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, TIMELINE_LIST, blocks, TIMELINE_EXEC));
    if t != 0 {
        cpu.write32(t + EVENTS, events);
        for field in [CURRENT, FRAME, FRAME + 4] {
            cpu.write32(t + field, 0);
        }
    }
    t
}
