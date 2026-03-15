use crate::{
    ppu::Ppu,
    apu::Apu,
    cartridge::Cartridge,
    controller::Controller,
};

pub struct Bus {
    pub ram: [u8; 2048],
    pub ppu: Ppu,
    pub apu: Apu,
    pub cartridge: Cartridge,
    pub controller1: Controller,
    pub controller2: Controller,

    // OAM DMA
    pub dma_page: u8,
    pub dma_addr: u8,
    pub dma_data: u8,
    pub dma_active: bool,
    pub dma_sync: bool,
}

impl Bus {
    pub fn new(cartridge: Cartridge, sample_rate: u32) -> Self {
        Self {
            ram: [0u8; 2048],
            ppu: Ppu::new(),
            apu: Apu::new(sample_rate),
            cartridge,
            controller1: Controller::new(),
            controller2: Controller::new(),
            dma_page: 0,
            dma_addr: 0,
            dma_data: 0,
            dma_active: false,
            dma_sync: false,
        }
    }

    pub fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize],
            0x2000..=0x3FFF => {
                let reg = addr & 0x0007;
                self.ppu.cpu_read(reg, &mut self.cartridge)
            }
            0x4015 => self.apu.cpu_read(addr),
            0x4016 => self.controller1.read(),
            0x4017 => self.controller2.read(),
            0x4020..=0xFFFF => self.cartridge.cpu_read(addr),
            _ => 0,
        }
    }

    pub fn cpu_write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1FFF => {
                let idx = (addr & 0x07FF) as usize;
                self.ram[idx] = val;
            }
            0x2000..=0x3FFF => {
                let reg = addr & 0x0007;
                self.ppu.cpu_write(reg, val, &mut self.cartridge);
            }
            0x4000..=0x4013 => self.apu.cpu_write(addr, val),
            0x4014 => {
                self.dma_page = val;
                self.dma_addr = 0;
                self.dma_active = true;
            }
            0x4015 => self.apu.cpu_write(addr, val),
            0x4016 => {
                self.controller1.write(val);
                self.controller2.write(val);
            }
            0x4017 => self.apu.cpu_write(addr, val),
            0x4020..=0xFFFF => self.cartridge.cpu_write(addr, val),
            _ => {}
        }
    }

    #[allow(dead_code)]
    pub fn tick_components(&mut self) -> bool {
        let mut frame_done = false;
        for _ in 0..3 {
            if self.ppu.tick(&mut self.cartridge) {
                frame_done = true;
            }
            if self.ppu.nmi_occurred {
                // Consumed by emulator
            }
        }
        self.apu.tick();
        frame_done
    }
}
