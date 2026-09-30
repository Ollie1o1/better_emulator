use super::{Mapper, MappedAddr};
use crate::cartridge::Mirroring;

/// AxROM — switchable 32 KB PRG bank, single-screen mirroring select, CHR RAM.
/// Used by: Battletoads, Marble Madness, Wizards & Warriors, etc.
pub struct Mapper007 {
    prg_banks32: u8,
    prg_bank: u8,
    upper_screen: bool,
}

impl Mapper007 {
    pub fn new(prg_banks: u8) -> Self {
        Self { prg_banks32: (prg_banks / 2).max(1), prg_bank: 0, upper_screen: false }
    }
}

impl Mapper for Mapper007 {
    fn cpu_map_read(&self, addr: u16) -> MappedAddr {
        match addr {
            0x8000..=0xFFFF => MappedAddr::PrgRom(self.prg_bank as usize * 0x8000 + (addr & 0x7FFF) as usize),
            _ => MappedAddr::None,
        }
    }

    fn cpu_map_write(&mut self, addr: u16, val: u8) -> MappedAddr {
        if addr >= 0x8000 {
            self.prg_bank = (val & 0x07) % self.prg_banks32;
            self.upper_screen = val & 0x10 != 0;
        }
        MappedAddr::None
    }

    fn ppu_map_read(&self, addr: u16) -> usize {
        (addr & 0x1FFF) as usize
    }

    fn ppu_map_write(&mut self, addr: u16) -> usize {
        (addr & 0x1FFF) as usize
    }

    fn mirroring(&self) -> Mirroring {
        if self.upper_screen { Mirroring::SingleScreenHigh } else { Mirroring::SingleScreenLow }
    }
}
