//! The SMPC (System Manager and Peripheral Control) at 0x00100000, byte registers on odd addresses:
//! commands (resets, the slave and sound CPUs' switches, the dot clock, INTBACK), the clock (RTC), the
//! area code, and the two parallel ports. On the ST-V the players' inputs come through the I/O chip,
//! so INTBACK's peripheral phase returns nothing, and the ports are wired to the board: port 1's bits
//! 2-4 to the settings EEPROM's CS, SK and DI; port 2's bit 0 from its DO, bits 3 and 4 to the SCSP's
//! and the sound CPU's resets (the sound CPU runs while both are low). From the SMPC user's manual
//! and the oracle's records.

use crate::eeprom::Eeprom;

/// What a finished command asks the machine to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    None,
    /// SYSRES: reset the machine (the master restarts at the manual-reset vector).
    SystemReset,
    SlaveOn,
    SlaveOff,
    SoundOn,
    SoundOff,
    /// CKCHG352 / CKCHG320: the dot clock changes (and the slave stops).
    Clock352,
    Clock320,
    Nmi,
}

#[derive(Clone, Debug)]
pub struct Smpc {
    pub ireg: [u8; 7],
    pub comreg: u8,
    pub oreg: [u8; 32],
    pub sr: u8,
    pub sf: u8,
    /// Each port's output data and direction, for SMPC control mode (0) and direct mode (1).
    pub data_out: [[u8; 2]; 2],
    pub data_dir: [[u8; 2]; 2],
    pub direct: [bool; 2],
    pub exle: [bool; 2],
    /// What each port's pins read.
    pub port_state: [u8; 2],
    pub eeprom: Eeprom,
    /// Port 2's sound control bits (3: SCSP reset, 4: sound CPU reset), and changes not yet acted on.
    pub sctrl: u8,
    pub sctrl_events: Vec<(u8, u8)>,
    bus_buffer: u8,
    /// The command under way and the cycles until it finishes.
    pub busy: Option<(u8, u32)>,
    /// INTBACK's peripheral phase, waiting for "continue".
    pub intback_waiting: bool,
    pub area: u8,
    pub dotsel: bool,
    pub reset_enabled: bool,
    pub sound_on: bool,
    pub smem: [u8; 4],
    /// The clock: seconds since 1994-01-01 00:00:00 (a Saturday), and cycles into the second.
    pub rtc_seconds: u64,
    pub rtc_cycles: u64,
}

impl Smpc {
    pub fn new(area: u8, eeprom: Option<&[u8]>) -> Smpc {
        let mut s = Smpc {
            ireg: [0; 7],
            comreg: 0,
            oreg: [0; 32],
            sr: 0,
            sf: 0,
            data_out: [[0; 2]; 2],
            data_dir: [[0; 2]; 2],
            direct: [false; 2],
            exle: [false; 2],
            port_state: [0x7F; 2],
            eeprom: Eeprom::new(eeprom),
            sctrl: 0xFF,
            sctrl_events: Vec::new(),
            bus_buffer: 0,
            busy: None,
            intback_waiting: false,
            area,
            dotsel: false,
            reset_enabled: true,
            sound_on: false,
            smem: [0; 4],
            rtc_seconds: 0,
            rtc_cycles: 0,
        };
        s.update_port(0);
        s.update_port(1);
        s
    }

    /// Drives a port's pins: lines the SMPC does not drive float high; the board answers on the rest.
    fn update_port(&mut self, p: usize) {
        let d = self.direct[p] as usize;
        let asserted = self.data_dir[p][d] & 0x7F;
        let out = (self.data_out[p][d] | !self.data_dir[p][d]) & 0x7F;
        let pins = if p == 0 {
            self.eeprom.update(out & 0x04 != 0, out & 0x08 != 0, out & 0x10 != 0);
            0x7F & !0x23
        } else {
            let sctrl = out & 0x18;
            if sctrl != self.sctrl {
                self.sctrl_events.push((self.sctrl, sctrl));
                self.sctrl = sctrl;
            }
            (0x7F & !1) | self.eeprom.dout as u8
        };
        self.port_state[p] = (out & asserted) | (pins & !asserted);
    }

    pub fn read(&mut self, a: u32) -> u8 {
        let r = (a >> 1) & 0x3F;
        match r {
            0x10..=0x2F => self.oreg[(r - 0x10) as usize],
            0x30 => self.sr,
            0x31 => (self.bus_buffer & !1) | self.sf,
            0x3A | 0x3B => {
                let p = (r - 0x3A) as usize;
                self.update_port(p);
                (self.bus_buffer & 0x80) | self.port_state[p]
            }
            _ => self.bus_buffer,
        }
    }

    /// A register write; `cycles_per_ms` scales command durations.
    pub fn write(&mut self, a: u32, v: u8, cycles_per_ms: u32) {
        self.bus_buffer = v;
        let r = (a >> 1) & 0x3F;
        match r {
            0x00..=0x06 => {
                self.ireg[r as usize] = v;
                if r == 0 && self.intback_waiting && self.busy.is_none() {
                    if v & 0x40 != 0 {
                        // break
                        self.intback_waiting = false;
                        self.sr &= !0x20;
                    } else if v & 0x80 != 0 {
                        // continue: the peripheral phase
                        self.busy = Some((0x10 | 0x80, cycles_per_ms / 4));
                    }
                }
            }
            0x0F => {
                self.comreg = v;
                let duration = match v {
                    0x10 => cycles_per_ms / 4,
                    0x0D..=0x0F => cycles_per_ms / 10,
                    _ => cycles_per_ms / 40,
                };
                self.busy = Some((v, duration.max(1)));
            }
            0x31 => self.sf |= v & 1,
            0x3A | 0x3B => {
                let p = (r - 0x3A) as usize;
                self.data_out[p][1] = v & 0x7F;
                self.update_port(p);
            }
            0x3C | 0x3D => {
                let p = (r - 0x3C) as usize;
                self.data_dir[p][1] = v & 0x7F;
                self.update_port(p);
            }
            0x3E => {
                self.direct = [v & 1 != 0, v & 2 != 0];
                self.update_port(0);
                self.update_port(1);
            }
            0x3F => self.exle = [v & 1 != 0, v & 2 != 0],
            _ => {}
        }
    }

    /// Advances `cycles`: the clock, and a command under way. Returns what a finished command asks
    /// for, and whether it raises the SMPC interrupt.
    pub fn clock(&mut self, cycles: u32, cycles_per_second: u64) -> (Effect, bool) {
        self.rtc_cycles += cycles as u64;
        while self.rtc_cycles >= cycles_per_second {
            self.rtc_cycles -= cycles_per_second;
            self.rtc_seconds += 1;
        }
        let Some((cmd, left)) = self.busy else {
            return (Effect::None, false);
        };
        if left > cycles {
            self.busy = Some((cmd, left - cycles));
            return (Effect::None, false);
        }
        self.busy = None;
        self.finish(cmd)
    }

    fn finish(&mut self, cmd: u8) -> (Effect, bool) {
        if cmd == 0x90 {
            // INTBACK's peripheral phase: the ST-V's ports have no peripherals to report
            self.intback_waiting = false;
            for o in &mut self.oreg[..31] {
                *o = 0;
            }
            self.sr = 0xC0 | 0x0F;
            self.sf = 0;
            return (Effect::None, true);
        }
        self.oreg[31] = cmd;
        let effect = match cmd {
            0x02 => Effect::SlaveOn,
            0x03 => Effect::SlaveOff,
            0x06 => {
                self.sound_on = true;
                Effect::SoundOn
            }
            0x07 => {
                self.sound_on = false;
                Effect::SoundOff
            }
            0x0D => Effect::SystemReset,
            0x0E => {
                self.dotsel = true;
                Effect::Clock352
            }
            0x0F => {
                self.dotsel = false;
                Effect::Clock320
            }
            0x10 => {
                self.intback_status();
                self.sf = 0;
                return (Effect::None, true);
            }
            0x16 => {
                self.set_time();
                Effect::None
            }
            0x17 => {
                self.smem.copy_from_slice(&self.ireg[..4]);
                Effect::None
            }
            0x18 => Effect::Nmi,
            0x19 => {
                self.reset_enabled = true;
                Effect::None
            }
            0x1A => {
                self.reset_enabled = false;
                Effect::None
            }
            _ => Effect::None,
        };
        self.sf = 0;
        (effect, false)
    }

    fn intback_status(&mut self) {
        let peripherals = self.ireg[1] & 0x08 != 0;
        if self.ireg[0] & 1 != 0 {
            self.oreg[0] = 0x80 | if self.reset_enabled { 0 } else { 0x40 };
            let t = self.rtc_bcd();
            self.oreg[1..8].copy_from_slice(&t);
            self.oreg[8] = 0; // cartridge code
            self.oreg[9] = self.area;
            self.oreg[10] = 0x34 | ((self.dotsel as u8) << 6) | 0x08 | 0x02 | (!self.sound_on as u8);
            self.oreg[11] = 0x42;
            self.oreg[12..16].copy_from_slice(&self.smem);
        }
        self.sr = 0x40 | if peripherals { 0x20 } else { 0 } | 0x0F;
        self.intback_waiting = peripherals;
    }

    /// The clock as INTBACK reports it: year (two BCD bytes), day of week and month, day, hour,
    /// minute, second.
    fn rtc_bcd(&self) -> [u8; 7] {
        let bcd = |v: u64| (((v / 10) << 4) | (v % 10)) as u8;
        let mut days = self.rtc_seconds / 86400;
        let secs = self.rtc_seconds % 86400;
        let dow = (6 + days) % 7;
        let mut year = 1994;
        loop {
            let len = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) { 366 } else { 365 };
            if days < len {
                break;
            }
            days -= len;
            year += 1;
        }
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let months = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        let mut month = 0;
        while days >= months[month] {
            days -= months[month];
            month += 1;
        }
        [
            bcd(year / 100),
            bcd(year % 100),
            ((dow as u8) << 4) | (month as u8 + 1),
            bcd(days + 1),
            bcd(secs / 3600),
            bcd(secs / 60 % 60),
            bcd(secs % 60),
        ]
    }

    /// SETTIME: IREG0-6 hold the time in INTBACK's layout.
    fn set_time(&mut self) {
        let unbcd = |b: u8| ((b >> 4) * 10 + (b & 0xF)) as u64;
        let year = unbcd(self.ireg[0]) * 100 + unbcd(self.ireg[1]);
        let month = (self.ireg[2] & 0xF) as u64;
        let mut days = 0u64;
        for y in 1994..year {
            days += if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) { 366 } else { 365 };
        }
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let months = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        for m in months.iter().take(month.saturating_sub(1).min(12) as usize) {
            days += m;
        }
        days += unbcd(self.ireg[3]).saturating_sub(1);
        self.rtc_seconds = days * 86400 + unbcd(self.ireg[4]) * 3600 + unbcd(self.ireg[5]) * 60 + unbcd(self.ireg[6]);
    }
}
