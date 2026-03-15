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

### Game Buttons

| NES Button | Keys |
|-----------|------|
| A | `Z` or `Alt` |
| B | `X` or `Ctrl` |
| **Start** | `Enter`, `Space`, or numpad Enter |
| Select | `Shift`, `Tab`, or `Backspace` |
| D-pad Up | `↑` or `W` |
| D-pad Down | `↓` or `S` |
| D-pad Left | `←` or `A` |
| D-pad Right | `→` or `D` |

### Emulator Controls

| Key | Action |
|-----|--------|
| `]` | Volume up (10%) |
| `[` | Volume down (10%) |
| `Escape` | Quit |

## Playing Games

### 1. Launch a game

```bash
cargo run --release -- roms/nova_the_squirrel.nes
```

Replace the path with any `.nes` file. On Windows you can also drag a ROM onto the executable.

### 2. At a title screen

Most games show a title screen first. Press **Start** (`Enter`) to begin. Some games also need **Select** (`Shift`) to choose a mode before starting.

### 3. In-game tips

- **2048** — Use the arrow keys or WASD to slide tiles. No Start needed, tiles move immediately.
- **Flappy Bird** — Press `Z` (A button) to flap.
- **Nova the Squirrel** — `Z` to jump, `X` to attack. **Start** pauses.
- **Twin Dragons** — Same controls. A second player can join by mapping a second controller (not yet supported; play in single-player mode).
- **Thwaite** — Arrow keys aim, `Z` fires.
- **Assimilate** — Arrow keys to move.

### 4. Adjust the volume

Press `]` to turn music up and `[` to turn it down. The teal bar in the status strip at the bottom of the window shows the current level (10 segments = 100%).

## Status Bar

The strip along the bottom of the window shows live info while you play:

```
[D-pad]  [SEL] [START]    [FPS]   [volume bar]   [B] [A]
```

| Element | What it shows |
|---------|---------------|
| D-pad cross | Which direction is held |
| SEL / START pads | Whether Select / Start is pressed |
| FPS counter | Frames per second — green ≥58, yellow ≥45, red <45 |
| Teal bar (10 segments) | Music volume (0–100%) |
| B / A circles | Whether B / A is pressed |

## Included Free Games

All games in `roms/` are free, open-source homebrew:

| File | Genre | How to start |
|------|-------|--------------|
| `nova_the_squirrel.nes` | Platformer | Press **Start** at title screen |
| `twin_dragons.nes` | Action platformer | Press **Start** at title screen |
| `assimilate.nes` | Puzzle | Press **Start** at title screen |
| `flappy_bird.nes` | Arcade | Press **A** (`Z`) to begin |
| `2048.nes` | Puzzle | Press **Start** at title screen |
| `thwaite.nes` | Shooter | Press **Start** at title screen |
| `nestest.nes` | CPU test ROM | Developer validation, no gameplay |

## Playing Your Own ROMs

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
- **8:7 aspect ratio** — NES pixels are non-square; display is stretched to 292×240
- **Status bar** — live button state, FPS counter, and volume indicator
- **Window title** — shows ROM name and live FPS

## Architecture

```
src/
├── main.rs              # SDL2 window, input, render loop
├── emulator.rs          # Clock orchestrator (CPU/PPU/APU sync)
├── bus.rs               # Memory bus — address decode and memory-mapped I/O
├── ui.rs                # Status bar renderer (embedded 3×5 pixel font, indicators)
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
