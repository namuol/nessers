use std::f32::consts::PI;

// https://www.nesdev.org/wiki/Cycle_reference_chart
//
// PPU clock speed = 21.477272 MHz ÷ 4
//
// This is roughly 3x the CPU clock speed.
const NTSC_PPU_CLOCK_FREQ: f32 = (21.477272 / 4.0) * 1_000_000.0;
const NTSC_CPU_CLOCK_FREQ: f32 = (21.477272 / 12.0) * 1_000_000.0;

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
  pub pulse_1_enable: bool,
  pub pulse_1_sample: f32,
  pub pulse_1_sequencer: Sequencer,
  pub pulse_1_osc: PulseOscillator,
  time_until_next_sample: f32,
  sample_clock: f32,
  clock_counter: u32,
  frame_clock_counter: u32,
  pub global_clock: f64,
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
      pulse_1_sequencer: Sequencer {
        // Not sure why this is 32 bits; seems we only care about the lower 8
        // bits:
        sequence: 0b0000_0000_0000_0000_0000_0000__0000_0000,
        timer: 0x0000,
        reload: 0x0000,
        output: 0x00,
      },
      pulse_1_osc: PulseOscillator::new(),

      sample_ready: false,

      time_until_next_sample: TIME_PER_SAMPLE,
      sample_clock: 0.0,

      clock_counter: 0,
      frame_clock_counter: 0,

      global_clock: 0.0,
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
    // match addr {
    //   0x4000 => {
    //     Some(())
    //   }
    //   _ => None,
    // }
    if (addr >= 0x4000 && addr <= 0x4013) || addr == 0x4015 || addr == 0x4017 {
      // Technically as-is this will break controller input since 4017 conflicts
      // with controller's 4017... that's okay because this should just be
      // temporary.
      match addr {
        0x4000 => match ((data & 0xC0) >> 6) {
          0x00 => {
            self.pulse_1_sequencer.sequence = 0b0000_0001;
            self.pulse_1_osc.duty_cycle = 1.0 / 8.0;
          }
          0x01 => {
            self.pulse_1_sequencer.sequence = 0b0000_0011;
            self.pulse_1_osc.duty_cycle = 2.0 / 8.0;
          }
          0x02 => {
            self.pulse_1_sequencer.sequence = 0b0000_1111;
            self.pulse_1_osc.duty_cycle = 4.0 / 8.0;
          }
          0x03 => {
            self.pulse_1_sequencer.sequence = 0b1111_1100;
            self.pulse_1_osc.duty_cycle = 6.0 / 8.0;
          }
          _ => {}
        },
        0x4002 => {
          self.pulse_1_sequencer.reload = (self.pulse_1_sequencer.reload & 0xFF00) | (data as u16);
        }
        0x4003 => {
          self.pulse_1_sequencer.reload =
            (((data as u16) & 0x07) << 8) | (self.pulse_1_sequencer.reload & 0x00FF);
          self.pulse_1_sequencer.timer = self.pulse_1_sequencer.reload;
        }
        0x4015 => {
          self.pulse_1_enable = (data & 0x01) == 1;
        }
        _ => {}
      }

      return Some(());
    }
    None
  }

  pub fn clock(&mut self) {
    // Sampling timing stuff:
    {
      self.time_until_next_sample -= TIME_PER_PPU_CLOCK;
      if self.time_until_next_sample < 0.0 {
        // Simple sin wave for now:
        self.sample_clock = (self.sample_clock + 1.0) % SYSTEM_SAMPLE_RATE;
        // self.pulse_1_sample =
        //   (self.sample_clock * 440.0 * 2.0 * std::f32::consts::PI / SYSTEM_SAMPLE_RATE).sin() * 0.1;
        self.sample_ready = true;
        self.time_until_next_sample += TIME_PER_SAMPLE;
      }
    }

    // https://www.nesdev.org/wiki/APU_Frame_Counter
    // self.global_clock += TIME_PER_PPU_CLOCK;
    self.global_clock += (0.33333333333 / NTSC_CPU_CLOCK_FREQ) as f64;
    if self.global_clock == 4.0 {
      self.global_clock = 0.0;
    }

    let mut quarter_frame: bool = false;
    let mut half_frame: bool = false;

    // The APU clock runs at half the rate of the CPU i.e. 1/6th the rate of the
    // PPU, so anything that works on the state of the APU happens in a clock
    // that is in total 1/6th the clock() rate which is 1x PPU rate:
    if self.clock_counter % 6 == 0 {
      self.frame_clock_counter = self.frame_clock_counter.wrapping_add(1);

      // Nasty raw 1-bit sound:
      //
      // self.pulse_1_sequencer.clock(
      //   self.pulse_1_enable,
      //   // Shift right by 1 bit, wrapping around.
      //   //
      //   // ```
      //   // 0b0000_0010_... -> 0b0000_0001_...
      //   // 0b0000_..._0001 -> 0b1000_..._0000
      //   // ```
      //   |s| ((s & 0x0000_0001) << 7) | ((s & 0x0000_00FE) >> 1),
      // );
      // self.pulse_1_sample = if self.pulse_1_sequencer.output == 0 {
      //   0.0
      // } else {
      //   1.0
      // };

      if self.pulse_1_enable {
        // Nicer simulated oscillator as a sum of sin-waves:
        // Calculate frequency from `reload`:
        self.pulse_1_osc.frequency =
          NTSC_CPU_CLOCK_FREQ / (16.0 * (self.pulse_1_sequencer.reload + 1) as f32);

        self.pulse_1_sample = self.pulse_1_osc.sample(self.global_clock as f32);
      }
    }

    self.clock_counter = self.clock_counter.wrapping_add(1);
  }

  pub fn reset(&mut self) {}
}

pub struct Sequencer {
  sequence: u32,
  timer: u16,
  reload: u16,
  output: u8,
}

impl Sequencer {
  pub fn clock(&mut self, enable: bool, manipulate_sequence: fn(u32) -> u32) -> u8 {
    if enable {
      self.timer = self.timer.wrapping_sub(1);
      if self.timer == 0xFFFF {
        self.timer = self.reload.wrapping_add(1);
        self.sequence = manipulate_sequence(self.sequence);
        // The output of our sequencer during this clock is just the lowest bit
        // of our sequence after the sequence has been manipulated.
        self.output = (self.sequence as u8) & 0b0000_0001;
      }
    }

    return self.output;
  }
}

pub struct PulseOscillator {
  pub frequency: f32,
  pub duty_cycle: f32,
  pub amplitude: f32,
  pub harmonics: u8,
}

impl PulseOscillator {
  pub fn new() -> Self {
    PulseOscillator {
      frequency: 0.0,
      duty_cycle: 0.0,
      amplitude: 1.0,
      harmonics: 30,
    }
  }

  // Why the heck does this need to be `&mut self`?
  pub fn sample(&mut self, t: f32) -> f32 {
    let mut a = 0.0;
    let mut b = 0.0;
    let p = self.duty_cycle * 2.0 * PI;
    for n in 1..self.harmonics {
      let n = n as f32;
      let c = n * self.frequency * 2.0 * PI * t;
      a += -(c).qsin() / n;
      b += -(c - p * n).qsin() / n;
    }

    return (2.0 * self.amplitude / PI) * (a - b);
  }
}

trait QuickSin {
  fn qsin(self) -> Self;
}

impl QuickSin for f32 {
  /// Cheap implementation of sin; approximation appropriate for audio
  /// synthesis.
  fn qsin(self) -> f32 {
    let mut j = self * 0.15915;
    j = j - (j.floor());
    20.785 * j * (j - 0.5) * (j - 1.0)
  }
}
