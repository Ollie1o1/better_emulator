use crate::{
    apu::Apu,
    cartridge::Cartridge,
    controller::Controller,
    ppu::Ppu,
};

/// The CPU's view of the system. Every CPU memory access goes through
/// `cpu_read`/`cpu_write`, and each one first advances the rest of the
/// console by one CPU cycle (3 PPU dots + 1 APU cycle). That keeps register
/// reads like $2002 in step with the PPU mid-instruction, instead of catching
/// the PPU up only after a whole instruction has run.
pub struct Bus {
    pub ram: [u8; 2048],
    pub ppu: Ppu,
    pub apu: Apu,
    pub cartridge: Cartridge,
    pub controller1: Controller,
    pub controller2: Controller,

    /// CPU cycles elapsed since power-on.
    pub cycles: u64,
    /// Set when the PPU finishes a frame; cleared by the emulator.
    pub frame_complete: bool,
    /// NMI edge seen from the PPU, waiting for the CPU to service it.
    pub nmi_pending: bool,
    /// Interrupt lines as they stood before the most recent cycle. The 6502
    /// polls interrupts before an instruction's last cycle, so an NMI/IRQ
    /// that arrives during that final cycle waits one more instruction.
    pub nmi_polled: bool,
    pub irq_polled: bool,
    /// Last value driven onto the CPU data bus (returned for unmapped reads).
    open_bus: u8,

    /// $4014 was written: OAM DMA runs before the next instruction.
    pub dma_page: Option<u8>,
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
            cycles: 0,
            frame_complete: false,
            nmi_pending: false,
            nmi_polled: false,
            irq_polled: false,
            open_bus: 0,
            dma_page: None,
        }
    }

    /// Advance everything except the CPU by one CPU cycle.
    pub fn tick(&mut self) {
        self.tick_components();

        // DMC memory reader: fetching a sample byte stalls the CPU ~4 cycles.
        if let Some(addr) = self.apu.dmc.fetch_address() {
            let byte = self.cartridge.cpu_read(addr);
            self.apu.dmc.provide_sample(byte);
            for _ in 0..3 { self.tick_components(); }
        }
    }

    fn tick_components(&mut self) {
        self.nmi_polled = self.nmi_pending;
        self.irq_polled = self.irq_line();
        self.cycles += 1;
        self.cartridge.cpu_cycle = self.cycles;
        for _ in 0..3 {
            if self.ppu.tick(&mut self.cartridge) {
                self.frame_complete = true;
            }
            if self.ppu.nmi_occurred {
                self.ppu.nmi_occurred = false;
                self.nmi_pending = true;
            }
        }
        self.apu.tick();
    }

    /// Level of the shared /IRQ line (APU frame counter, DMC, mapper).
    pub fn irq_line(&self) -> bool {
        self.apu.irq() || self.cartridge.irq_active()
    }

    /// CPU read: one bus cycle.
    pub fn cpu_read(&mut self, addr: u16) -> u8 {
        self.tick();
        let val = self.read_no_tick(addr);
        self.open_bus = val;
        val
    }

    /// CPU write: one bus cycle.
    pub fn cpu_write(&mut self, addr: u16, val: u8) {
        self.tick();
        self.open_bus = val;
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize] = val,
            0x2000..=0x3FFF => self.ppu.cpu_write(addr & 0x0007, val, &mut self.cartridge),
            0x4014 => self.dma_page = Some(val),
            0x4016 => {
                self.controller1.write(val);
                self.controller2.write(val);
            }
            0x4000..=0x4017 => self.apu.cpu_write(addr, val),
            0x4020..=0xFFFF => self.cartridge.cpu_write(addr, val),
            _ => {}
        }
    }

    fn read_no_tick(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize],
            0x2000..=0x3FFF => self.ppu.cpu_read(addr & 0x0007, &mut self.cartridge),
            // $4015 bit 5 isn't driven — it keeps the open-bus value
            0x4015 => self.apu.cpu_read(addr) | (self.open_bus & 0x20),
            0x4016 => (self.controller1.read() & 0x1F) | (self.open_bus & 0xE0),
            0x4017 => (self.controller2.read() & 0x1F) | (self.open_bus & 0xE0),
            0x4020..=0xFFFF => self.cartridge.cpu_read_mapped(addr).unwrap_or(self.open_bus),
            _ => self.open_bus,
        }
    }

    /// Side-effect-free read of RAM or cartridge space, for tests and tooling.
    /// Returns 0 for I/O registers.
    pub fn peek(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize],
            0x4020..=0xFFFF => self.cartridge.cpu_read(addr),
            _ => 0,
        }
    }

    /// OAM DMA ($4014): 1 wait cycle (+1 if starting on an odd cycle), then
    /// 256 read/write pairs copying CPU page `page` into sprite memory.
    pub fn run_oam_dma(&mut self, page: u8) {
        self.tick();
        if self.cycles % 2 == 1 {
            self.tick();
        }
        for i in 0..=255u8 {
            let v = self.cpu_read((page as u16) << 8 | i as u16);
            self.tick();
            self.ppu.oam_dma_write(v);
        }
    }
}
