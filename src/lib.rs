//! NES emulator core: 6502 CPU, PPU, APU, cartridge mappers and controllers.
//!
//! The core has no platform dependencies (no SDL, no filesystem), so the same
//! code drives the desktop SDL2 frontend (`src/main.rs`) and the WebAssembly
//! build (`web/`).

pub mod apu;
pub mod bus;
pub mod cartridge;
pub mod controller;
pub mod cpu;
pub mod emulator;
pub mod ppu;

pub use controller::buttons;
pub use emulator::Emulator;
pub use ppu::{SCREEN_HEIGHT, SCREEN_WIDTH};
