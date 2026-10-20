//! The ST-V's settings EEPROM: a 93C46 (64 words of 16 bits) on the SMPC's parallel ports. Serial
//! instructions are clocked in on SK's rising edges while CS is high: a start bit, a two-bit opcode
//! and a six-bit address (and, for WRITE and WRAL, sixteen data bits). From the 93C46 data sheet.

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
            1 | 0 if self.bits == 25 => {
                let data = (self.shift & 0xFFFF) as u16;
                let addr = ((self.shift >> 16) & 63) as usize;
                if self.write_enabled {
                    if op == 1 {
                        self.mem[addr] = data;
                    } else {
                        self.mem = [data; 64];
                    }
                }
                self.dout = true;
            }
            _ => {}
        }
    }
}
