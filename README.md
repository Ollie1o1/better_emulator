# NES Emulator

A low-level Nintendo Entertainment System emulator written in Rust. Runs classic NES games from `.nes` ROM files.

## Features

- **Accurate 6502 CPU** — all 56 official opcodes plus common unofficial opcodes
- **Full PPU** — background rendering, sprites (8 per scanline), Loopy scroll registers, nametable mirroring
- **APU** — pulse (×2), triangle, noise, and DMC audio channels with real-time SDL2 output
- **iNES ROM support** — loads standard `.nes` files
- **Mapper 0 (NROM)** — Super Mario Bros, Donkey Kong, Pac-Man, Galaga
- **Mapper 1 (MMC1)** — Legend of Zelda, Mega Man 2, Metroid, Castlevania II, Tetris
- **Modular architecture** — designed to be extended with additional mappers and systems

## Requirements

- [Rust](https://rustup.rs) (stable toolchain)
- [CMake](https://cmake.org/download/) (required to compile bundled SDL2)

```powershell
winget install Rustlang.Rustup
winget install Kitware.CMake
```

## Building

```bash
cargo build --release
```

## Running

```bash
cargo run --release -- path/to/rom.nes
```

Place your `.nes` ROM files in the `roms/` directory (gitignored).

## Controls

| Key | NES Button |
|-----|-----------|
| Arrow keys | D-pad |
| Z | A |
| X | B |
| Enter | Start |
| Right Shift | Select |
| Escape | Quit |

## Project Structure

```
src/
├── main.rs              # SDL2 window, input, render loop
├── emulator.rs          # Clock orchestrator (CPU/PPU/APU sync)
├── bus.rs               # Memory bus — all address decode and memory-mapped I/O
├── cpu/
│   └── mod.rs           # Ricoh 2A03 (6502) CPU
├── ppu/
│   └── mod.rs           # Picture Processing Unit
├── apu/
│   ├── mod.rs           # Audio Processing Unit
│   ├── pulse.rs         # Pulse wave channels
│   ├── triangle.rs      # Triangle wave channel
│   ├── noise.rs         # Noise channel
│   └── dmc.rs           # Delta modulation channel
├── cartridge/
│   ├── mod.rs           # iNES ROM loader
│   └── mappers/
│       ├── mod.rs       # Mapper trait and enum dispatch
│       ├── mapper000.rs # NROM
│       └── mapper001.rs # MMC1
└── controller/
    └── mod.rs           # Standard NES controller
```

## Architecture

Components communicate through a central `Bus` struct — no `Rc<RefCell<T>>` anywhere. The CPU drives the master clock; the PPU is ticked 3 times per CPU cycle (matching NTSC hardware). NMI and IRQ signals are propagated from the PPU/mapper through the emulator clock loop.

## Adding Mappers

Implement the `Mapper` trait in `src/cartridge/mappers/`, add a variant to `MapperEnum`, and handle the new mapper ID in `Cartridge::from_ines`. The most impactful mappers to add next are:

- **Mapper 2 (UxROM)** — Mega Man, Castlevania, Contra
- **Mapper 3 (CNROM)** — Gradius, Paperboy
- **Mapper 4 (MMC3)** — Super Mario Bros 2 & 3, Mega Man 3–6, Kirby's Adventure

## Debug Logging

Set `RUST_LOG=debug` (or `warn`) before running to enable log output:

```bash
RUST_LOG=warn cargo run --release -- rom.nes
```
