//! WebAssembly exports for the NES core.
//!
//! Plain `extern "C"` functions over a single emulator instance, so the page
//! needs nothing but `WebAssembly.instantiate` — no wasm-bindgen glue. Buffers
//! (ROM upload, RGBA frame, audio samples) live in wasm memory and are shared
//! with JavaScript by pointer + length.

use nes_emulator::{Emulator, SCREEN_HEIGHT, SCREEN_WIDTH};

struct State {
    emu: Option<Emulator>,
    rom: Vec<u8>,
    rgba: Vec<u8>,
    audio: Vec<f32>,
}

static mut STATE: State = State { emu: None, rom: Vec::new(), rgba: Vec::new(), audio: Vec::new() };

// The page is single-threaded and calls in one at a time.
#[allow(static_mut_refs)]
fn state() -> &'static mut State {
    unsafe { &mut STATE }
}

/// Reserve `len` bytes for a ROM image; JS copies the file here, then calls `load_rom`.
#[no_mangle]
pub extern "C" fn rom_buffer(len: usize) -> *mut u8 {
    let s = state();
    s.rom = vec![0; len];
    s.rom.as_mut_ptr()
}

/// Boot the ROM previously copied into `rom_buffer`. Returns 1 on success.
#[no_mangle]
pub extern "C" fn load_rom(sample_rate: u32) -> u32 {
    let s = state();
    match Emulator::new(&s.rom, sample_rate) {
        Ok(emu) => {
            s.emu = Some(emu);
            s.rgba = vec![0; SCREEN_WIDTH * SCREEN_HEIGHT * 4];
            1
        }
        Err(_) => {
            s.emu = None;
            0
        }
    }
}

/// Controller state as NES button bitmasks (A=1, B=2, Select=4, Start=8, Up..Right=16..128).
#[no_mangle]
pub extern "C" fn set_buttons(p1: u8, p2: u8) {
    if let Some(emu) = state().emu.as_mut() {
        emu.bus.controller1.buttons = p1;
        emu.bus.controller2.buttons = p2;
    }
}

/// Emulate one video frame; the RGBA image and new audio become readable.
#[no_mangle]
pub extern "C" fn run_frame() {
    let s = state();
    let Some(emu) = s.emu.as_mut() else { return };
    emu.run_frame();

    // PPU writes BGRA; canvas ImageData wants RGBA
    for (dst, src) in s.rgba.chunks_exact_mut(4).zip(emu.bus.ppu.frame_buffer.chunks_exact(4)) {
        dst[0] = src[2];
        dst[1] = src[1];
        dst[2] = src[0];
        dst[3] = 0xFF;
    }
    s.audio.clear();
    emu.bus.apu.drain_samples(&mut s.audio);
}

#[no_mangle]
pub extern "C" fn frame_ptr() -> *const u8 {
    state().rgba.as_ptr()
}

#[no_mangle]
pub extern "C" fn audio_ptr() -> *const f32 {
    state().audio.as_ptr()
}

#[no_mangle]
pub extern "C" fn audio_len() -> usize {
    state().audio.len()
}

#[no_mangle]
pub extern "C" fn reset() {
    if let Some(emu) = state().emu.as_mut() {
        emu.reset();
    }
}
