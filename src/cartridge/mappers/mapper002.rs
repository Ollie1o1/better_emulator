use super::{Mapper, MappedAddr};
use crate::cartridge::Mirroring;

/// UxROM — switchable lower PRG bank, fixed upper bank.
/// Used by: Castlevania, Contra, Mega Man, Twin Dragons, etc.
pub struct Mapper002 {
    prg_banks: u8,
    selected_bank: u8,
    mirroring: Mirroring,
}

impl Mapper002 {
    pub fn new(prg_banks: u8, mirroring: Mirroring) -> Self {
        Self { prg_banks, selected_bank: 0, mirroring }
    }
}

impl Mapper for Mapper002 {
    fn cpu_map_read(&self, addr: u16) -> MappedAddr {
        match addr {
            0x8000..=0xBFFF => {
                let offset = (addr & 0x3FFF) as usize;
                MappedAddr::PrgRom(self.selected_bank as usize * 0x4000 + offset)
            }
            0xC000..=0xFFFF => {
                let last = (self.prg_banks - 1) as usize;
                let offset = (addr & 0x3FFF) as usize;
                MappedAddr::PrgRom(last * 0x4000 + offset)
            }
            0x6000..=0x7FFF => MappedAddr::PrgRam((addr & 0x1FFF) as usize),
            _ => MappedAddr::None,
        }
    }

    fn cpu_map_write(&mut self, addr: u16, val: u8) -> MappedAddr {
        if addr >= 0x8000 {
            self.selected_bank = val & 0x0F;
        } else if addr >= 0x6000 {
            return MappedAddr::PrgRam((addr & 0x1FFF) as usize);
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
        self.mirroring
    }
}
