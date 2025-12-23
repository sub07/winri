use joy_vector::gen_vector;

gen_vector!(Position<f32, 2> with two_dim);
gen_vector!(Size<f32, 2> with two_dim);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Bounds {
    pub fn position(&self) -> Position {
        Position([self.left, self.top])
    }

    pub fn size(&self) -> Size {
        Size([self.right - self.left, self.bottom - self.top])
    }
}
