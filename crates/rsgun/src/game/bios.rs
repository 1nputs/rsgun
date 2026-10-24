//! The ST-V BIOS's services as the game calls them after the BIOS hands over (BIOS EPR-20091):
//! its code the BIOS copies into work RAM-H at 0x06000c00-0x06003000 (the service table at
//! 0x06000200-0x06000800 points into it: interrupts, the SCU's mask, the vertical blank's hook,
//! coins and credits, continues, the backup RAM) and the few routines the game reaches in the
//! BIOS ROM (the operating time's count, an empty hook, the backup RAM's helpers). The machine's
//! code, not the game's: read where the BIOS puts it, never copied into the source.
//!
//! The exception stubs (0x06001f48-0x06001ff8) are natives of the exception kind (stv::native):
//! entered as the CPU takes an interrupt, they end with `rte`.
//!
//! Left to the SH-2 code: the code that never runs in a game (the pad modes the cabinet's setting
//! does not select, the service menu's helpers, the stubs of vectors no game uses).

use super::system::bios;
use super::{nested, Cpu};
use stv::sh2::{Bus, Sh2, SR_MASK};

/// The BIOS's work area.
const DATA: u32 = 0x0600_0700;
/// The four pads' records (8 bytes: the buttons now, before, newly pressed, newly released), the
/// cabinet's switches' record after them.
const PADS: u32 = DATA + 0x08;
const SWITCHES: u32 = DATA + 0x28;
/// The coin state, a 4-byte entry for each coin slot (credits, coins, coins a credit takes, and the
/// state: 0 nothing, 1 coins in but short, 2 a credit to play with), then the settings byte and the
/// byte after it (17 or 18). The game's copy is system.rs's COINS.
const COINS: u32 = DATA + 0x40;
const COIN_SETTINGS: u32 = DATA + 0x50;
const COIN_SETTINGS_2: u32 = DATA + 0x51;
/// Coin pulses seen for each slot, not yet counted.
const COIN_PULSES: u32 = DATA + 0x54;
/// The ring of slots whose coins were counted (tail, head, the 16 entries), and the ring of the
/// events the game reads (1 a coin, 2 a credit; the same).
const COUNTED_TAIL: u32 = DATA + 0x5c;
const COUNTED_HEAD: u32 = DATA + 0x5d;
const COUNTED_RING: u32 = DATA + 0x60;
const EVENTS_TAIL: u32 = DATA + 0x5e;
const EVENTS_HEAD: u32 = DATA + 0x5f;
const EVENTS_RING: u32 = DATA + 0x70;
/// The settings the coin code reads: the coins a credit takes (+0x91), the credits a play takes
/// (+0x90), a coin chute's value (+0x92, +0x93), the mode flags (+0x95: bit 0 free play, bit 1 a
/// continue costs two, bit 2 a slot for each player), and a count (+0x9a, bits 0-1 the last slot).
const CREDITS_PER_PLAY: u32 = DATA + 0x90;
const COINS_PER_CREDIT: u32 = DATA + 0x91;
const CHUTE_VALUE: u32 = DATA + 0x92;
const MODE_FLAGS: u32 = DATA + 0x95;
const SLOTS_WORD: u32 = DATA + 0x9a;
const FREE_PLAY: u32 = 1;
const CONTINUE_COSTS_TWO: u32 = 2;
const SLOT_EACH: u32 = 4;
/// Each chute's coins not yet worth a credit (16 bits), the counts of what the lamps show (bytes),
/// and their countdowns.
const CHUTE_COINS: u32 = DATA + 0x114;
const LAMP_STEPS: u32 = DATA + 0x11c;
const LAMP_COUNTDOWNS: u32 = DATA + 0x11e;
/// A word of the BIOS's flags (bit 7 set: the sound CPU's switches are to be read), and each
/// player's continue flag (the game sets it through the BIOS's function).
const FLAGS_124: u32 = DATA + 0x124;
const CONTINUING: u32 = DATA + 0x128;
/// Not 0 while the backup RAM's copy of the settings is not written back (the service sets it).
const DIRTY_SERVICE: u32 = DATA + 0x100;
const SERVICE_104: u32 = DATA + 0x104;
const SWITCHES_PRESSED: u32 = SWITCHES + 4;

/// Bytes and words of the BIOS's, below the work area.
const SCU_MASK_COPY: u32 = 0x0600_0348;
const IMS: u32 = 0x25fe_00a0;
const BIOS_SETTING: u32 = 0x0600_0248;
const REGION: u32 = 0x0600_0650;
const BACKUP_DIRTY: u32 = 0x0600_0653;
const PORT_BITS: u32 = 0x0600_0730;
const CABINET_FLAGS: u32 = 0x0600_0798;
const OPTION_FLAGS: u32 = 0x0600_0795;
const MODE_FLAGS_824: u32 = 0x0600_0824;
const VECTORS: u32 = 0x0600_0900;
const PRIORITIES: u32 = 0x0600_0a80;
const COINS_LIMITS: u32 = 0x0600_2810;
const PAD_HANDLERS: u32 = 0x0600_2850;
const REGION_TABLE: u32 = 0x0600_06b4;
/// The BIOS's table: its functions (their addresses).
const BIOS_SERVICE_0604: u32 = 0x0600_0604;
const BIOS_BACKUP_IO: u32 = 0x0600_0644;
const BIOS_MARK_DIRTY: u32 = 0x0600_0648;
const BIOS_CREDITS: u32 = 0x0600_0668;
/// The BIOS ROM's table of routines: the operating time's count, a routine the service tests,
/// another called until it answers 0.
const ROM_TIME: u32 = 0x10e8;
const ROM_SERVICE: u32 = 0x10e4;
const ROM_COIN_SOUND: u32 = 0x10f8;

/// The I/O board's ports: the three switch bytes, the two after them, the cabinet's (inverted into
/// the BIOS's words below), and the lamps' port.
const PORTS: [(u32, u32); 6] = [
    (0x2040_0001, 0x0600_2864),
    (0x2040_0003, 0x0600_2868),
    (0x2040_0005, 0x0600_286c),
    (0x2040_0009, 0x0600_2870),
    (0x2040_000b, 0x0600_2874),
    (0x2010_0077, 0x0600_2878),
];
const PORT_1: u32 = PORTS[0].1;
const PORT_2: u32 = PORTS[1].1;
const PORT_3: u32 = PORTS[2].1;
const PORT_4: u32 = PORTS[3].1;
const PORT_5: u32 = PORTS[4].1;
const PORT_6: u32 = PORTS[5].1;
const LAMPS: u32 = 0x2040_0007;
const SOUND_SWITCHES: u32 = 0x25a0_04f1;

/// The backup RAM (the cartridge slot's: 8 bits wide, on the odd bytes) and where the BIOS keeps
/// its counters in it.
const BACKUP: u32 = 0x2018_0000;
const BACKUP_COINS: u32 = BACKUP + 0x3d04;
const BACKUP_CREDITS: u32 = BACKUP + 0x3d5c;
const BACKUP_TOTAL: u32 = BACKUP + 0x3d2c;
const BACKUP_BY_CHUTE: u32 = BACKUP + 0x3d0c;
const BACKUP_SUMS: u32 = BACKUP + 0x3d34;
const BACKUP_PLAYS: u32 = BACKUP + 0x3d44;
const BACKUP_BY_SETTING: u32 = BACKUP + 0x3d80;
const BACKUP_CHECK: u32 = BACKUP + 0x3d00;
const BACKUP_MIRROR: u32 = BACKUP + 0x3e80;
const BACKUP_CHECKED_BYTES: u32 = 0x17c;
const BACKUP_MIRRORED: u32 = 96;
/// The backup function's kinds: a 16-bit value read (0) or written (2), a 32-bit one read (1) or
/// written (3).
const READ_LONG: u32 = 1;
const WRITE_WORD: u32 = 2;
const WRITE_LONG: u32 = 3;

/// The BIOS's checksum: CRC-16 (polynomial 0x1021) kept in the top half of a 32-bit register,
/// started from this.
const CRC_START: u32 = 0xdebd_eb00;
const CRC_POLYNOMIAL: u32 = 0x1021_0000;

/// 0x06000c00: sets the SCU's interrupt mask (and the BIOS's copy of it) to `mask`.
pub fn scu_mask_set(cpu: &mut Cpu, mask: u32) {
    cpu.write32(SCU_MASK_COPY, mask);
    cpu.write32(IMS, mask);
}

/// 0x06000c0a: the SCU's mask changed: the bits in `bits` set, then only those in `mask` kept.
pub fn scu_mask_change(cpu: &mut Cpu, mask: u32, bits: u32) {
    let m = (cpu.read32(SCU_MASK_COPY) | bits) & mask;
    cpu.write32(SCU_MASK_COPY, m);
    cpu.write32(IMS, m);
}

/// 0x06000c34 (the BIOS's table 0x06000618): the frame's work, then, where the cabinet's flag
/// asks for it, the coin sound for what the credits' ring says: for a coin (1) or a credit (2),
/// the ROM's routine is called until it answers 0.
pub fn service(cpu: &mut Cpu) {
    nested!(cpu, 0x0600_0d14, frame(cpu));
    if cpu.read32(MODE_FLAGS_824) & 0x40 == 0 {
        return;
    }
    let event = bios(cpu, BIOS_CREDITS, &[]);
    if event == 1 || event == 2 {
        loop {
            let f = cpu.read32(ROM_COIN_SOUND);
            if cpu.call(f, &[1, 0, 2, 0]) & 0xff == 0 {
                break;
            }
        }
    }
}

/// 0x06000d14 (the VBlank hook, the BIOS's table 0x06000610): the ports read, and unless the
/// service holds the BIOS: the ROM's time count, the coins counted, the coin state brought up to
/// date, the ROM's service routines called when a button of the cabinet asks, the backup RAM's
/// settings written back with a new checksum when they changed; then the lamps.
pub fn frame(cpu: &mut Cpu) {
    nested!(cpu, 0x0600_2098, read_ports(cpu));
    if cpu.read32(DIRTY_SERVICE) == 0 {
        let f = cpu.read32(ROM_TIME);
        cpu.call(f, &[]);
        nested!(cpu, 0x0600_1988, coin_frame(cpu));
        nested!(cpu, 0x0600_14fe, coin_states(cpu));
        if cpu.read16(SWITCHES_PRESSED) as u32 & 0x204 != 0 && cpu.read32(SERVICE_104) != 0 {
            if cpu.read8(REGION) == 4 {
                bios(cpu, BIOS_SERVICE_0604, &[]);
            }
            let region = cpu.read8(REGION) as u32;
            if cpu.read8(REGION_TABLE + region) & 1 == 0 {
                bios(cpu, BIOS_SERVICE_0604, &[]);
            }
        }
        if cpu.read32(CABINET_FLAGS) & 0x20 == 0 {
            let f = cpu.read32(ROM_SERVICE);
            if cpu.call(f, &[]) != 0 {
                cpu.call(0x0600_0e22, &[]);
            }
        }
        if cpu.read8(BACKUP_DIRTY) != 0 {
            let io = cpu.read32(BIOS_BACKUP_IO);
            let sum = nested!(cpu, 0x0600_13e6, crc_words(cpu, BACKUP_CHECK + 4, BACKUP_CHECKED_BYTES));
            cpu.call(io, &[WRITE_WORD, BACKUP_CHECK, sum & 0xffff]);
            for i in 0..BACKUP_MIRRORED {
                let v = cpu.read32(BACKUP_CHECK + 4 * i);
                cpu.write32(BACKUP_MIRROR + 4 * i, v);
            }
            cpu.write8(BACKUP_DIRTY, 0);
        }
    }
    lamps(cpu);
}

/// The lamps' port (the tail of 0x06000d14): its bits kept but the low two, which the blink steps
/// of the first two coin slots give.
fn lamps(cpu: &mut Cpu) {
    let kept = cpu.read8(LAMPS) as u32 & 0xfc;
    let mut v = kept;
    v |= nested!(cpu, 0x0600_1240, lamp_step(cpu, 0));
    v |= nested!(cpu, 0x0600_1240, lamp_step(cpu, 1));
    cpu.write8(LAMPS, v as u8);
}

/// 0x06001190: the backup RAM's settings changed (they are written back in the frame).
pub fn mark_dirty(cpu: &mut Cpu) {
    cpu.write8(BACKUP_DIRTY, 1);
}

/// 0x06001198 (the BIOS's table 0x06000668): the next event of the credits' ring (1 a coin, 2 a
/// credit), 0 if there is none.
pub fn next_event(cpu: &mut Cpu) -> u32 {
    let tail = cpu.read8(EVENTS_TAIL) as i8 as i32 as u32;
    let head = cpu.read8(EVENTS_HEAD) as i8 as i32 as u32;
    if head == tail {
        return 0;
    }
    cpu.write8(EVENTS_TAIL, (tail.wrapping_add(1) & 15) as u8);
    cpu.read8(EVENTS_RING.wrapping_add(tail)) as i8 as i32 as u32
}

/// 0x060011c8 (the BIOS's table 0x0600066c): the four bytes at `flags` (each player's continue
/// flag) copied to the BIOS's.
pub fn set_continuing(cpu: &mut Cpu, flags: u32) {
    for i in 0..4 {
        let b = cpu.read8(flags + i);
        cpu.write8(CONTINUING + i, b);
    }
}

/// 0x060011e6 (the BIOS's table 0x06000670): the next slot of the counted ring (the slot whose coins
/// were counted), -1 if there is none.
pub fn next_counted(cpu: &mut Cpu) -> u32 {
    let tail = cpu.read8(COUNTED_TAIL) as i8 as i32 as u32;
    let head = cpu.read8(COUNTED_HEAD) as i8 as i32 as u32;
    if head == tail {
        return u32::MAX;
    }
    cpu.write8(COUNTED_TAIL, (tail.wrapping_add(1) & 15) as u8);
    cpu.read8(COUNTED_RING.wrapping_add(tail)) as i8 as i32 as u32
}

/// 0x06001240: one step of `slot`'s lamp: a countdown (+0x11e) run down, else the lamp's count
/// (+0x11c) down by one and the countdown restarted at 15; the lamp's bit shifted to the slot's
/// place (bit 3 of the countdown to bit `slot`).
pub fn lamp_step(cpu: &mut Cpu, slot: u32) -> u32 {
    let countdown = LAMP_COUNTDOWNS + slot;
    let steps = LAMP_STEPS + slot;
    let c = cpu.read8(countdown);
    if c != 0 {
        cpu.write8(countdown, c.wrapping_sub(1));
    } else if cpu.read8(steps) != 0 {
        let s = cpu.read8(steps);
        cpu.write8(steps, s.wrapping_sub(1));
        cpu.write8(countdown, 15);
    }
    let bit = cpu.read8(countdown) as i8 as i32 as u32 & 8;
    shift_right_signed(cpu, bit, 3u32.wrapping_sub(slot))
}

/// 0x060013bc: the BIOS's checksum of `count` bytes at `at`.
pub fn crc_bytes(cpu: &mut Cpu, at: u32, count: u32) -> u32 {
    let mut crc = CRC_START;
    let (mut at, mut count) = (at, count);
    loop {
        crc |= cpu.read8(at) as u32;
        at = at.wrapping_add(1);
        for _ in 0..8 {
            crc = crc_step(crc);
        }
        count = count.wrapping_sub(1);
        if count == 0 {
            break;
        }
    }
    for _ in 0..8 {
        crc = crc_step(crc);
    }
    crc >> 16
}

/// 0x060013e6: the same over the low bytes of `bytes / 2` 16-bit reads from `at` (the backup RAM's
/// bytes).
pub fn crc_words(cpu: &mut Cpu, at: u32, bytes: u32) -> u32 {
    let mut crc = CRC_START;
    let (mut at, mut count) = (at, bytes >> 1);
    loop {
        crc |= cpu.read16(at) as u8 as u32;
        at = at.wrapping_add(2);
        for _ in 0..8 {
            crc = crc_step(crc);
        }
        count = count.wrapping_sub(1);
        if count == 0 {
            break;
        }
    }
    for _ in 0..8 {
        crc = crc_step(crc);
    }
    crc >> 16
}

fn crc_step(crc: u32) -> u32 {
    let top = crc >> 31;
    let crc = crc << 1;
    if top != 0 { crc ^ CRC_POLYNOMIAL } else { crc }
}

/// 0x06001412 (the BIOS's table 0x06000644): the backup RAM's access `kind` (0 and 1 read 16 and
/// 32 bits, 2 and 3 write them) at `at` (`value` for a write): the routine from the table in the
/// code (0x0600141c) does it.
pub fn backup_io(cpu: &mut Cpu, kind: u32, at: u32, value: u32) -> u32 {
    let routine = cpu.read32(0x0600_141c + 4 * kind);
    // the SH-2 code's `mova` leaves r0 at the table, which a routine that sets no r0 hands back
    cpu.leave(&[(0, 0x0600_141c)]);
    cpu.call(routine, &[kind << 2, at, value])
}

/// 0x0600142c: the 16-bit value in the odd bytes from `at`+1 (the first the high byte).
pub fn backup_read_word(cpu: &mut Cpu, _kind: u32, at: u32) -> u32 {
    let high = cpu.read8(at + 1) as u32;
    let low = cpu.read8(at + 3) as u32;
    high << 8 | low
}

/// 0x06001458: the low 16 bits of `value` written to the odd bytes from `at`+1 (the high byte first).
pub fn backup_write_word(cpu: &mut Cpu, _kind: u32, at: u32, value: u32) {
    cpu.write8(at + 3, value as u8);
    cpu.write8(at + 1, (value >> 8) as u8);
}

/// 0x0600143e: the 32-bit value in the odd bytes from `at`+1 (the first the highest).
pub fn backup_read_long(cpu: &mut Cpu, _kind: u32, at: u32) -> u32 {
    let mut v = cpu.read8(at + 1) as u32;
    for i in 1..4 {
        v = v << 8 | cpu.read8(at + 1 + 2 * i) as u32;
    }
    v
}

/// 0x06001464: `value` written to the odd bytes from `at`+1 (the first the highest). Leaves 0.
pub fn backup_write_long(cpu: &mut Cpu, _kind: u32, at: u32, value: u32) -> u32 {
    for i in 0..4 {
        cpu.write8(at + 7 - 2 * i, (value >> (8 * i)) as u8);
    }
    0
}

/// 0x060014a8 (the BIOS's table 0x06000300): the handler for interrupt `vector` set in the BIOS's
/// table (the default, a plain return from the exception, where `handler` is 0).
pub fn set_interrupt(cpu: &mut Cpu, vector: u32, handler: u32) {
    let handler = if handler == 0 { 0x0600_2052 } else { handler };
    cpu.write32(VECTORS + 4 * vector, handler);
}

/// 0x060014c0 (the BIOS's table 0x06000280): the 32 interrupt priorities at `from` copied to the
/// BIOS's table, interrupts masked meanwhile.
pub fn set_priorities(cpu: &mut Cpu, from: u32) {
    let sr = cpu.sh2.sr;
    cpu.sh2.sr = 0xf0;
    for i in 0..32 {
        let v = cpu.read32(from + 4 * i);
        cpu.write32(PRIORITIES + 4 * i, v);
    }
    cpu.sh2.sr = sr;
}

/// 0x060014e0 (the BIOS's table 0x06000660): the BIOS's coin state (system.rs's COINS) copied to
/// `to`.
pub fn coins_copy(cpu: &mut Cpu, to: u32) {
    for i in 0..20 {
        let b = cpu.read8(COINS + i);
        cpu.write8(to + i, b);
    }
}

/// 0x060014fe: the coin state of each slot brought up to date from the backup RAM's counts: the
/// settings byte (the slots' count and the mode), and for a slot its coins (the backup RAM's odd
/// byte at 0x3d05+2n), credits (0x3d5d+2n), the credits a play takes, and the state byte (0
/// nothing, 1 coins in but short, 2 a credit to play with; 2 on free play).
pub fn coin_states(cpu: &mut Cpu) {
    let last = cpu.read16(SLOTS_WORD) as u32 & 3;
    let flags = cpu.read8(MODE_FLAGS) as i8 as i32 as u32;
    let mode = if flags & FREE_PLAY != 0 { flags & 5 } else { flags & 7 };
    cpu.write8(COIN_SETTINGS, (last << 3 | mode) as u8);
    let mut coins_at = BACKUP_COINS + 1;
    cpu.write8(COIN_SETTINGS_2, if mode & 2 != 0 { 18 } else { 17 });
    if cpu.read8(MODE_FLAGS) as u32 & SLOT_EACH != 0 {
        for slot in 0..=last {
            let mut extra = 1;
            let at = COINS + 4 * slot;
            let coins = cpu.read8(coins_at);
            coins_at += 2;
            cpu.write8(at, coins);
            let coins = cpu.read8(at) as u32;
            let credits = cpu.read8(BACKUP_CREDITS + 1 + 2 * slot);
            cpu.write8(at + 1, credits);
            let credits = credits as u32;
            let per_play = cpu.read8(CREDITS_PER_PLAY);
            cpu.write8(at + 2, per_play);
            let state = if cpu.read8(MODE_FLAGS) as u32 & FREE_PLAY != 0 {
                2
            } else if credits | coins == 0 {
                0
            } else {
                if cpu.read8(MODE_FLAGS) as u32 & CONTINUE_COSTS_TWO != 0 && cpu.read8(CONTINUING + slot) == 0 {
                    extra += 1;
                }
                if extra > coins { 1 } else { 2 }
            };
            cpu.write8(at + 3, state);
        }
    } else {
        let coins = cpu.read8(coins_at);
        cpu.write8(COINS, coins);
        let coins = cpu.read8(COINS) as u32;
        let credits = cpu.read8(BACKUP_CREDITS + 1);
        cpu.write8(COINS + 1, credits);
        let credits = credits as u32;
        let per_play = cpu.read8(CREDITS_PER_PLAY);
        cpu.write8(COINS + 2, per_play);
        for slot in 0..=last {
            let mut extra = 1;
            let state = if cpu.read8(MODE_FLAGS) as u32 & FREE_PLAY != 0 {
                2
            } else if coins | credits == 0 {
                0
            } else {
                if cpu.read8(MODE_FLAGS) as u32 & CONTINUE_COSTS_TWO != 0 && cpu.read8(CONTINUING + slot) == 0 {
                    extra += 1;
                }
                if extra > coins { 1 } else { 2 }
            };
            cpu.write8(COINS + 4 * slot + 3, state);
        }
    }
}

/// 0x060016b0 (the BIOS's table 0x06000664): takes a credit for `slot`'s player if it has one to
/// play with (state 2): the backup RAM's play counts up, the coins (or the continue's credits)
/// down. 1 if taken, else 0.
pub fn use_credit(cpu: &mut Cpu, slot: u32) -> u32 {
    nested!(cpu, 0x0600_14fe, coin_states(cpu));
    if cpu.read8(COINS + 4 * slot + 3) != 2 {
        return 0;
    }
    let region = cpu.read8(REGION) as u32;
    let by_setting = BACKUP_BY_SETTING + (region << 6);
    let counted = by_setting + 12;
    count_up(cpu, BACKUP_PLAYS);
    count_up(cpu, by_setting + 4);
    let flags = cpu.read8(MODE_FLAGS) as i8 as i32 as u32;
    let chute = if flags & SLOT_EACH != 0 { slot } else { 0 };
    if flags & FREE_PLAY != 0 {
        if cpu.read8(CONTINUING + slot) != 0 {
            count_up(cpu, counted);
        }
    } else {
        if cpu.read8(MODE_FLAGS) as u32 & SLOT_EACH == 0 {
            cpu.write16(CHUTE_COINS, 0);
            cpu.write16(CHUTE_COINS + 2, 0);
        } else {
            cpu.write16(CHUTE_COINS + 2 * slot, 0);
        }
        let coins = BACKUP_COINS + 1 + 2 * chute;
        if cpu.read8(CONTINUING + slot) == 0 {
            let v = cpu.read8(coins);
            cpu.write8(coins, v.wrapping_sub(1));
            if cpu.read8(MODE_FLAGS) as u32 & CONTINUE_COSTS_TWO != 0 {
                let v = cpu.read8(coins);
                cpu.write8(coins, v.wrapping_sub(1));
            }
        } else {
            let v = cpu.read8(coins);
            cpu.write8(coins, v.wrapping_sub(1));
            count_up(cpu, counted);
        }
    }
    bios(cpu, BIOS_MARK_DIRTY, &[]);
    1
}

/// The 32-bit count at `at` in the backup RAM up by one (a count that is -1 stays).
fn count_up(cpu: &mut Cpu, at: u32) {
    let v = bios(cpu, BIOS_BACKUP_IO, &[READ_LONG, at, 0]);
    let v = nested!(cpu, 0x0600_136a, super::system::next_unless_end(cpu, v));
    bios(cpu, BIOS_BACKUP_IO, &[WRITE_LONG, at, v]);
}

/// 0x06001860: a coin pulse of `slot` taken, if there is one: the count down, the slot put in the
/// counted ring (if it is not full; the count is down anyway). 1 if there was one.
pub fn take_pulse(cpu: &mut Cpu, slot: u32) -> u32 {
    let pulses = COIN_PULSES + slot;
    if cpu.read8(pulses) == 0 {
        return 0;
    }
    let head = cpu.read8(COUNTED_HEAD) as i8 as i32 as u32;
    let n = cpu.read8(pulses);
    cpu.write8(pulses, n.wrapping_sub(1));
    let next = head.wrapping_add(1) & 15;
    if next != cpu.read8(COUNTED_TAIL) as i8 as i32 as u32 {
        cpu.write8(COUNTED_RING.wrapping_add(head), slot as u8);
        cpu.write8(COUNTED_HEAD, next as u8);
    }
    1
}

/// 0x060018ac: the cabinet's coin switches (the switches' record's newly released bits: bit 0, 1,
/// 11 and 12) counted into the slots' pulse counts. `slots` is the cabinet setting's (low 2 bits of
/// 0x06000798): the switch bits of slots past it are not looked at, and those slots' counts are
/// cleared (all but the first when the mode has no slot for each player).
pub fn coin_pulses(cpu: &mut Cpu, slots: u32) {
    let released = cpu.read16(SWITCHES + 6) as i16 as i32 as u32 & 0xffff;
    let bump = |cpu: &mut Cpu, at: u32| {
        let v = cpu.read8(at);
        cpu.write8(at, v.wrapping_add(1));
    };
    if released & 1 != 0 {
        bump(cpu, COIN_PULSES);
    }
    if cpu.read8(MODE_FLAGS) as u32 & SLOT_EACH == 0 {
        if released & 2 != 0 {
            bump(cpu, COIN_PULSES + 1);
        }
        cpu.write8(COIN_PULSES + 2, 0);
        cpu.write8(COIN_PULSES + 3, 0);
        return;
    }
    match slots {
        0 => {
            for i in 1..4 {
                cpu.write8(COIN_PULSES + i, 0);
            }
        }
        1 => {
            if released & 2 != 0 {
                bump(cpu, COIN_PULSES + 1);
            }
            cpu.write8(COIN_PULSES + 2, 0);
            cpu.write8(COIN_PULSES + 3, 0);
        }
        2 => {
            if released & 2 != 0 {
                bump(cpu, COIN_PULSES + 1);
            }
            if released & 0x800 != 0 {
                bump(cpu, COIN_PULSES + 2);
            }
            cpu.write8(COIN_PULSES + 3, 0);
        }
        3 => {
            if released & 2 != 0 {
                bump(cpu, COIN_PULSES + 1);
            }
            if released & 0x800 != 0 {
                bump(cpu, COIN_PULSES + 2);
            }
            if released & 0x1000 != 0 {
                bump(cpu, COIN_PULSES + 3);
            }
        }
        _ => {}
    }
}

/// 0x06001988: the coin pulses counted, then each slot's pulse taken and counted in (06001bae),
/// the slots a slot for each player or the first two chutes; a service button asks the coin
/// helper (0x06001dec); the coins and credits of the frame added to the backup RAM's totals.
pub fn coin_frame(cpu: &mut Cpu) {
    let slots = cpu.read32(CABINET_FLAGS) & 3;
    nested!(cpu, 0x0600_18ac, coin_pulses(cpu, slots));
    let each = cpu.read8(COINS + 16) as u32 & SLOT_EACH != 0;
    // the frame's coins and credits (two longs on the stack)
    cpu.push(&[0, 0]);
    let counters = cpu.stack();
    if each {
        for slot in 0..4 {
            if nested!(cpu, 0x0600_1860, take_pulse(cpu, slot)) != 0 {
                nested!(cpu, 0x0600_1bae, coin_in(cpu, slot, 0, counters));
            }
            if slots == slot {
                break;
            }
        }
        if cpu.read16(SWITCHES_PRESSED) as u32 & 0x408 != 0 {
            for slot in 0..4 {
                cpu.call(0x0600_1dec, &[slot, counters]);
                if slots == slot {
                    break;
                }
            }
        }
    } else {
        if nested!(cpu, 0x0600_1860, take_pulse(cpu, 0)) != 0 {
            nested!(cpu, 0x0600_1bae, coin_in(cpu, 0, 0, counters));
        }
        if nested!(cpu, 0x0600_1860, take_pulse(cpu, 1)) != 0 {
            nested!(cpu, 0x0600_1bae, coin_in(cpu, 0, 1, counters));
        }
        if cpu.read16(SWITCHES_PRESSED) as u32 & 0x408 != 0 {
            cpu.call(0x0600_1dec, &[0, counters]);
        }
    }
    for (at, counter) in [(BACKUP_SUMS, counters), (BACKUP_SUMS + 8, counters + 4)] {
        let sum = bios(cpu, BIOS_BACKUP_IO, &[READ_LONG, at, 0]);
        let add = cpu.read32(counter);
        bios(cpu, BIOS_BACKUP_IO, &[WRITE_LONG, at, add.wrapping_add(sum)]);
    }
    cpu.pop(8);
}

/// 0x06001b84: `event` (1 a coin, 2 a credit) put in the credits' ring, if it is not full.
pub fn push_event(cpu: &mut Cpu, event: u32) {
    let head = cpu.read8(EVENTS_HEAD) as i8 as i32 as u32;
    let next = head.wrapping_add(1) & 15;
    if next != cpu.read8(EVENTS_TAIL) as i8 as i32 as u32 {
        cpu.write8(EVENTS_RING.wrapping_add(head), event as u8);
        cpu.write8(EVENTS_HEAD, next as u8);
    }
}

/// 0x06001bae: a coin from chute `chute` (the chute the coin slot reads, or `slot` with a slot for
/// each player) counted in for `slot`'s player, unless the backup RAM's coins for them are at the
/// limit (a table by the cabinet's setting) or the game is on free play: the chute's value added
/// to its coins and the player's backup RAM count, a credit counted in for each time the coins
/// reach the coins a credit takes, plays counted in for each time the credits reach the credits a
/// play takes (at the limit the credits are cleared); an event for the game and the totals in the
/// backup RAM. `counters` (two longs) take the plays counted in (the second).
pub fn coin_in(cpu: &mut Cpu, slot: u32, chute: u32, counters: u32) {
    let flags = cpu.read8(MODE_FLAGS) as i8 as i32 as u32;
    let sel = if flags & SLOT_EACH != 0 { slot } else { chute };
    let coins = BACKUP_COINS + 1 + 2 * slot;
    let credits = BACKUP_CREDITS + 1 + 2 * slot;
    let limit = |cpu: &mut Cpu| {
        let i = cpu.read32(BIOS_SETTING) >> 12 & 15;
        cpu.read32(COINS_LIMITS + 4 * i) as i32
    };
    let stored = cpu.read8(coins) as i8 as i32;
    let limit_now = limit(cpu);
    if !(stored >= limit_now) && flags & FREE_PLAY == 0 {
        let value = cpu.read8(CHUTE_VALUE + chute);
        let c = cpu.read8(credits);
        cpu.write8(credits, c.wrapping_add(value));
        let mut granted: u32 = 0;
        if cpu.read8(COINS_PER_CREDIT) as i8 != 0 {
            let at = CHUTE_COINS + 2 * sel;
            let value = cpu.read8(CHUTE_VALUE + chute) as i8 as i32;
            let v = cpu.read16(at) as i16 as i32;
            cpu.write16(at, v.wrapping_add(value) as u16);
            while cpu.read16(at) as i16 as i32 >= cpu.read8(COINS_PER_CREDIT) as i8 as i32 {
                let per = cpu.read8(COINS_PER_CREDIT) as i8 as i32;
                let v = cpu.read16(at) as i16 as i32;
                cpu.write16(at, v.wrapping_sub(per) as u16);
                let c = cpu.read8(credits);
                cpu.write8(credits, c.wrapping_add(1));
            }
        }
        while cpu.read8(credits) as i8 as i32 >= cpu.read8(CREDITS_PER_PLAY) as i8 as i32 {
            granted += 1;
            let c = cpu.read8(coins).wrapping_add(1);
            cpu.write8(coins, c);
            if c as i8 as i32 == limit(cpu) {
                cpu.write16(CHUTE_COINS + 2 * sel, 0);
                cpu.write8(credits, 0);
                break;
            }
            let per = cpu.read8(CREDITS_PER_PLAY);
            let c = cpu.read8(credits);
            cpu.write8(credits, c.wrapping_sub(per));
        }
        if granted == 0 {
            nested!(cpu, 0x0600_1b84, push_event(cpu, 1));
        } else {
            nested!(cpu, 0x0600_1b84, push_event(cpu, 2));
            let v = cpu.read32(counters + 4);
            cpu.write32(counters + 4, v.wrapping_add(granted));
        }
    }
    count_up_plain(cpu, BACKUP_TOTAL);
    count_up_plain(cpu, BACKUP_BY_CHUTE + 8 * sel);
    let lamp = if cpu.read8(MODE_FLAGS) as u32 & SLOT_EACH != 0 || cpu.read8(CHUTE_VALUE) == cpu.read8(CHUTE_VALUE + 1) {
        LAMP_STEPS
    } else {
        LAMP_STEPS + chute
    };
    let v = cpu.read8(lamp);
    cpu.write8(lamp, v.wrapping_add(1));
    bios(cpu, BIOS_MARK_DIRTY, &[]);
}

/// The 32-bit count at `at` in the backup RAM up by one (wrapping).
fn count_up_plain(cpu: &mut Cpu, at: u32) {
    let v = bios(cpu, BIOS_BACKUP_IO, &[READ_LONG, at, 0]);
    bios(cpu, BIOS_BACKUP_IO, &[WRITE_LONG, at, v.wrapping_add(1)]);
}

/// 0x06002098: the I/O board's ports read, each inverted into a word of the BIOS's (PORT_1 ...);
/// the handler for the cabinet's setting (0x06000798 & 3, a table at 0x06002850) turns them into
/// the pads' records; the port bits the lamps' code reads (0x06000730) kept.
pub fn read_ports(cpu: &mut Cpu) {
    for (port, word) in PORTS {
        let v = !(cpu.read8(port) as u32) & 0xff;
        cpu.write32(word, v);
    }
    let mode = cpu.read32(CABINET_FLAGS) & 3;
    let handler = cpu.read32(PAD_HANDLERS + 4 * mode);
    cpu.call(handler, &[]);
    let mut bits = cpu.read32(PORT_1) >> 1 & 4;
    bits |= cpu.read32(PORT_3) & 3;
    bits |= cpu.read32(PORT_2) & 8;
    cpu.write8(PORT_BITS, !bits as u8);
}

/// 0x060021a4 (the pad handler for the cabinet setting 1): the sound CPU's switches, the first two
/// pads' records from the ports, the other two cleared, the cabinet's switches (all four chutes).
pub fn pads_two(cpu: &mut Cpu) {
    nested!(cpu, 0x0600_249c, sound_switches(cpu, 1));
    let w = cpu.read32(PORT_5);
    nested!(cpu, 0x0600_229c, pad_one(cpu, PADS, w << 3 & 0x38));
    let w = cpu.read32(PORT_5);
    nested!(cpu, 0x0600_22dc, pad_two(cpu, PADS + 8, w >> 1 & 0x38));
    for pad in [PADS + 16, PADS + 24] {
        cpu.write32(pad, 0);
        cpu.write32(pad + 4, 0);
    }
    switches(cpu, 3);
}

/// A pad's record updated to `now`: the old buttons kept, the newly pressed and released bits.
fn pad_set(cpu: &mut Cpu, pad: u32, old: u32, now: u32) {
    cpu.write16(pad, now as u16);
    cpu.write16(pad + 4, (!old & now) as u16);
    cpu.write16(pad + 6, (!now & old) as u16);
}

/// 0x0600229c: the first pad's record from the ports: PORT_1's bits 4-7 and 0-2, PORT_3's bit 4
/// (to bit 10), and `extra`.
pub fn pad_one(cpu: &mut Cpu, pad: u32, extra: u32) {
    let old = cpu.read16(pad) as u32;
    cpu.write16(pad + 2, old as u16);
    let p1 = cpu.read32(PORT_1);
    let p3 = cpu.read32(PORT_3);
    let p1b = cpu.read32(PORT_1);
    let now = (p1 << 2 & 0x3c0) | (p3 << 6 & 0x400) | (p1b & 7) | extra;
    pad_set(cpu, pad, old, now);
}

/// 0x060022dc: the second pad's record: the same from PORT_2 (PORT_3's bit 5 to bit 10).
pub fn pad_two(cpu: &mut Cpu, pad: u32, extra: u32) {
    let old = cpu.read16(pad) as u32;
    cpu.write16(pad + 2, old as u16);
    let p2 = cpu.read32(PORT_2);
    let p3 = cpu.read32(PORT_3);
    let p2b = cpu.read32(PORT_2);
    let now = (p2 << 2 & 0x3c0) | (p3 << 5 & 0x400) | (p2b & 7) | extra;
    pad_set(cpu, pad, old, now);
}

/// 0x0600249c: while the BIOS's flag (bit 7 of FLAGS_124) says the sound CPU's switches are to be
/// read, their byte (inverted) is put in the cabinet's switches record's low bits, as `n` (0-3,
/// the number of slots) says: bits 0-1, with bits 11-12 for the third and fourth; the flag cleared.
pub fn sound_switches(cpu: &mut Cpu, n: u32) {
    if cpu.read32(FLAGS_124) & 0x80 == 0 {
        return;
    }
    let v = !(cpu.read8(SOUND_SWITCHES) as i8 as i32 as u32);
    let flags = cpu.read8(MODE_FLAGS);
    let now = cpu.read16(SWITCHES) as u32;
    if flags & SLOT_EACH as u8 == 0 {
        cpu.write16(SWITCHES, (now & 0xfffc | v & 3) as u16);
    } else {
        let now = match n {
            0 => Some(now & 0xfffe | v & 1),
            1 => Some(now & 0xfffc | v & 3),
            2 => Some(now & 0xf7fc | v & 3 | v << 9 & 0x800),
            3 => Some(now & 0xe7fc | v & 3 | v << 9 & 0x1800),
            _ => None,
        };
        if let Some(now) = now {
            cpu.write16(SWITCHES, now as u16);
        }
    }
    let f = cpu.read32(FLAGS_124);
    cpu.write32(FLAGS_124, f & 0xffff_ff7f);
}

/// The cabinet's switches' record (0x06000728), from the ports and `chutes` (the mask of the coin
/// chutes' bits to read: 1, 2, 4 and 8): the record's buttons now, before, newly pressed and
/// newly released. (The tail 0x06002364 of the pad handlers.)
fn switches(cpu: &mut Cpu, chutes: u32) {
    let old = cpu.read16(SWITCHES) as u32;
    cpu.write16(SWITCHES + 2, old as u16);
    let p3 = cpu.read32(PORT_3);
    let p3b = cpu.read32(PORT_3);
    let p6 = cpu.read32(PORT_6);
    let mut now = (p6 << 8 & 0x600) | (p3 << 7 & 0x4000) | (p3b & 12);
    if cpu.read32(CABINET_FLAGS) & 4 != 0 {
        now |= cpu.read32(PORT_3) << 7 & 0x2000;
    }
    if cpu.read8(OPTION_FLAGS) as u32 & 4 == 0 {
        now |= cpu.read32(PORT_3) & 3;
        if chutes & 1 != 0 {
            now |= cpu.read32(PORT_3) & 0x10;
        }
        if chutes & 2 != 0 {
            now |= cpu.read32(PORT_3) & 0x20;
        }
        if chutes & 4 != 0 {
            now |= cpu.read32(PORT_4) << 3 & 0x40;
        }
        if chutes & 8 != 0 {
            now |= cpu.read32(PORT_5) << 4 & 0x80;
        }
    } else {
        if chutes & 1 != 0 {
            now |= cpu.read32(PORT_3) & 0x11;
        }
        if chutes & 2 != 0 {
            now |= cpu.read32(PORT_3) & 0x22;
        }
        if chutes & 4 != 0 {
            let a = cpu.read32(PORT_4) << 3 & 0x40;
            let b = cpu.read32(PORT_1) << 8 & 0x800;
            now |= a | b;
        }
        if chutes & 8 != 0 {
            let a = cpu.read32(PORT_5) << 4 & 0x80;
            let b = cpu.read32(PORT_2) << 9 & 0x1000;
            now |= a | b;
        }
    }
    cpu.write16(SWITCHES, now as u16);
    cpu.write16(SWITCHES + 4, (!old & now) as u16);
    cpu.write16(SWITCHES + 6, (!now & old) as u16);
}

/// 0x06002654: r0 shifted right by r1, the sign kept (r1 below 0: r0 as it is; 32 and over: the
/// sign). A function of the BIOS's own convention (value in r0, count in r1).
pub fn shift_right_signed_regs(cpu: &mut Cpu) -> u32 {
    let (x, n) = (cpu.reg(0), cpu.reg(1) as i32);
    match n {
        ..0 => x,
        0..32 => ((x as i32) >> n) as u32,
        _ => ((x as i32) >> 31) as u32,
    }
}

/// `x` shifted right by `n` with 0x06002654.
fn shift_right_signed(cpu: &mut Cpu, x: u32, n: u32) -> u32 {
    cpu.leave(&[(0, x), (1, n)]);
    nested!(cpu, 0x0600_2654, shift_right_signed_regs(cpu))
}

/// The BIOS's exception stubs push r0 and set it to the stub's code (the vector's handler and
/// priority are found by it), and all share the dispatch at 0x06001ffc: the tables of the
/// handlers (VECTORS) and of the SR and SCU mask for each (PRIORITIES), indexed by the code.
const DISPATCH_RETURN: u32 = 0x0600_2030;
/// The dispatch's table of SR and SCU mask bits for each code (the same entries as PRIORITIES,
/// which starts at code 64).
const SR_MASKS: u32 = 0x0600_0980;
/// SR the dispatch leaves the handler's return with: every interrupt masked.
const SR_MASKED: u32 = 0xf0;

/// The stubs at 0x06001f48, 0x06001f4e, ... (each `push r0; r0 = code; bra 0x06001ffc`) and the
/// dispatch at 0x06001ffc: an exception with `code` was taken (SR and PC pushed). It saves r0-r7,
/// PR, GBR and the SCU's mask, sets SR and the SCU's mask for the exception (SR_MASKS: SR in the
/// high half, the mask bits to add in the low), calls the handler VECTORS holds (PR
/// DISPATCH_RETURN, as `jsr` leaves it, which a handler that pushes PR leaves on the stack), then
/// takes everything back and ends with `rte`. It registers (stv::native::Natives::register_exception)
/// so that the machine enters it as it takes the exception.
pub fn exception(sh2: &mut Sh2, bus: &mut dyn Bus, code: u32) {
    let fp = sh2.r[14];
    let mut cpu = Cpu { sh2, bus, caller_r14: fp, r14_out: None };
    let [r0, r1, r2, r3, r4, r5, r6, r7, ..] = cpu.sh2.r;
    let (pr, gbr) = (cpu.sh2.pr, cpu.sh2.gbr);
    cpu.push(&[r0]);
    let offset = code << 2;
    cpu.push(&[r1]);
    cpu.push(&[r2]);
    let mask = cpu.read32(SCU_MASK_COPY);
    cpu.push(&[r3]);
    cpu.push(&[mask]);
    cpu.push(&[r4]);
    cpu.push(&[r5]);
    let entry = cpu.read32(SR_MASKS + offset);
    cpu.sh2.sr = (entry >> 16) & SR_MASK;
    let mask_now = mask | entry as u16 as i16 as u32;
    cpu.write32(SCU_MASK_COPY, mask_now);
    cpu.write32(IMS, mask_now);
    cpu.push(&[r6]);
    let handler = cpu.read32(VECTORS + offset);
    cpu.push(&[r7]);
    cpu.push(&[pr]);
    cpu.push(&[gbr]);
    // what the handler is handed in r0-r6
    cpu.leave(&[(0, offset), (1, SCU_MASK_COPY), (2, mask_now), (3, VECTORS), (4, entry as u16 as i16 as u32), (5, IMS), (6, handler)]);
    cpu.call_returning(DISPATCH_RETURN, handler, &[]);
    let pull = |cpu: &mut Cpu| {
        let v = cpu.read32(cpu.sh2.r[15]);
        cpu.pop(4);
        v
    };
    cpu.sh2.gbr = pull(&mut cpu);
    cpu.sh2.pr = pull(&mut cpu);
    cpu.sh2.r[7] = pull(&mut cpu);
    cpu.sh2.r[0] = SR_MASKED;
    cpu.sh2.r[6] = pull(&mut cpu);
    cpu.sh2.r[3] = SCU_MASK_COPY;
    cpu.sh2.r[5] = pull(&mut cpu);
    cpu.sh2.r[1] = IMS;
    cpu.sh2.r[4] = pull(&mut cpu);
    let mask = pull(&mut cpu);
    cpu.sh2.r[2] = mask;
    cpu.sh2.sr = SR_MASKED;
    cpu.write32(SCU_MASK_COPY, mask);
    cpu.sh2.r[3] = pull(&mut cpu);
    cpu.write32(IMS, mask);
    cpu.sh2.r[2] = pull(&mut cpu);
    cpu.sh2.r[1] = pull(&mut cpu);
    cpu.sh2.r[0] = pull(&mut cpu);
    stv::native::rte(cpu.sh2, cpu.bus);
}

/// The operating time's count, in the BIOS's work RAM: a 32-bit sum that wraps.
const TIME_COUNT: u32 = 0x0600_0758;
/// What the ROM's time routine adds to the count each time it is called.
const TIME_STEP: u32 = 0x5592_9fad;
const READ_LONG_ROUTINE: u32 = 0x0600_143e;
const WRITE_LONG_ROUTINE: u32 = 0x0600_1464;

/// 0x00000efc (the ROM's table at 0x000010e8, called each frame): the operating time's count
/// (TIME_COUNT) goes up by TIME_STEP; when the signed sum overflows, the backup RAM's 32-bit value
/// at its start (read with 0x0600143e) is run through `saturating_add` with 3 and written back
/// (0x06001464).
pub fn time_count(cpu: &mut Cpu) {
    let (sum, overflow) = (cpu.read32(TIME_COUNT) as i32).overflowing_add(TIME_STEP as i32);
    cpu.write32(TIME_COUNT, sum as u32);
    if !overflow {
        return;
    }
    let (kind, link) = (cpu.reg(4), 0x0000_0f12);
    let v = cpu.call_returning(link, READ_LONG_ROUTINE, &[kind, BACKUP]);
    let n = clamp_add(v, 3);
    cpu.call_returning(0x0000_0f20, WRITE_LONG_ROUTINE, &[v, BACKUP, n]);
}

/// 0x00000ecc: `a` + `b` for a non-negative `a`; for a negative `a` the sum, but -1 where it comes
/// out non-negative (a count that ran below 0 and is brought back to none).
fn clamp_add(a: u32, b: u32) -> u32 {
    let sum = a.wrapping_add(b);
    if (a as i32) >= 0 || (sum as i32) < 0 { sum } else { u32::MAX }
}

/// 0x0000372c (the ROM's area routine): the address in the backup RAM of the settings block of the
/// region byte at REGION (0x0f00 bytes each). Leaves r2 and r3 (its address base and the size).
pub fn area(cpu: &mut Cpu) -> u32 {
    const BASE: u32 = 0x2018_0100;
    const SIZE: u32 = 0x0f00;
    let region = cpu.read8(REGION) as u32;
    cpu.leave(&[(2, BASE), (3, SIZE)]);
    BASE.wrapping_add((region * SIZE) as i16 as u32)
}

/// 0x000044fc: the ROM's empty hook (a function of the tables that nothing hooks into).
pub fn empty_hook(_cpu: &mut Cpu) {}

/// 0x00004dc0: the 8 bytes of the region's block in the BIOS's settings table at 0x0600079c (10
/// bytes each, the first two skipped) copied to `dest`. Leaves the registers its loop ends with.
pub fn settings_copy(cpu: &mut Cpu, dest: u32) {
    const TABLE: u32 = 0x0600_079c;
    let region = cpu.read8(REGION) as i8 as i32;
    let offset = region.wrapping_mul(10) as i8 as i32;
    let mut from = TABLE.wrapping_add(offset as u32).wrapping_add(2);
    let mut to = dest;
    let mut last = (0, 0);
    for _ in 0..4 {
        let a = cpu.read8(from);
        cpu.write8(to, a);
        let b = cpu.read8(from + 1);
        last = (b as u32, to.wrapping_add(1));
        cpu.write8(to + 1, b);
        from = from.wrapping_add(2);
        to = to.wrapping_add(2);
    }
    cpu.leave(&[(1, last.0), (2, TABLE), (3, last.1), (4, to), (5, 0), (6, from)]);
    cpu.leave_t(true);
}

/// The BIOS's table entries the ROM's routines call through: a service that answers the running
/// count (0x06000640), the checksum of a block of the backup RAM (0x06000600).
const BIOS_COUNT: u32 = 0x0600_0640;
const BIOS_CRC: u32 = 0x0600_0600;
/// The count as the BIOS last took it, and the counts at the starts of up to 16 plays (a word each).
const COUNT_AT_STOP: u32 = DATA + 0x140;
const COUNT_AT_START: u32 = DATA + 0x130;
/// The backup RAM's totals (counters in 32-bit values): the running time, and by setting (a block of
/// 64 bytes each, BACKUP_BY_SETTING) the time at +36 and +44 and the longest at +20 and +28.
const BACKUP_TIME: u32 = BACKUP + 0x3d54;

/// 0x00003842 (called by the game): the running time's bookkeeping, for the operation `op` (its
/// low nibble `n`, its next the kind). Kind 0, n 0: the count taken now is kept as the count at stop;
/// n 1: the count since then added (the sum clamps) to the backup RAM's total time and to this
/// setting's. Kind 1: the count taken now kept as the start of play `n`. Kind 2: the count since play
/// `n` started added to the totals as for kind 0, and made the setting's longest time if it is
/// longer (in two of them, the second also when it is 0). All end by the BIOS's backup-changed
/// service, jumped to with the caller's return address.
pub fn play_counts(cpu: &mut Cpu, op: u32) {
    let caller = cpu.pr();
    let low = op & 15;
    let region = cpu.read8(REGION) as u32;
    let setting = BACKUP_BY_SETTING + (region << 6);
    // the backup service, read from the BIOS's table at each use as the SH-2 code does
    let io = |cpu: &mut Cpu, pr: u32, kind: u32, at: u32, value: u32| {
        let f = cpu.read32(BIOS_BACKUP_IO);
        cpu.call_returning(pr, f, &[kind, at, value])
    };
    match op >> 4 {
        0 => match low {
            0 => {
                let f = cpu.read32(BIOS_COUNT);
                let now = cpu.call_returning(0x0000_3884, f, &[]);
                cpu.write32(COUNT_AT_STOP, now);
            }
            1 => {
                let f = cpu.read32(BIOS_COUNT);
                let now = cpu.call_returning(0x0000_3892, f, &[]);
                let pushed = cpu.read32(BIOS_BACKUP_IO);
                let delta = now.wrapping_sub(cpu.read32(COUNT_AT_STOP));
                cpu.push(&[pushed]);
                let total = io(cpu, 0x0000_38ac, READ_LONG, BACKUP_TIME, 0);
                let sum = clamp_add(total, delta);
                cpu.pop(4);
                cpu.call_returning(0x0000_38be, pushed, &[WRITE_LONG, BACKUP_TIME, sum]);
                let f = cpu.read32(BIOS_BACKUP_IO);
                cpu.push(&[f]);
                let total = io(cpu, 0x0000_38ce, READ_LONG, setting + 44, 0);
                let sum = clamp_add(total, delta);
                cpu.pop(4);
                cpu.call_returning(0x0000_38e0, f, &[WRITE_LONG, setting + 44, sum]);
            }
            _ => {}
        },
        1 => {
            let f = cpu.read32(BIOS_COUNT);
            let now = cpu.call_returning(0x0000_3928, f, &[]);
            cpu.write32(COUNT_AT_START + 4 * low, now);
        }
        2 => {
            let f = cpu.read32(BIOS_COUNT);
            let now = cpu.call_returning(0x0000_3932, f, &[]);
            let started = cpu.read32(COUNT_AT_START + 4 * low);
            let pushed = cpu.read32(BIOS_BACKUP_IO);
            let delta = now.wrapping_sub(started);
            cpu.push(&[pushed]);
            let total = io(cpu, 0x0000_3950, READ_LONG, setting + 36, 0);
            let sum = clamp_add(total, delta);
            cpu.pop(4);
            cpu.call_returning(0x0000_3962, pushed, &[WRITE_LONG, setting + 36, sum]);
            let longest = io(cpu, 0x0000_396e, READ_LONG, setting + 20, 0);
            if longest < delta {
                io(cpu, 0x0000_3980, WRITE_LONG, setting + 20, delta);
            }
            let longest = io(cpu, 0x0000_398c, READ_LONG, setting + 28, 0);
            if longest == 0 || longest > delta {
                io(cpu, 0x0000_39a2, WRITE_LONG, setting + 28, delta);
            }
        }
        _ => {}
    }
    // the SH-2 code's tail: its frame (8 words) is let go and the service jumped to, as for the caller
    let mark = cpu.read32(BIOS_MARK_DIRTY);
    cpu.pop(32);
    cpu.call_returning(caller, mark, &[]);
}

/// 0x00003a9c (called by the game): the backup RAM's settings block for the region written with the
/// defaults (0x00004908), then the checksum of the block (the BIOS's service, 0x17c bytes) written
/// as its first word and the first 96 words copied to the mirror.
pub fn backup_reset(cpu: &mut Cpu) {
    cpu.write32(cpu.stack(), BACKUP_MIRROR);
    let region = cpu.read8(REGION) as u32;
    nested!(cpu, 0x0000_4908, returning 0x0000_3aca, backup_defaults(cpu, region, BACKUP_BY_SETTING + (region << 6)));
    let io = cpu.read32(BIOS_BACKUP_IO);
    let crc = cpu.read32(BIOS_CRC);
    let sum = cpu.call_returning(0x0000_3ada, crc, &[BACKUP_CHECK + 4, BACKUP_CHECKED_BYTES]);
    cpu.call_returning(0x0000_3ae2, io, &[WRITE_WORD, BACKUP_CHECK, sum & 0xffff]);
    for i in 0..BACKUP_MIRRORED {
        let v = cpu.read32(BACKUP_CHECK + 4 * i);
        cpu.write32(BACKUP_MIRROR + 4 * i, v);
    }
}

/// The words the defaults' first word is taken from (one for each region), and the ROM's block of
/// 60 bytes the rest is copied from.
const DEFAULT_WORDS: u32 = 0x0600_06b8;
const DEFAULTS: u32 = 0x0000_67b8;

/// 0x00004908: the backup RAM's settings block at `dest` for `region` initialised: the region's word
/// written by the service (a 16-bit write), then the ROM's 60 bytes of defaults copied after it
/// from `dest`+4.
pub fn backup_defaults(cpu: &mut Cpu, region: u32, dest: u32) {
    let frame = cpu.stack();
    cpu.write32(frame, region);
    cpu.write32(frame + 8, DEFAULTS);
    cpu.write32(frame + 4, dest);
    let f = cpu.read32(BIOS_BACKUP_IO);
    let word = cpu.read16(DEFAULT_WORDS + 2 * region) as u32;
    cpu.call_returning(0x0000_4932, f, &[WRITE_WORD, dest, word]);
    for i in 0..30 {
        let v = cpu.read16(DEFAULTS + 2 * i);
        cpu.write16(dest + 4 + 2 * i, v);
    }
    cpu.write32(frame + 4, dest + 64);
    cpu.write32(frame + 8, DEFAULTS + 60);
}
