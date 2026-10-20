//! The players' controls: each frame the first two digital pads SGL's peripheral read found on the
//! ports become `PAD_INF`s (dma.c's types): what is held, what changed, what was pressed, what was
//! let go. The ST-V's I/O board shows SGL a digital pad on each port, so both are always found.

use super::Cpu;

/// `PAD_INF pad[4]`: the two players', then a copy of them.
const PAD_INF: u32 = 0x0602_ee10;
const PADS: u32 = 2;
/// `PAD_INF`: held, changed since the last frame, pressed, let go (bits as SGL's PER_DGT_*, set
/// when held).
const DATA: u32 = 0;
const EDGE: u32 = 2;
const TRIG: u32 = 4;
const PULL: u32 = 6;

/// SGL's `Smpc_Peripheral` (a pointer): what each port has, 15 devices a port, `PerDigital` each.
const SMPC_PERIPHERAL: u32 = 0x0603_453c;
const DEVICES: u32 = 15;
const DEVICE_SIZE: u32 = 0x18;
/// `PerDigital`: the device's id (0xFF none), its buttons (clear when held).
const ID: u32 = 0;
const BUTTONS: u32 = 2;
const NONE: u8 = 0xFF;
/// The ids read as digital pads (SGL's PER_ID_Digital, and two the game takes as one).
const DIGITAL: [u8; 3] = [0x02, 0xE2, 0xE1];
/// How far into a port the game looks.
const LOOKED_AT: u32 = 6;

/// A word of the BIOS's: unless its bits 12-15 are 1, the pads' bit 9 is cleared (what the
/// setting is, is not known yet).
const BIOS_SETTING: u32 = 0x0600_0248;
const BUTTON_9: u16 = 1 << 9;

fn pad(n: u32) -> u32 {
    PAD_INF + 8 * n
}

/// Clears the four PAD_INFs.
pub fn pad_init(cpu: &mut Cpu) {
    for n in 0..4 {
        for field in [DATA, EDGE, TRIG, PULL] {
            cpu.write16(pad(n) + field, 0);
        }
    }
}

/// Reads the players' pads for the frame.
pub fn pad_read(cpu: &mut Cpu) {
    // what was held (a pad not reached keeps it: none changed; the SH-2 code took stack garbage)
    let mut before = [0u16; PADS as usize];
    for (n, b) in before.iter_mut().enumerate() {
        *b = cpu.read16(pad(n as u32) + DATA);
    }
    // the first two digital pads, port by port; a pad with no device seen anywhere yet reads
    // as nothing held
    let peripherals = cpu.read32(SMPC_PERIPHERAL);
    let (mut n, mut port, mut device) = (0, 0, 0);
    let mut seen = false;
    while n < PADS && port < 2 {
        let mut found = None;
        while device < LOOKED_AT {
            let at = peripherals + (port * DEVICES + device) * DEVICE_SIZE;
            let id = cpu.read8(at + ID);
            if id != NONE {
                seen = true;
                if DIGITAL.contains(&id) {
                    found = Some(at);
                    break;
                }
            }
            device += 1;
        }
        match found {
            Some(at) => {
                let held = !cpu.read16(at + BUTTONS);
                cpu.write16(pad(n) + DATA, held);
                n += 1;
                device += 1;
            }
            None => {
                if !seen {
                    cpu.write16(pad(n) + DATA, 0);
                    n += 1;
                }
                port += 1;
                device = 0;
            }
        }
    }
    if (cpu.read32(BIOS_SETTING) >> 12) & 0xF != 1 {
        for n in 0..PADS {
            let held = cpu.read16(pad(n) + DATA);
            cpu.write16(pad(n) + DATA, held & !BUTTON_9);
        }
    }
    for n in 0..PADS {
        let held = cpu.read16(pad(n) + DATA);
        let edge = before[n as usize] ^ held;
        let trig = edge & held;
        cpu.write16(pad(n) + EDGE, edge);
        cpu.write16(pad(n) + TRIG, trig);
        cpu.write16(pad(n) + PULL, edge ^ trig);
    }
    for n in 0..PADS {
        for field in [DATA, EDGE, TRIG, PULL] {
            let v = cpu.read16(pad(n) + field);
            cpu.write16(pad(n + PADS) + field, v);
        }
    }
}
