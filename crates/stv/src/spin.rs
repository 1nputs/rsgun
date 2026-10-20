//! Spinning: an SH-2 waiting in a loop that only reads memory nothing changes while it runs (a flag
//! the other CPU or an interrupt will set) goes round the same states until something does. The
//! machine records one turn of such a loop and plays it forward without executing it, which changes
//! nothing but the time the host takes: the CPU's states, its steps and its timer are the ones it
//! would have had (`stv-check spin` compares a machine that does this with one that does not).
//!
//! A loop qualifies when a short backward branch comes back to the same target with the CPU's whole
//! state the same and nothing in between but pure reads: work RAM, ROM, the cache's data array and
//! the FRT's status register (which holds while the FRT reaches no event). While the turn is recorded
//! and while it plays, the other CPU's writes to anything it read (its instructions included), or to
//! anything but work RAM, stop it; so does the end of `Machine::run_cpus`, as the rest of the
//! machine runs between those calls.

/// What one CPU's bus accesses tell the loop watch.
#[derive(Clone, Debug, Default)]
pub struct SpinBus {
    /// The CPU is being watched: its accesses are classified.
    pub watching: bool,
    /// Something other than a pure read happened since the watch last looked.
    pub impure: bool,
    /// Recording a turn: the work RAM it reads (as [`ram_key`] ranges), instructions included.
    pub recording: bool,
    pub reads: Vec<(u32, u32)>,
    /// The turn read the FRT's status register.
    pub reads_ftcsr: bool,
    /// The loop is playing: the other CPU's writes are checked against `reads`.
    pub spinning: bool,
    /// The other CPU wrote something the loop may read: the playing stops.
    pub broken: bool,
}

/// FTCSR, the FRT's control/status register.
const FTCSR: u32 = 0xFFFF_FE11;

impl SpinBus {
    /// A data read by the watched CPU.
    pub fn read(&mut self, a: u32, size: u32) {
        match a >> 29 {
            0 | 1 => {
                if let Some(k) = ram_key(a) {
                    if self.recording {
                        self.reads.push((k, k + size));
                    }
                } else {
                    let p = a & 0x07FF_FFFF;
                    let rom = p < 0x0010_0000 || (0x0200_0000..0x04FF_FFF0).contains(&p);
                    if !rom {
                        self.impure = true;
                    }
                }
            }
            6 => {} // the cache's data array: only this CPU reaches it
            7 if a == FTCSR && size == 1 => self.reads_ftcsr = true,
            _ => self.impure = true,
        }
    }

    /// An instruction fetch by the watched CPU.
    pub fn fetch(&mut self, a: u32) {
        if self.recording
            && let Some(k) = ram_key(a)
        {
            self.reads.push((k, k + 2));
        }
    }

    /// A write by the other CPU while this one's loop plays (or a turn of it is recorded).
    pub fn other_wrote(&mut self, a: u32, size: u32) {
        if a >> 29 == 6 {
            return;
        }
        match ram_key(a) {
            Some(k) => {
                if self.reads.iter().any(|&(s, e)| k < e && s < k + size) {
                    self.broken = true;
                }
            }
            None => self.broken = true,
        }
    }

    /// Back to unwatched.
    pub fn clear(&mut self) {
        self.watching = false;
        self.impure = false;
        self.recording = false;
        self.reads.clear();
        self.reads_ftcsr = false;
        self.spinning = false;
        self.broken = false;
    }
}

/// Work RAM's address as one key for all its mirrors and cache areas; None for anything else.
pub fn ram_key(a: u32) -> Option<u32> {
    if a >> 29 > 1 {
        return None;
    }
    let p = a & 0x07FF_FFFF;
    match p {
        0x0020_0000..=0x002F_FFFF => Some(0x0020_0000 | (p & 0xF_FFFF)),
        0x0600_0000..=0x07FF_FFFF => Some(0x0600_0000 | (p & 0xF_FFFF)),
        _ => None,
    }
}
