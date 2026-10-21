//! Text on VDP2's text plane: characters as tiles (a font of one tile a character, and a large one
//! of 2x2), and the players' lines along the screen's edge ("INSERT COIN(S)", "PRESS START
//! BUTTON", the credits): each player's line and credit line are kept in buffers and sent to the
//! plane at the VBlank, where their place depends on the display's mode and on whether each player
//! has a coin slot.

use super::nested;
use super::globals::{GAME_FLAGS, HELD};
use super::{objects, screen, sound, system, Cpu};

/// The font: each character's tile (i16 a character, plus TILE_BASE), and the large font's (four
/// tiles a character, 2x2; from index LARGE_NARROW on, one column of two).
const FONT: u32 = 0x0607_3828;
const LARGE_FONT: u32 = 0x0607_36b0;
const TILE_BASE: u16 = 0x21;
const LARGE_NARROW: i32 = 0x29;
/// The plane's rows (tiles).
const ROW: u32 = 0x40;

/// The text display's mode (0, 1, 2) and the mode it takes at the next VBlank.
const MODE: u32 = 0x0607_35ec;
const NEXT_MODE: u32 = 0x0607_35f0;
/// Each player's line and credit line: to be cleared at the VBlank, and the buffer to send (0:
/// none). (player 0, player 1 at +4.)
const CLEAR_LINE: u32 = 0x0607_35f4;
const CLEAR_CREDIT: u32 = 0x0607_35fc;
const LINE: u32 = 0x0607_3604;
const CREDIT_LINE: u32 = 0x0607_360c;
/// The players' own buffers (line, credit line), and the lines made from messages (19 tiles a
/// player).
const BUFFERS: [(u32, u32); 2] = [(0x0607_3614, 0x0607_3632), (0x0607_3662, 0x0607_3680)];
const MESSAGE_LINES: u32 = 0x0607_4850;
const MESSAGE_LINE: u32 = 0x26;
const LINE_TILES: u32 = 19;
/// The messages (their addresses, by number).
const MESSAGES: u32 = 0x0607_38d0;
/// While -1 the text plane is the text's own (lines are cleared and sent whole); else the lines
/// are laid over it (blank tiles left alone). Last frame's, as the screen moves it on.
const PLANE_SHARED: u32 = 0x0607_474c;
const OWN: u32 = u32::MAX;
const BLANK: u16 = 0x21;

/// The credit display (an object, 0 when there is none): shown (+0x10), and the credits and coins
/// it last drew for each player (+0x14, +0x18).
const CREDITS: u32 = 0x0607_35e8;
const CREDITS_SIZE: u32 = 0x6c;
const CREDITS_EXEC: u32 = 0x0606_0e7c;
const SHOWN: u32 = 0x10;
const DRAWN: u32 = 0x14;

/// A character's place in the font.
pub fn char_index(_cpu: &mut Cpu, c: u32) -> u32 {
    let c = c as i8 as i32;
    let i = match c as u8 {
        b' ' => 0,
        b'\'' => 0x2f,
        b'(' => 0x2d,
        b')' => 0x2e,
        b',' => 0x29,
        b'-' => 0x26,
        b'.' => 0x2b,
        b'/' => 0x28,
        b'0'..=b'9' => c - 0x15,
        b':' => 0x2a,
        b';' => 0x2c,
        b'?' => 0x27,
        b'_' => 0x25,
        _ if c < 0x61 => c - 0x40,
        _ => c - 0x31,
    };
    i as u32
}

fn tile(cpu: &mut Cpu, font: u32, index: u32) -> u16 {
    cpu.read16(font.wrapping_add(2 * index)).wrapping_add(TILE_BASE)
}

/// Puts character `c` at `at` (attributes `attr` or'd in): 1 (tile).
pub fn put_char(cpu: &mut Cpu, c: u32, at: u32, attr: u32) -> u32 {
    let i = nested!(cpu, 0x0606_0ff8, char_index(cpu, c));
    let t = tile(cpu, FONT, i);
    cpu.write16(at, attr as u16 | t);
    1
}

/// Puts the string at `s` from `at` on.
pub fn put_string(cpu: &mut Cpu, s: u32, at: u32, attr: u32) {
    let (mut s, mut at) = (s, at);
    loop {
        let c = cpu.read8(s);
        if c == 0 {
            break;
        }
        let i = nested!(cpu, 0x0606_0ff8, char_index(cpu, c as u32));
        let t = tile(cpu, FONT, i);
        cpu.write16(at, t | attr as u16);
        s += 1;
        at += 2;
    }
}

/// Puts `n` (0-99) at `at`: the digits put.
pub fn put_number(cpu: &mut Cpu, n: u32, at: u32) -> u32 {
    let n = n as i32;
    if n < 10 {
        nested!(cpu, 0x0606_10e4, put_char(cpu, (b'0' as i32 + n) as u32, at, 0));
        return 1;
    }
    let tens = n / 10;
    nested!(cpu, 0x0606_10e4, put_char(cpu, (b'0' as i32 + tens) as u32, at, 0));
    nested!(cpu, 0x0606_10e4, put_char(cpu, (b'0' as i32 + n - 10 * tens) as u32, at + 2, 0));
    2
}

/// Puts character `c` in the large font at `at`: the columns it took.
pub fn put_large_char(cpu: &mut Cpu, c: u32, at: u32, attr: u32) -> u32 {
    let i = nested!(cpu, 0x0606_0ff8, char_index(cpu, c)) as i32;
    let attr = attr as u16;
    let base = LARGE_FONT.wrapping_add((8 * i) as u32);
    if i < LARGE_NARROW {
        for (k, offset) in [0, 2, 2 * ROW, 2 * ROW + 2].into_iter().enumerate() {
            let t = (cpu.read16(base + 2 * k as u32)).wrapping_add(TILE_BASE);
            cpu.write16(at + offset, t | attr);
        }
        2
    } else {
        for (k, offset) in [0, 2 * ROW].into_iter().enumerate() {
            let t = (cpu.read16(base + 2 * k as u32)).wrapping_add(TILE_BASE);
            cpu.write16(at + offset, t | attr);
        }
        1
    }
}

/// Puts the string at `s` in the large font from `at` on.
pub fn put_large_string(cpu: &mut Cpu, s: u32, at: u32, attr: u32) {
    let (mut s, mut at) = (s, at);
    while cpu.read8(s) != 0 {
        let c = cpu.read8(s) as u32;
        // inline: put_large_char's code is the SH-2 function's own here (it calls char_index)
        let columns = put_large_char(cpu, c, at, attr);
        at += 2 * columns;
        s += 1;
    }
}

/// The display's mode for the next VBlank, both players' lines hidden.
pub fn set_mode(cpu: &mut Cpu, mode: u32) {
    cpu.write32(NEXT_MODE, mode);
    nested!(cpu, 0x0606_0b34, hide_line(cpu, 0));
    nested!(cpu, 0x0606_0b34, hide_line(cpu, 1));
}

/// Player `p`'s line and credit line hidden (cleared at the VBlank if they were shown).
pub fn hide_line(cpu: &mut Cpu, p: u32) {
    let p4 = 4 * p;
    if cpu.read32(LINE + p4) != 0 {
        cpu.write32(CLEAR_LINE + p4, 1);
        cpu.write32(LINE + p4, 0);
    }
    if cpu.read32(CREDIT_LINE + p4) != 0 {
        cpu.write32(CLEAR_CREDIT + p4, 1);
        cpu.write32(CREDIT_LINE + p4, 0);
    }
}

/// Player `p`'s line shows message `message` (its credit line hidden).
pub fn show_message(cpu: &mut Cpu, p: u32, message: u32) {
    let text = cpu.read32(MESSAGES + 4 * message);
    let line = MESSAGE_LINES + MESSAGE_LINE * p;
    for i in 0..LINE_TILES {
        let c = cpu.read8(text.wrapping_add(i)) as i8 as i32 as u32;
        nested!(cpu, 0x0606_10e4, put_char(cpu, c, line + 2 * i, 0));
    }
    let p4 = 4 * p;
    cpu.write32(LINE + p4, line);
    if cpu.read32(CREDIT_LINE + p4) != 0 {
        cpu.write32(CLEAR_CREDIT + p4, 1);
        cpu.write32(CREDIT_LINE + p4, 0);
    }
}

/// Player `p`'s own buffers shown (line and credit line).
pub fn show_buffers(cpu: &mut Cpu, p: u32) {
    nested!(cpu, 0x0606_0b34, hide_line(cpu, p));
    if let Some(&(line, credit)) = BUFFERS.get(p as usize) {
        cpu.write32(LINE + 4 * p, line);
        cpu.write32(CREDIT_LINE + 4 * p, credit);
    }
}

/// The credit display shown: 0, or -1 if there is none.
pub fn credits_shown(cpu: &mut Cpu) -> u32 {
    let d = cpu.read32(CREDITS);
    if d == 0 {
        return u32::MAX;
    }
    cpu.write8(d + SHOWN, 1);
    0
}

/// The credit display hidden, to draw anew: 0, or -1 if there is none.
pub fn credits_hidden(cpu: &mut Cpu) -> u32 {
    let d = cpu.read32(CREDITS);
    if d == 0 {
        return u32::MAX;
    }
    cpu.write8(d + SHOWN, 0);
    cpu.write32(d + DRAWN, u32::MAX);
    cpu.write32(d + DRAWN + 4, u32::MAX);
    0
}

/// The credit display made (object list 0): it, or 0.
pub fn credits_set(cpu: &mut Cpu) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, CREDITS_SIZE));
    nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 0, blocks as i8 as i32 as u32, CREDITS_EXEC))
}

/// Where player `p`'s line goes on the plane in the current mode (None: it has none: one coin slot
/// for both, the second player).
fn line_place(cpu: &mut Cpu, p: u32) -> Option<u32> {
    let mode = cpu.read32(MODE);
    let shared = |cpu: &mut Cpu| nested!(cpu, 0x0604_0f24, system::coin_settings_shown(cpu)) & 4 == 0;
    let either = |p: u32, first: u32, second: u32| if p != 0 { second } else { first };
    let at = match mode {
        0 => either(p, 0x25e7_e104, 0x25e7_e12e),
        1 if shared(cpu) => {
            if p != 0 {
                return None;
            }
            0x25e7_ea18
        }
        1 => either(p, 0x25e7_ea04, 0x25e7_ea2e),
        _ if shared(cpu) => {
            if p != 0 {
                return None;
            }
            0x25e7_e698
        }
        _ => either(p, 0x25e7_e684, 0x25e7_e6ae),
    };
    Some(at)
}

fn clear_tiles(cpu: &mut Cpu, at: u32) {
    if cpu.read32(PLANE_SHARED) == OWN {
        for i in 0..LINE_TILES {
            cpu.write16(at + 2 * i, 0);
        }
    }
}

/// Player `p`'s line cleared on the plane (when the plane is the text's own).
pub fn clear_line(cpu: &mut Cpu, p: u32) {
    if let Some(at) = line_place(cpu, p) {
        clear_tiles(cpu, at);
    }
}

/// Player `p`'s credit line cleared on the plane (when the plane is the text's own).
pub fn clear_credit_line(cpu: &mut Cpu, p: u32) {
    let at = if p != 0 { 0x25e7_e1ae } else { 0x25e7_e184 };
    clear_tiles(cpu, at);
}

/// `tiles` tiles from `src` to the plane at `dst`: all of them when the plane is the text's own,
/// else only those not blank.
pub fn send_line(cpu: &mut Cpu, src: u32, dst: u32, tiles: u32) {
    let own = cpu.read32(PLANE_SHARED) == OWN;
    for i in 0..(tiles as i32).max(0) as u32 {
        let t = cpu.read16(src + 2 * i);
        if own || (t & 0x3ff != 0 && t & 0x3ff != BLANK) {
            cpu.write16(dst + 2 * i, t);
        }
    }
}

/// At the VBlank, first: the lines hidden since cleared, the display's next mode taken.
pub fn clear_hidden_lines(cpu: &mut Cpu) {
    for p in 0..2 {
        if cpu.read32(CLEAR_LINE + 4 * p) != 0 {
            nested!(cpu, 0x0606_06dc, clear_line(cpu, p));
            cpu.write32(CLEAR_LINE + 4 * p, 0);
        }
    }
    for p in 0..2 {
        if cpu.read32(CLEAR_CREDIT + 4 * p) != 0 {
            nested!(cpu, 0x0606_0790, clear_credit_line(cpu, p));
            cpu.write32(CLEAR_CREDIT + 4 * p, 0);
        }
    }
    let next = cpu.read32(NEXT_MODE);
    cpu.write32(MODE, next);
}

/// The credit display's sections, one a player (from +0x2c, 0x20 bytes each: "CREDITS 00 00/00"
/// made into tiles), and each one's width in tiles (+0x24).
const SECTION: u32 = 0x2c;
const SECTION_SIZE: u32 = 0x20;
const WIDTH: u32 = 0x24;
const COINS_DRAWN: u32 = 0x1c;
const TEMPLATE: u32 = 0x0607_3924;
const TEMPLATE_LEN: u32 = 16;
/// "S " (the plural).
const PLURAL: u32 = 0x0606_0cd4;
/// The director (director.rs), and its flag that a game is on (bit 15 of +0x44).
const GAME: u32 = 0x0607_46e0;
const GAME_FLAGS_44: u32 = 0x44;
const IN_GAME: u32 = 0x8000;
/// Where the credits go on the plane: one line for both players, or one each.
const CREDITS_SHARED_AT: u32 = 0x25e7_ed24;
const CREDITS_AT: [u32; 2] = [0x25e7_ed06, 0x25e7_ed52];

/// Player `p`'s credit section made anew: "CREDIT" (with an S from two), the credits, and with
/// coins toward the next, those and the coins a credit takes ("n/m").
pub fn credit_text(cpu: &mut Cpu, d: u32, p: u32, credits: u32, coins: u32) {
    let mut at = d + SECTION + SECTION_SIZE * p;
    let mut width;
    if (credits as i32) < 2 {
        nested!(cpu, 0x0606_10e4, put_char(cpu, b' ' as u32, at + 0xc, 0));
        at += 0xe;
        width = 7;
    } else {
        nested!(cpu, 0x0606_11b8, put_string(cpu, PLURAL, at + 0xc, 0));
        at += 0x10;
        width = 8;
    }
    let n = nested!(cpu, 0x0606_0c58, put_number(cpu, credits, at));
    at += 2 * n;
    width += n;
    if coins != 0 {
        nested!(cpu, 0x0606_10e4, put_char(cpu, b' ' as u32, at, 0));
        at += 2;
        let m = nested!(cpu, 0x0606_0c58, put_number(cpu, coins, at));
        at += 2 * m;
        nested!(cpu, 0x0606_10e4, put_char(cpu, b'/' as u32, at, 0));
        let per = system::coins_per_credit(cpu, p);
        let k = nested!(cpu, 0x0606_0c58, put_number(cpu, per, at + 2));
        width += m + 2 + k;
    }
    cpu.write32(d + WIDTH + 4 * p, width);
}

/// Player `p`'s credits on the plane at `at` (the second player's to its left): made anew when
/// they changed, sent when shown or not zero.
pub fn draw_credits(cpu: &mut Cpu, d: u32, p: u32, at: u32) {
    let coins = nested!(cpu, 0x0604_0fc0, system::coins(cpu, p));
    let credits = nested!(cpu, 0x0604_0f74, system::credits(cpu, p));
    let e = d + 4 * p;
    if cpu.read32(e + DRAWN) != credits || cpu.read32(e + COINS_DRAWN) != coins {
        let width = cpu.read32(e + WIDTH);
        let dst = if p != 0 { at.wrapping_sub(2 * width) } else { at };
        for i in 0..(width as i32).max(0) as u32 {
            cpu.write16(dst + 2 * i, 0);
        }
        nested!(cpu, 0x0606_0cd8, credit_text(cpu, d, p, credits, coins));
        cpu.write32(e + DRAWN, credits);
        cpu.write32(e + COINS_DRAWN, coins);
    }
    if cpu.read8(d + SHOWN) != 0 || credits != 0 || coins != 0 {
        let width = cpu.read32(e + WIDTH);
        let dst = if p != 0 { at.wrapping_sub(2 * width) } else { at };
        nested!(cpu, 0x0606_07c4, send_line(cpu, d + SECTION + SECTION_SIZE * p, dst, width));
    }
}

/// The credit display's frame: drawn while a game is on (made ready the first frame); else it
/// stops.
pub fn credits_exec(cpu: &mut Cpu, d: u32) {
    const MODE: u32 = 3;
    let in_game = |cpu: &mut Cpu| {
        let game = cpu.read32(GAME);
        game != 0 && cpu.read32(game + GAME_FLAGS_44) & IN_GAME != 0
    };
    match cpu.read8(d + MODE) {
        0 => {
            if !in_game(cpu) || cpu.read32(CREDITS) != d {
                if cpu.read32(GAME) == 0 || !in_game(cpu) {
                    cpu.write32(CREDITS, 0);
                }
                cpu.write8(d + MODE, b'p');
                return;
            }
            for p in 0..2 {
                let at = d + SECTION + SECTION_SIZE * p;
                for i in 0..TEMPLATE_LEN {
                    let c = cpu.read8(TEMPLATE + i) as i8 as i32 as u32;
                    nested!(cpu, 0x0606_10e4, put_char(cpu, c, at + 2 * i, 0));
                }
                cpu.write32(d + WIDTH + 4 * p, 0);
                cpu.write32(d + DRAWN + 4 * p, u32::MAX);
                cpu.write32(d + COINS_DRAWN + 4 * p, u32::MAX);
            }
            cpu.write8(d + SHOWN, 1);
            cpu.write8(d + MODE, 1);
        }
        1 => {}
        b'p' => {
            cpu.write8(d + MODE, b's');
            return;
        }
        b's' => {
            nested!(cpu, 0x0600_579c, objects::obj_free(cpu, d));
            return;
        }
        _ => return,
    }
    if !in_game(cpu) {
        cpu.write8(d + MODE, b'p');
        cpu.write32(CREDITS, 0);
        return;
    }
    if nested!(cpu, 0x0604_0f24, system::coin_settings_shown(cpu)) & 4 == 0 {
        nested!(cpu, 0x0606_0dac, draw_credits(cpu, d, 0, CREDITS_SHARED_AT));
    } else {
        nested!(cpu, 0x0606_0dac, draw_credits(cpu, d, 0, CREDITS_AT[0]));
        nested!(cpu, 0x0606_0dac, draw_credits(cpu, d, 1, CREDITS_AT[1]));
    }
}

/// Where each player's buffers go in mode 0: the line (its own buffer a little in, a message line
/// from the edge), and the credit line.
const MODE0_LINE: [(u32, u32); 2] = [(0x25e7_e108, 0x25e7_e104), (0x25e7_e132, 0x25e7_e12e)];
const MODE0_CREDIT: [u32; 2] = [0x25e7_e188, 0x25e7_e1b2];
const OWN_TILES: u32 = 15;

/// At the VBlank: the players' lines sent to the plane where the mode puts them.
pub fn send_lines(cpu: &mut Cpu) {
    if cpu.read32(MODE) == 0 {
        for p in 0..2usize {
            let line = cpu.read32(LINE + 4 * p as u32);
            if line != 0 {
                let (dst, tiles) = if line == BUFFERS[p].0 { (MODE0_LINE[p].0, OWN_TILES) } else { (MODE0_LINE[p].1, LINE_TILES) };
                nested!(cpu, 0x0606_07c4, send_line(cpu, line, dst, tiles));
            }
            let credit = cpu.read32(CREDIT_LINE + 4 * p as u32);
            if credit != 0 {
                nested!(cpu, 0x0606_07c4, send_line(cpu, credit, MODE0_CREDIT[p], OWN_TILES));
            }
        }
        return;
    }
    let (shared_at, each_at) = if cpu.read32(MODE) == 1 { (0x25e7_ea18, [0x25e7_ea04, 0x25e7_ea2e]) } else { (0x25e7_e698, [0x25e7_e684, 0x25e7_e6ae]) };
    if nested!(cpu, 0x0604_0f24, system::coin_settings_shown(cpu)) & 4 == 0 {
        let line = cpu.read32(LINE);
        if line != 0 {
            nested!(cpu, 0x0606_07c4, send_line(cpu, line, shared_at, LINE_TILES));
        }
    } else {
        for p in 0..2 {
            let line = cpu.read32(LINE + 4 * p);
            if line != 0 {
                nested!(cpu, 0x0606_07c4, send_line(cpu, line, each_at[p as usize], LINE_TILES));
            }
        }
    }
}

/// An object's mode byte (objects.rs) as the game's objects use it: 0 new, 1 running, ENDING set
/// to end (the next frame ENDED, the frame after that freed).
const OBJ_MODE: u32 = 3;
const ENDING: u8 = 0x70;
const ENDED: u8 = 0x73;

/// A typewriter (an object, list 0): a string put on the text plane a character a frame with a
/// cursor after it and a click, then the cursor blinking for its frames before it ends. Its fields:
/// the phase (0 typing, 1 blinking), the attributes ("^" and a digit in the string set the palette),
/// where the next character goes, the string's index, the flags (TYPE_*), the blink's frames, the
/// string.
const TYPE_SIZE: u32 = 0x28;
const TYPE_EXEC: u32 = 0x0606_131c;
const PHASE: u32 = 0x10;
const TYPE_ATTR: u32 = 0x12;
const TYPE_AT: u32 = 0x14;
const TYPE_INDEX: u32 = 0x18;
const TYPE_FLAGS: u32 = 0x1c;
const BLINK: u32 = 0x20;
const TYPE_STRING: u32 = 0x24;
/// The flags: the large font; no click; the text plane cleared as it ends.
const TYPE_LARGE: u32 = 1;
const TYPE_QUIET: u32 = 1 << 30;
const TYPE_CLEARS: u32 = 1 << 31;
const PALETTE_MARK: i8 = b'^' as i8;
const CURSOR: u32 = b'_' as u32;
const SE_CLICK: u32 = 0x35;

/// Makes a typewriter for string `s` at `at`: it, or 0.
pub fn typewriter_set(cpu: &mut Cpu, s: u32, at: u32, flags: u32, blink: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, TYPE_SIZE));
    let obj = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 0, blocks as i8 as i32 as u32, TYPE_EXEC));
    if obj != 0 {
        cpu.write32(obj + TYPE_STRING, s);
        cpu.write32(obj + TYPE_AT, at);
        cpu.write32(obj + TYPE_FLAGS, flags);
        cpu.write32(obj + BLINK, blink);
    }
    obj
}

/// A character put by the typewriter's font: the columns it took.
fn type_char(cpu: &mut Cpu, large: bool, c: u32, at: u32, attr: u32) -> u32 {
    if large {
        nested!(cpu, 0x0606_111c, put_large_char(cpu, c, at, attr))
    } else {
        nested!(cpu, 0x0606_10e4, put_char(cpu, c, at, attr))
    }
}

/// A typewriter's frame (not while the game is held).
pub fn typewriter_exec(cpu: &mut Cpu, obj: u32) {
    if cpu.read32(GAME_FLAGS) & HELD != 0 {
        return;
    }
    match cpu.read8(obj + OBJ_MODE) {
        1 => {}
        0 => {
            cpu.write32(obj + TYPE_INDEX, 0);
            cpu.write16(obj + TYPE_ATTR, 0);
            cpu.write8(obj + PHASE, 0);
            cpu.write8(obj + PHASE + 1, 0);
            cpu.write8(obj + OBJ_MODE, 1);
        }
        ENDING => {
            cpu.write8(obj + OBJ_MODE, ENDED);
            return;
        }
        ENDED => {
            if cpu.read32(obj + TYPE_FLAGS) & TYPE_CLEARS != 0 {
                nested!(cpu, 0x0605_fde8, screen::request_clear_text_plane(cpu));
            }
            nested!(cpu, 0x0600_579c, objects::obj_free(cpu, obj));
            return;
        }
        _ => return,
    }
    match cpu.read8(obj + PHASE) {
        0 => {
            let s = cpu.read32(obj + TYPE_STRING);
            let i = cpu.read32(obj + TYPE_INDEX);
            let mut c = cpu.read8(s.wrapping_add(i)) as i8;
            if c == 0 {
                cpu.write8(obj + PHASE, 1);
                return;
            }
            if c == PALETTE_MARK {
                cpu.write32(obj + TYPE_INDEX, i + 2);
                let s = cpu.read32(obj + TYPE_STRING);
                let digit = cpu.read8(s.wrapping_add(i + 1)) as i8 as i32 - b'0' as i32;
                cpu.write16(obj + TYPE_ATTR, (digit << 12) as u16);
                let s = cpu.read32(obj + TYPE_STRING);
                let i = cpu.read32(obj + TYPE_INDEX);
                c = cpu.read8(s.wrapping_add(i)) as i8;
            }
            let large = cpu.read32(obj + TYPE_FLAGS) & TYPE_LARGE != 0;
            let attr = cpu.read16(obj + TYPE_ATTR) as u32;
            let at = cpu.read32(obj + TYPE_AT);
            let columns = type_char(cpu, large, c as i32 as u32, at, attr);
            let at = cpu.read32(obj + TYPE_AT).wrapping_add(2 * columns);
            cpu.write32(obj + TYPE_AT, at);
            type_char(cpu, large, CURSOR, at, 0);
            if cpu.read32(obj + TYPE_FLAGS) & TYPE_QUIET == 0 {
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SE_CLICK));
            }
            let i = cpu.read32(obj + TYPE_INDEX);
            cpu.write32(obj + TYPE_INDEX, i.wrapping_add(1));
        }
        1 => {
            let on = cpu.read32(obj + BLINK) & 4 != 0;
            let large = cpu.read32(obj + TYPE_FLAGS) & TYPE_LARGE != 0;
            let at = cpu.read32(obj + TYPE_AT);
            type_char(cpu, large, if on { CURSOR } else { b' ' as u32 }, at, 0);
            let n = cpu.read32(obj + BLINK);
            cpu.write32(obj + BLINK, n.wrapping_sub(1));
            if n == 0 {
                cpu.write8(obj + OBJ_MODE, ENDING);
            }
        }
        _ => {}
    }
}

/// A message (an object): four lines typed one after another, each when the last is done (the
/// first three in the large font; the last blinks its cursor for LAST_BLINK frames and clears the
/// text plane as it ends). Its fields: the lines (four string addresses), the line typed, the
/// typewriter typing it.
const MESSAGE_LINES_AT: u32 = 0x10;
const LINE_TYPED: u32 = 0x15;
const TYPEWRITER: u32 = 0x18;
const MESSAGE_PLACES: [u32; 4] = [0x25e7_e988, 0x25e7_ea88, 0x25e7_eb88, 0x25e7_ec88];
const LAST_BLINK: u32 = 180;

/// A message's frame (not while the game is held).
pub fn message_exec(cpu: &mut Cpu, obj: u32) {
    if cpu.read32(GAME_FLAGS) & HELD != 0 {
        return;
    }
    match cpu.read8(obj + OBJ_MODE) {
        1 => {}
        0 => {
            cpu.write8(obj + LINE_TYPED - 1, 0);
            cpu.write8(obj + LINE_TYPED, 0);
            cpu.write8(obj + OBJ_MODE, 1);
            return;
        }
        ENDING => {
            cpu.write8(obj + OBJ_MODE, ENDED);
            return;
        }
        ENDED => {
            nested!(cpu, 0x0600_579c, objects::obj_free(cpu, obj));
            return;
        }
        _ => return,
    }
    let line = cpu.read8(obj + LINE_TYPED) as u32;
    if line > 3 {
        return;
    }
    if line > 0 {
        let last = cpu.read32(obj + TYPEWRITER);
        if (cpu.read8(last + OBJ_MODE) as i8) < ENDING as i8 {
            return;
        }
    }
    let lines = cpu.read32(obj + MESSAGE_LINES_AT);
    let s = cpu.read32(lines + 4 * line);
    if line < 3 {
        let t = nested!(cpu, 0x0606_14b0, typewriter_set(cpu, s, MESSAGE_PLACES[line as usize], TYPE_LARGE, 0));
        cpu.write32(obj + TYPEWRITER, t);
        let n = cpu.read8(obj + LINE_TYPED);
        cpu.write8(obj + LINE_TYPED, n.wrapping_add(1));
    } else {
        let t = nested!(cpu, 0x0606_14b0, typewriter_set(cpu, s, MESSAGE_PLACES[3], TYPE_CLEARS, LAST_BLINK));
        cpu.write32(obj + TYPEWRITER, t);
        cpu.write8(obj + OBJ_MODE, ENDING);
    }
}

/// Other objects that write the text plane: their exec and the function at their +0x38 (not known
/// yet).
const PLANE_WRITER_EXEC: u32 = 0x0604_dcb4;
const PLANE_WRITER_FUNC: u32 = 0x0605_975c;
const MESSAGE_EXEC: u32 = 0x0606_1504;

/// Ends every typewriter and message (and the other writers of the text plane) in list 0, and has
/// the text plane cleared.
pub fn stop_text_objects(cpu: &mut Cpu) {
    let mut obj = cpu.read32(objects::OBJ_INF);
    while obj != 0 {
        let exec = cpu.read32(obj + objects::EXEC);
        if exec == TYPE_EXEC || exec == MESSAGE_EXEC || (exec == PLANE_WRITER_EXEC && cpu.read32(obj + 0x38) == PLANE_WRITER_FUNC) {
            cpu.write8(obj + OBJ_MODE, ENDING);
        }
        obj = cpu.read32(obj + objects::NEXT);
    }
    nested!(cpu, 0x0605_fde8, screen::request_clear_text_plane(cpu));
}

/// A stage's opening message (with the BIOS's setting 1): after a wait, typed out a line at a
/// time on the text plane, the first and second lines its own, the second in the large font,
/// then three lines of its own 30 frames apart; an underline beneath grows meanwhile. After 180
/// frames it is cleared and ends.
const MS_PHASE: u32 = 0x3c;
const MS_LINES: u32 = 0x40;
const MS_WAIT: u32 = 0x50;
const MS_BAR: u32 = 0x52;
const MS_SIZE: u32 = 0x54;
const MS_EXEC: u32 = 0x0605_975c;
const MS_DRAW: u32 = 0x0605_9920;
const MS_FLAGS: u32 = 0xc0_0000;
const MS_DRAW_AT: u32 = 0x34;
const PLAIN_EXEC: u32 = 0x0604_dcb4;
/// The lines' places on the plane (VDP2 RAM), the fixed lines, the blank that clears them.
const MS_AT: [u32; 6] = [0x25e7_e808, 0x25e7_e888, 0x25e7_ea08, 0x25e7_eb08, 0x25e7_eb88, 0x25e7_ec08];
const MS_TITLE: u32 = 0x0607_3210;
const MS_RULE: u32 = 0x0607_322c;
const MS_BLANK: u32 = 0x0607_3248;
/// Each phase's typing: the line (0 its own first, then 0 its own n), where, flags, blink, and
/// the wait after.
const MS_LAST_LINES: [(u16, usize); 3] = [(150, 1), (120, 2), (90, 3)];
const MS_BAR_LONGEST: u32 = 0x80;
const MS_BAR_GROWTH: u32 = 16;

fn ms_type(cpu: &mut Cpu, s: u32, at: u32, flags: u32, blink: u32) {
    nested!(cpu, 0x0606_14b0, typewriter_set(cpu, s, at, flags, blink));
}

pub fn opening_exec(cpu: &mut Cpu, o: u32) {
    let phase = cpu.read8(o + MS_PHASE);
    let tick = |cpu: &mut Cpu| {
        let w = cpu.read16(o + MS_WAIT).wrapping_sub(1);
        cpu.write16(o + MS_WAIT, w);
        w as i16
    };
    match phase {
        0..=2 => {
            if tick(cpu) < 0 {
                cpu.write8(o + MS_PHASE, phase + 1);
                let (s, at, flags, blink, wait) = match phase {
                    0 => (MS_TITLE, MS_AT[0], 0, 30, 36),
                    1 => (cpu.read32(o + MS_LINES), MS_AT[1], 1, 0xbc, 60),
                    _ => (MS_RULE, MS_AT[2], 0, 0, 180),
                };
                cpu.write16(o + MS_WAIT, wait);
                ms_type(cpu, s, at, flags, blink);
            }
        }
        3 => {
            tick(cpu);
            for (when, k) in MS_LAST_LINES {
                if (cpu.read16(o + MS_WAIT) as i16) == when as i16 {
                    let s = cpu.read32(o + MS_LINES + 4 * k as u32);
                    ms_type(cpu, s, MS_AT[2 + k], 0, 0);
                }
            }
            if (cpu.read16(o + MS_WAIT) as i16) < 0 {
                cpu.write8(o + OBJ_MODE, ENDING);
                for (k, at) in MS_AT.into_iter().enumerate() {
                    if k == 1 {
                        nested!(cpu, 0x0606_1204, put_large_string(cpu, MS_BLANK, at, 0));
                    } else {
                        nested!(cpu, 0x0606_11b8, put_string(cpu, MS_BLANK, at, 0));
                    }
                }
            }
        }
        _ => {}
    }
    if cpu.read8(o + MS_PHASE) > 1 {
        let bar = cpu.read16(o + MS_BAR) as i16 as i32 as u32;
        let bar = nested!(cpu, 0x0604_6168, super::actors::approach(cpu, bar, MS_BAR_LONGEST, MS_BAR_GROWTH));
        cpu.write16(o + MS_BAR, bar as u16);
    }
}

/// The message's underline drawn (its draw; `s` its sprite): a polygon a pixel high, as long as
/// the bar, at (-144, 39), once the second line is out.
const UL_FOUR_CORNERS: u32 = 2;
const UL_CORNERS_AT: u32 = 0x24;
const UL_CORNERS: u32 = 0x2c;
const UL_LIFT: u8 = 0x7e;
const UL_COLOUR: u16 = 12;
const UL_PLACE: (u32, u32) = (0xff70_0000, 0x27_0000);
const CHAR_BASE: u32 = 0x0607_41fc;

pub fn opening_bar_draw(cpu: &mut Cpu, s: u32) {
    let f = cpu.stack();
    if cpu.read8(s + MS_PHASE - 0x10) <= 1 {
        return;
    }
    nested!(cpu, 0x0604_6264, super::sprites::sprite_init(cpu, f));
    cpu.write32(f, UL_FOUR_CORNERS);
    cpu.write32(f + UL_CORNERS_AT, f + UL_CORNERS);
    cpu.write8(f + 0x23, UL_LIFT);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(UL_COLOUR);
    cpu.write16(f + 0x18, colour);
    cpu.write32(f + 4, UL_PLACE.0);
    cpu.write32(f + 8, UL_PLACE.1);
    cpu.write32(f + 0xc, 0);
    let bar = cpu.read16(s + MS_BAR - 0x10);
    for (k, v) in [0, 0, bar, 0, bar, 1, 0, 1].into_iter().enumerate() {
        cpu.write16(f + UL_CORNERS + 2 * k as u32, v);
    }
    nested!(cpu, 0x0604_0960, super::sprites::draw(cpu, f));
}

/// Makes a stage's opening message (with the BIOS's setting 1): its own lines `l1`..`l4`, after
/// `wait` frames.
pub fn opening_set(cpu: &mut Cpu, l1: u32, l2: u32, l3: u32, l4: u32, wait: u32) {
    if !super::globals::bios_setting_is_1(cpu) {
        return;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, MS_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 0, blocks, PLAIN_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, super::sprites::sprite_init(cpu, o + 0x10));
    cpu.write32(o + 0x38, MS_EXEC);
    cpu.write32(o + 0x10, MS_FLAGS);
    cpu.write32(o + MS_DRAW_AT, MS_DRAW);
    for (k, l) in [l1, l2, l3, l4].into_iter().enumerate() {
        cpu.write32(o + MS_LINES + 4 * k as u32, l);
    }
    cpu.write8(o + MS_PHASE, 0);
    cpu.write16(o + MS_WAIT, wait as u16);
    cpu.write16(o + MS_BAR, 0);
}
