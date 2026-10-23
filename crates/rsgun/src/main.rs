//! Radiant Silvergun on the ST-V runtime: the machine in a window, the keyboard and pads as the
//! cabinet's controls, the settings EEPROM and backup RAM kept between runs.
//!
//!     rsgun [--rom DIR] [--save DIR] [--script FILE] [--scale N] [--fullscreen] [--smooth] [--mute]
//!           [--native] [--cold] [--native-boot] [--cpu-speed N] [--frame-delay MS] [--no-gpu-sync]
//!     rsgun --headless --frames N [--script FILE] [--shot FILE.ppm] [--wav FILE.wav] [--evidence FILE]
//!           [--dump F,F --dump-dir DIR] [--native | --compare REPORT] [--cold] [--cpu-speed N] [--rom DIR]
//!           [--save DIR] [--load-state FILE] [--save-state FILE] [--states F,F]
//!
//! `--rom`: the images tools/rom/mkcart.py makes (default build/rom). `--save`: where the EEPROM,
//! the backup RAM and the boot state (boot.rs) are kept (default $XDG_DATA_HOME/rsgun or
//! ~/.local/share/rsgun; a headless run writes them back only when it is given). A run starts from the boot state, where the BIOS hands over to the game,
//! unless `--cold` (or `--evidence`, `--compare`) has it go through the BIOS's boot from power on, or
//! `--native-boot` has it start from the state native_boot.rs builds from the images (no BIOS run).
//! `--script`: an input script (controls.rs) played instead of the keyboard and pads, its frames
//! counted from power on. `--headless`: no window; runs to frame N and writes the last picture
//! (`--shot`), the sound (`--wav`) and the evidence of what code ran (`--evidence`, stv::evidence's
//! text form), and the memories at frames (`--dump`, as stv-check boot writes them). `--native` runs
//! the port's code (build/port, tools/port) in place of the SH-2 code it was made from; `--compare`
//! runs the SH-2 code and checks each call the port has against it (stv::compare), writing what it
//! found to REPORT. `--cpu-speed N` runs the SH-2s N times as fast as built: no slowdown.
//! `--frame-delay MS` waits that long after each refresh before reading the controls and running
//! the frame (less lag; a frame's run must still fit in what is left of the refresh: try 6-10 with
//! `--native`); `--no-gpu-sync` lets the driver queue frames ahead (by default each present waits
//! for the frame to be drawn, which keeps the picture at most one refresh behind the controls).
//! `--load-state FILE` starts a headless run from a save state (one `--save-state FILE` wrote at
//! the end of another, or `--states F,F` at frames, into the `--dump-dir`; not while native code is
//! in a call, so not in native play).
//!
//! Keys: arrows move; Z, X, C are buttons A, B, C; A, S, D are A+B, A+C, B+C; space is all three;
//! 5 and 6 insert coins, 1 and 2 start; F2 test, 9 service; P pauses, Tab runs fast while held,
//! F11 switches full screen, F12 saves a screenshot, Esc quits. Pads: the d-pad or left stick
//! moves; south, west, north are A, B, C; east is B+C; the shoulders are A+B and A+C, the right
//! trigger all three; start starts and back inserts a coin. The first pad is player 1, unless the
//! keyboard is, then player 2.

mod audio;
mod boot;
mod controls;
mod game;
mod native_boot;

// the port's generated code (crates/rsgun/build.rs)
include!(concat!(env!("OUT_DIR"), "/port.rs"));

/// Where the game loads its modes' and stages' code: work RAM-H from 0x06080000; the window's first
/// 4 KB is code in each of them (tools/port/overlays.py).
fn overlay() -> stv::evidence::Overlay {
    stv::evidence::Overlay { window: 0x0608_0000..0x060A_5000, print: 0x0608_0000..0x0608_1000 }
}

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use boot::Boot;
use controls::Script;
use stv::machine::Machine;

/// The ST-V's frame rate (NTSC).
const FRAME_HZ: f64 = 59.94;
/// Frames a coin is held when its key or button is pressed.
const COIN_FRAMES: u32 = 4;

struct Options {
    rom: PathBuf,
    save: PathBuf,
    /// `--save` was given: a headless run writes its saves there too.
    save_given: bool,
    script: Option<PathBuf>,
    scale: u32,
    fullscreen: bool,
    smooth: bool,
    /// Waits for the GPU after each present (`--no-gpu-sync` not), so the driver cannot queue
    /// frames ahead of the display and the pictures (and the controls behind them) lag.
    gpu_sync: bool,
    /// `--frame-delay MS`: waits this long after the display's refresh before reading the controls
    /// and running the frame, so they are fresher when it is shown.
    frame_delay: Duration,
    headless: bool,
    frames: u64,
    shot: Option<PathBuf>,
    wav: Option<PathBuf>,
    mute: bool,
    evidence: Option<PathBuf>,
    dumps: Vec<u64>,
    dump_dir: PathBuf,
    native: bool,
    compare: Option<PathBuf>,
    cold: bool,
    cpu_speed: u32,
    load_state: Option<PathBuf>,
    save_state: Option<PathBuf>,
    states: Vec<u64>,
    /// `--reach ADDR`: the frame the master first reaches ADDR is printed (development).
    reach: Option<u32>,
    /// `--native-boot-check DIR` (development): compares the native boot's state with the reference
    /// DIR/entry.state (tools/boot/reach); `--boot-eeprom` and `--boot-backup` name the EEPROM and
    /// backup RAM files the reference run had (else the images' own EEPROM and a blank backup).
    boot_check: Option<PathBuf>,
    /// `--native-boot`: starts from the state native_boot.rs builds from the images (not the saved
    /// boot state, not the BIOS's boot).
    native_boot: bool,
    boot_eeprom: Option<PathBuf>,
    boot_backup: Option<PathBuf>,
}

fn default_save_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("XDG_DATA_HOME").filter(|d| !d.is_empty()) {
        return PathBuf::from(d).join("rsgun");
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    home.join(".local/share/rsgun")
}

fn parse_args() -> Result<Options, String> {
    let mut o = Options {
        rom: PathBuf::from("build/rom"),
        save: default_save_dir(),
        save_given: false,
        script: None,
        scale: 3,
        fullscreen: false,
        smooth: false,
        gpu_sync: true,
        frame_delay: Duration::ZERO,
        headless: false,
        frames: 0,
        shot: None,
        wav: None,
        mute: false,
        evidence: None,
        dumps: Vec::new(),
        dump_dir: PathBuf::from("out/machine"),
        native: false,
        compare: None,
        cold: false,
        cpu_speed: 1,
        load_state: None,
        save_state: None,
        states: Vec::new(),
        reach: None,
        boot_check: None,
        native_boot: false,
        boot_eeprom: None,
        boot_backup: None,
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().cloned().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "--rom" => o.rom = PathBuf::from(value()?),
            "--save" => {
                o.save = PathBuf::from(value()?);
                o.save_given = true;
            }
            "--script" => o.script = Some(PathBuf::from(value()?)),
            "--scale" => o.scale = value()?.parse().map_err(|_| "--scale: a number")?,
            "--fullscreen" => o.fullscreen = true,
            "--smooth" => o.smooth = true,
            "--no-gpu-sync" => o.gpu_sync = false,
            "--frame-delay" => {
                let ms: u64 = value()?.parse().map_err(|_| "--frame-delay: milliseconds")?;
                o.frame_delay = Duration::from_millis(ms.min(14));
            }
            "--headless" => o.headless = true,
            "--frames" => o.frames = value()?.parse().map_err(|_| "--frames: a number")?,
            "--shot" => o.shot = Some(PathBuf::from(value()?)),
            "--wav" => o.wav = Some(PathBuf::from(value()?)),
            "--evidence" => o.evidence = Some(PathBuf::from(value()?)),
            "--dump" => o.dumps = value()?.split(',').filter_map(|f| f.parse().ok()).collect(),
            "--dump-dir" => o.dump_dir = PathBuf::from(value()?),
            "--native" => o.native = true,
            "--compare" => o.compare = Some(PathBuf::from(value()?)),
            "--cold" => o.cold = true,
            "--load-state" => o.load_state = Some(PathBuf::from(value()?)),
            "--save-state" => o.save_state = Some(PathBuf::from(value()?)),
            "--states" => o.states = value()?.split(',').filter_map(|f| f.parse().ok()).collect(),
            "--reach" => o.reach = Some(u32::from_str_radix(value()?.trim_start_matches("0x"), 16).map_err(|_| "--reach: a hex address")?),
            "--cpu-speed" => o.cpu_speed = value()?.parse().map_err(|_| "--cpu-speed: a number")?,
            "--native-boot" => o.native_boot = true,
            "--native-boot-check" => o.boot_check = Some(PathBuf::from(value()?)),
            "--boot-eeprom" => o.boot_eeprom = Some(PathBuf::from(value()?)),
            "--boot-backup" => o.boot_backup = Some(PathBuf::from(value()?)),
            "--mute" => o.mute = true,
            "-h" | "--help" => return Err(String::new()),
            _ => return Err(format!("unknown option {a}")),
        }
    }
    Ok(o)
}

/// The machine from the ROM images, with the saved EEPROM and backup RAM (or, for the EEPROM, the
/// one beside the images), at the boot state unless the run is to be cold (and the boot state's
/// watch, when there is none yet).
fn load_machine(o: &Options) -> Result<(Machine, Option<Boot>), String> {
    let read = |p: PathBuf| {
        fs::read(&p).map_err(|e| format!("{}: {e} (make the images with tools/rom/mkcart.py, or pass --rom)", p.display()))
    };
    let bios = read(o.rom.join("bios-epr20091.bin"))?;
    let cart = read(o.rom.join("rsgun-cart.bin"))?;
    let eeprom = fs::read(o.save.join("eeprom.bin")).or_else(|_| fs::read(o.rom.join("eeprom.bin"))).ok();
    // the boot state depends on the EEPROM and on the CPUs' speed
    let key = stv::mem::fnv1a(eeprom.as_deref().unwrap_or_default()).wrapping_add((o.cpu_speed as u64 - 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let backup = fs::read(o.save.join("backup.bin")).ok();
    let mut m = if o.native_boot {
        let img = native_boot::Inputs { bios: &bios, cart: &cart, eeprom: eeprom.as_deref(), backup: backup.as_deref(), area: 1 };
        native_boot::build(&img)
    } else {
        Machine::new(bios, cart, 1, eeprom.as_deref())
    };
    m.cpu_speed = o.cpu_speed;
    // the picture's lines in bands, on the cores there are
    m.render_threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(4));
    if o.native || o.compare.is_some() {
        m.sys.natives.overlay = Some(overlay());
        generated::register(&mut m.sys.natives);
        game::register(&mut m.sys.natives);
    }
    if !o.native_boot
        && let Some(backup) = backup
        && backup.len() == m.sys.backup.len()
    {
        m.sys.backup = backup;
    }
    let cold = o.native_boot || o.cold || o.evidence.is_some() || o.compare.is_some() || o.load_state.is_some();
    let boot = (!cold).then(|| Boot::start(&mut m, o.save.join("boot.state"), key));
    Ok((m, boot))
}

fn save(m: &Machine, dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let eeprom: Vec<u8> = m.sys.smpc.eeprom.mem.iter().flat_map(|w| w.to_be_bytes()).collect();
    for (name, data) in [("eeprom.bin", &eeprom), ("backup.bin", &m.sys.backup)] {
        let p = dir.join(name);
        fs::write(&p, data).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    Ok(())
}

fn write_ppm(path: &Path, (w, h, rgb): &(usize, usize, Vec<u8>)) -> Result<(), String> {
    let mut data = format!("P6\n{w} {h}\n255\n").into_bytes();
    data.extend_from_slice(rgb);
    fs::write(path, data).map_err(|e| format!("{}: {e}", path.display()))
}

/// Coins: a press holds the coin line for a few frames.
#[derive(Default)]
struct CoinPulse {
    was_down: bool,
    left: u32,
}

impl CoinPulse {
    fn update(&mut self, down: bool) -> bool {
        if down && !self.was_down {
            self.left = COIN_FRAMES;
        }
        self.was_down = down;
        let on = self.left > 0;
        self.left = self.left.saturating_sub(1);
        on
    }
}

fn headless(o: &Options, script: Option<&Script>) -> Result<(), String> {
    let (mut m, mut boot) = load_machine(o)?;
    if let Some(p) = &o.load_state {
        let data = fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
        m.load_state(&data).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    let frames = if o.frames > 0 { o.frames } else { script.map_or(600, |s| s.last_frame() + 60) };
    if o.compare.is_some() {
        m.compare = Some(Default::default());
    }
    if let Some(a) = o.reach {
        m.breakpoint = Some((0, a));
    }
    if o.evidence.is_some() {
        m.evidence = Some(Box::new(stv::evidence::Evidence { overlay: Some(overlay()), ..Default::default() }));
    }
    let t = Instant::now();
    let first = m.sys.frame;
    let mut sound = Vec::new();
    while m.sys.frame < frames {
        let held = script.map_or([0; 2], |s| s.held(m.sys.frame));
        m.sys.inputs = controls::ports(held);
        m.run_frame();
        if let Some(b) = &mut boot {
            b.frame(&mut m, held == [0; 2]);
        }
        let f = m.sys.frame;
        if o.dumps.contains(&f) {
            let d = o.dump_dir.join(format!("f{f:06}"));
            m.dump(&d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        if o.states.contains(&f) {
            fs::create_dir_all(&o.dump_dir).map_err(|e| format!("{}: {e}", o.dump_dir.display()))?;
            let p = o.dump_dir.join(format!("f{f:06}.state"));
            fs::write(&p, m.save_state()?).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        let samples = std::mem::take(&mut m.sys.scsp.out);
        if o.wav.is_some() {
            sound.extend_from_slice(&samples);
        }
    }
    if o.save_given {
        save(&m, &o.save)?;
    }
    if let Some(p) = &o.save_state {
        let data = m.save_state()?;
        fs::write(p, data).map_err(|e| format!("{}: {e}", p.display()))?;
        println!("state at frame {} written to {}", m.sys.frame, p.display());
    }
    if let Some(a) = o.reach {
        match m.reached {
            Some(f) => println!("{a:08x} first reached in frame {f}"),
            None => println!("{a:08x} not reached"),
        }
    }
    let run = frames.saturating_sub(first);
    let ms = t.elapsed().as_secs_f64() * 1000.0 / run.max(1) as f64;
    let part = |d: Duration| d.as_secs_f64() * 1000.0 / run.max(1) as f64;
    let c = m.sys.costs;
    println!(
        "{run} frames (from frame {first}), {ms:.2} ms a frame (VDP2 {:.2}, VDP1 {:.2}, sound {:.2})",
        part(c.compose),
        part(c.draw),
        part(c.sound)
    );
    if o.native {
        let n = &m.sys.natives;
        println!("native: {} functions, {} calls run natively, {} passed to the interpreter", n.len(), n.native_calls, n.interpreted_calls);
        let mut by: Vec<_> = n.interpreted_by.iter().collect();
        by.sort_by_key(|&(_, &(_, steps))| std::cmp::Reverse(steps));
        for (target, (calls, steps)) in by.into_iter().take(12) {
            println!("  passed to the interpreter: {target:08x}, {calls} calls, {steps} instructions");
        }
        let [a, b] = m.sys.interpreted;
        println!("SH-2 instructions still interpreted: master {a}, slave {b}");
        let mut named: Vec<_> = n.named().map(|(fp, name)| format!("{fp:016x} {}", name.unwrap_or("(no name)"))).collect();
        named.sort();
        println!("overlay code seen: {}", named.join(", "));
    }
    if let (Some(p), Some(c)) = (&o.compare, &m.compare) {
        let report = c.report();
        fs::write(p, &report).map_err(|err| format!("{}: {err}", p.display()))?;
        println!("compare: {} (report: {})", report.lines().next().unwrap_or(""), p.display());
    }
    if let Some(p) = &o.shot {
        write_ppm(p, &m.picture)?;
        println!("picture {}x{} written to {}", m.picture.0, m.picture.1, p.display());
    }
    if let (Some(p), Some(e)) = (&o.evidence, &m.evidence) {
        fs::write(p, e.to_text()).map_err(|err| format!("{}: {err}", p.display()))?;
        // each overlay's window, for the port's translation: beside the evidence, by fingerprint
        let dir = p.with_extension("overlays");
        fs::create_dir_all(&dir).map_err(|err| format!("{}: {err}", dir.display()))?;
        for (fp, code) in &e.code {
            let f = dir.join(format!("{fp:016x}.bin"));
            fs::write(&f, code).map_err(|err| format!("{}: {err}", f.display()))?;
        }
        println!("evidence: {} addresses, {} transfers written to {}", e.executed.len(), e.transfers.len(), p.display());
    }
    if let Some(p) = &o.wav {
        audio::write_wav(p, &sound)?;
        println!("{:.1} s of sound written to {}", sound.len() as f64 / audio::RATE as f64, p.display());
    }
    Ok(())
}

fn windowed(o: &Options, script: Option<&Script>) -> Result<(), String> {
    use sdl2::controller::{Axis, Button, GameController};
    use sdl2::event::Event;
    use sdl2::keyboard::Scancode;
    use sdl2::pixels::{Color, PixelFormatEnum};
    use sdl2::rect::Rect;
    use sdl2::video::FullscreenType;

    let (mut m, mut boot) = load_machine(o)?;
    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let pads_sys = sdl.game_controller()?;
    sdl2::hint::set("SDL_RENDER_SCALE_QUALITY", if o.smooth { "linear" } else { "nearest" });
    let (ww, wh) = (320 * o.scale, 240 * o.scale);
    let mut window = video.window("Radiant Silvergun", ww, wh).position_centered().resizable().build().map_err(|e| e.to_string())?;
    if o.fullscreen {
        window.set_fullscreen(FullscreenType::Desktop)?;
    }
    let refresh = window.display_mode().map(|d| d.refresh_rate).unwrap_or(0);
    let mut canvas = window.into_canvas().present_vsync().build().map_err(|e| e.to_string())?;
    let creator = canvas.texture_creator();
    let mut texture = None;
    let mut texture_size = (0, 0);
    let mut events = sdl.event_pump()?;
    let mut pads: Vec<GameController> = Vec::new();
    let mut sound = if o.mute {
        None
    } else {
        audio::Output::open(&sdl).map_err(|e| eprintln!("rsgun: no sound: {e}")).ok()
    };

    // one frame a refresh on a 60 Hz display (vsync paces); otherwise frames by the clock
    let mut lock_to_vsync = (59..=61).contains(&refresh);
    let period = Duration::from_secs_f64(1.0 / FRAME_HZ);
    let mut next = Instant::now();
    let mut presents: Vec<Instant> = Vec::new();
    let (mut paused, mut frame, mut shots) = (false, m.sys.frame, 0u32);
    let first = frame;
    let mut coins = [CoinPulse::default(), CoinPulse::default()];
    let started = Instant::now();
    let mut last_loop = started;
    'run: loop {
        for event in events.poll_iter() {
            match event {
                Event::Quit { .. } => break 'run,
                Event::KeyDown { scancode: Some(sc), repeat: false, .. } => match sc {
                    Scancode::Escape => break 'run,
                    Scancode::P => paused = !paused,
                    Scancode::F11 => {
                        let w = canvas.window_mut();
                        let to = if w.fullscreen_state() == FullscreenType::Off { FullscreenType::Desktop } else { FullscreenType::Off };
                        w.set_fullscreen(to)?;
                    }
                    Scancode::F12 => {
                        fs::create_dir_all(&o.save).map_err(|e| e.to_string())?;
                        shots += 1;
                        let p = o.save.join(format!("shot-{frame:06}-{shots}.ppm"));
                        write_ppm(&p, &m.picture)?;
                        eprintln!("screenshot: {}", p.display());
                    }
                    _ => {}
                },
                Event::ControllerDeviceAdded { which, .. } => {
                    if let Ok(pad) = pads_sys.open(which) {
                        pads.push(pad);
                    }
                }
                Event::ControllerDeviceRemoved { which, .. } => pads.retain(|p| p.instance_id() != which),
                _ => {}
            }
        }
        let keys = events.keyboard_state();
        let since_loop = last_loop;
        last_loop = Instant::now();
        let fast = keys.is_scancode_pressed(Scancode::Tab);
        let due = if paused {
            0
        } else if fast {
            8
        } else if lock_to_vsync {
            // a frame a refresh; when presents stall (the window not shown), catch up a little
            let since = last_loop - since_loop;
            if since > period * 5 / 2 { ((since.as_secs_f64() / period.as_secs_f64()) as u32).min(4) } else { 1 }
        } else {
            let now = Instant::now();
            let mut n = 0;
            while next <= now {
                next += period;
                n += 1;
            }
            if n > 4 {
                // far behind (a stall): start the clock again rather than run to catch up
                next = now + period;
                n = 1;
            }
            n
        };
        for _ in 0..due {
            let held = match script {
                Some(s) => s.held(frame),
                None => {
                    let k = |sc| keys.is_scancode_pressed(sc);
                    let mut held = [0u32; 2];
                    let key_map = [
                        (Scancode::Up, controls::UP),
                        (Scancode::Down, controls::DOWN),
                        (Scancode::Left, controls::LEFT),
                        (Scancode::Right, controls::RIGHT),
                        (Scancode::Z, controls::A),
                        (Scancode::X, controls::B),
                        (Scancode::C, controls::C),
                        (Scancode::A, controls::AB),
                        (Scancode::S, controls::AC),
                        (Scancode::D, controls::BC),
                        (Scancode::Space, controls::ABC),
                        (Scancode::Num1, controls::START),
                        (Scancode::F2, controls::TEST),
                        (Scancode::Num9, controls::SERVICE),
                    ];
                    for (sc, bit) in key_map {
                        if k(sc) {
                            held[0] |= bit;
                        }
                    }
                    if k(Scancode::Num2) {
                        held[1] |= controls::START;
                    }
                    let mut coin_keys = [k(Scancode::Num5), k(Scancode::Num6)];
                    // pads: player 1's unless the keyboard is playing, then player 2's
                    let keyboard_playing = held[0] != 0;
                    for (i, pad) in pads.iter().enumerate().take(2) {
                        let p = (i + keyboard_playing as usize).min(1);
                        let b = |button| pad.button(button);
                        let (x, y) = (pad.axis(Axis::LeftX), pad.axis(Axis::LeftY));
                        const DEAD: i16 = 12000;
                        let pad_map = [
                            (b(Button::DPadUp) || y < -DEAD, controls::UP),
                            (b(Button::DPadDown) || y > DEAD, controls::DOWN),
                            (b(Button::DPadLeft) || x < -DEAD, controls::LEFT),
                            (b(Button::DPadRight) || x > DEAD, controls::RIGHT),
                            (b(Button::A), controls::A),
                            (b(Button::X), controls::B),
                            (b(Button::Y), controls::C),
                            (b(Button::B), controls::BC),
                            (b(Button::LeftShoulder), controls::AB),
                            (b(Button::RightShoulder), controls::AC),
                            (pad.axis(Axis::TriggerRight) > DEAD, controls::ABC),
                            (b(Button::Start), controls::START),
                        ];
                        for (on, bit) in pad_map {
                            if on {
                                held[p] |= bit;
                            }
                        }
                        coin_keys[p] |= b(Button::Back);
                    }
                    for (p, c) in coins.iter_mut().enumerate() {
                        if c.update(coin_keys[p]) {
                            held[p] |= controls::COIN;
                        }
                    }
                    held
                }
            };
            m.sys.inputs = controls::ports(held);
            m.run_frame();
            if let Some(b) = &mut boot {
                b.frame(&mut m, held == [0; 2]);
            }
            frame += 1;
        }
        let samples = std::mem::take(&mut m.sys.scsp.out);
        if let Some(out) = &mut sound {
            if fast || paused {
                out.flush();
            } else {
                out.play(&samples)?;
            }
        }

        // the picture, 4:3 in the window
        let (w, h, rgb) = &m.picture;
        if *w > 0 {
            if texture_size != (*w, *h) {
                texture = Some(creator.create_texture_streaming(PixelFormatEnum::RGB24, *w as u32, *h as u32).map_err(|e| e.to_string())?);
                texture_size = (*w, *h);
            }
            if let Some(t) = &mut texture {
                t.update(None, rgb, w * 3).map_err(|e| e.to_string())?;
            }
        }
        canvas.set_draw_color(Color::BLACK);
        canvas.clear();
        if let Some(t) = &texture {
            let (cw, ch) = canvas.output_size()?;
            let (dw, dh) = if cw * 3 > ch * 4 { (ch * 4 / 3, ch) } else { (cw, cw * 3 / 4) };
            canvas.copy(t, None, Some(Rect::new(((cw - dw) / 2) as i32, ((ch - dh) / 2) as i32, dw, dh)))?;
        }
        canvas.present();
        if o.gpu_sync && !fast {
            // a pixel read back waits for the frame to be drawn: no frames queued ahead
            let _ = canvas.read_pixels(Some(Rect::new(0, 0, 1, 1)), PixelFormatEnum::RGB24);
        }

        // vsync that does not wait (no compositor, or a driver that ignores it): pace by the clock
        if lock_to_vsync && !fast && !paused {
            presents.push(Instant::now());
            if presents.len() == 61 {
                let mean = (presents[60] - presents[0]) / 60;
                if mean < Duration::from_millis(12) {
                    lock_to_vsync = false;
                    next = Instant::now();
                    eprintln!("vsync does not hold frames ({mean:?} a frame): pacing by the clock");
                }
                presents.clear();
            }
        }
        if !lock_to_vsync && !fast {
            let now = Instant::now();
            if next > now {
                std::thread::sleep(next - now);
            }
        } else if paused {
            std::thread::sleep(Duration::from_millis(15));
        } else if !fast && !o.frame_delay.is_zero() {
            // just after the refresh: the controls are read this much later
            std::thread::sleep(o.frame_delay);
        }
    }
    let secs = started.elapsed().as_secs_f64();
    eprintln!("{} frames in {secs:.1} s ({:.2} a second)", frame - first, (frame - first) as f64 / secs);
    save(&m, &o.save)
}

/// `--native-boot-check`: the native boot state against the reference run's (native_boot.rs).
fn native_boot_check(o: &Options, dir: &Path) -> Result<usize, String> {
    let read = |p: PathBuf| fs::read(&p).map_err(|e| format!("{}: {e}", p.display()));
    let bios = read(o.rom.join("bios-epr20091.bin"))?;
    let cart = read(o.rom.join("rsgun-cart.bin"))?;
    let eeprom = read(o.boot_eeprom.clone().unwrap_or_else(|| o.rom.join("eeprom.bin")))?;
    let backup = o.boot_backup.clone().map(read).transpose()?;
    let state = read(dir.join("entry.state"))?;
    let img = native_boot::Inputs { bios: &bios, cart: &cart, eeprom: Some(&eeprom), backup: backup.as_deref(), area: 1 };
    native_boot::check(&state, &img, 40)
}

fn main() -> ExitCode {
    let o = match parse_args() {
        Ok(o) => o,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("rsgun: {e}");
            }
            eprintln!("usage: rsgun [--rom DIR] [--save DIR] [--script FILE] [--scale N] [--fullscreen] [--smooth] [--mute] [--native] [--cold] [--cpu-speed N] [--frame-delay MS] [--no-gpu-sync]");
            eprintln!("       rsgun --headless --frames N [--script FILE] [--shot FILE.ppm] [--wav FILE.wav] [--native] [--cold] [--native-boot] [--cpu-speed N] [--rom DIR] [--save DIR]");
            return ExitCode::from(2);
        }
    };
    if let Some(dir) = &o.boot_check {
        return match native_boot_check(&o, dir) {
            Ok(0) => ExitCode::SUCCESS,
            Ok(_) => ExitCode::FAILURE,
            Err(e) => {
                eprintln!("rsgun: {e}");
                ExitCode::from(2)
            }
        };
    }
    let script = match &o.script {
        Some(p) => match fs::read_to_string(p).map_err(|e| e.to_string()).and_then(|t| Script::parse(&t)) {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!("rsgun: {}: {e}", p.display());
                return ExitCode::from(2);
            }
        },
        None => None,
    };
    let result = if o.headless { headless(&o, script.as_ref()) } else { windowed(&o, script.as_ref()) };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("rsgun: {e}");
            ExitCode::FAILURE
        }
    }
}
