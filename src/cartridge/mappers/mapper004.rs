use super::{Mapper, MappedAddr};
use crate::cartridge::Mirroring;

/// MMC3 (TxROM) — 8 KB PRG / 1-2 KB CHR banking plus a scanline IRQ counter.
/// Used by: Super Mario Bros. 2 & 3, Kirby's Adventure, Mega Man 3-6, etc.
pub struct Mapper004 {
    bank_select: u8,
    regs: [u8; 8],
    prg_banks8: usize,
    chr_banks1: usize,
    mirroring: Mirroring,
    four_screen: bool,

    irq_latch: u8,
    irq_counter: u8,
    irq_reload: bool,
    irq_enabled: bool,
    irq_flag: bool,
}

impl Mapper004 {
    pub fn new(prg_banks: u8, chr_banks: u8, mirroring: Mirroring) -> Self {
        Self {
            bank_select: 0,
            regs: [0, 2, 4, 5, 6, 7, 0, 1],
            prg_banks8: (prg_banks as usize * 2).max(2),
            // CHR RAM carts (0 banks) still get 8 KB of 1 KB banks
            chr_banks1: (chr_banks as usize * 8).max(8),
            mirroring,
            four_screen: mirroring == Mirroring::FourScreen,
            irq_latch: 0,
            irq_counter: 0,
            irq_reload: false,
            irq_enabled: false,
            irq_flag: false,
        }
    }

    fn prg_bank_for(&self, addr: u16) -> usize {
        let last = self.prg_banks8 - 1;
        let second_last = self.prg_banks8 - 2;
        let r6 = self.regs[6] as usize & 0x3F;
        let r7 = self.regs[7] as usize & 0x3F;
        let swap = self.bank_select & 0x40 != 0;
        let bank = match (addr >> 13) & 3 {
            0 => if swap { second_last } else { r6 },
            1 => r7,
            2 => if swap { r6 } else { second_last },
            _ => last,
        };
        bank % self.prg_banks8
    }
}

impl Mapper for Mapper004 {
    fn cpu_map_read(&self, addr: u16) -> MappedAddr {
        match addr {
            0x6000..=0x7FFF => MappedAddr::PrgRam((addr & 0x1FFF) as usize),
            0x8000..=0xFFFF => MappedAddr::PrgRom(self.prg_bank_for(addr) * 0x2000 + (addr & 0x1FFF) as usize),
            _ => MappedAddr::None,
        }
    }

    fn cpu_map_write(&mut self, addr: u16, val: u8) -> MappedAddr {
        let even = addr & 1 == 0;
        match addr {
            0x6000..=0x7FFF => return MappedAddr::PrgRam((addr & 0x1FFF) as usize),
            0x8000..=0x9FFF if even => self.bank_select = val,
            0x8000..=0x9FFF => self.regs[(self.bank_select & 7) as usize] = val,
            0xA000..=0xBFFF if even => {
                if !self.four_screen {
                    self.mirroring = if val & 1 == 0 { Mirroring::Vertical } else { Mirroring::Horizontal };
                }
            }
            0xA000..=0xBFFF => {} // PRG RAM protect — RAM is always enabled here
            0xC000..=0xDFFF if even => self.irq_latch = val,
            0xC000..=0xDFFF => { self.irq_counter = 0; self.irq_reload = true; }
            0xE000..=0xFFFF if even => { self.irq_enabled = false; self.irq_flag = false; }
            0xE000..=0xFFFF => self.irq_enabled = true,
            _ => {}
        }
        MappedAddr::None
    }

    fn ppu_map_read(&self, addr: u16) -> usize {
        let addr = addr & 0x1FFF;
        // CHR A12 inversion swaps the 2 KB and 1 KB halves
        let a = if self.bank_select & 0x80 != 0 { addr ^ 0x1000 } else { addr };
        let bank = match a >> 10 {
            0 => self.regs[0] as usize & 0xFE,
            1 => self.regs[0] as usize | 1,
            2 => self.regs[1] as usize & 0xFE,
            3 => self.regs[1] as usize | 1,
            4 => self.regs[2] as usize,
            5 => self.regs[3] as usize,
            6 => self.regs[4] as usize,
            _ => self.regs[5] as usize,
        };
        (bank % self.chr_banks1) * 0x400 + (addr & 0x3FF) as usize
    }

    fn ppu_map_write(&mut self, addr: u16) -> usize {
        self.ppu_map_read(addr)
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    fn scanline(&mut self) {
        if self.irq_counter == 0 || self.irq_reload {
            self.irq_counter = self.irq_latch;
            self.irq_reload = false;
        } else {
            self.irq_counter -= 1;
        }
        if self.irq_counter == 0 && self.irq_enabled {
            self.irq_flag = true;
        }
    }

    fn irq_active(&self) -> bool {
        self.irq_flag
    }

    fn irq_clear(&mut self) {
        self.irq_flag = false;
    }
}
