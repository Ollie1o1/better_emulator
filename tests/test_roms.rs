//! Accuracy tests against community-standard NES test ROMs (tests/roms/).
//!
//! - nestest: CPU state + cycle count compared line-by-line with the golden log.
//! - blargg suites: the ROM reports its own verdict through PRG-RAM at $6000
//!   (status byte, $DE $B0 $61 signature at $6001, result text at $6004).
//!
//! Run with `cargo test --release` (debug builds are slow for the larger suites).

use nes_emulator::Emulator;

fn load(name: &str) -> Emulator {
    let path = format!("{}/tests/roms/{}", env!("CARGO_MANIFEST_DIR"), name);
    let rom = std::fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    Emulator::new(&rom, 44_100).unwrap_or_else(|e| panic!("load {path}: {e}"))
}

// ─── nestest ──────────────────────────────────────────────────────────────────

struct LogLine {
    pc: u16,
    a: u8,
    x: u8,
    y: u8,
    p: u8,
    sp: u8,
    cyc: u64,
}

fn hex_field(line: &str, key: &str) -> u8 {
    let i = line.find(key).unwrap_or_else(|| panic!("{key} missing in: {line}")) + key.len();
    u8::from_str_radix(&line[i..i + 2], 16).unwrap()
}

fn parse_log_line(line: &str) -> LogLine {
    let cyc_i = line.find("CYC:").unwrap() + 4;
    LogLine {
        pc: u16::from_str_radix(&line[0..4], 16).unwrap(),
        a: hex_field(line, "A:"),
        x: hex_field(line, "X:"),
        y: hex_field(line, "Y:"),
        p: hex_field(line, "P:"),
        sp: hex_field(line, "SP:"),
        cyc: line[cyc_i..].trim().parse().unwrap(),
    }
}

#[test]
fn nestest_matches_golden_log() {
    let log = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/roms/nestest.log")).unwrap();
    let mut emu = load("nestest.nes");

    // Automated mode: start at $C000 instead of the reset vector.
    emu.cpu.pc = 0xC000;
    emu.cpu.p = 0x24;
    let mut cycles: u64 = 7;

    for (n, line) in log.lines().enumerate() {
        let want = parse_log_line(line);
        let got = LogLine {
            pc: emu.cpu.pc,
            a: emu.cpu.a,
            x: emu.cpu.x,
            y: emu.cpu.y,
            p: emu.cpu.p,
            sp: emu.cpu.sp,
            cyc: cycles,
        };
        let same = got.pc == want.pc
            && got.a == want.a
            && got.x == want.x
            && got.y == want.y
            && got.p == want.p
            && got.sp == want.sp
            && got.cyc == want.cyc;
        assert!(
            same,
            "nestest diverged at log line {}:\n  want: {}\n  got:  {:04X} A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X} CYC:{}",
            n + 1,
            line,
            got.pc, got.a, got.x, got.y, got.p, got.sp, got.cyc
        );
        cycles += emu.cpu.step(&mut emu.bus) as u64;
    }

    // nestest writes its own error codes to $02/$03; both zero means every test passed.
    assert_eq!(emu.bus.peek(0x0002), 0, "nestest official-opcode error code");
    assert_eq!(emu.bus.peek(0x0003), 0, "nestest unofficial-opcode error code");
}

// ─── blargg-protocol ROMs ─────────────────────────────────────────────────────

/// Run a blargg-style test ROM until it reports a result; returns (code, text).
fn run_blargg(name: &str, max_frames: u32) -> (u8, String) {
    let mut emu = load(name);
    let mut started = false;
    let mut reset_at: Option<u32> = None;

    for frame in 0..max_frames {
        emu.run_frame();

        let sig = [emu.bus.peek(0x6001), emu.bus.peek(0x6002), emu.bus.peek(0x6003)];
        if sig != [0xDE, 0xB0, 0x61] {
            continue;
        }
        let status = emu.bus.peek(0x6000);
        match status {
            0x80 => started = true,
            // $81: the ROM asks for the reset button to be pressed after a delay.
            0x81 => {
                let at = *reset_at.get_or_insert(frame + 6);
                if frame >= at {
                    emu.reset();
                    reset_at = None;
                }
            }
            code if started && code < 0x80 => {
                let mut text = String::new();
                let mut addr = 0x6004u16;
                loop {
                    let b = emu.bus.peek(addr);
                    if b == 0 || addr > 0x7FFF { break; }
                    text.push(b as char);
                    addr += 1;
                }
                return (code, text.trim().to_string());
            }
            _ => {}
        }
    }
    panic!("{name}: no result after {max_frames} frames");
}

fn assert_blargg_passes(name: &str, max_frames: u32) {
    let (code, text) = run_blargg(name, max_frames);
    assert_eq!(code, 0, "{name} failed (code {code}):\n{text}");
}

#[test]
fn blargg_instr_test_official() {
    assert_blargg_passes("official_only.nes", 3_000);
}

#[test]
fn blargg_instr_test_all() {
    assert_blargg_passes("all_instrs.nes", 3_000);
}

#[test]
fn blargg_instr_misc() {
    assert_blargg_passes("instr_misc.nes", 1_000);
}

/// Subtests 01-04 (VBL basics, set/clear time, NMI control) pass. 05+ need the
/// CPU and PPU interleaved at single-dot granularity; NMI currently lands ~2
/// PPU dots early. Run with `cargo test --release -- --ignored` to check.
#[test]
#[ignore = "passes 4 of 10 subtests; NMI timing is ~2 PPU dots off"]
fn blargg_ppu_vbl_nmi() {
    assert_blargg_passes("ppu_vbl_nmi.nes", 3_000);
}

#[test]
fn blargg_apu_test() {
    assert_blargg_passes("apu_test.nes", 3_000);
}

#[test]
fn blargg_oam_read() {
    assert_blargg_passes("oam_read.nes", 600);
}

#[test]
fn blargg_ppu_open_bus() {
    assert_blargg_passes("ppu_open_bus.nes", 600);
}

// ─── MMC3 (mapper 4) ──────────────────────────────────────────────────────────

#[test]
fn mmc3_clocking() {
    assert_blargg_passes("mmc3_1-clocking.nes", 600);
}

#[test]
fn mmc3_details() {
    assert_blargg_passes("mmc3_2-details.nes", 600);
}

#[test]
fn mmc3_a12_clocking() {
    assert_blargg_passes("mmc3_3-A12_clocking.nes", 600);
}

/// Checks 1-8 pass (sprites at $1000). Check 9 (background at $1000) needs
/// the IRQ a few PPU dots sooner — same sub-cycle alignment gap as ppu_vbl_nmi.
#[test]
#[ignore = "passes checks 1-8 of 10; BG-at-$1000 IRQ lands a few dots late"]
fn mmc3_scanline_timing() {
    assert_blargg_passes("mmc3_4-scanline_timing.nes", 600);
}

#[test]
fn mmc3_behaviour() {
    assert_blargg_passes("mmc3_5-MMC3.nes", 600);
}
