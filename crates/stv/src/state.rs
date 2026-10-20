//! Save states: the machine's state as bytes, and back (Machine::save_state, load_state). Each part
//! names its fields with [`saved!`]: the ones saved, in order, and the ones that are not state (the
//! ROMs, caches, development tools). A field in neither list does not compile, so a new field is
//! saved or said not to be.

/// A part of the machine's state.
pub trait State {
    fn save(&self, w: &mut Vec<u8>);
    /// Takes the saved state back in place (the parts that are not state stay as they are).
    fn load(&mut self, r: &mut Reader) -> Result<(), String>;

    /// A run of them (bytes go as they are).
    fn save_all(items: &[Self], w: &mut Vec<u8>)
    where
        Self: Sized,
    {
        for x in items {
            x.save(w);
        }
    }

    fn load_all(items: &mut [Self], r: &mut Reader) -> Result<(), String>
    where
        Self: Sized,
    {
        for x in items {
            x.load(r)?;
        }
        Ok(())
    }
}

/// Saved state being read.
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Reader<'a> {
        Reader { data, pos: 0 }
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.data.len()).ok_or("the state ends early")?;
        let s = &self.data[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    /// Whether all of it has been read.
    pub fn done(&self) -> bool {
        self.pos == self.data.len()
    }
}

/// Implements [`State`] for a struct: `saved!(Name { saved fields } skip { the others })`.
macro_rules! saved {
    ($t:ident { $($f:ident),* $(,)? } $(skip { $($s:ident),* $(,)? })?) => {
        impl $crate::state::State for $t {
            fn save(&self, w: &mut Vec<u8>) {
                let $t { $($f,)* $($($s: _,)*)? } = self;
                $( $crate::state::State::save($f, w); )*
            }
            fn load(&mut self, r: &mut $crate::state::Reader) -> Result<(), String> {
                let $t { $($f,)* $($($s: _,)*)? } = self;
                $( $crate::state::State::load($f, r)?; )*
                Ok(())
            }
        }
    };
}
pub(crate) use saved;

macro_rules! numbers {
    ($($t:ty),*) => {$(
        impl State for $t {
            fn save(&self, w: &mut Vec<u8>) {
                w.extend_from_slice(&self.to_le_bytes());
            }
            fn load(&mut self, r: &mut Reader) -> Result<(), String> {
                let b = r.take(size_of::<$t>())?;
                *self = <$t>::from_le_bytes(b.try_into().expect("its size"));
                Ok(())
            }
        }
    )*};
}
numbers!(u16, u32, u64, i8, i16, i32, i64);

impl State for u8 {
    fn save(&self, w: &mut Vec<u8>) {
        w.push(*self);
    }
    fn load(&mut self, r: &mut Reader) -> Result<(), String> {
        *self = r.take(1)?[0];
        Ok(())
    }
    fn save_all(items: &[u8], w: &mut Vec<u8>) {
        w.extend_from_slice(items);
    }
    fn load_all(items: &mut [u8], r: &mut Reader) -> Result<(), String> {
        items.copy_from_slice(r.take(items.len())?);
        Ok(())
    }
}

impl State for usize {
    fn save(&self, w: &mut Vec<u8>) {
        (*self as u64).save(w);
    }
    fn load(&mut self, r: &mut Reader) -> Result<(), String> {
        let mut v = 0u64;
        v.load(r)?;
        *self = usize::try_from(v).map_err(|_| "a size too big")?;
        Ok(())
    }
}

impl State for bool {
    fn save(&self, w: &mut Vec<u8>) {
        w.push(*self as u8);
    }
    fn load(&mut self, r: &mut Reader) -> Result<(), String> {
        *self = match r.take(1)?[0] {
            0 => false,
            1 => true,
            b => return Err(format!("{b} where a flag was saved")),
        };
        Ok(())
    }
}

impl<T: State, const N: usize> State for [T; N] {
    fn save(&self, w: &mut Vec<u8>) {
        T::save_all(self, w);
    }
    fn load(&mut self, r: &mut Reader) -> Result<(), String> {
        T::load_all(self, r)
    }
}

impl<T: State + Default + Clone> State for Vec<T> {
    fn save(&self, w: &mut Vec<u8>) {
        self.len().save(w);
        T::save_all(self, w);
    }
    fn load(&mut self, r: &mut Reader) -> Result<(), String> {
        let mut n = 0usize;
        n.load(r)?;
        if n > r.data.len() - r.pos {
            return Err("a length past the state's end".into());
        }
        self.clear();
        self.resize(n, T::default());
        T::load_all(self, r)
    }
}

impl<T: State + Default> State for Option<T> {
    fn save(&self, w: &mut Vec<u8>) {
        self.is_some().save(w);
        if let Some(x) = self {
            x.save(w);
        }
    }
    fn load(&mut self, r: &mut Reader) -> Result<(), String> {
        let mut some = false;
        some.load(r)?;
        if !some {
            *self = None;
            return Ok(());
        }
        self.get_or_insert_with(T::default).load(r)
    }
}

impl<T: State> State for Box<T> {
    fn save(&self, w: &mut Vec<u8>) {
        (**self).save(w);
    }
    fn load(&mut self, r: &mut Reader) -> Result<(), String> {
        (**self).load(r)
    }
}

impl<A: State, B: State> State for (A, B) {
    fn save(&self, w: &mut Vec<u8>) {
        self.0.save(w);
        self.1.save(w);
    }
    fn load(&mut self, r: &mut Reader) -> Result<(), String> {
        self.0.load(r)?;
        self.1.load(r)
    }
}

impl<A: State, B: State, C: State> State for (A, B, C) {
    fn save(&self, w: &mut Vec<u8>) {
        self.0.save(w);
        self.1.save(w);
        self.2.save(w);
    }
    fn load(&mut self, r: &mut Reader) -> Result<(), String> {
        self.0.load(r)?;
        self.1.load(r)?;
        self.2.load(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default, Debug, PartialEq)]
    struct Part {
        a: u32,
        b: Vec<u8>,
        c: Option<(u16, bool)>,
        d: [i16; 3],
        cache: u64,
    }
    saved!(Part { a, b, c, d } skip { cache });

    #[test]
    fn round_trip() {
        let p = Part { a: 7, b: vec![1, 2, 3], c: Some((9, true)), d: [-1, 0, 1], cache: 5 };
        let mut w = Vec::new();
        p.save(&mut w);
        let mut q = Part { cache: 6, ..Default::default() };
        let mut r = Reader::new(&w);
        q.load(&mut r).unwrap();
        assert!(r.done());
        assert_eq!(q, Part { cache: 6, ..p });
        assert!(Part::default().load(&mut Reader::new(&w[..w.len() - 1])).is_err());
    }
}
