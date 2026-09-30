mod mapper000;
mod mapper001;
mod mapper002;
mod mapper003;
mod mapper004;
mod mapper007;

pub use mapper000::Mapper000;
pub use mapper001::Mapper001;
pub use mapper002::Mapper002;
pub use mapper003::Mapper003;
pub use mapper004::Mapper004;
pub use mapper007::Mapper007;

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
    /// Called on each filtered PPU A12 rising edge (≈ once per scanline) for MMC3.
    fn scanline(&mut self) {}
    fn irq_active(&self) -> bool { false }
    fn irq_clear(&mut self) {}
}

pub enum MapperEnum {
    M000(Mapper000),
    M001(Mapper001),
    M002(Mapper002),
    M003(Mapper003),
    M004(Mapper004),
    M007(Mapper007),
}

/// Forward a call to whichever mapper is inside the enum (static dispatch).
macro_rules! dispatch {
    ($self:expr, $m:ident => $body:expr) => {
        match $self {
            MapperEnum::M000($m) => $body,
            MapperEnum::M001($m) => $body,
            MapperEnum::M002($m) => $body,
            MapperEnum::M003($m) => $body,
            MapperEnum::M004($m) => $body,
            MapperEnum::M007($m) => $body,
        }
    };
}

impl MapperEnum {
    pub fn cpu_map_read(&self, addr: u16) -> MappedAddr { dispatch!(self, m => m.cpu_map_read(addr)) }
    pub fn cpu_map_write(&mut self, addr: u16, val: u8) -> MappedAddr { dispatch!(self, m => m.cpu_map_write(addr, val)) }
    pub fn ppu_map_read(&self, addr: u16) -> usize { dispatch!(self, m => m.ppu_map_read(addr)) }
    pub fn ppu_map_write(&mut self, addr: u16) -> usize { dispatch!(self, m => m.ppu_map_write(addr)) }
    pub fn mirroring(&self) -> Mirroring { dispatch!(self, m => m.mirroring()) }
    pub fn scanline(&mut self) { dispatch!(self, m => m.scanline()) }
    pub fn irq_active(&self) -> bool { dispatch!(self, m => m.irq_active()) }
    pub fn irq_clear(&mut self) { dispatch!(self, m => m.irq_clear()) }
}

macro_rules! impl_from {
    ($($t:ident => $v:ident),*) => {
        $(impl From<$t> for MapperEnum { fn from(m: $t) -> Self { Self::$v(m) } })*
    };
}
impl_from!(Mapper000 => M000, Mapper001 => M001, Mapper002 => M002,
           Mapper003 => M003, Mapper004 => M004, Mapper007 => M007);
