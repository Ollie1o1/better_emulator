use crate::cartridge::{Cartridge, Mirroring};

pub const SCREEN_WIDTH: usize = 256;
pub const SCREEN_HEIGHT: usize = 240;

// NES master palette: 64 colors → RGB
static PALETTE: [(u8, u8, u8); 64] = [
    (84,84,84),(0,30,116),(8,16,144),(48,0,136),(68,0,100),(92,0,48),(84,4,0),(60,24,0),
    (32,42,0),(8,58,0),(0,64,0),(0,60,0),(0,50,60),(0,0,0),(0,0,0),(0,0,0),
    (152,150,152),(8,76,196),(48,50,236),(92,30,228),(136,20,176),(160,20,100),(152,34,32),(120,60,0),
    (84,90,0),(40,114,0),(8,124,0),(0,118,40),(0,102,120),(0,0,0),(0,0,0),(0,0,0),
    (236,238,236),(76,154,236),(120,124,236),(176,98,236),(228,84,236),(236,88,180),(236,106,100),(212,136,32),
    (160,170,0),(116,196,0),(76,208,32),(56,204,108),(56,180,204),(60,60,60),(0,0,0),(0,0,0),
    (236,238,236),(168,204,236),(188,188,236),(212,178,236),(236,174,236),(236,174,212),(236,180,176),(228,196,144),
    (204,210,120),(180,222,120),(168,226,144),(152,226,180),(160,214,228),(160,162,160),(0,0,0),(0,0,0),
];

pub struct Ppu {
    // Registers exposed to CPU
    pub ctrl:     u8,  // $2000
    pub mask:     u8,  // $2001
    pub status:   u8,  // $2002
    pub oam_addr: u8,  // $2003

    // Loopy registers
    pub v: u16,   // current VRAM address
    pub t: u16,   // temporary VRAM address
    pub x: u8,    // fine X scroll (3 bits)
    pub w: bool,  // write toggle

    // Data buffer for PPUDATA reads
    data_buf: u8,

    // VRAM
    pub name_table: [[u8; 0x400]; 2],
    pub palette_ram: [u8; 32],

    // OAM
    pub oam: [u8; 256],
    oam2: [u8; 32],

    // Background shift registers
    bg_pattern_lo: u16,
    bg_pattern_hi: u16,
    bg_attrib_lo: u16,
    bg_attrib_hi: u16,
    bg_at_latch_lo: bool,
    bg_at_latch_hi: bool,

    // Next-cycle fetched tile data
    next_nt: u8,
    next_at: u8,
    next_lo: u8,
    next_hi: u8,

    // Sprite data for current scanline
    sprite_count: u8,
    sprite_patterns_lo: [u8; 8],
    sprite_patterns_hi: [u8; 8],
    sprite_attribs: [u8; 8],
    sprite_x: [u8; 8],
    sprite_zero_hit_possible: bool,
    sprite_zero_being_rendered: bool,

    // Timing
    pub scanline: i16,
    pub dot: u16,
    pub frame: u64,

    // NMI signals
    pub nmi_occurred: bool,
    nmi_output: bool,
    nmi_prev: bool,

    // Frame buffer (ARGB8888)
    pub frame_buffer: Box<[u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4]>,
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            ctrl: 0, mask: 0, status: 0, oam_addr: 0,
            v: 0, t: 0, x: 0, w: false,
            data_buf: 0,
            name_table: [[0u8; 0x400]; 2],
            palette_ram: [0u8; 32],
            oam: [0u8; 256],
            oam2: [0u8; 32],
            bg_pattern_lo: 0, bg_pattern_hi: 0,
            bg_attrib_lo: 0, bg_attrib_hi: 0,
            bg_at_latch_lo: false, bg_at_latch_hi: false,
            next_nt: 0, next_at: 0, next_lo: 0, next_hi: 0,
            sprite_count: 0,
            sprite_patterns_lo: [0; 8], sprite_patterns_hi: [0; 8],
            sprite_attribs: [0; 8], sprite_x: [0; 8],
            sprite_zero_hit_possible: false, sprite_zero_being_rendered: false,
            scanline: 0, dot: 0, frame: 0,
            nmi_occurred: false, nmi_output: false, nmi_prev: false,
            frame_buffer: Box::new([0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4]),
        }
    }

    pub fn reset(&mut self) {
        self.ctrl = 0;
        self.mask = 0;
        self.v = 0;
        self.t = 0;
        self.x = 0;
        self.w = false;
        self.scanline = 0;
        self.dot = 0;
        self.frame = 0;
    }

    // ---- CPU register access ----

    pub fn cpu_read(&mut self, reg: u16, cart: &mut Cartridge) -> u8 {
        match reg {
            0x0002 => {
                let val = (self.status & 0xE0) | (self.data_buf & 0x1F);
                self.status &= !0x80; // clear vblank
                self.w = false;
                self.update_nmi();
                val
            }
            0x0004 => self.oam[self.oam_addr as usize],
            0x0007 => {
                let mut val = self.data_buf;
                self.data_buf = self.ppu_read(self.v, cart);
                if self.v >= 0x3F00 {
                    val = self.data_buf;
                }
                self.increment_v();
                val
            }
            _ => 0,
        }
    }

    pub fn cpu_write(&mut self, reg: u16, val: u8, cart: &mut Cartridge) {
        match reg {
            0x0000 => {
                self.ctrl = val;
                self.nmi_output = val & 0x80 != 0;
                // t: ...GH.. ........ = d: ......GH
                self.t = (self.t & 0xF3FF) | (((val as u16) & 0x03) << 10);
                self.update_nmi();
            }
            0x0001 => { self.mask = val; }
            0x0003 => { self.oam_addr = val; }
            0x0004 => {
                self.oam[self.oam_addr as usize] = val;
                self.oam_addr = self.oam_addr.wrapping_add(1);
            }
            0x0005 => {
                if !self.w {
                    // First write: fine X and coarse X of t
                    self.x = val & 0x07;
                    self.t = (self.t & 0xFFE0) | ((val as u16) >> 3);
                } else {
                    // Second write: fine Y and coarse Y of t
                    self.t = (self.t & 0x8C1F)
                           | (((val as u16) & 0x07) << 12)
                           | (((val as u16) & 0xF8) << 2);
                }
                self.w = !self.w;
            }
            0x0006 => {
                if !self.w {
                    // High byte: t = .FEDCBA ........
                    self.t = (self.t & 0x80FF) | (((val as u16) & 0x3F) << 8);
                } else {
                    self.t = (self.t & 0xFF00) | val as u16;
                    self.v = self.t;
                }
                self.w = !self.w;
            }
            0x0007 => {
                self.ppu_write(self.v, val, cart);
                self.increment_v();
            }
            _ => {}
        }
    }

    fn increment_v(&mut self) {
        if self.ctrl & 0x04 != 0 {
            self.v = self.v.wrapping_add(32);
        } else {
            self.v = self.v.wrapping_add(1);
        }
    }

    fn update_nmi(&mut self) {
        let nmi = self.nmi_output && (self.status & 0x80 != 0);
        if nmi && !self.nmi_prev {
            self.nmi_occurred = true;
        }
        self.nmi_prev = nmi;
    }

    // ---- PPU internal memory access ----

    fn ppu_read(&self, addr: u16, cart: &Cartridge) -> u8 {
        let addr = addr & 0x3FFF;
        match addr {
            0x0000..=0x1FFF => cart.ppu_read(addr),
            0x2000..=0x3EFF => {
                let idx = self.mirror_nt_addr(addr, cart.mirroring());
                self.name_table[idx >> 10][idx & 0x3FF]
            }
            0x3F00..=0x3FFF => {
                let idx = (addr & 0x1F) as usize;
                // Mirrors: 0x10,0x14,0x18,0x1C are mirrors of 0x00,0x04,0x08,0x0C
                let idx = if idx & 0x13 == 0x10 { idx & !0x10 } else { idx };
                self.palette_ram[idx]
            }
            _ => 0,
        }
    }

    fn ppu_write(&mut self, addr: u16, val: u8, cart: &mut Cartridge) {
        let addr = addr & 0x3FFF;
        match addr {
            0x0000..=0x1FFF => cart.ppu_write(addr, val),
            0x2000..=0x3EFF => {
                let idx = self.mirror_nt_addr(addr, cart.mirroring());
                self.name_table[idx >> 10][idx & 0x3FF] = val;
            }
            0x3F00..=0x3FFF => {
                let idx = (addr & 0x1F) as usize;
                let idx = if idx & 0x13 == 0x10 { idx & !0x10 } else { idx };
                self.palette_ram[idx] = val;
            }
            _ => {}
        }
    }

    fn mirror_nt_addr(&self, addr: u16, mirroring: Mirroring) -> usize {
        let addr = (addr & 0x0FFF) as usize;
        match mirroring {
            Mirroring::Horizontal   => ((addr >> 1) & 0x400) | (addr & 0x3FF),
            Mirroring::Vertical     => addr & 0x7FF,
            Mirroring::SingleScreenLow  => addr & 0x3FF,
            Mirroring::SingleScreenHigh => (addr & 0x3FF) | 0x400,
            Mirroring::FourScreen   => addr & 0xFFF,
        }
    }

    // ---- Tick ----

    /// Advance PPU by 1 dot. Returns true when a frame is complete.
    pub fn tick(&mut self, cart: &mut Cartridge) -> bool {
        let mut frame_done = false;

        match self.scanline {
            -1 | 261 => self.tick_prerender(cart),
            0..=239  => self.tick_visible(cart),
            240      => {} // post-render idle
            241      => {
                if self.dot == 1 {
                    self.status |= 0x80; // set vblank
                    self.update_nmi();
                }
            }
            _ => {}
        }

        // Advance dot/scanline
        self.dot += 1;
        if self.dot > 340 {
            self.dot = 0;
            self.scanline += 1;
            if self.scanline > 260 {
                self.scanline = -1;
                self.frame += 1;
                frame_done = true;
                // Odd-frame skip on pre-render dot 0 when rendering enabled
            }
        }

        frame_done
    }

    fn rendering_enabled(&self) -> bool {
        self.mask & 0x18 != 0
    }

    fn tick_prerender(&mut self, cart: &mut Cartridge) {
        if self.dot == 1 {
            self.status &= !0xE0; // clear vblank, sprite-zero-hit, overflow
            self.update_nmi();    // falling edge: reset nmi_prev so next vblank re-triggers NMI
        }
        if self.rendering_enabled() {
            if self.dot >= 1 && self.dot <= 256 || self.dot >= 321 && self.dot <= 336 {
                self.fetch_bg_tile(cart);
            }
            if self.dot >= 1 && self.dot <= 336 {
                self.shift_bg();
            }

            // Copy horizontal bits from t to v on dot 257
            if self.dot == 257 {
                self.copy_horiz();
            }
            // Copy vertical bits from t to v on dots 280-304
            if self.dot >= 280 && self.dot <= 304 {
                self.copy_vert();
            }
        }
        // Odd frame skip
        if self.dot == 339 && self.frame & 1 == 1 && self.rendering_enabled() {
            self.dot = 340; // will be incremented to 0 next cycle effectively skipping
        }
    }

    fn tick_visible(&mut self, cart: &mut Cartridge) {
        if self.dot == 0 {
            // Idle
            return;
        }
        if self.rendering_enabled() {
            if self.dot <= 256 {
                self.fetch_bg_tile(cart);
                self.render_pixel(cart);
                self.shift_bg();
            } else if self.dot == 257 {
                self.copy_horiz();
                self.load_sprites(cart);
            } else if self.dot >= 321 && self.dot <= 336 {
                self.fetch_bg_tile(cart);
                self.shift_bg();
            }
        }
    }

    fn fetch_bg_tile(&mut self, cart: &mut Cartridge) {
        match self.dot % 8 {
            1 => {
                // Reload shift registers from latches
                self.bg_pattern_lo = (self.bg_pattern_lo & 0xFF00) | self.next_lo as u16;
                self.bg_pattern_hi = (self.bg_pattern_hi & 0xFF00) | self.next_hi as u16;
                self.bg_at_latch_lo = self.next_at & 1 != 0;
                self.bg_at_latch_hi = self.next_at & 2 != 0;
                // Fetch nametable byte
                self.next_nt = self.ppu_read(0x2000 | (self.v & 0x0FFF), cart);
            }
            3 => {
                // Fetch attribute byte
                let v = self.v;
                let addr = 0x23C0
                    | (v & 0x0C00)
                    | ((v >> 4) & 0x38)
                    | ((v >> 2) & 0x07);
                let shift = ((v >> 4) & 0x04) | (v & 0x02);
                self.next_at = (self.ppu_read(addr, cart) >> shift) & 0x03;
            }
            5 => {
                // Fetch low pattern byte
                let base = if self.ctrl & 0x10 != 0 { 0x1000u16 } else { 0 };
                let fine_y = (self.v >> 12) & 0x07;
                let addr = base | ((self.next_nt as u16) << 4) | fine_y;
                self.next_lo = self.ppu_read(addr, cart);
            }
            7 => {
                // Fetch high pattern byte
                let base = if self.ctrl & 0x10 != 0 { 0x1000u16 } else { 0 };
                let fine_y = (self.v >> 12) & 0x07;
                let addr = base | ((self.next_nt as u16) << 4) | fine_y | 8;
                self.next_hi = self.ppu_read(addr, cart);
            }
            0 => {
                // Increment horizontal VRAM component
                self.incr_coarse_x();
            }
            _ => {}
        }
        // Increment fine Y at dot 256
        if self.dot == 256 {
            self.incr_fine_y();
        }
    }

    fn shift_bg(&mut self) {
        self.bg_pattern_lo <<= 1;
        self.bg_pattern_hi <<= 1;
        self.bg_attrib_lo <<= 1;
        self.bg_attrib_hi <<= 1;
        if self.bg_at_latch_lo { self.bg_attrib_lo |= 1; }
        if self.bg_at_latch_hi { self.bg_attrib_hi |= 1; }
    }

    fn render_pixel(&mut self, _cart: &Cartridge) {
        if self.dot == 0 || self.dot > 256 {
            return;
        }
        let x = (self.dot - 1) as usize;
        let y = self.scanline as usize;

        let bit = 0x8000 >> self.x;

        // Background pixel
        let mut bg_pixel = 0u8;
        let mut bg_palette = 0u8;
        if self.mask & 0x08 != 0 && (self.mask & 0x02 != 0 || x >= 8) {
            bg_pixel = (((self.bg_pattern_hi & bit) != 0) as u8) << 1
                     | (((self.bg_pattern_lo & bit) != 0) as u8);
            bg_palette = (((self.bg_attrib_hi & bit) != 0) as u8) << 1
                       | (((self.bg_attrib_lo & bit) != 0) as u8);
        }

        // Sprite pixel
        let mut sp_pixel = 0u8;
        let mut sp_palette = 0u8;
        let mut sp_priority = false;
        let mut sprite_zero_rendered = false;

        if self.mask & 0x10 != 0 && (self.mask & 0x04 != 0 || x >= 8) {
            for i in 0..self.sprite_count as usize {
                let sx = self.sprite_x[i] as i32;
                let dx = x as i32 - sx;
                if dx < 0 || dx > 7 {
                    continue;
                }
                let bit_pos = 7 - dx as u8;
                let lo = (self.sprite_patterns_lo[i] >> bit_pos) & 1;
                let hi = (self.sprite_patterns_hi[i] >> bit_pos) & 1;
                let pixel = (hi << 1) | lo;
                if pixel == 0 {
                    continue;
                }
                sp_pixel = pixel;
                sp_palette = (self.sprite_attribs[i] & 0x03) + 4;
                sp_priority = self.sprite_attribs[i] & 0x20 == 0;
                if i == 0 {
                    sprite_zero_rendered = true;
                }
                break;
            }
        }

        // Sprite-zero hit
        if sprite_zero_rendered && self.sprite_zero_being_rendered
            && bg_pixel != 0 && sp_pixel != 0 && x != 255
        {
            self.status |= 0x40;
        }

        // Compose final pixel
        let (palette, pixel) = if bg_pixel == 0 && sp_pixel == 0 {
            (0u8, 0u8)
        } else if bg_pixel == 0 {
            (sp_palette, sp_pixel)
        } else if sp_pixel == 0 {
            (bg_palette, bg_pixel)
        } else if sp_priority {
            (sp_palette, sp_pixel)
        } else {
            (bg_palette, bg_pixel)
        };

        let color_idx = self.palette_ram[((palette << 2) | pixel) as usize & 0x1F] as usize & 0x3F;
        let (r, g, b) = PALETTE[color_idx];

        let off = (y * SCREEN_WIDTH + x) * 4;
        self.frame_buffer[off]     = b;
        self.frame_buffer[off + 1] = g;
        self.frame_buffer[off + 2] = r;
        self.frame_buffer[off + 3] = 0xFF;
    }

    fn load_sprites(&mut self, cart: &mut Cartridge) {
        // Simple sprite evaluation: scan OAM for sprites on the next scanline
        let next_y = self.scanline + 1;
        self.sprite_count = 0;
        self.sprite_zero_hit_possible = false;
        self.sprite_zero_being_rendered = false;

        let sprite_height: i16 = if self.ctrl & 0x20 != 0 { 16 } else { 8 };

        let mut count = 0u8;
        for i in 0..64 {
            let sy = self.oam[i * 4] as i16;
            let dy = next_y - sy;
            if dy < 0 || dy >= sprite_height {
                continue;
            }
            if count >= 8 {
                self.status |= 0x20; // sprite overflow
                break;
            }

            if i == 0 {
                self.sprite_zero_hit_possible = true;
            }

            let tile = self.oam[i * 4 + 1];
            let attrib = self.oam[i * 4 + 2];
            let sx = self.oam[i * 4 + 3];
            let flip_v = attrib & 0x80 != 0;
            let flip_h = attrib & 0x40 != 0;

            let row = if flip_v { sprite_height as u8 - 1 - dy as u8 } else { dy as u8 };

            let (tile_addr, row_final) = if sprite_height == 16 {
                // 8x16 sprite
                let bank = (tile as u16 & 1) << 12;
                let tile_num = tile as u16 & !1;
                let t = if row >= 8 { tile_num + 1 } else { tile_num };
                (bank | (t << 4), row % 8)
            } else {
                let bank = if self.ctrl & 0x08 != 0 { 0x1000u16 } else { 0 };
                (bank | ((tile as u16) << 4), row)
            };

            let mut lo = self.ppu_read(tile_addr + row_final as u16, cart);
            let mut hi = self.ppu_read(tile_addr + row_final as u16 + 8, cart);

            if flip_h {
                lo = lo.reverse_bits();
                hi = hi.reverse_bits();
            }

            let c = count as usize;
            self.sprite_patterns_lo[c] = lo;
            self.sprite_patterns_hi[c] = hi;
            self.sprite_attribs[c] = attrib;
            self.sprite_x[c] = sx;

            if i == 0 && self.sprite_zero_hit_possible {
                self.sprite_zero_being_rendered = true;
            }

            count += 1;
        }
        self.sprite_count = count;
    }

    // ---- Loopy helpers ----

    fn incr_coarse_x(&mut self) {
        if !self.rendering_enabled() { return; }
        if self.v & 0x001F == 31 {
            self.v &= !0x001F;
            self.v ^= 0x0400; // switch horizontal nametable
        } else {
            self.v += 1;
        }
    }

    fn incr_fine_y(&mut self) {
        if !self.rendering_enabled() { return; }
        if (self.v & 0x7000) != 0x7000 {
            self.v += 0x1000;
        } else {
            self.v &= !0x7000;
            let y = (self.v & 0x03E0) >> 5;
            let y = if y == 29 {
                self.v ^= 0x0800; // switch vertical nametable
                0
            } else if y == 31 {
                0
            } else {
                y + 1
            };
            self.v = (self.v & !0x03E0) | (y << 5);
        }
    }

    fn copy_horiz(&mut self) {
        if !self.rendering_enabled() { return; }
        // v: ....A.. ...BCDEF = t: ....A.. ...BCDEF
        self.v = (self.v & !0x041F) | (self.t & 0x041F);
    }

    fn copy_vert(&mut self) {
        if !self.rendering_enabled() { return; }
        // v: GHIA.BC DEF..... = t: GHIA.BC DEF.....
        self.v = (self.v & !0x7BE0) | (self.t & 0x7BE0);
    }
}

impl Default for Ppu {
    fn default() -> Self { Self::new() }
}
