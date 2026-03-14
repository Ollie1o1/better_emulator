pub mod buttons {
    pub const A:      u8 = 1 << 0;
    pub const B:      u8 = 1 << 1;
    pub const SELECT: u8 = 1 << 2;
    pub const START:  u8 = 1 << 3;
    pub const UP:     u8 = 1 << 4;
    pub const DOWN:   u8 = 1 << 5;
    pub const LEFT:   u8 = 1 << 6;
    pub const RIGHT:  u8 = 1 << 7;
}

pub struct Controller {
    pub buttons: u8,
    strobe: bool,
    shift: u8,
}

impl Controller {
    pub fn new() -> Self {
        Self { buttons: 0, strobe: false, shift: 0xFF }
    }

    pub fn write(&mut self, val: u8) {
        let new_strobe = val & 1 == 1;
        // Latch on falling edge (1 → 0): freeze current button state into shift register.
        // While strobe is high the shift register is continuously refreshed; reads return A.
        if self.strobe && !new_strobe {
            self.shift = self.buttons;
        }
        self.strobe = new_strobe;
    }

    pub fn read(&mut self) -> u8 {
        if self.strobe {
            // While strobe is high: continuously return current A button state.
            return (self.buttons & buttons::A != 0) as u8;
        }
        // Serial output: shift out one bit per read.
        // Reads 1-8 → A, B, Select, Start, Up, Down, Left, Right.
        // Reads 9+ return 1 (open bus / unconnected).
        let bit = self.shift & 1;
        self.shift = (self.shift >> 1) | 0x80; // shift in 1s for reads 9+
        bit
    }
}

impl Default for Controller {
    fn default() -> Self { Self::new() }
}
