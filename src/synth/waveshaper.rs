pub struct Waveshaper {
    pub drive: f32,
    pub mix: f32,
}

impl Waveshaper {
    pub fn new() -> Self { Self { drive: 1.0, mix: 0.0 } }

    pub fn process(&self, x: f32) -> f32 {
        if self.mix < 0.001 { return x; }
        let driven = x * self.drive;
        let shaped = driven.tanh();
        x * (1.0 - self.mix) + shaped * self.mix / self.drive.max(1.0).sqrt()
    }
}
