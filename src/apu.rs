/// The audio processing unit.
///
/// (Not to be confused with the man behind the Kwik-E-Mart counter)
pub struct Apu {
  pulse_1_enable: bool,
  pulse_1_sample: f32,
  // pulse_2_sample: f32,
  // triangle_sample: f32,
  // noise_sample: f32,
  // dmc_sample: f32,
}

impl Apu {
  pub fn new() -> Self {
    Apu {
      pulse_1_enable: false,
      pulse_1_sample: 0.0,
    }
  }

  pub fn sample(self) -> f32 {
    // Simple for now:
    self.pulse_1_sample
  }

  pub fn cpu_write(&mut self, addr: u16, data: u8) -> Option<()> {
    if (addr >= 0x4000 && addr <= 0x4013) || addr == 0x4015 || addr == 0x4017 {
      // Technically as-is this will break controller input since 4017 conflicts
      // with controller's 4017... that's okay because this should just be
      // temporary.
      return Some(());
    }
    None
  }

  pub fn clock(&mut self) {}
}
