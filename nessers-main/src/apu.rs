// https://www.nesdev.org/wiki/Cycle_reference_chart
//
// PPU clock speed = 21.477272 MHz ÷ 4
//
// This is roughly 3x the CPU clock speed.
const NTSC_PPU_CLOCK_FREQ: f32 = (21.477272 / 4.0) * 1_000_000.0;
// 44.1 kHz - this doesn't need to be hard-coded but I'm doing it this way for
// simplicity, for now.
const SYSTEM_SAMPLE_RATE: f32 = 44.1 * 1_000.0;

const TIME_PER_PPU_CLOCK: f32 = 1.0 / NTSC_PPU_CLOCK_FREQ;
const TIME_PER_SAMPLE: f32 = 1.0 / SYSTEM_SAMPLE_RATE;

/// The audio processing unit.
///
/// (Not to be confused with the man behind the Kwik-E-Mart counter)
pub struct Apu {
  pub sample_ready: bool,
  pulse_1_enable: bool,
  pulse_1_sample: f32,
  time_until_next_sample: f32,
  sample_clock: f32,
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
      sample_ready: false,
      time_until_next_sample: TIME_PER_SAMPLE,
      sample_clock: 0.0,
    }
  }

  pub fn sample(&mut self) -> f32 {
    if !self.sample_ready {
      panic!("No sample ready!");
    }

    self.sample_ready = false;

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

  pub fn clock(&mut self) {
    self.time_until_next_sample -= TIME_PER_PPU_CLOCK;
    if self.time_until_next_sample < 0.0 {
      // Simple sin wave for now:
      self.sample_clock = (self.sample_clock + 1.0) % SYSTEM_SAMPLE_RATE;
      self.pulse_1_sample =
        (self.sample_clock * 440.0 * 2.0 * std::f32::consts::PI / SYSTEM_SAMPLE_RATE).sin() * 0.1;
      self.sample_ready = true;
      self.time_until_next_sample += TIME_PER_SAMPLE;
    }
  }

  pub fn reset(&mut self) {}
}
