use super::{Mapper, MappedAddr};
use crate::cartridge::Mirroring;

/// CNROM — fixed PRG, switchable 8 KB CHR bank.
/// Used by: Gradius, Paperboy, Arkanoid, Solomon's Key, etc.
pub struct Mapper003 {
    prg_banks: u8,
    chr_banks: u8,
    chr_bank: u8,
    mirroring: Mirroring,
}

impl Mapper003 {
    pub fn new(prg_banks: u8, chr_banks: u8, mirroring: Mirroring) -> Self {
        Self { prg_banks, chr_banks: chr_banks.max(1), chr_bank: 0, mirroring }
    }
}

impl Mapper for Mapper003 {
    fn cpu_map_read(&self, addr: u16) -> MappedAddr {
        match addr {
            // 16 KB carts mirror their one bank into $C000
            0x8000..=0xFFFF => {
                let mask = if self.prg_banks > 1 { 0x7FFF } else { 0x3FFF };
                MappedAddr::PrgRom((addr & mask) as usize)
            }
            _ => MappedAddr::None,
        }
    }

    fn cpu_map_write(&mut self, addr: u16, val: u8) -> MappedAddr {
        if addr >= 0x8000 {
            self.chr_bank = val % self.chr_banks;
        }
        MappedAddr::None
    }

    fn ppu_map_read(&self, addr: u16) -> usize {
        self.chr_bank as usize * 0x2000 + (addr & 0x1FFF) as usize
    }

    fn ppu_map_write(&mut self, addr: u16) -> usize {
        self.ppu_map_read(addr)
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }
}
