//! The ST-V's settings EEPROM: a 93C46 (64 words of 16 bits) on the SMPC's parallel ports. Serial
//! instructions are clocked in on SK's rising edges while CS is high: a start bit, a two-bit opcode
//! and a six-bit address (and, for WRITE and WRAL, sixteen data bits). From the 93C46 data sheet.

use crate::state::saved;

#[derive(Clone, Debug)]
pub struct Eeprom {
    pub mem: [u16; 64],
    cs: bool,
    sk: bool,
    /// Bits shifted in since CS rose (and how many).
    shift: u32,
    bits: u32,
    write_enabled: bool,
    /// A READ's data going out: the word and how many of its bits remain to be clocked out.
    out: Option<(u16, u32)>,
    out_addr: u8,
    pub dout: bool,
}

saved!(Eeprom { mem, cs, sk, shift, bits, write_enabled, out, out_addr, dout });

impl Eeprom {
    pub fn new(image: Option<&[u8]>) -> Eeprom {
        let mut mem = [0xFFFF; 64];
        if let Some(img) = image {
            for (i, w) in mem.iter_mut().enumerate() {
                if let Some(b) = img.get(2 * i..2 * i + 2) {
                    *w = u16::from_be_bytes([b[0], b[1]]);
                }
            }
        }
        Eeprom { mem, cs: false, sk: false, shift: 0, bits: 0, write_enabled: false, out: None, out_addr: 0, dout: true }
    }

    /// The pins as the SMPC's port drives them.
    pub fn update(&mut self, cs: bool, sk: bool, di: bool) {
        if !cs {
            self.cs = false;
            self.sk = sk;
            self.shift = 0;
            self.bits = 0;
            self.out = None;
            self.dout = true;
            return;
        }
        if !self.cs {
            self.cs = true;
            self.shift = 0;
            self.bits = 0;
        }
        let rising = sk && !self.sk;
        self.sk = sk;
        if !rising {
            return;
        }
        if let Some((word, left)) = self.out {
            // shifting a READ's data out, most significant bit first, then the next word
            let left = left - 1;
            self.dout = (word >> left) & 1 != 0;
            if left == 0 {
                self.out_addr = (self.out_addr + 1) & 63;
                self.out = Some((self.mem[self.out_addr as usize], 16));
            } else {
                self.out = Some((word, left));
            }
            return;
        }
        if self.bits == 0 && !di {
            return; // waiting for the start bit
        }
        self.shift = (self.shift << 1) | di as u32;
        self.bits += 1;
        if self.bits < 9 {
            return;
        }
        let op = (self.shift >> 6) & 3;
        let addr = (self.shift & 63) as u8;
        match op {
            2 if self.bits == 9 => {
                self.out_addr = addr;
                // the dummy 0 now; the word's bits on the following clocks
                self.out = Some((self.mem[addr as usize], 16));
                self.dout = false;
            }
            3 if self.bits == 9 => {
                if self.write_enabled {
                    self.mem[addr as usize] = 0xFFFF;
                }
                self.dout = true;
            }
            0 if self.bits == 9 => match addr >> 4 {
                3 => self.write_enabled = true,
                0 => self.write_enabled = false,
                2
                    if self.write_enabled => {
                        self.mem = [0xFFFF; 64];
                    }
                _ => {} // WRAL: the data follows
            },
            _ => {}
        }
        if self.bits == 25 {
            // WRITE and WRAL: the start bit, opcode and address have shifted up by the sixteen data
            // bits since the ninth clock, so the opcode is read from here (not from `op` above)
            let (op, addr, data) = ((self.shift >> 22) & 3, ((self.shift >> 16) & 63) as usize, (self.shift & 0xFFFF) as u16);
            if self.write_enabled {
                match (op, addr >> 4) {
                    (1, _) => self.mem[addr] = data,
                    (0, 1) => self.mem = [data; 64],
                    _ => {}
                }
            }
            if op == 1 || (op == 0 && addr >> 4 == 1) {
                self.dout = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Clocks `bits` (most significant first, `n` of them) in with CS high.
    fn send(e: &mut Eeprom, value: u32, n: u32) {
        for i in (0..n).rev() {
            let b = (value >> i) & 1 != 0;
            e.update(true, false, b);
            e.update(true, true, b);
        }
    }

    /// An instruction: start bit, opcode, address (and the sixteen data bits when `data` is given).
    fn instruction(e: &mut Eeprom, op: u32, addr: u32, data: Option<u16>) {
        e.update(false, false, false);
        send(e, 1, 1);
        send(e, op, 2);
        send(e, addr, 6);
        if let Some(d) = data {
            send(e, d.into(), 16);
        }
        e.update(false, false, false);
    }

    fn read(e: &mut Eeprom, addr: u32) -> u16 {
        e.update(false, false, false);
        send(e, 0b110, 3);
        send(e, addr, 6);
        let mut w = 0;
        for _ in 0..16 {
            e.update(true, false, false);
            e.update(true, true, false);
            w = (w << 1) | e.dout as u16;
        }
        e.update(false, false, false);
        w
    }

    #[test]
    fn the_93c46s_commands() {
        let mut e = Eeprom::new(None);
        // writes do nothing before EWEN
        instruction(&mut e, 1, 5, Some(0x1234));
        assert_eq!(e.mem[5], 0xFFFF);
        instruction(&mut e, 0, 0b110000, None); // EWEN
        instruction(&mut e, 1, 5, Some(0x1234)); // WRITE
        assert_eq!(e.mem[5], 0x1234);
        assert_eq!(read(&mut e, 5), 0x1234);
        instruction(&mut e, 1, 6, Some(0xBEEF));
        assert_eq!((e.mem[5], e.mem[6], e.mem[7]), (0x1234, 0xBEEF, 0xFFFF));
        instruction(&mut e, 3, 5, None); // ERASE
        assert_eq!((e.mem[5], e.mem[6]), (0xFFFF, 0xBEEF));
        instruction(&mut e, 0, 0b010000, Some(0xA5A5)); // WRAL
        assert!(e.mem.iter().all(|&w| w == 0xA5A5));
        instruction(&mut e, 0, 0b100000, None); // ERAL
        assert!(e.mem.iter().all(|&w| w == 0xFFFF));
        instruction(&mut e, 0, 0b000000, None); // EWDS
        instruction(&mut e, 1, 9, Some(0x0001));
        assert_eq!(e.mem[9], 0xFFFF);
    }
}
