//! Runs the machine from power on to the first time the master reaches an address, and writes the
//! state at that instant (entry.state), the memories (dump/) and the last-writer map (lastw.bin).
//!
//!     driver ROOT OUT ADDR   (env: EEPROM=file BACKUP=file RTC=seconds-added)
//!
//! ROOT holds build/rom (the images). Needs the patched stv crate (stv-reach.patch).
use stv::machine::Machine;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let root = &a[1];
    let out = &a[2];
    let addr = u32::from_str_radix(a[3].trim_start_matches("0x"), 16).unwrap();
    let bios = std::fs::read(format!("{root}/build/rom/bios-epr20091.bin")).unwrap();
    let cart = std::fs::read(format!("{root}/build/rom/rsgun-cart.bin")).unwrap();
    let ee = std::env::var("EEPROM").unwrap_or(format!("{root}/build/rom/eeprom.bin"));
    let eeprom = std::fs::read(ee).ok();
    let area: u8 = std::env::var("AREA").ok().map_or(1, |a| a.parse().unwrap());
    let mut m = Machine::new(bios, cart, area, eeprom.as_deref());
    if let Ok(b) = std::env::var("BACKUP") {
        let b = std::fs::read(b).unwrap();
        assert_eq!(b.len(), m.sys.backup.len());
        m.sys.backup = b;
    }
    if let Ok(r) = std::env::var("RTC") {
        m.sys.smpc.rtc_seconds += r.parse::<u64>().unwrap();
    }
    m.render_threads = 4;
    m.breakpoint = Some((0, addr));
    while m.reached.is_none() && m.sys.frame < 3000 {
        m.run_frame();
    }
    println!("reached in frame {:?}", m.reached);
    if let Some(s) = m.reach_state.take() {
        std::fs::create_dir_all(out).unwrap();
        std::fs::write(format!("{out}/entry.state"), &s).unwrap();
        m.load_state(&s).unwrap();
        m.dump(std::path::Path::new(&format!("{out}/dump"))).unwrap();
        let lw: Vec<u8> = m.reach_lastw.iter().flat_map(|x| x.to_le_bytes()).collect();
        std::fs::write(format!("{out}/lastw.bin"), lw).unwrap();
        println!("pc {:08x} frame {}", m.cpu[0].pc, m.sys.frame);
    }
}
