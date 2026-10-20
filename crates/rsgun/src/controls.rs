//! The cabinet's controls: what a player holds, from the keyboard, a pad or a script, as the I/O
//! chip's ports see it. Radiant Silvergun has three buttons (A vulcan, B homing, C spread); its
//! other weapons are pairs of them and all three, so those get buttons of their own here.

/// What one player holds: a set of these bits.
pub const UP: u32 = 1 << 0;
pub const DOWN: u32 = 1 << 1;
pub const LEFT: u32 = 1 << 2;
pub const RIGHT: u32 = 1 << 3;
pub const A: u32 = 1 << 4;
pub const B: u32 = 1 << 5;
pub const C: u32 = 1 << 6;
pub const AB: u32 = 1 << 7;
pub const AC: u32 = 1 << 8;
pub const BC: u32 = 1 << 9;
pub const ABC: u32 = 1 << 10;
pub const START: u32 = 1 << 11;
pub const COIN: u32 = 1 << 12;
/// The cabinet's test and service switches (player 1's set carries them).
pub const TEST: u32 = 1 << 13;
pub const SERVICE: u32 = 1 << 14;

/// The names an input script uses, and their bits.
pub const NAMES: [(&str, u32); 15] = [
    ("up", UP),
    ("down", DOWN),
    ("left", LEFT),
    ("right", RIGHT),
    ("a", A),
    ("b", B),
    ("c", C),
    ("ab", AB),
    ("ac", AC),
    ("bc", BC),
    ("abc", ABC),
    ("start", START),
    ("coin", COIN),
    ("test", TEST),
    ("service", SERVICE),
];

/// The I/O chip's ports A-H (active low) for two players' controls.
pub fn ports(players: [u32; 2]) -> [u8; 8] {
    let mut ports = [0xFF; 8];
    for (p, &held) in players.iter().enumerate() {
        let mut bits = 0u8;
        for (mask, bit) in [(UP, 0x20), (DOWN, 0x10), (LEFT, 0x80), (RIGHT, 0x40)] {
            if held & mask != 0 {
                bits |= bit;
            }
        }
        // buttons 1-3 at bits 0-2: A, B, C
        for (mask, bit) in [(A, 1), (B, 2), (C, 4), (AB, 3), (AC, 5), (BC, 6), (ABC, 7)] {
            if held & mask != 0 {
                bits |= bit;
            }
        }
        // opposite directions cancel, as a stick cannot hold both
        if bits & 0x30 == 0x30 {
            bits &= !0x30;
        }
        if bits & 0xC0 == 0xC0 {
            bits &= !0xC0;
        }
        ports[p] &= !bits;
        let mut cab = 0u8;
        if held & COIN != 0 {
            cab |= 1 << p;
        }
        if held & START != 0 {
            cab |= 0x10 << p;
        }
        if p == 0 && held & TEST != 0 {
            cab |= 0x04;
        }
        if p == 0 && held & SERVICE != 0 {
            cab |= 0x08;
        }
        ports[2] &= !cab;
    }
    ports
}

/// An input script: from each listed frame on, what each player holds (until the next line).
///
///     # frame  player 1         [| player 2]
///     600      coin
///     604
///     700      start
///     706      up a
pub struct Script {
    lines: Vec<(u64, [u32; 2])>,
}

impl Script {
    pub fn parse(text: &str) -> Result<Script, String> {
        let mut lines = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let (frame, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
            let frame: u64 = frame.parse().map_err(|_| format!("line {}: not a frame number: {frame}", n + 1))?;
            let mut held = [0u32; 2];
            for (p, part) in rest.split('|').take(2).enumerate() {
                for word in part.split_whitespace() {
                    let bit = NAMES.iter().find(|(name, _)| *name == word).map(|&(_, b)| b);
                    held[p] |= bit.ok_or_else(|| format!("line {}: no control named {word}", n + 1))?;
                }
            }
            lines.push((frame, held));
        }
        if lines.windows(2).any(|w| w[0].0 > w[1].0) {
            return Err("the frames must not go back".into());
        }
        Ok(Script { lines })
    }

    /// What the players hold in `frame`.
    pub fn held(&self, frame: u64) -> [u32; 2] {
        match self.lines.partition_point(|&(f, _)| f <= frame) {
            0 => [0; 2],
            i => self.lines[i - 1].1,
        }
    }

    /// The last frame the script names.
    pub fn last_frame(&self) -> u64 {
        self.lines.last().map_or(0, |l| l.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports_are_active_low() {
        assert_eq!(ports([0, 0]), [0xFF; 8]);
        let p = ports([UP | A | COIN | START, BC]);
        assert_eq!(p[0], !0x21);
        assert_eq!(p[1], !0x06);
        assert_eq!(p[2], !0x11);
        assert_eq!(ports([LEFT | RIGHT, 0])[0], 0xFF);
    }

    #[test]
    fn script_holds_until_the_next_line() {
        let s = Script::parse("# test\n600 coin\n604\n700 start | start\n").unwrap();
        assert_eq!(s.held(599), [0, 0]);
        assert_eq!(s.held(600), [COIN, 0]);
        assert_eq!(s.held(603), [COIN, 0]);
        assert_eq!(s.held(604), [0, 0]);
        assert_eq!(s.held(800), [START, START]);
        assert!(Script::parse("5 jump").is_err());
    }
}
