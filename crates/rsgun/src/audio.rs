//! The SCSP's samples to the sound card: an SDL queue kept near a target fill by playing the
//! samples a little faster or slower (at most 0.5%, inaudible), as the game's frames come at the
//! display's rate rather than exactly the ST-V's 59.94.

/// The SCSP's rate.
pub const RATE: i32 = 44_100;
/// The queue's fill aimed at, and the most it may hold before it is emptied (frames of audio).
const TARGET: f64 = 2048.0;
const MOST: u32 = 8192;

pub struct Output {
    queue: sdl2::audio::AudioQueue<i16>,
    /// The position between the last two input samples (resampling), and the last sample.
    pos: f64,
    last: [i16; 2],
    buf: Vec<i16>,
}

impl Output {
    pub fn open(sdl: &sdl2::Sdl) -> Result<Output, String> {
        let audio = sdl.audio()?;
        let spec = sdl2::audio::AudioSpecDesired { freq: Some(RATE), channels: Some(2), samples: Some(512) };
        let queue = audio.open_queue::<i16, _>(None, &spec)?;
        queue.resume();
        Ok(Output { queue, pos: 0.0, last: [0; 2], buf: Vec::new() })
    }

    /// Frames of audio waiting to be played.
    fn queued(&self) -> u32 {
        self.queue.size() / 4
    }

    /// Plays a frame's samples.
    pub fn play(&mut self, samples: &[[i16; 2]]) -> Result<(), String> {
        let queued = self.queued();
        if queued > MOST {
            self.queue.clear();
        }
        // above the target: step through the input a little faster, below it a little slower
        let step = 1.0 + ((queued as f64 - TARGET) / TARGET * 0.005).clamp(-0.005, 0.005);
        self.buf.clear();
        for &s in samples {
            while self.pos < 1.0 {
                let f = self.pos;
                for (&a, &b) in self.last.iter().zip(&s) {
                    self.buf.push((a as f64 + (b as f64 - a as f64) * f) as i16);
                }
                self.pos += step;
            }
            self.pos -= 1.0;
            self.last = s;
        }
        self.queue.queue_audio(&self.buf)
    }

    /// Drops what is waiting (a pause, a fast run).
    pub fn flush(&mut self) {
        self.queue.clear();
    }
}

/// A stereo 16-bit WAV file of samples.
pub fn write_wav(path: &std::path::Path, samples: &[[i16; 2]]) -> Result<(), String> {
    let data_len = samples.len() as u32 * 4;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&(RATE as u32).to_le_bytes());
    out.extend_from_slice(&(RATE as u32 * 4).to_le_bytes());
    out.extend_from_slice(&4u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s[0].to_le_bytes());
        out.extend_from_slice(&s[1].to_le_bytes());
    }
    std::fs::write(path, out).map_err(|e| format!("{}: {e}", path.display()))
}
