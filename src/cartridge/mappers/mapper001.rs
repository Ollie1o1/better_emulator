use super::{Mapper, MappedAddr};
use crate::cartridge::Mirroring;

/// MMC1 — serial shift register, flexible bank switching.
/// Used by: Legend of Zelda, Mega Man 2, Metroid, etc.
pub struct Mapper001 {
    shift: u8,
    shift_count: u8,

    // Internal registers written via shift register
    control: u8,
    chr_bank0: u8,
    chr_bank1: u8,
    prg_bank: u8,

    prg_banks: u8,
    chr_banks: u8,
    base_mirroring: Mirroring,
}

impl Mapper001 {
    pub fn new(prg_banks: u8, chr_banks: u8, mirroring: Mirroring) -> Self {
        Self {
            shift: 0x10,
            shift_count: 0,
            control: 0x0C,  // PRG fix-last mode by default
            chr_bank0: 0,
            chr_bank1: 0,
            prg_bank: 0,
            prg_banks,
            chr_banks,
            base_mirroring: mirroring,
        }
    }

    fn write_register(&mut self, addr: u16, val: u8) {
        if val & 0x80 != 0 {
            self.shift = 0x10;
            self.shift_count = 0;
            self.control |= 0x0C;
            return;
        }

        self.shift = (self.shift >> 1) | ((val & 1) << 4);
        self.shift_count += 1;

        if self.shift_count == 5 {
            let reg = (addr >> 13) & 0x3;
            match reg {
                0 => self.control   = self.shift & 0x1F,
                1 => self.chr_bank0 = self.shift & 0x1F,
                2 => self.chr_bank1 = self.shift & 0x1F,
                3 => self.prg_bank  = self.shift & 0x0F,
                _ => unreachable!(),
            }
            self.shift = 0x10;
            self.shift_count = 0;
        }
    }

    fn prg_mode(&self) -> u8 { (self.control >> 2) & 0x3 }
    fn chr_mode(&self) -> u8 { (self.control >> 4) & 0x1 }
}

impl Mapper for Mapper001 {
    fn cpu_map_read(&self, addr: u16) -> MappedAddr {
        if addr >= 0x6000 && addr < 0x8000 {
            return MappedAddr::PrgRam((addr & 0x1FFF) as usize);
        }
        if addr < 0x8000 {
            return MappedAddr::None;
        }

        let last = (self.prg_banks - 1) as usize;
        let bank = match self.prg_mode() {
            0 | 1 => {
                // 32 KB mode, ignore low bit
                let b = (self.prg_bank & 0xFE) as usize;
                if addr < 0xC000 { b } else { b + 1 }
            }
            2 => {
                // Fix first bank at $8000, switch $C000
                if addr < 0xC000 { 0 } else { self.prg_bank as usize }
            }
            3 => {
                // Switch $8000, fix last bank at $C000
                if addr < 0xC000 { self.prg_bank as usize } else { last }
            }
            _ => unreachable!(),
        };

        let offset = (addr & 0x3FFF) as usize;
        MappedAddr::PrgRom((bank * 0x4000) + offset)
    }

    fn cpu_map_write(&mut self, addr: u16, val: u8) -> MappedAddr {
        if addr >= 0x6000 && addr < 0x8000 {
            return MappedAddr::PrgRam((addr & 0x1FFF) as usize);
        }
        if addr >= 0x8000 {
            self.write_register(addr, val);
        }
        MappedAddr::None
    }

    fn ppu_map_read(&self, addr: u16) -> usize {
        let addr = addr & 0x1FFF;
        if self.chr_banks == 0 {
            return addr as usize; // CHR RAM, no banking
        }
        if self.chr_mode() == 0 {
            // 8 KB mode
            let bank = (self.chr_bank0 & 0xFE) as usize;
            (bank * 0x2000) + addr as usize
        } else {
            // 4 KB mode
            if addr < 0x1000 {
                (self.chr_bank0 as usize * 0x1000) + addr as usize
            } else {
                (self.chr_bank1 as usize * 0x1000) + (addr & 0x0FFF) as usize
            }
        }
    }

    fn ppu_map_write(&mut self, addr: u16) -> usize {
        (addr & 0x1FFF) as usize
    }

    fn mirroring(&self) -> Mirroring {
        match self.control & 0x3 {
            0 => Mirroring::SingleScreenLow,
            1 => Mirroring::SingleScreenHigh,
            2 => Mirroring::Vertical,
            3 => Mirroring::Horizontal,
            _ => self.base_mirroring,
        }
    }
}
