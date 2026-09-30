pub mod mappers;

use mappers::{Mapper000, Mapper001, Mapper002, Mapper003, Mapper004, Mapper007, MapperEnum};

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
    UnsupportedMapper(u16),
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
    /// CPU cycle of the last write to $8000-$FFFF (MMC1 ignores back-to-back writes).
    last_rom_write: u64,
    /// Current CPU cycle, kept up to date by the bus.
    pub cpu_cycle: u64,
    /// PPU address line A12 state, and the PPU dot it last went low.
    a12_high: bool,
    a12_low_since: u64,
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
        if prg_banks == 0 {
            return Err(CartridgeError::InvalidHeader);
        }
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
        let nes2 = flags7 & 0x0C == 0x08;
        // Old dumps often have junk ("DiskDude!") in bytes 7-15; if the
        // unused tail isn't zero, flags 7 can't be trusted for the mapper.
        let mapper_hi = if nes2 || data[12..16].iter().all(|&b| b == 0) { flags7 & 0xF0 } else { 0 };
        let mapper_id = mapper_hi | (flags6 >> 4);
        // NES 2.0 mapper numbers above 255
        if nes2 && data[8] & 0x0F != 0 {
            return Err(CartridgeError::UnsupportedMapper(((data[8] as u16 & 0x0F) << 8) | mapper_id as u16));
        }

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
            2 => Mapper002::new(prg_banks as u8, mirroring).into(),
            3 => Mapper003::new(prg_banks as u8, chr_banks as u8, mirroring).into(),
            4 => Mapper004::new(prg_banks as u8, chr_banks as u8, mirroring).into(),
            7 => Mapper007::new(prg_banks as u8).into(),
            _ => return Err(CartridgeError::UnsupportedMapper(mapper_id as u16)),
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
            last_rom_write: 0,
            cpu_cycle: 0,
            a12_high: false,
            a12_low_since: 0,
        })
    }

    pub fn cpu_read(&self, addr: u16) -> u8 {
        self.cpu_read_mapped(addr).unwrap_or(0)
    }

    /// Cartridge read; `None` when nothing drives the bus (open bus).
    pub fn cpu_read_mapped(&self, addr: u16) -> Option<u8> {
        use mappers::MappedAddr;
        match self.mapper.cpu_map_read(addr) {
            MappedAddr::PrgRom(i) => self.prg_rom.get(i).copied(),
            MappedAddr::PrgRam(i) => self.prg_ram.get(i).copied(),
            MappedAddr::None => None,
        }
    }

    pub fn cpu_write(&mut self, addr: u16, val: u8) {
        use mappers::MappedAddr;
        if addr >= 0x8000 {
            // MMC1's serial port ignores a write on the cycle right after
            // another (the dummy write of an RMW instruction like INC $8000).
            let back_to_back = self.cpu_cycle == self.last_rom_write + 1;
            self.last_rom_write = self.cpu_cycle;
            if back_to_back && matches!(self.mapper, MapperEnum::M001(_)) {
                return;
            }
        }
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

    /// Every address the PPU puts on its bus. MMC3 clocks its scanline
    /// counter on A12 rising edges, ignoring edges unless A12 has been low
    /// for a few CPU cycles (so the 8 back-to-back sprite fetches of one
    /// line count once).
    pub fn ppu_bus_address(&mut self, addr: u16, ppu_dot: u64) {
        const MIN_LOW_DOTS: u64 = 10; // ~3 CPU cycles
        let high = addr & 0x1000 != 0;
        if high && !self.a12_high && ppu_dot.saturating_sub(self.a12_low_since) >= MIN_LOW_DOTS {
            self.mapper.scanline();
        }
        if !high && self.a12_high {
            self.a12_low_since = ppu_dot;
        }
        self.a12_high = high;
    }

    pub fn irq_active(&self) -> bool {
        self.mapper.irq_active()
    }

    pub fn irq_clear(&mut self) {
        self.mapper.irq_clear();
    }
}
