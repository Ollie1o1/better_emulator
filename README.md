# NES Emulator

A low-level Nintendo Entertainment System emulator written in Rust. Load any `.nes` ROM file and play.

## Quick Start

```bash
cargo run --release -- roms/nova_the_squirrel.nes
```

## Requirements

| Tool | Why |
|------|-----|
| [Rust](https://rustup.rs) | Build toolchain |
| [CMake](https://cmake.org/download/) | Compiles bundled SDL2 |

```powershell
winget install Rustlang.Rustup
winget install Kitware.CMake
```

## Controls

| NES Button | Keys |
|-----------|------|
| A | `Z` or `Alt` |
| B | `X` or `Ctrl` |
| **Start** | **`Enter`, `Space`, or numpad Enter** |
| Select | `Shift`, `Tab`, or `Backspace` |
| D-pad | Arrow keys or `WASD` |
| Quit | `Escape` |

## Included Free Games

All games in `roms/` are free, open-source homebrew:

| File | Genre | Notes |
|------|-------|-------|
| `nova_the_squirrel.nes` | Platformer | Full game — closest to Mario in feel |
| `twin_dragons.nes` | Action platformer | 1 or 2 player co-op |
| `assimilate.nes` | Puzzle | Pac-Man style |
| `flappy_bird.nes` | Arcade | NES port of Flappy Bird |
| `2048.nes` | Puzzle | 2048 on NES |
| `thwaite.nes` | Shooter | Missile Command style |
| `nestest.nes` | CPU test | Developer validation ROM |

To play a commercial game (e.g. Super Mario Bros), provide your own legally-obtained `.nes` ROM:

```bash
cargo run --release -- path/to/your/game.nes
```

## Compatible Games

The emulator runs games using the three most common NES mappers:

| Mapper | Examples |
|--------|---------|
| **0 — NROM** | Super Mario Bros, Donkey Kong, Pac-Man, Galaga |
| **1 — MMC1** | Legend of Zelda, Mega Man 2, Metroid, Castlevania II |
| **2 — UxROM** | Mega Man, Castlevania, Contra, Duck Tales |

## UI

- **Scanlines** — every other row dimmed 20% for a CRT look
- **8:7 aspect ratio** — NES pixels are non-square; the display is stretched correctly to 292×240
- **Status bar** — shows live D-pad, button state, and FPS counter with color coding (green ≥58fps, yellow ≥45fps, red <45fps)
- **Window title** — shows ROM name and FPS

## Architecture

```
src/
├── main.rs              # SDL2 window, input, render loop
├── emulator.rs          # Clock orchestrator (CPU/PPU/APU sync)
├── bus.rs               # Memory bus — all address decode and memory-mapped I/O
├── ui.rs                # Status bar renderer (embedded 3×5 pixel font, button indicators)
├── cpu/mod.rs           # Ricoh 2A03 (6502) — all official + common unofficial opcodes
├── ppu/mod.rs           # PPU — Loopy scroll, background/sprite rendering, 256×240 ARGB output
├── apu/                 # APU — pulse (×2), triangle, noise, DMC channels
├── cartridge/
│   ├── mod.rs           # iNES ROM loader
│   └── mappers/         # Mapper 0 (NROM), Mapper 1 (MMC1), Mapper 2 (UxROM)
└── controller/mod.rs    # NES controller — correct falling-edge latch, serial shift register
```

**Key design decisions:**
- Central `Bus` struct owns all components — no `Rc<RefCell<T>>` anywhere
- CPU drives the master clock; PPU ticks 3× per CPU cycle (matching NTSC hardware)
- NMI/IRQ propagate from PPU/mapper through the emulator loop to the CPU

## Adding Mappers

1. Create `src/cartridge/mappers/mapperNNN.rs` implementing the `Mapper` trait
2. Add a variant to `MapperEnum` in `src/cartridge/mappers/mod.rs`
3. Handle the new mapper ID in `Cartridge::from_ines`

**Next mappers to add** (by game library impact):

| Mapper | Examples |
|--------|---------|
| **4 — MMC3** | Super Mario Bros 2 & 3, Mega Man 3–6, Kirby's Adventure |
| **7 — AxROM** | Battletoads, Marble Madness |
| **3 — CNROM** | Gradius, Paperboy |

## Debug Logging

```bash
RUST_LOG=warn cargo run --release -- rom.nes
```
