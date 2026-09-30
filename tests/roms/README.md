# Test ROMs

Community-standard NES accuracy tests, used by `tests/test_roms.rs`:

- `nestest.nes` / `nestest.log` — Kevin Horton's CPU test and its golden trace.
- `official_only.nes`, `all_instrs.nes` (instr_test-v5), `instr_misc.nes`,
  `apu_test.nes`, `ppu_vbl_nmi.nes`, `ppu_open_bus.nes`, `oam_read.nes`,
  `mmc3_*.nes` (mmc3_test_2), `cpu_timing_test.nes`, `cpu_dummy_reads.nes`,
  `01.basics.nes`, `1.Branch_Basics.nes` — Shay Green's (blargg) test ROMs.

All are freely redistributed test programs; copies here come from the
collection at https://github.com/christopherpow/nes-test-roms.
