// 1.789773 MHz
//
// TODO: Is this really 1/3 the true clock time, since the CPU doesn't actually
// tick each time we call `clock()`?
const NTSC_CPU_CLOCK_FREQ: f32 = 1.789773 * 1_000_000.0;
// 44.1 kHz
const APU_SAMPLE_FREQ: f32 = 44.1 * 1_000.0;

const TIME_PER_CPU_CLOCK: f32 = 1.0 / NTSC_CPU_CLOCK_FREQ;
const TIME_PER_SAMPLE: f32 = 1.0 / APU_SAMPLE_FREQ;

/// The audio processing unit.
///
/// (Not to be confused with the man behind the Kwik-E-Mart counter)
pub struct Apu {
  pulse_1_enable: bool,
  pulse_1_sample: f32,
  time_until_next_sample: f32,
  sample_clock: f32,
  pub sample_ready: bool,
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
    self.time_until_next_sample -= TIME_PER_CPU_CLOCK;
    if self.time_until_next_sample < 0.0 {
      // Simple sin wave for now:
      self.sample_clock = (self.sample_clock + 1.0) % APU_SAMPLE_FREQ;
      self.pulse_1_sample =
        (self.sample_clock * 440.0 * 2.0 * std::f32::consts::PI / APU_SAMPLE_FREQ).sin() * 0.1;
      self.sample_ready = true;
      self.time_until_next_sample += TIME_PER_SAMPLE;
    }
  }
}
