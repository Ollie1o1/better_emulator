mod mapper000;
mod mapper001;

pub use mapper000::Mapper000;
pub use mapper001::Mapper001;

use crate::cartridge::Mirroring;

pub enum MappedAddr {
    PrgRom(usize),
    PrgRam(usize),
    None,
}

pub trait Mapper {
    fn cpu_map_read(&self, addr: u16) -> MappedAddr;
    fn cpu_map_write(&mut self, addr: u16, val: u8) -> MappedAddr;
    fn ppu_map_read(&self, addr: u16) -> usize;
    fn ppu_map_write(&mut self, addr: u16) -> usize;
    fn mirroring(&self) -> Mirroring;
    fn irq_active(&self) -> bool { false }
    fn irq_clear(&mut self) {}
}

pub enum MapperEnum {
    M000(Mapper000),
    M001(Mapper001),
}

impl MapperEnum {
    pub fn cpu_map_read(&self, addr: u16) -> MappedAddr {
        match self {
            Self::M000(m) => m.cpu_map_read(addr),
            Self::M001(m) => m.cpu_map_read(addr),
        }
    }
    pub fn cpu_map_write(&mut self, addr: u16, val: u8) -> MappedAddr {
        match self {
            Self::M000(m) => m.cpu_map_write(addr, val),
            Self::M001(m) => m.cpu_map_write(addr, val),
        }
    }
    pub fn ppu_map_read(&self, addr: u16) -> usize {
        match self {
            Self::M000(m) => m.ppu_map_read(addr),
            Self::M001(m) => m.ppu_map_read(addr),
        }
    }
    pub fn ppu_map_write(&mut self, addr: u16) -> usize {
        match self {
            Self::M000(m) => m.ppu_map_write(addr),
            Self::M001(m) => m.ppu_map_write(addr),
        }
    }
    pub fn mirroring(&self) -> Mirroring {
        match self {
            Self::M000(m) => m.mirroring(),
            Self::M001(m) => m.mirroring(),
        }
    }
    pub fn irq_active(&self) -> bool {
        match self {
            Self::M000(m) => m.irq_active(),
            Self::M001(m) => m.irq_active(),
        }
    }
    pub fn irq_clear(&mut self) {
        match self {
            Self::M000(m) => m.irq_clear(),
            Self::M001(m) => m.irq_clear(),
        }
    }
}

impl From<Mapper000> for MapperEnum {
    fn from(m: Mapper000) -> Self { Self::M000(m) }
}
impl From<Mapper001> for MapperEnum {
    fn from(m: Mapper001) -> Self { Self::M001(m) }
}
