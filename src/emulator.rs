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

    /// Battery-backed PRG RAM (the cartridge's save data), if the cart has a battery.
    /// Persisting it is the frontend's job (a `.sav` file, browser storage, ...).
    pub fn battery_ram(&self) -> Option<&[u8]> {
        self.bus.cartridge.has_battery.then(|| self.bus.cartridge.prg_ram.as_slice())
    }

    /// Restore battery-backed PRG RAM saved by an earlier session.
    pub fn load_battery_ram(&mut self, data: &[u8]) {
        let ram = &mut self.bus.cartridge.prg_ram;
        let len = data.len().min(ram.len());
        ram[..len].copy_from_slice(&data[..len]);
    }

    /// Press the console's reset button.
    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.bus);
        self.bus.apu.cpu_write(0x4015, 0); // reset silences the APU
    }

    /// Total CPU cycles executed since power-on.
    pub fn cycles(&self) -> u64 {
        self.total_cycles
    }

    /// Step the system by one CPU instruction (or one OAM DMA transfer).
    /// Returns true when a new video frame is complete.
    pub fn clock(&mut self) -> bool {
        if let Some(page) = self.bus.dma_page.take() {
            self.bus.run_oam_dma(page);
        } else {
            // Interrupts as polled before the previous instruction's last cycle.
            if self.bus.nmi_polled {
                self.bus.nmi_pending = false;
                self.bus.nmi_polled = false;
                self.cpu.nmi_pending = true;
            }
            self.cpu.irq_pending = self.bus.irq_polled;

            // Memory accesses inside step() advance the PPU/APU as they happen;
            // internal cycles with no bus access are made up afterwards.
            let start = self.bus.cycles;
            let cycles = self.cpu.step(&mut self.bus) as u64;
            let used = self.bus.cycles - start;
            for _ in used..cycles {
                self.bus.tick();
            }
        }
        self.total_cycles = self.bus.cycles;

        std::mem::take(&mut self.bus.frame_complete)
    }

    /// Run until one complete frame is produced.
    pub fn run_frame(&mut self) {
        loop {
            if self.clock() { break; }
        }
    }
}
