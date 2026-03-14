use super::{Mapper, MappedAddr};
use crate::cartridge::Mirroring;

/// NROM — no bank switching, simplest possible mapper.
/// Used by: Donkey Kong, Super Mario Bros, Pac-Man, etc.
pub struct Mapper000 {
    prg_banks: u8,  // 1 = 16 KB mirrored, 2 = 32 KB
    mirroring: Mirroring,
}

impl Mapper000 {
    pub fn new(prg_banks: u8, _chr_banks: u8, mirroring: Mirroring) -> Self {
        Self { prg_banks, mirroring }
    }
}

impl Mapper for Mapper000 {
    fn cpu_map_read(&self, addr: u16) -> MappedAddr {
        if addr >= 0x8000 {
            let mask: u16 = if self.prg_banks > 1 { 0x7FFF } else { 0x3FFF };
            MappedAddr::PrgRom((addr & mask) as usize)
        } else if addr >= 0x6000 {
            MappedAddr::PrgRam((addr & 0x1FFF) as usize)
        } else {
            MappedAddr::None
        }
    }

    fn cpu_map_write(&mut self, addr: u16, _val: u8) -> MappedAddr {
        if addr >= 0x6000 && addr < 0x8000 {
            MappedAddr::PrgRam((addr & 0x1FFF) as usize)
        } else {
            MappedAddr::None
        }
    }

    fn ppu_map_read(&self, addr: u16) -> usize {
        (addr & 0x1FFF) as usize
    }

    fn ppu_map_write(&mut self, addr: u16) -> usize {
        (addr & 0x1FFF) as usize
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }
}
