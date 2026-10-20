//! The boot state: the machine as the BIOS hands over to the game, so that a run starts there
//! instead of going through the BIOS's boot (about 19 seconds). It is made the first time a run
//! passes the handover with nothing pressed (the machine's state at the last frame's end before
//! it), kept beside the EEPROM and backup RAM, and made again when the ROMs or the EEPROM it started
//! from change.
//!
//! The backup RAM is not taken from it: the run goes on with the backup RAM as saved. Checked with
//! play1 to frame 4000: started so with another backup RAM, a run matches the run from power on
//! with that backup RAM but for two bytes, the BIOS's count of the time it has been on, which misses
//! the boot's frames.

use std::fs;
use std::path::PathBuf;
use stv::machine::Machine;

/// Where the BIOS hands over: the game's entry, `_main` (the linker map's fragment in the cart).
const HANDOVER: u32 = 0x0600_4000;
/// The file's start (and version).
const MAGIC: &[u8; 8] = b"RSGBOOT1";
/// How long a run is watched for the handover before it is taken not to come (the BIOS's test mode).
const WATCH_FRAMES: u64 = 3600;

pub struct Boot {
    path: PathBuf,
    key: u64,
    /// Watching the run for the handover, and the state at the last frame's end before it (and
    /// room for the next).
    watching: bool,
    last: Option<Vec<u8>>,
    next: Vec<u8>,
}

impl Boot {
    /// For a machine at power on (`key`: what the boot depends on that the save state does not
    /// check, the EEPROM's contents): starts it at the handover, from the boot state at `path` if
    /// one was made from the same; else watches its run to make one.
    pub fn start(m: &mut Machine, path: PathBuf, key: u64) -> Boot {
        let mut boot = Boot { path, key, watching: false, last: None, next: Vec::new() };
        if let Ok(data) = fs::read(&boot.path) {
            let backup = m.sys.backup.clone();
            match boot.take(m, &data) {
                Ok(()) => {
                    m.sys.backup = backup;
                    return boot;
                }
                Err(e) => eprintln!("rsgun: {}: {e}: booting from power on", boot.path.display()),
            }
        }
        boot.watching = true;
        boot.last = m.save_state().ok();
        m.breakpoint = Some((0, HANDOVER));
        boot
    }

    fn take(&self, m: &mut Machine, data: &[u8]) -> Result<(), String> {
        if data.len() < 16 || &data[..8] != MAGIC {
            return Err("not a boot state".into());
        }
        if data[8..16] != self.key.to_le_bytes() {
            return Err("made with other EEPROM settings".into());
        }
        m.load_state(&data[16..])
    }

    /// After each frame (`idle`: nothing was pressed): keeps the state at its end, until the
    /// handover comes; then writes the state before it.
    pub fn frame(&mut self, m: &mut Machine, idle: bool) {
        if !self.watching {
            return;
        }
        if m.reached.is_some() || !idle || m.sys.frame > WATCH_FRAMES {
            self.watching = false;
            m.breakpoint = None;
            if let (Some(_), true, Some(state)) = (m.reached, idle, self.last.take()) {
                let mut data = MAGIC.to_vec();
                data.extend_from_slice(&self.key.to_le_bytes());
                data.extend_from_slice(&state);
                let written = self.path.parent().map_or(Ok(()), fs::create_dir_all).and_then(|()| fs::write(&self.path, data));
                match written {
                    Ok(()) => eprintln!("rsgun: boot state saved to {} (later runs start at the game)", self.path.display()),
                    Err(e) => eprintln!("rsgun: {}: {e}", self.path.display()),
                }
            }
            self.last = None;
            self.next = Vec::new();
            return;
        }
        if m.save_state_into(&mut self.next).is_ok() {
            let spent = self.last.replace(std::mem::take(&mut self.next));
            self.next = spent.unwrap_or_default();
        }
    }
}
