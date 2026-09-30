//! Headless screenshot tool: run a ROM for N frames with scripted input and
//! save the final frame as a PPM image.
//!
//!   cargo run --release --example snapshot -- rom.nes 600 out.ppm "120-130:START,300-400:RIGHT+A"
//!
//! Input script: comma-separated `FROM-TO:BUTTON+BUTTON` frame ranges.
//!
//! Set `SNAPSHOT_SEQ=FROM-TO/STEP` to also write every STEP-th frame in that
//! range as `<out>.NNNNN.ppm` (for building GIFs).

use nes_emulator::{buttons, Emulator, SCREEN_HEIGHT, SCREEN_WIDTH};

fn parse_script(script: &str) -> Vec<(u32, u32, u8)> {
    script
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|entry| {
            let (range, btns) = entry.split_once(':').expect("FROM-TO:BUTTONS");
            let (from, to) = range.split_once('-').unwrap_or((range, range));
            let mask = btns.split('+').fold(0u8, |m, b| m | match b {
                "A" => buttons::A,
                "B" => buttons::B,
                "SELECT" => buttons::SELECT,
                "START" => buttons::START,
                "UP" => buttons::UP,
                "DOWN" => buttons::DOWN,
                "LEFT" => buttons::LEFT,
                "RIGHT" => buttons::RIGHT,
                other => panic!("unknown button {other}"),
            });
            (from.parse().unwrap(), to.parse().unwrap(), mask)
        })
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: snapshot <rom> <frames> <out.ppm> [script]");
        std::process::exit(1);
    }
    let rom = std::fs::read(&args[1]).expect("read rom");
    let frames: u32 = args[2].parse().expect("frames");
    let script = parse_script(args.get(4).map(String::as_str).unwrap_or(""));

    let mut emu = Emulator::new(&rom, 44_100).expect("load rom");
    let seq: Option<(u32, u32, u32)> = std::env::var("SNAPSHOT_SEQ").ok().map(|v| {
        let (range, step) = v.split_once('/').expect("FROM-TO/STEP");
        let (a, b) = range.split_once('-').expect("FROM-TO/STEP");
        (a.parse().unwrap(), b.parse().unwrap(), step.parse().unwrap())
    });
    let mut sink = Vec::new();
    for f in 0..frames {
        emu.bus.controller1.buttons = script
            .iter()
            .filter(|(a, b, _)| (*a..=*b).contains(&f))
            .fold(0, |m, (_, _, b)| m | b);
        emu.run_frame();
        emu.bus.apu.drain_samples(&mut sink);
        sink.clear();
        if let Some((a, b, step)) = seq {
            if f >= a && f <= b && (f - a) % step == 0 {
                write_ppm(&format!("{}.{:05}.ppm", args[3], f), emu.bus.ppu.frame_buffer.as_ref());
            }
        }
    }
    write_ppm(&args[3], emu.bus.ppu.frame_buffer.as_ref());
}

fn write_ppm(path: &str, fb: &[u8]) {
    let mut out = format!("P6 {} {} 255\n", SCREEN_WIDTH, SCREEN_HEIGHT).into_bytes();
    for px in fb.chunks_exact(4) {
        out.extend_from_slice(&[px[2], px[1], px[0]]); // BGRA → RGB
    }
    std::fs::write(path, out).expect("write image");
}
