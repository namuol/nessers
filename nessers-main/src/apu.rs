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

  // Okay, um, really need to collect all of this into a reusable unit...
  pub pulse_1_enable: bool,
  pub pulse_1_sample: f32,
  pub pulse_1_sequencer: Sequencer,
  pub pulse_1_osc: PulseOscillator,
  pub pulse_1_length_counter: u8,
  pub pulse_1_length_counter_halt: bool,
  pub pulse_1_envelope: Envelope,

  pub pulse_2_enable: bool,
  pub pulse_2_sample: f32,
  pub pulse_2_sequencer: Sequencer,
  pub pulse_2_osc: PulseOscillator,
  pub pulse_2_length_counter: u8,
  pub pulse_2_length_counter_halt: bool,
  pub pulse_2_envelope: Envelope,

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
      pulse_1_length_counter: 0x00,
      pulse_1_length_counter_halt: false,
      pulse_1_envelope: Envelope::new(),

      pulse_2_enable: false,
      pulse_2_sample: 0.0,
      pulse_2_sequencer: Sequencer {
        // Not sure why this is 32 bits; seems we only care about the lower 8
        // bits:
        sequence: 0b0000_0000_0000_0000_0000_0000__0000_0000,
        timer: 0x0000,
        reload: 0x0000,
        output: 0x00,
      },
      pulse_2_osc: PulseOscillator::new(),
      pulse_2_length_counter: 0x00,
      pulse_2_length_counter_halt: false,
      pulse_2_envelope: Envelope::new(),

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
    self.pulse_1_sample + self.pulse_2_sample
  }

  pub fn cpu_write(&mut self, addr: u16, data: u8) -> Option<()> {
    if (addr >= 0x4000 && addr <= 0x4013) || addr == 0x4015 || addr == 0x4017 {
      // Technically as-is this will break controller input since 4017 conflicts
      // with controller's 4017... that's okay because this should just be
      // temporary.
      match addr {
        0x4000 => {
          // Duty Cycle
          match (data & 0b1100_0000) >> 6 {
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
          };

          // Constant Volume flag
          self.pulse_1_envelope.constant_volume_flag = (data & 0b0001_0000) != 0;
          // Constant volume level or Envelope length
          self.pulse_1_envelope.divider.reload = (data & 0b0000_1111) as u16; // Why is this u16 again?

          // Length Counter Halt
          self.pulse_1_length_counter_halt = (data & 0b0010_0000) != 0;
        }

        0x4004 => {
          // Duty Cycle
          match (data & 0b1100_0000) >> 6 {
            0x00 => {
              self.pulse_2_sequencer.sequence = 0b0000_0001;
              self.pulse_2_osc.duty_cycle = 1.0 / 8.0;
            }
            0x01 => {
              self.pulse_2_sequencer.sequence = 0b0000_0011;
              self.pulse_2_osc.duty_cycle = 2.0 / 8.0;
            }
            0x02 => {
              self.pulse_2_sequencer.sequence = 0b0000_1111;
              self.pulse_2_osc.duty_cycle = 4.0 / 8.0;
            }
            0x03 => {
              self.pulse_2_sequencer.sequence = 0b1111_1100;
              self.pulse_2_osc.duty_cycle = 6.0 / 8.0;
            }
            _ => {}
          };

          // Constant Volume flag
          self.pulse_2_envelope.constant_volume_flag = (data & 0b0001_0000) != 0;
          // Constant volume level or Envelope length
          self.pulse_2_envelope.divider.reload = (data & 0b0000_1111) as u16; // Why is this u16 again?

          // Length Counter Halt
          self.pulse_2_length_counter_halt = (data & 0b0010_0000) != 0;
        }

        0x4002 => {
          self.pulse_1_sequencer.reload = (self.pulse_1_sequencer.reload & 0xFF00) | (data as u16);
        }
        0x4006 => {
          self.pulse_2_sequencer.reload = (self.pulse_2_sequencer.reload & 0xFF00) | (data as u16);
        }

        0x4003 => {
          self.pulse_1_sequencer.reload =
            (((data as u16) & 0x07) << 8) | (self.pulse_1_sequencer.reload & 0x00FF);

          self.pulse_1_sequencer.timer = self.pulse_1_sequencer.reload;

          // Length Counter/Envelope start flag
          //
          // Basically, start playing a note.
          if self.pulse_1_enable {
            // Start Flag; should this also be triggered only when the pulse is
            // enabled? Unclear from here:
            // https://www.nesdev.org/wiki/APU_Envelope
            self.pulse_1_envelope.start_flag = true;

            self.pulse_1_length_counter = get_length_counter((data & 0b1111_1000) >> 3);
          }
        }
        0x4007 => {
          self.pulse_2_sequencer.reload =
            (((data as u16) & 0x07) << 8) | (self.pulse_2_sequencer.reload & 0x00FF);

          self.pulse_2_sequencer.timer = self.pulse_2_sequencer.reload;

          // Length Counter/Envelope start flag
          //
          // Basically, start playing a note.
          if self.pulse_2_enable {
            // Start Flag; should this also be triggered only when the pulse is
            // enabled? Unclear from here:
            // https://www.nesdev.org/wiki/APU_Envelope
            self.pulse_2_envelope.start_flag = true;

            self.pulse_2_length_counter = get_length_counter((data & 0b1111_1000) >> 3);
          }
        }

        0x4015 => {
          self.pulse_1_enable = (data & 0b0000_0001) != 0;
          if !self.pulse_1_enable {
            self.pulse_1_length_counter = 0;
          }

          self.pulse_2_enable = (data & 0b0000_0010) != 0;
          if !self.pulse_2_enable {
            self.pulse_2_length_counter = 0;
          }
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

      // https://www.nesdev.org/wiki/APU_Frame_Counter
      //
      // Assume 4-step sequence, for now.
      //
      // Really we need to check a flag which is set by writing to 0x4017; see
      // the link above for details.
      if true {
        if self.frame_clock_counter == 3729 {
          quarter_frame = true;
        }

        if self.frame_clock_counter == 7457 {
          quarter_frame = true;
          half_frame = true;
        }

        if self.frame_clock_counter == 11186 {
          quarter_frame = true;
        }

        if self.frame_clock_counter == 14915 {
          quarter_frame = true;
          half_frame = true;
          self.frame_clock_counter = 0;
        }
      }

      // TODO: 5-step sequence mode...
      if false {
        // ...
      }

      if quarter_frame {
        // Update envelopes
        self.pulse_1_envelope.clock();
        self.pulse_2_envelope.clock();
      }

      if half_frame {
        // Update length counters

        // PC1
        if !self.pulse_1_length_counter_halt && self.pulse_1_length_counter > 0 {
          self.pulse_1_length_counter -= 1;
        }
        if self.pulse_1_length_counter == 0 {
          self.pulse_1_osc.amplitude = 0.0;
        } else {
          self.pulse_1_osc.amplitude = self.pulse_1_envelope.volume_level();
        }

        // PC2
        if !self.pulse_2_length_counter_halt && self.pulse_2_length_counter > 0 {
          self.pulse_2_length_counter -= 1;
        }
        if self.pulse_2_length_counter == 0 {
          self.pulse_2_osc.amplitude = 0.0;
        } else {
          // TODO: This should be controlled by envelope/constant volume:
          self.pulse_2_osc.amplitude = self.pulse_2_envelope.volume_level();
        }
      }

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

      // Nicer simulated oscillator as a sum of sin-waves:
      // Calculate frequency from `reload`:
      if self.pulse_1_enable {
        self.pulse_1_osc.frequency =
          NTSC_CPU_CLOCK_FREQ / (16.0 * (self.pulse_1_sequencer.reload + 1) as f32);
        self.pulse_1_sample = self.pulse_1_osc.sample(self.global_clock as f32);
      }

      if self.pulse_2_enable {
        self.pulse_2_osc.frequency =
          NTSC_CPU_CLOCK_FREQ / (16.0 * (self.pulse_2_sequencer.reload + 1) as f32);
        self.pulse_2_sample = self.pulse_2_osc.sample(self.global_clock as f32);
      }
    }

    self.clock_counter = self.clock_counter.wrapping_add(1);
  }

  pub fn reset(&mut self) {
    self.cpu_write(0x4015, 0x00);
  }
}

fn get_length_counter(pattern: u8) -> u8 {
  match pattern & 0b0001_1111 {
    // https://www.nesdev.org/wiki/APU_Length_Counter#Table_structure
    //
    // Legend:
    // <bit pattern> (<value of bit pattern>) => <note length>

    // Linear length values:
    // 1 1111 (1F) => 30
    0x1F => 30,
    // 1 1101 (1D) => 28
    0x1D => 28,
    // 1 1011 (1B) => 26
    0x1B => 26,
    // 1 1001 (19) => 24
    0x19 => 24,
    // 1 0111 (17) => 22
    0x17 => 22,
    // 1 0101 (15) => 20
    0x15 => 20,
    // 1 0011 (13) => 18
    0x13 => 18,
    // 1 0001 (11) => 16
    0x11 => 16,
    // 0 1111 (0F) => 14
    0x0F => 14,
    // 0 1101 (0D) => 12
    0x0D => 12,
    // 0 1011 (0B) => 10
    0x0B => 10,
    // 0 1001 (09) => 8
    0x09 => 8,
    // 0 0111 (07) => 6
    0x07 => 6,
    // 0 0101 (05) => 4
    0x05 => 4,
    // 0 0011 (03) => 2
    0x03 => 2,
    // 0 0001 (01) => 254
    0x01 => 254,

    // Notes with base length 12 (4/4 at 75 bpm):
    // 1 1110 (1E) => 32  (96 times 1/3, quarter note triplet)
    0x1E => 32,
    // 1 1100 (1C) => 16  (48 times 1/3, eighth note triplet)
    0x1C => 16,
    // 1 1010 (1A) => 72  (48 times 1 1/2, dotted quarter)
    0x1A => 72,
    // 1 1000 (18) => 192 (Whole note)
    0x18 => 192,
    // 1 0110 (16) => 96  (Half note)
    0x16 => 96,
    // 1 0100 (14) => 48  (Quarter note)
    0x14 => 48,
    // 1 0010 (12) => 24  (Eighth note)
    0x12 => 24,
    // 1 0000 (10) => 12  (Sixteenth)
    0x10 => 12,

    // Notes with base length 10 (4/4 at 90 bpm, with relative durations being the same as above):
    // 0 1110 (0E) => 26  (Approx. 80 times 1/3, quarter note triplet)
    0x0E => 26,
    // 0 1100 (0C) => 14  (Approx. 40 times 1/3, eighth note triplet)
    0x0C => 14,
    // 0 1010 (0A) => 60  (40 times 1 1/2, dotted quarter)
    0x0A => 60,
    // 0 1000 (08) => 160 (Whole note)
    0x08 => 160,
    // 0 0110 (06) => 80  (Half note)
    0x06 => 80,
    // 0 0100 (04) => 40  (Quarter note)
    0x04 => 40,
    // 0 0010 (02) => 20  (Eighth note)
    0x02 => 20,
    // 0 0000 (00) => 10  (Sixteenth)
    0x00 => 10,

    // This should technically be exhaustive since we're working with a 5-bit
    // value.
    _ => 0,
  }
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

/// https://www.nesdev.org/wiki/APU#Glossary
///
/// - A divider outputs a clock periodically. It contains a period `reload`
///   value, P, and a `counter`, that starts at P. When the divider is clocked,
///   if the counter is currently 0, it is reloaded with P and generates an
///   output clock, otherwise the counter is decremented. In other words, the
///   divider's period is P + 1.
/// - A divider can also be forced to reload its counter immediately (counter =
///   P), but this does not output a clock. Similarly, changing a divider's
///   period reload value does not affect the counter. Some counters offer no
///   way to force a reload, but setting P to 0 at least synchronizes it to a
///   known state once the current count expires.
/// - A divider may be implemented as a down counter (5, 4, 3, ...) or as a
///   linear feedback shift register (LFSR). The dividers in the pulse and
///   triangle channels are linear down-counters. The dividers for noise, DMC,
///   and the APU Frame Counter are implemented as LFSRs to save gates compared
///   to the equivalent down counter.
pub struct Divider {
  reload: u16,
  counter: u16,
}

impl Divider {
  pub fn new() -> Self {
    Divider {
      reload: 0x0000,
      counter: 0x0000,
    }
  }

  pub fn clock(&mut self) -> bool {
    if self.counter == 0 {
      self.counter = self.reload;
      true
    } else {
      self.counter -= 1;
      false
    }
  }
}

/// https://www.nesdev.org/wiki/APU_Envelope
///
/// Each volume envelope unit contains the following: start flag, divider, and
/// decay level counter.

pub struct Envelope {
  pub start_flag: bool,
  pub divider: Divider,
  /// Counts down from 15 to 0:
  pub decay_level: u8,
  pub loop_flag: bool,
  pub constant_volume_flag: bool,
}

impl Envelope {
  pub fn new() -> Self {
    Envelope {
      start_flag: false,
      divider: Divider::new(),
      decay_level: 0x00,
      loop_flag: false,
      constant_volume_flag: false,
    }
  }

  /// When clocked by the frame counter, one of two actions occurs: if the start
  /// flag is clear, the divider is clocked, otherwise the start flag is
  /// cleared, the decay level counter is loaded with 15, and the divider's
  /// period is immediately reloaded. When the divider is clocked while at 0, it
  /// is loaded with V and clocks the decay level counter. Then one of two
  /// actions occurs: If the counter is non-zero, it is decremented, otherwise
  /// if the loop flag is set, the decay level counter is loaded with 15.
  pub fn clock(&mut self) {
    if self.start_flag {
      self.start_flag = false;
      self.decay_level = 15;
      self.divider.counter = self.divider.reload;
    } else {
      if self.divider.clock() {
        if self.decay_level > 0 {
          self.decay_level -= 1;
        } else if self.loop_flag {
          self.decay_level = 15;
        }
      }
    }
  }

  // Why the heck does this need to be `&mut self`?
  pub fn volume_level(&mut self) -> f32 {
    if self.constant_volume_flag {
      (self.divider.reload as f32) / 16.0
    } else {
      (self.decay_level as f32) / 16.0
    }
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
      amplitude: 0.25,
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
