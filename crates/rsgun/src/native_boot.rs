//! The native boot: the machine as the BIOS hands over to the game (the master about to execute
//! the game's entry at 0x06004000), built from the images alone instead of running the BIOS's
//! boot. `docs/HANDOFF.md` ("The native boot: the handover state") maps what the state holds and
//! where each part comes from; `tools/boot/` makes the reference (the interpreted boot's state at
//! that instant) that [`check`] compares this with.
//!
//! Work RAM is made as the BIOS makes it, routine by routine (each function names the ROM
//! routine it stands for): what the BIOS copies or fills from its tables is read from the BIOS
//! ROM image, the settings from the EEPROM image, the game from the cart. The pointers and small
//! variables the BIOS's code stores are the literals of that code (addresses of its own routines,
//! flags); where the BIOS got one from hardware that has no state here (the cabinet's slots, its
//! switches) the value it found at the handover is used and said so.

use stv::machine::Machine;

/// The images and files a run has.
pub struct Inputs<'a> {
    pub bios: &'a [u8],
    pub cart: &'a [u8],
    /// The settings EEPROM (128 bytes, big-endian words); `None`: the machine's factory one.
    pub eeprom: Option<&'a [u8]>,
    /// The backup RAM file; `None`: blank.
    pub backup: Option<&'a [u8]>,
    pub area: u8,
}

const RAM_H: u32 = 0x0600_0000;
const RAM_L: u32 = 0x0020_0000;

/// Work RAM-H with the BIOS's stores.
struct Ram<'a> {
    h: &'a mut [u8],
    bios: &'a [u8],
}

impl Ram<'_> {
    fn idx(a: u32) -> usize {
        (a - RAM_H) as usize
    }

    fn w8(&mut self, a: u32, v: u8) {
        self.h[Self::idx(a)] = v;
    }

    fn w16(&mut self, a: u32, v: u16) {
        self.h[Self::idx(a)..][..2].copy_from_slice(&v.to_be_bytes());
    }

    fn w32(&mut self, a: u32, v: u32) {
        self.h[Self::idx(a)..][..4].copy_from_slice(&v.to_be_bytes());
    }

    fn r16(&self, a: u32) -> u16 {
        u16::from_be_bytes(self.h[Self::idx(a)..][..2].try_into().unwrap())
    }

    fn r32(&self, a: u32) -> u32 {
        u32::from_be_bytes(self.h[Self::idx(a)..][..4].try_into().unwrap())
    }

    fn bytes(&mut self, a: u32, data: &[u8]) {
        self.h[Self::idx(a)..][..data.len()].copy_from_slice(data);
    }

    fn fill(&mut self, a: u32, len: usize, v: u8) {
        self.h[Self::idx(a)..][..len].fill(v);
    }

    /// A long word of the BIOS ROM (`a`: an address as its code sees it, 0 or 0x2000_0000 based).
    fn rom32(&self, a: u32) -> u32 {
        let o = (a & 0x7_FFFF) as usize;
        u32::from_be_bytes(self.bios[o..o + 4].try_into().unwrap())
    }

    fn rom16(&self, a: u32) -> u16 {
        let o = (a & 0x7_FFFF) as usize;
        u16::from_be_bytes(self.bios[o..o + 2].try_into().unwrap())
    }

    /// ROM 0x0d10: copies a record (count, destination, source) of long words from a table in the
    /// ROM; returns the table's next address.
    fn copy_record(&mut self, table: u32) -> u32 {
        let (count, dst, src) = (self.rom32(table) as usize, self.rom32(table + 4), self.rom32(table + 8));
        for i in 0..count as u32 {
            let v = self.rom32(src + 4 * i);
            self.w32(dst + 4 * i, v);
        }
        table + 12
    }
}

/// The BIOS's fill word, "SEGA".
const SEGA: [u8; 4] = *b"SEGA";

fn fill_sega(mem: &mut [u8]) {
    for c in mem.as_chunks_mut::<4>().0 {
        *c = SEGA;
    }
}

/// The ROM tables the reset code (ROM 0x0268) works from.
mod rom {
    /// ROM 0x0268-0x0298: records (count in long words, destination) filled with "SEGA" by 0x0d00:
    /// work RAM-L, RAM-H from 0x0600c000, and RAM-H 0x06004000-0x0600c000.
    pub const FILLS: u32 = 0x494;
    /// The record (count, destination) of zeros after them (RAM-H 0x06000b00-0x06000c00).
    pub const ZEROS: u32 = 0x4ac;
    /// The copy records (count, destination, source) of 0x0d10: the boot program (0x06010000;
    /// the game's image covers it all), the vectors and services (0x06000000), the RAM code
    /// (0x06000c00) and 0x060002c0.
    pub const COPIES: u32 = 0x4b4;
    /// The pointer table of 0x02a6: halfword offsets from 0x06000200 ending at a negative one, and
    /// the long words stored at them.
    pub const POINTER_OFFSETS: u32 = 0x530;
    pub const POINTER_VALUES: u32 = 0x564;
    /// ROM 0x0370: 32 long words the RAM code copies to 0x06000a80.
    pub const TABLE_A80: u32 = 0x370;
}

/// The cart header's boot table (cart file 0x200f10; ROM 0x4500 picks the record).
const CART_BOOT_TABLE: usize = 0x20_0F10;
/// The cart header's titles, 16 bytes a region.
const CART_TITLES: usize = 0x20_0060;
/// Where the cart's game is copied to, from the boot table.
const GAME_BASE: u32 = 0x0600_4000;

/// Work RAM-H as the handover finds it.
fn work_ram_h(h: &mut [u8], img: &Inputs, eeprom: &[u8; 128], backup: &[u8]) -> Loader {
    let mut ram = Ram { h, bios: img.bios };

    // ROM 0x0268: the fills (0x0d00) and copies (0x0d10) from its tables. (Work RAM-H below
    // 0x06004000 starts as zeros here and is covered by the copies.)
    // (the first record is work RAM-L's, made in `build`)
    for rec in 1..3 {
        let t = rom::FILLS + 8 * rec;
        let (count, dst) = (ram.rom32(t) as usize, ram.rom32(t + 4));
        let region = &mut ram.h[Ram::idx(dst)..][..count * 4];
        fill_sega(region);
    }
    let (count, dst) = (ram.rom32(rom::ZEROS) as usize, ram.rom32(rom::ZEROS + 4));
    ram.fill(dst, count * 4, 0);
    // (the first copy, the boot program to 0x06010000, is overwritten by the game below)
    let mut t = rom::COPIES + 12;
    for _ in 0..3 {
        t = ram.copy_record(t);
    }

    // ROM 0x02a6: the pointers into the BIOS's own routines
    let mut off = rom::POINTER_OFFSETS;
    let mut val = rom::POINTER_VALUES;
    while (ram.rom16(off) as i16) >= 0 {
        let v = ram.rom32(val);
        ram.w32(RAM_H + 0x200 + ram.rom16(off) as u32, v);
        off += 2;
        val += 4;
    }
    boot_variables(&mut ram, eeprom);

    // the game: the boot table's record for the area (ROM 0x4500), copied by 0x4526
    let area_index = 1usize;
    let rec = CART_BOOT_TABLE + 16 * area_index;
    let be = |o: usize| u32::from_be_bytes(img.cart[o..o + 4].try_into().unwrap());
    let (src, dst, count) = (be(rec) as usize & 0x7F_FFFF, be(rec + 4), be(rec + 8) as usize);
    debug_assert_eq!(dst, GAME_BASE);
    ram.bytes(dst, &img.cart[src..src + count * 4]);
    boot_program(&mut ram, img.cart, eeprom, backup);
    // ROM 0x4516: the stack is the long word at 0x060002a8 (the game's, if it set one) else ROM's
    let sp = match ram.r32(0x0600_02A8) {
        0 => ram.rom32(0x510),
        sp => sp,
    };
    Loader { entry: be(rec + 12), dst_end: dst + 4 * count as u32, src_end: 0x2200_0000 + src as u32 + 4 * count as u32, sp }
}

/// The BIOS's variables in work RAM-H (0x06000200-0x06000c00) and the stores of its init
/// routines, in the order the BIOS runs them (ROM 0x31c4 on; each store says its routine).
fn boot_variables(ram: &mut Ram, eeprom: &[u8; 128]) {
    // ROM 0x3080: zeros 0x180 bytes from 0x06000700; 0x3058: four halfwords at 0x06000814
    ram.fill(0x0600_0700, 0x180, 0);
    // the EEPROM's image, read once by ROM 0x47c8 (the settings are in two copies)
    ram.bytes(0x0600_0780, eeprom);
    // ROM 0x3206: the start-up mark
    ram.w32(0x0600_0240, 0x3252_4459);
    // ROM 0x339a: the settings word: the flag 0x1000 (ROM 0x3018) or the EEPROM's word at
    // 0x06000798 (its word 12) in the low 12 bits
    let settings = 0x1000 | (ram.r16(0x0600_0798) as u32 & 0x0FFF);
    ram.w32(0x0600_0248, settings);
    // ROM 0x02b8
    ram.w32(0x0600_024C, 0);
    // ROM 0x4718: the entry for the cart's boot (0x06004100)
    ram.w32(0x0600_0254, 0x0600_4100);
    // ROM 0x326e-0x3278: the exits, unless the EEPROM's word at 0x0600079a selects the other mode
    if (ram.r16(0x0600_079A) >> 8) & 3 != 1 {
        ram.w32(0x0600_0268, 0x4574);
        ram.w32(0x0600_026C, 0x4574);
        ram.w32(0x0600_0288, 0x458A);
        ram.w32(0x0600_028C, 0x4578);
    }
    // ROM 0x4738: the size of the cart's boot record's header
    ram.w32(0x0600_02A0, 0x1000);
    // ROM 0x32c4
    ram.w32(0x0600_02C4, 0x32CE);
    // ROM 0x43e2: the result of reading the SCU's A-bus region (1: nothing there)
    ram.w32(0x0600_0324, 1);
    // the RAM code at 0x06002046: the interrupt mask the BIOS leaves (ROM 0x3210, r4 = ~0x8001)
    ram.w32(0x0600_0348, 0xFFFF_7FFE);
    // ROM 0x32be: the area byte the boot table is chosen by (1: the first record); 0x3798 and
    // 0x4d54: settings bytes the EEPROM leaves at their defaults
    ram.w8(0x0600_0656, 1);
    ram.w8(0x0600_0650, 0);
    ram.w8(0x0600_0653, 0);
    // the slot scan (boot program 0x0601270c; ROM 0x459c): what each of the four slots held
    ram.w16(0x0600_064C, 0x3332);
    ram.w16(0x0600_0658, 0xFFF0);
    ram.w16(0x0600_065A, 0);
}

/// The stores of the boot program (ROM 0x8000, run at 0x06010000) and of the RAM code that stay
/// after the game's copy over the program's own area.
fn boot_program(ram: &mut Ram, cart: &[u8], eeprom: &[u8; 128], backup: &[u8]) {
    let _ = eeprom;
    // 0x06012eac, 0x060127b0, 0x06012770, 0x06012762: per slot a type byte and a halfword,
    // the first slot (the cart's) from the EEPROM's word 14, the others empty (0xff)
    ram.fill(0x0600_06B4, 12, 0xFF);
    ram.w8(0x0600_06B4, 0);
    let w14 = ram.r16(0x0600_079C);
    ram.w16(0x0600_06B8, w14);
    // 0x0601306e: the title (16 bytes of the cart header's, the region of the settings word)
    // over 0xff
    let region = (ram.r32(0x0600_0248) >> 12) as usize & 0xF;
    ram.fill(0x0600_06C0, 0x40, 0xFF);
    ram.bytes(0x0600_06C0, &cart[CART_TITLES + 16 * region..][..16]);
    ram.w8(0x0600_0730, 0xFF);
    device_scan(ram, backup);
    // ROM 0x0f06: the operating-time count (the BIOS adds to it on each pass of its test; a
    // fixed value here: two of its bytes are all the game sees)
    ram.w32(0x0600_0758, 0x13CE_8A80);
    // 0x06010018, ROM 0x32d8 and 0x32e8: flags
    ram.w8(0x0600_0807, 1);
    ram.w16(0x0600_0820, 1);
    ram.w32(0x0600_0824, 0x24D);
    // RAM code 0x060014b4: the first two entries of the vector table at 0x06000a00 (the
    // handlers), ROM 0x44f4: the rest empty hooks (a `rts`), RAM code 0x060014ce: 32 pairs
    ram.w32(0x0600_0A00, 0x0600_0C34);
    ram.w32(0x0600_0A04, 0x0601_53DE);
    let hook = 0x44FC;
    for i in 0..30 {
        ram.w32(0x0600_0A08 + 4 * i, hook);
    }
    for i in 0..32 {
        let v = ram.rom32(rom::TABLE_A80 + 4 * i);
        ram.w32(0x0600_0A80 + 4 * i, v);
    }
    // RAM code 0x060020a6-0x060020ec: the variables inside the RAM code
    ram.fill(0x0600_2864, 0x18, 0);
    ram.w32(0x0600_2878, 0x98);
    // ROM 0x115c-0x11be: the SCSP's registers' addresses, kept for the RAM code
    for (i, a) in [0x480u32, 0x440, 0x700, 0x500, 0x780, 0x790, 0x794, 0x7A0, 0x7B0, 0x700].into_iter().enumerate() {
        ram.w32(0x0600_2CA0 + 4 * i as u32, 0x25A0_0000 + a);
    }
    // ROM 0x3388, 0x338a: the EEPROM's bit 12 of the word at 0x0600079a, twice
    let bit = ((ram.r16(0x0600_079A) >> 12) & 1) as u8;
    ram.w8(0x0600_2CC8, bit);
    ram.w8(0x0600_2CC9, bit);
    ram.w32(0x0600_2CCC, 0x24);
    ram.w32(0x0600_2CD0, 0x0D);
    leftovers(ram);
    // the boot program's data after the game: copied from the BIOS ROM (0x066000), zeros after
    let data = ram.bios[0x6_6000..0x7_0000].to_vec();
    ram.bytes(0x060F_0000, &data);
    ram.fill(0x060F_A000, 0x1600, 0);
}

/// RAM code 0x0600150e-0x060016ae: the status of the machine's devices at 0x06000740-0x06000750
/// from the EEPROM's settings (the bytes at 0x06000795 and 0x0600079a) and the backup RAM's
/// settings block. Only the case where bit 2 of the byte at 0x06000795 is clear is written here.
fn device_scan(ram: &mut Ram, backup: &[u8]) {
    let b795 = ram.h[Ram::idx(0x0600_0795)];
    let kind = (ram.r16(0x0600_079A) & 3) as u8;
    // 0x0600151e-0x06001558: the bits of the byte (an odd one drops bit 1)
    let c = if b795 & 1 != 0 { b795 & 5 & !2 } else { b795 & 7 };
    ram.w8(0x0600_0750, (kind << 3) | c);
    ram.w8(0x0600_0751, if c & 2 != 0 { 0x12 } else { 0x11 });
    if b795 & 4 != 0 {
        return; // the per-device scan of the backup RAM (0x060015a2) is not ported
    }
    // 0x06001628: the settings block's first byte and the byte after the block's rows
    let (first, second) = (backup[0x3D05], backup[0x3D5D]);
    ram.w8(0x0600_0740, first);
    ram.w8(0x0600_0741, second);
    ram.w8(0x0600_0742, ram.h[Ram::idx(0x0600_0790)]);
    // 0x0600164a: two entries (0x06000743, 0x06000747)
    for i in 0..2u32 {
        let v = if b795 & 1 != 0 {
            2
        } else if first | second == 0 {
            0
        } else {
            // a word of the table at 0x06000828 + i, set by ROM 0x32e8: nothing there at the handover
            let busy = b795 & 2 != 0 && ram.h[Ram::idx(0x0600_0828 + i)] == 0;
            let r10 = 1 + u8::from(busy);
            if r10 > first { 1 } else { 2 }
        };
        ram.w8(0x0600_0743 + 4 * i, v);
    }
}

/// The BIOS's changes to the backup RAM: its boot counter (a 16-bit count at logical bytes 2-3
/// that grows by 0x0483 each boot) and, in a blank one, the settings block (two copies) that
/// the EEPROM's word 14 goes into. Each logical byte is on an odd address.
fn backup_boot(backup: &mut [u8], eeprom: &[u8; 128]) {
    let count = u16::from_be_bytes([backup[5], backup[7]]).wrapping_add(0x0483);
    backup[5] = (count >> 8) as u8;
    backup[7] = count as u8;
    if backup[0x3D01..0x3D09].iter().all(|&b| b == 0) {
        let rows = [(0x3D01usize, 0xA4u8, 0x09u8), (0x3D81, eeprom[28], eeprom[29]), (0x3DC1, 0xFF, 0xFF), (0x3E01, 0xFF, 0xFF), (0x3E41, 0xFF, 0xFF)];
        for copy in [0usize, 0x180] {
            for (at, hi, lo) in rows {
                backup[at + copy] = hi;
                backup[at + copy + 2] = lo;
            }
        }
    }
}

/// What the BIOS leaves that nothing reads (docs/HANDOFF.md, "What does not matter"): the boot
/// program's variables inside the game's image (0x06041aa0-0x06041b28; the game's `.bss` is
/// there, cleared by its `main`) and the stack below the master's r15 (0x06100000): an interrupt's
/// register frame and the loader's calls. Kept so that the state is byte for byte the BIOS's.
fn leftovers(ram: &mut Ram) {
    ram.fill(0x0604_1AA0, 0x88, 0);
    for a in (0x0604_1AD4..0x0604_1AE0).step_by(4).chain((0x0604_1B14..0x0604_1B20).step_by(4)) {
        ram.w32(a, u32::from_be_bytes(SEGA));
    }
    for (a, v) in [(0x0604_1AA4, 0x40), (0x0604_1AE4, 0x9_E520), (0x0604_1AFC, 0xFFFF_FFF8), (0x0604_1B24, 0x9_E4E0)] {
        ram.w32(a, v);
    }
    const STACK: [u32; 51] = [
        0x060f_ff40, 0, 0, 0x0600_2cc8, 0x2b, 0x0601_547e, 0x060f_ff58, 0, 0x060f_ff60, 0, 0x060f_ff68, 0, 0x14b6,
        0x1458, 0x1_0000, 1, 1, 0x0600_081d, 0x0600_12ca, 0x0c00_0c3e, 0x0600_0c3e, 0x14b6, 0x0600_2030, 0x060f_ff9c,
        0, 0, 0, 0, 0, 0, 0x0600_081d, 0x0600_12ca, 0x0c00_0c3e, 0x0600_0c3e, 0, 0x0600_2030, 0, 0xffff_fe00, 0x4516,
        0x0600_4000, 0x50d, 0x222b_6bd0, 0x0603_abcc, 0xffff_7ffe, 0x0600_0254, 0x0600_40ec, 0x2220_0000, 0, 0x4550,
        0, 0x4516,
    ];
    for (i, v) in STACK.into_iter().enumerate() {
        ram.w32(0x060F_FF34 + 4 * i as u32, v);
    }
}

/// Builds the machine at the handover.
pub fn build(img: &Inputs) -> Machine {
    let mut m = Machine::new(img.bios.to_vec(), img.cart.to_vec(), img.area, img.eeprom);
    let eeprom: [u8; 128] = {
        let mut e = [0u8; 128];
        for (i, w) in m.sys.smpc.eeprom.mem.iter().enumerate() {
            e[2 * i..2 * i + 2].copy_from_slice(&w.to_be_bytes());
        }
        e
    };
    if let Some(b) = img.backup
        && b.len() == m.sys.backup.len()
    {
        m.sys.backup.copy_from_slice(b);
    }
    fill_sega(&mut m.sys.wram_l);
    let loader = work_ram_h(&mut m.sys.wram_h, img, &eeprom, &m.sys.backup);
    backup_boot(&mut m.sys.backup, &eeprom);
    master(&mut m, &loader);
    onchip(&mut m);
    devices(&mut m, img);
    m
}

/// What the BIOS's loader (ROM 0x4500-0x4550) leaves in the master's registers.
struct Loader {
    /// The game's entry, from the boot table.
    entry: u32,
    /// The end of the destination and of the source of the copy, in the CPU's view.
    dst_end: u32,
    src_end: u32,
    /// The stack pointer the loader takes: the game's own (the long word at 0x06000 2a8) or ROM's.
    sp: u32,
}

/// The master at the handover: the loader has just jumped to the game's entry (`jmp @r7` after
/// `bsr`, so pr is ROM 0x4516, the instruction after the call); no delayed branch. Registers
/// r0-r5 are the loader's leftovers (r0 the address of the ROM's stack literal, r1 the cart's
/// bus address, r2 the game's header word pointer, r3 the settings pointer at 0x06000254).
fn master(m: &mut Machine, l: &Loader) {
    let c = &mut m.cpu[0];
    c.r = [0x510, 0x2220_0000, GAME_BASE + 0xEC, 0x0600_0254, l.dst_end, l.src_end, 0, l.entry, 0, 0, 0, 0, 0, 0, 0, l.sp];
    c.pc = l.entry;
    c.pr = 0x4516;
    c.sr = 1;
    c.gbr = 0xFFFF_FE00;
    c.vbr = RAM_H;
    c.mach = 0;
    c.macl = 0;
    c.delay = None;
    c.sleeping = false;
}

/// The master's on-chip registers the BIOS sets (ROM 0x3020: vectors and the bus controller,
/// 0x3170 and 0x43c0 the cache and FRT) and, as the instant has them, the free-running timer.
fn onchip(m: &mut Machine) {
    let o = &mut m.sys.onchip[0];
    o.bsc = [0x03F1, 0x00FC, 0x5555, 0x0078, 0x0008, 0, 0x36];
    o.iprb = 0x0100;
    (o.vcra, o.vcrb, o.vcrc, o.vcrd, o.vcrwdt) = (0x6061, 0x6263, 0x6465, 0x6600, 0x6869);
    o.divu.vcrdiv = 0x6E;
    o.dma[0].vcr = 0x6D;
    o.dma[1].vcr = 0x6C;
    o.icr = 1;
    o.ccr = 1;
    o.wtcsr = 0x1E;
    o.frt.tier = 0x81;
    // run dependent (the boot's length): the counter and its flags at the instant
    o.frt.ftcsr = 0x0E;
    o.frt.frc = 0x9CB0;
}

/// The devices as the BIOS's init (ROM 0x31c4 on) leaves them.
fn devices(m: &mut Machine, img: &Inputs) {
    let s = &mut m.sys;
    // the SCU (ROM 0x3130, 0x3162 and others): VBlank-IN unmasked alone, all three DMA levels in
    // mode 7 (start by GO), the timers and the A-bus
    s.scu.ims = 0x3FFE;
    for d in &mut s.scu.dma {
        d.mode = 7;
    }
    s.scu.t0c = 0x3FF;
    s.scu.t1s = 0x1FF;
    s.scu.aiack = 1;
    s.scu.asr = [0x1FF0_1FF0; 2];
    s.scu.aref = 0x1F;
    s.scu.rsel = 1;
    // the instant's: the pending sources (VBlank-OUT, HBlank-IN, sprite end) and timer 0's lines
    s.scu.ist = 0x2006;
    s.scu.t0_count = 229;
    // the SMPC: RESENAB was the last command, SNDON and CKCHG352 were done (the machine's
    // `sound_on`, `clock352`), the ports are direct with the EEPROM's and sound lines as outputs
    let p = &mut s.smpc;
    p.comreg = 0x19;
    p.oreg[31] = 0x19;
    p.data_dir = [[0, 0x3F], [0, 0x18]];
    p.direct = [true; 2];
    p.exle = [true; 2];
    p.port_state = [0x40, 0x67];
    p.eeprom = stv::eeprom::Eeprom::new(img.eeprom);
    p.sctrl = 0;
    p.sctrl_events.clear();
    p.dotsel = true;
    p.reset_enabled = true;
    // run dependent: the clock at the instant
    p.rtc_seconds = 19;
    p.rtc_cycles = 9_903_420;
    s.sound_on = true;
    s.clock352 = true;
    s.line = HANDOVER_LINE;
    s.frame = HANDOVER_FRAME;
    // VDP1 (ROM 0x3162...): the registers
    s.vdp1.ptmr = 2;
    s.vdp1.ewrr = 0x58E0;
    s.vdp1.edsr = 3;
    // VDP2: the display registers the BIOS set
    for (a, v) in VDP2_REGS {
        s.vdp2.regs[a..a + 2].copy_from_slice(&v.to_be_bytes());
    }
}

/// The instant's line and frame count (the oracle's), which a native boot may start at.
const HANDOVER_LINE: u32 = 228;
const HANDOVER_FRAME: u64 = 1159;

/// VDP2's non-zero registers (offset, value) the BIOS leaves: TVMD, RAMCTL, the cycle and screen
/// settings of its last screen.
const VDP2_REGS: [(usize, u16); 28] = [
    (0x000, 0x0001), (0x00E, 0x1323), (0x010, 0xFFFF), (0x012, 0xFFFF), (0x014, 0x4455), (0x016, 0xFFFF),
    (0x018, 0xFFFF), (0x01A, 0xFFFF), (0x01C, 0x0031), (0x01E, 0x1FF7), (0x020, 0x0008), (0x036, 0xC00C),
    (0x04C, 0x3131), (0x04E, 0x3131), (0x078, 0x0001), (0x07C, 0x0001), (0x088, 0x0001), (0x08C, 0x0001),
    (0x0BC, 0x12F1), (0x0E0, 0x0020), (0x0E4, 0x4000), (0x0F8, 0x0204), (0x0FA, 0x0703), (0x0FC, 0x0005),
    (0x110, 0x0048), (0x114, 0xFF00), (0x116, 0xFF00), (0x118, 0xFF00),
];

// ---- the check -------------------------------------------------------------------------------

use stv::state::State;

/// The extents (start, end) where `a` and `b` differ, differences closer than `gap` joined.
fn extents(a: &[u8], b: &[u8], gap: usize) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    for i in (0..a.len().min(b.len())).filter(|&i| a[i] != b[i]) {
        match out.last_mut() {
            Some(l) if i < l.1 + gap => l.1 = i + 1,
            _ => out.push((i, i + 1)),
        }
    }
    out
}

/// Compares a memory; prints the extents; returns the number of bytes that differ.
fn cmp_mem(name: &str, base: u32, native: &[u8], reference: &[u8], show: usize) -> usize {
    let n = native.iter().zip(reference).filter(|(a, b)| a != b).count();
    let ex = extents(native, reference, 16);
    if n == 0 {
        println!("{name}: equal ({} bytes)", native.len());
        return 0;
    }
    println!("{name}: {n:#x} bytes differ in {} extents (native / reference)", ex.len());
    for &(s, e) in ex.iter().take(show) {
        let end = (s + 12).min(e);
        println!("  {:08x}-{:08x}  {} / {}", base as usize + s, base as usize + e, hex(&native[s..end]), hex(&reference[s..end]));
    }
    if ex.len() > show {
        println!("  ... {} more extents", ex.len() - show);
    }
    n
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Compares two parts through their `Debug` text, naming the fields that differ.
fn cmp_debug(name: &str, native: &dyn std::fmt::Debug, reference: &dyn std::fmt::Debug, show: usize) -> usize {
    let (a, b) = (format!("{native:#?}"), format!("{reference:#?}"));
    let (la, lb): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    let mut path: Vec<(usize, String)> = Vec::new();
    let mut diffs = Vec::new();
    for (x, y) in la.iter().zip(&lb) {
        let indent = x.len() - x.trim_start().len();
        while path.last().is_some_and(|p| p.0 >= indent) {
            path.pop();
        }
        if x != y {
            let names: Vec<&str> = path.iter().map(|p| p.1.as_str()).collect();
            diffs.push(format!("{}  {} / {}", names.join("."), x.trim().trim_end_matches(','), y.trim().trim_end_matches(',')));
        }
        if let Some(f) = x.trim().strip_suffix(['[', '{', '(']) {
            path.push((indent, f.trim_end_matches([':', ' ']).to_string()));
        }
    }
    if la.len() != lb.len() {
        diffs.push(format!("(different lengths: {} / {} lines)", la.len(), lb.len()));
    }
    if diffs.is_empty() {
        println!("{name}: equal");
    } else {
        println!("{name}: {} lines differ (native / reference)", diffs.len());
        for d in diffs.iter().take(show) {
            println!("  {d}");
        }
        if diffs.len() > show {
            println!("  ... {} more", diffs.len() - show);
        }
    }
    diffs.len()
}

/// Compares a part's saved bytes.
fn cmp_saved(name: &str, native: &dyn State, reference: &dyn State) -> usize {
    let (mut a, mut b) = (Vec::new(), Vec::new());
    native.save(&mut a);
    reference.save(&mut b);
    let n = if a.len() == b.len() { extents(&a, &b, 1).len() } else { usize::MAX };
    if n == 0 {
        println!("{name}: equal ({} bytes saved)", a.len());
    } else {
        println!("{name}: differs ({} extents of {} saved bytes)", n, a.len());
    }
    n
}

/// Builds the native boot state and compares it with the reference (`entry.state`, made by
/// `tools/boot/reach`) part by part; prints the differences. Returns the number of parts that
/// differ.
pub fn check(entry_state: &[u8], img: &Inputs, show: usize) -> Result<usize, String> {
    let native = build(img);
    let mut reference = Machine::new(img.bios.to_vec(), img.cart.to_vec(), img.area, img.eeprom);
    reference.load_state(entry_state)?;
    let (n, r) = (&native.sys, &reference.sys);
    let mut bad = 0;
    let mut part = |d: usize| bad += usize::from(d > 0);
    println!("== work RAM, backup RAM");
    part(cmp_mem("work RAM-H", RAM_H, &n.wram_h, &r.wram_h, show));
    part(cmp_mem("work RAM-L", RAM_L, &n.wram_l, &r.wram_l, show));
    part(cmp_mem("backup RAM", 0, &n.backup, &r.backup, show));
    println!("== CPUs");
    for i in 0..2 {
        part(cmp_debug(&format!("cpu[{i}]"), &native.cpu[i], &reference.cpu[i], show));
        part(cmp_debug(&format!("onchip[{i}]"), &n.onchip[i], &r.onchip[i], show));
    }
    part(usize::from(n.slave_on != r.slave_on));
    println!("slave_on: native {} reference {}", n.slave_on, r.slave_on);
    println!("== devices");
    part(cmp_debug("scu", &n.scu, &r.scu, show));
    part(cmp_debug("smpc", &n.smpc, &r.smpc, show));
    part(cmp_mem("vdp1 vram", 0, &n.vdp1.vram, &r.vdp1.vram, show));
    part(cmp_mem("vdp2 vram", 0, &n.vdp2.vram, &r.vdp2.vram, show));
    part(cmp_mem("vdp2 cram", 0, &n.vdp2.cram, &r.vdp2.cram, show));
    part(cmp_mem("vdp2 regs", 0, &n.vdp2.regs, &r.vdp2.regs, show));
    part(cmp_mem("sound RAM", 0, &n.scsp.ram, &r.scsp.ram, show));
    part(cmp_saved("vdp1 (all)", &n.vdp1, &r.vdp1));
    part(cmp_saved("vdp2 (all)", &n.vdp2, &r.vdp2));
    part(cmp_saved("scsp (all)", &n.scsp, &r.scsp));
    part(cmp_debug("m68k", &n.m68k, &r.m68k, show));
    println!("== machine");
    for (name, a, b) in [
        ("sound_on", n.sound_on as u64, r.sound_on as u64),
        ("clock352", n.clock352 as u64, r.clock352 as u64),
        ("line", n.line as u64, r.line as u64),
        ("frame", n.frame, r.frame),
        ("sound_steps", n.sound_steps, r.sound_steps),
    ] {
        if a != b {
            println!("{name}: native {a} reference {b}");
            bad += 1;
        }
    }
    println!("{bad} parts differ");
    Ok(bad)
}
