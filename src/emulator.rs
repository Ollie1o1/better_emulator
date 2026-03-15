use crate::{
    bus::Bus,
    cpu::Cpu,
    cartridge::Cartridge,
};

pub struct Emulator {
    pub cpu: Cpu,
    pub bus: Bus,
    total_cycles: u64,
}

impl Emulator {
    pub fn new(rom_data: &[u8], audio_sample_rate: u32) -> Result<Self, Box<dyn std::error::Error>> {
        let cartridge = Cartridge::from_ines(rom_data)?;
        let mut bus = Bus::new(cartridge, audio_sample_rate);
        let mut cpu = Cpu::new();
        cpu.reset(&mut bus);
        Ok(Self { cpu, bus, total_cycles: 0 })
    }

    /// Step the system by one CPU instruction.
    /// Returns true when a new video frame is complete.
    pub fn clock(&mut self) -> bool {
        let mut frame_done = false;

        // Handle OAM DMA
        if self.bus.dma_active {
            if !self.bus.dma_sync {
                if self.total_cycles % 2 == 1 {
                    self.bus.dma_sync = true;
                }
            } else {
                if self.total_cycles % 2 == 0 {
                    self.bus.dma_data = self.bus.cpu_read(
                        (self.bus.dma_page as u16) << 8 | self.bus.dma_addr as u16
                    );
                } else {
                    self.bus.ppu.oam[self.bus.dma_addr as usize] = self.bus.dma_data;
                    self.bus.dma_addr = self.bus.dma_addr.wrapping_add(1);
                    if self.bus.dma_addr == 0 {
                        self.bus.dma_active = false;
                        self.bus.dma_sync = false;
                    }
                }
            }
            // Tick PPU 3x even during DMA
            for _ in 0..3 {
                if self.bus.ppu.tick(&mut self.bus.cartridge) {
                    frame_done = true;
                }
                if self.bus.ppu.nmi_occurred {
                    self.bus.ppu.nmi_occurred = false;
                    self.cpu.nmi_pending = true;
                }
            }
            self.bus.apu.tick();
            self.total_cycles += 1;
            return frame_done;
        }

        let cpu_cycles = self.cpu.step(&mut self.bus);

        for _ in 0..cpu_cycles {
            self.total_cycles += 1;

            for _ in 0..3 {
                if self.bus.ppu.tick(&mut self.bus.cartridge) {
                    frame_done = true;
                }
                if self.bus.ppu.nmi_occurred {
                    self.bus.ppu.nmi_occurred = false;
                    self.cpu.nmi_pending = true;
                }
            }

            self.bus.apu.tick();

            if self.bus.cartridge.irq_active() {
                self.bus.cartridge.irq_clear();
                self.cpu.irq_pending = true;
            }

            if self.bus.apu.frame_irq {
                self.bus.apu.frame_irq = false;
                self.cpu.irq_pending = true;
            }
        }

        frame_done
    }

    /// Run until one complete frame is produced.
    pub fn run_frame(&mut self) {
        loop {
            if self.clock() { break; }
        }
        // Debug: print PPU state every 60 frames for first 5 seconds
    }
}
