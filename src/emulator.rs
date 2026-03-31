use crate::{
    bus::Bus,
    cpu::Cpu,
    cartridge::Cartridge,
};

pub struct Emulator {
    pub cpu: Cpu,
    pub bus: Bus,
    total_cycles: u64,
    rom_path: String,
}

impl Emulator {
    pub fn new(rom_data: &[u8], audio_sample_rate: u32, rom_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut cartridge = Cartridge::from_ines(rom_data)?;

        // Load battery-backed save RAM if it exists
        if cartridge.has_battery {
            let sav_path = sav_path_for(rom_path);
            match std::fs::read(&sav_path) {
                Ok(data) => {
                    let len = data.len().min(cartridge.prg_ram.len());
                    cartridge.prg_ram[..len].copy_from_slice(&data[..len]);
                    log::info!("Loaded battery save from {}", sav_path);
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => log::warn!("Could not load battery save {}: {}", sav_path, e),
            }
        }

        let mut bus = Bus::new(cartridge, audio_sample_rate);
        let mut cpu = Cpu::new();
        cpu.reset(&mut bus);
        Ok(Self { cpu, bus, total_cycles: 0, rom_path: rom_path.to_string() })
    }

    /// Write battery-backed PRG RAM to a .sav file next to the ROM.
    pub fn save_battery(&self) {
        if !self.bus.cartridge.has_battery {
            return;
        }
        let sav_path = sav_path_for(&self.rom_path);
        match std::fs::write(&sav_path, &self.bus.cartridge.prg_ram) {
            Ok(_) => log::info!("Saved battery RAM to {}", sav_path),
            Err(e) => eprintln!("Failed to save battery RAM to {}: {}", sav_path, e),
        }
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
    }
}

fn sav_path_for(rom_path: &str) -> String {
    // Replace .nes extension with .sav, or append .sav if no extension
    if let Some(stem) = rom_path.strip_suffix(".nes") {
        format!("{}.sav", stem)
    } else {
        format!("{}.sav", rom_path)
    }
}
