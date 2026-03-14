pub mod mappers;

use mappers::{Mapper000, Mapper001, MapperEnum};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mirroring {
    Horizontal,
    Vertical,
    FourScreen,
    SingleScreenLow,
    SingleScreenHigh,
}

#[derive(Debug)]
pub enum CartridgeError {
    InvalidHeader,
    UnsupportedMapper(u8),
    TooShort,
}

impl std::fmt::Display for CartridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidHeader => write!(f, "Invalid iNES header"),
            Self::UnsupportedMapper(n) => write!(f, "Unsupported mapper: {}", n),
            Self::TooShort => write!(f, "ROM data too short"),
        }
    }
}

impl std::error::Error for CartridgeError {}

pub struct Cartridge {
    pub prg_rom: Vec<u8>,
    pub chr_rom: Vec<u8>,
    pub chr_ram: Vec<u8>,
    pub prg_ram: Vec<u8>,
    pub has_battery: bool,
    pub mapper: MapperEnum,
    pub base_mirroring: Mirroring,
}

impl Cartridge {
    pub fn from_ines(data: &[u8]) -> Result<Self, CartridgeError> {
        if data.len() < 16 {
            return Err(CartridgeError::TooShort);
        }
        if &data[0..4] != b"NES\x1A" {
            return Err(CartridgeError::InvalidHeader);
        }

        let prg_banks = data[4] as usize;
        let chr_banks = data[5] as usize;
        let flags6 = data[6];
        let flags7 = data[7];

        let mirroring = if flags6 & 0x08 != 0 {
            Mirroring::FourScreen
        } else if flags6 & 0x01 != 0 {
            Mirroring::Vertical
        } else {
            Mirroring::Horizontal
        };

        let has_battery = flags6 & 0x02 != 0;
        let has_trainer = flags6 & 0x04 != 0;
        let mapper_id = (flags7 & 0xF0) | (flags6 >> 4);

        let trainer_offset = if has_trainer { 512 } else { 0 };
        let prg_start = 16 + trainer_offset;
        let prg_size = prg_banks * 16384;
        let chr_start = prg_start + prg_size;
        let chr_size = chr_banks * 8192;

        if data.len() < chr_start + chr_size {
            return Err(CartridgeError::TooShort);
        }

        let prg_rom = data[prg_start..prg_start + prg_size].to_vec();
        let chr_rom = data[chr_start..chr_start + chr_size].to_vec();
        let chr_ram = if chr_banks == 0 { vec![0u8; 8192] } else { Vec::new() };
        let prg_ram = vec![0u8; 8192];

        let mapper: MapperEnum = match mapper_id {
            0 => Mapper000::new(prg_banks as u8, chr_banks as u8, mirroring).into(),
            1 => Mapper001::new(prg_banks as u8, chr_banks as u8, mirroring).into(),
            _ => return Err(CartridgeError::UnsupportedMapper(mapper_id)),
        };

        log::info!(
            "Loaded ROM: mapper={}, PRG={} banks, CHR={} banks, mirroring={:?}, battery={}",
            mapper_id, prg_banks, chr_banks, mirroring, has_battery
        );

        Ok(Cartridge {
            prg_rom,
            chr_rom,
            chr_ram,
            prg_ram,
            has_battery,
            mapper,
            base_mirroring: mirroring,
        })
    }

    pub fn cpu_read(&self, addr: u16) -> u8 {
        use mappers::MappedAddr;
        match self.mapper.cpu_map_read(addr) {
            MappedAddr::PrgRom(i) => self.prg_rom.get(i).copied().unwrap_or(0),
            MappedAddr::PrgRam(i) => self.prg_ram.get(i).copied().unwrap_or(0),
            MappedAddr::None => 0,
        }
    }

    pub fn cpu_write(&mut self, addr: u16, val: u8) {
        use mappers::MappedAddr;
        match self.mapper.cpu_map_write(addr, val) {
            MappedAddr::PrgRam(i) => {
                if i < self.prg_ram.len() {
                    self.prg_ram[i] = val;
                }
            }
            _ => {}
        }
    }

    pub fn ppu_read(&self, addr: u16) -> u8 {
        let addr = addr & 0x1FFF;
        let i = self.mapper.ppu_map_read(addr);
        if self.chr_rom.is_empty() {
            self.chr_ram.get(i).copied().unwrap_or(0)
        } else {
            self.chr_rom.get(i).copied().unwrap_or(0)
        }
    }

    pub fn ppu_write(&mut self, addr: u16, val: u8) {
        let addr = addr & 0x1FFF;
        let i = self.mapper.ppu_map_write(addr);
        if self.chr_rom.is_empty() && i < self.chr_ram.len() {
            self.chr_ram[i] = val;
        }
    }

    pub fn mirroring(&self) -> Mirroring {
        self.mapper.mirroring()
    }

    pub fn irq_active(&self) -> bool {
        self.mapper.irq_active()
    }

    pub fn irq_clear(&mut self) {
        self.mapper.irq_clear();
    }
}
