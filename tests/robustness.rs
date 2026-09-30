//! Robustness: the core must never panic or hang, whatever bytes it is fed.
//!
//! The web build loads arbitrary files people drop on the page, and a panic
//! there aborts the whole WebAssembly instance. These tests boot random
//! program code under every supported mapper and header-flag combination
//! (random code pokes mapper, PPU and APU registers in every way), plus
//! malformed and truncated headers.
//!
//! `NES_FUZZ_ROUNDS=50 cargo test --release --test robustness` for a longer soak.

use std::panic::{catch_unwind, AssertUnwindSafe};

use nes_emulator::Emulator;

/// xorshift64*: deterministic, so a failure reproduces from its seed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn byte(&mut self) -> u8 {
        self.next() as u8
    }
}

fn rom(mapper: u8, flags6_low: u8, prg_banks: u8, chr_banks: u8, rng: &mut Rng) -> Vec<u8> {
    #[rustfmt::skip]
    let mut data = vec![
        b'N', b'E', b'S', 0x1A,
        prg_banks, chr_banks,
        (mapper << 4) | flags6_low, mapper & 0xF0,
        0, 0, 0, 0, 0, 0, 0, 0,
    ];
    let trainer = if flags6_low & 0x04 != 0 { 512 } else { 0 };
    let body = trainer + prg_banks as usize * 16384 + chr_banks as usize * 8192;
    data.extend((0..body).map(|_| rng.byte()));
    data
}

/// Run a few frames with random input; returns the panic message, if any.
fn run(data: &[u8], frames: usize, rng: &mut Rng) -> Result<(), String> {
    let seed = rng.next();
    catch_unwind(AssertUnwindSafe(|| {
        let mut rng = Rng(seed | 1);
        let Ok(mut emu) = Emulator::new(data, 44_100) else {
            return;
        };
        for f in 0..frames {
            emu.bus.controller1.buttons = rng.byte();
            emu.bus.controller2.buttons = rng.byte();
            emu.run_frame();
            if f == frames / 2 {
                emu.reset();
            }
        }
        let mut audio = Vec::new();
        emu.bus.apu.drain_samples(&mut audio);
    }))
    .map_err(|e| {
        e.downcast_ref::<String>()
            .cloned()
            .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default()
    })
}

fn rounds() -> u64 {
    std::env::var("NES_FUZZ_ROUNDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
}

#[test]
fn random_code_on_every_mapper_and_header() {
    let mut failures = Vec::new();
    for round in 0..rounds() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ round.wrapping_mul(0xA24B_AED4_963E_E407));
        for mapper in [0u8, 1, 2, 3, 4, 7] {
            // mirroring, battery, trainer, four-screen
            for flags in 0..16u8 {
                for (prg, chr) in [(0, 0), (1, 0), (1, 1), (2, 1), (3, 3), (8, 0), (16, 16), (32, 32)] {
                    let data = rom(mapper, flags, prg, chr, &mut rng);
                    if let Err(msg) = run(&data, 6, &mut rng) {
                        failures.push(format!("round {round} mapper {mapper} flags6 {flags:#x} prg {prg} chr {chr}: {msg}"));
                    }
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} panics:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn malformed_headers_are_rejected_not_panics() {
    let mut rng = Rng(42);
    let mut failures = Vec::new();
    let valid = rom(4, 0, 2, 1, &mut rng);
    // Every truncation of a valid image
    for len in (0..valid.len()).step_by(997).chain(0..20) {
        if let Err(msg) = run(&valid[..len], 1, &mut rng) {
            failures.push(format!("truncated to {len}: {msg}"));
        }
    }
    // Random header bytes over a large random body
    for _ in 0..300 {
        let mut data = vec![b'N', b'E', b'S', 0x1A];
        data.extend((0..12).map(|_| rng.byte()));
        data[4] %= 8;
        data[5] %= 8;
        data.extend((0..200_000).map(|_| rng.byte()));
        if let Err(msg) = run(&data, 2, &mut rng) {
            failures.push(format!("header {:02X?}: {msg}", &data[..16]));
        }
    }
    assert!(
        failures.is_empty(),
        "{} panics:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
