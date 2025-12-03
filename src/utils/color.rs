#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn from_abgr_packed(abgr: u32) -> Self {
        Self {
            a: ((abgr >> 24) & 0xFF) as u8,
            b: ((abgr >> 16) & 0xFF) as u8,
            g: ((abgr >> 8) & 0xFF) as u8,
            r: (abgr & 0xFF) as u8,
        }
    }

    pub const fn without_alpha(self) -> Self {
        Self {
            r: self.r,
            g: self.g,
            b: self.b,
            a: 255,
        }
    }
}
