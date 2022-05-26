use std::f32::consts::PI;

use lazy_static::lazy_static;

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

  pub pulse: [Pulse; 2],
  pub triangle: Triangle,
  pub noise: Noise,

  time_until_next_sample: f32,
  sample_clock: f32,
  clock_counter: u32,
  frame_clock_counter: u32,
  pub global_clock: f64,
}

impl Apu {
  pub fn new() -> Self {
    Apu {
      pulse: [Pulse::new(), Pulse::new()],
      triangle: Triangle::new(),
      noise: Noise::new(),
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

    let mut sample: f32 = 0.0;
    for i in 0..self.pulse.len() {
      sample += self.pulse[i].sample * 0.25;
    }

    sample += self.triangle.sample * 0.25;
    sample += self.noise.sample * 0.25;
    sample
  }

  pub fn cpu_write(&mut self, addr: u16, data: u8) -> Option<()> {
    if (addr >= 0x4000 && addr <= 0x4013) || addr == 0x4015 || addr == 0x4017 {
      // Technically as-is this will break controller input since 4017 conflicts
      // with controller's 4017... that's okay because this should just be
      // temporary.
      match addr {
        // Pulse 1 & 2
        0x4000 | 0x4004 => {
          let i = if addr == 0x4000 { 0 } else { 1 };

          // Duty Cycle
          match (data & 0b1100_0000) >> 6 {
            0x00 => {
              self.pulse[i].sequencer.sequence = 0b0000_0001;
              self.pulse[i].osc.duty_cycle = 1.0 / 8.0;
            }
            0x01 => {
              self.pulse[i].sequencer.sequence = 0b0000_0011;
              self.pulse[i].osc.duty_cycle = 2.0 / 8.0;
            }
            0x02 => {
              self.pulse[i].sequencer.sequence = 0b0000_1111;
              self.pulse[i].osc.duty_cycle = 4.0 / 8.0;
            }
            0x03 => {
              self.pulse[i].sequencer.sequence = 0b1111_1100;
              self.pulse[i].osc.duty_cycle = 6.0 / 8.0;
            }
            _ => {}
          };

          // Constant Volume flag
          self.pulse[i].envelope.constant_volume_flag = (data & 0b0001_0000) != 0;
          // Constant volume level or Envelope length
          self.pulse[i].envelope.divider.reload = (data & 0b0000_1111) as u16; // Why is this u16 again?

          // Length Counter Halt
          self.pulse[i].length_counter_halt = (data & 0b0010_0000) != 0;
        }

        0x4001 | 0x4005 => {
          let i = if addr == 0x4001 { 0 } else { 1 };

          // Sweep
          self.pulse[i].sweep.enabled = (0b1000_0000 & data) != 0;
          self.pulse[i].sweep.divider.reload = ((0b0111_0000 & data) >> 4) as u16;
          self.pulse[i].sweep.negate = (0b0000_1000 & data) != 0;
          self.pulse[i].sweep.shift_count = 0b0000_0111 & data;
          self.pulse[i].sweep.divider.force_reload = true;

          // if self.pulse[i].sweep.enabled {
          //   println!(
          //     "p{} e:{} p:{} n:{} s:{}",
          //     i,
          //     self.pulse[i].sweep.enabled,
          //     self.pulse[i].sweep.divider.reload,
          //     self.pulse[i].sweep.negate,
          //     self.pulse[i].sweep.shift_count
          //   );
          // }
        }

        0x4002 | 0x4006 => {
          let i = if addr == 0x4002 { 0 } else { 1 };

          self.pulse[i].sequencer.reload =
            (self.pulse[i].sequencer.reload & 0xFF00) | (data as u16);
        }

        0x4003 | 0x4007 => {
          let i = if addr == 0x4003 { 0 } else { 1 };

          self.pulse[i].sequencer.reload =
            (((data as u16) & 0x07) << 8) | (self.pulse[i].sequencer.reload & 0x00FF);

          self.pulse[i].sequencer.timer = self.pulse[i].sequencer.reload;

          // Length Counter/Envelope start flag
          //
          // Basically, start playing a note.
          if self.pulse[i].enable {
            // Start Flag; should this also be triggered only when the pulse is
            // enabled? Unclear from here:
            // https://www.nesdev.org/wiki/APU_Envelope
            self.pulse[i].envelope.start_flag = true;

            self.pulse[i].length_counter = get_length_counter((data & 0b1111_1000) >> 3);
          }
        }

        0x4015 => {
          self.pulse[0].enable = (data & 0b0000_0001) != 0;
          if !self.pulse[0].enable {
            self.pulse[0].length_counter = 0;
          }

          self.pulse[1].enable = (data & 0b0000_0010) != 0;
          if !self.pulse[1].enable {
            self.pulse[1].length_counter = 0;
          }

          self.triangle.enable = (data & 0b0000_0100) != 0;
          if !self.triangle.enable {
            self.triangle.length_counter = 0;
          }

          self.noise.enable = (data & 0b0000_0100) != 0;
          if !self.noise.enable {
            self.noise.length_counter = 0;
          }
        }

        // Triangle
        0x4008 => {
          // Also the length counter halt apparently
          self.triangle.control = (0b1000_0000 & data) != 0;
          self.triangle.linear_counter_reload_value = 0b0111_1111 & data;
        }

        0x400A => {
          // Timer lower 8 bits
          self.triangle.sequencer.reload =
            (self.triangle.sequencer.reload & 0xFF00) | (data as u16);
        }

        0x400B => {
          // Timer high 5 bits
          self.triangle.sequencer.reload =
            (((data as u16) & 0x07) << 8) | (self.triangle.sequencer.reload & 0x00FF);

          self.triangle.length_counter = get_length_counter((data & 0b1111_1000) >> 3);
          self.triangle.linear_counter_reload = true;
        }

        // Noise
        0x400C => {
          // Length Counter Halt
          self.noise.length_counter_halt = (0b0010_0000 & data) != 0;
          // Constant Volume flag
          self.noise.envelope.constant_volume_flag = (0b0001_0000 & data) != 0;
          // Constant volume level or Envelope length
          self.noise.envelope.divider.reload = (data & 0b0000_1111) as u16; // Why is this u16 again?
        }

        0x400E => {
          self.noise.mode_flag = (0b1000_0000 & data) != 0;
          self.noise.sequencer.reload = get_noise_sequencer_period(data & 0b0000_1111) as u16;
          self.noise.sequencer.timer = self.noise.sequencer.reload;
        }

        0x400F => {
          self.noise.length_counter = get_length_counter((data & 0b1111_1000) >> 3);
          self.noise.envelope.start_flag = true;
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
      // Don't need wrapping_add here since we're always resetting to 0:
      self.frame_clock_counter += 1;

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
        for i in 0..self.pulse.len() {
          self.pulse[i].envelope.clock();
        }
        self.noise.envelope.clock();

        // Update Triangle linear counter
        self.triangle.linear_counter_clock();
      }

      if half_frame {
        for i in 0..self.pulse.len() {
          // Update Sweeps
          self.pulse[i].sequencer.reload = self.pulse[i]
            .sweep
            .clock(self.pulse[i].sequencer.reload, i != 0);

          // Update length counters
          if !self.pulse[i].length_counter_halt && self.pulse[i].length_counter > 0 {
            self.pulse[i].length_counter -= 1;
          }

          // Set amplitude
          if self.pulse[i].length_counter == 0 || self.pulse[i].sweep.muting {
            self.pulse[i].osc.amplitude = 0.0;
          } else {
            self.pulse[i].osc.amplitude = self.pulse[i].envelope.volume_level() * 0.25;
          }
        }

        // Update length counters
        if !self.triangle.control && self.triangle.length_counter > 0 {
          self.triangle.length_counter -= 1;
        }

        if !self.noise.length_counter_halt && self.noise.length_counter > 0 {
          self.noise.length_counter -= 1
        }
      }

      // Nasty raw 1-bit sound:
      //
      // for i in 0..self.pulse.len() {
      //   self.pulse[i].sequencer.clock(
      //     self.pulse[i].enable,
      //     // Shift right by 1 bit, wrapping around.
      //     //
      //     // ```
      //     // 0b0000_0010_... -> 0b0000_0001_...
      //     // 0b0000_..._0001 -> 0b1000_..._0000
      //     // ```
      //     |s| ((s & 0x0000_0001) << 7) | ((s & 0x0000_00FE) >> 1),
      //   );
      //   self.pulse[i].sample = if self.pulse[i].sequencer.output == 0 {
      //     0.0
      //   } else {
      //     self.pulse[i].envelope.volume_level()
      //   };
      // }

      // Nicer simulated oscillator as a sum of sin-waves:
      for i in 0..self.pulse.len() {
        if self.pulse[i].enable {
          // Calculate frequency from `reload` which is sometimes referred to as
          // the "period" of the pulse wave. Should I rename this? Maybe. I got
          // started from the OLC youtube tutorial which used these names which
          // I found really confusing, especially since ultimately the sequencer
          // approach to generating samples was replaced with an oscillator.
          self.pulse[i].osc.frequency = period_to_frequency(self.pulse[i].sequencer.reload);
          self.pulse[i].sample = self.pulse[i].osc.sample(self.global_clock as f32);
        }
      }

      self.noise.sequencer.clock(self.noise.enable, &mut |_| {
        Noise::clock(&mut self.noise.lfsr, self.noise.mode_flag) as u32
      });
      self.noise.sample = self.noise.get_sample();
    }

    // The triangle's sequencer runs at twice the rate of the pulse sequencers:
    if self.clock_counter % 3 == 0 {
      // Triangle 4-bit sound:
      if self.triangle.length_counter != 0 && self.triangle.linear_counter != 0 {
        self
          .triangle
          .sequencer
          .clock(self.triangle.enable, &mut |s| (s + 1) % 32);
        self.triangle.sample = self.triangle.get_sample();
      }
    }

    self.clock_counter = self.clock_counter.wrapping_add(1);
  }

  pub fn reset(&mut self) {
    self.cpu_write(0x4015, 0x00);
  }
}

fn period_to_frequency(period: u16) -> f32 {
  NTSC_CPU_CLOCK_FREQ / (16.0 * ((period as u32) + 1) as f32)
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

/// Takes a 4-bit number (top 4 bits ignored) and produces a length for the
/// period of the noise channel's.
///
/// ```
/// Rate  $0 $1  $2  $3  $4  $5   $6   $7   $8   $9   $A   $B   $C    $D    $E    $F
///       --------------------------------------------------------------------------
/// NTSC   4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068
/// PAL    4, 8, 14, 30, 60, 88, 118, 148, 188, 236, 354, 472, 708,  944, 1890, 3778
/// ```
fn get_noise_sequencer_period(data: u8) -> u16 {
  match data & 0b0000_1111 {
    0x0 => 4,
    0x1 => 8,
    0x2 => 16,
    0x3 => 32,
    0x4 => 64,
    0x5 => 96,
    0x6 => 128,
    0x7 => 160,
    0x8 => 202,
    0x9 => 254,
    0xA => 380,
    0xB => 508,
    0xC => 762,
    0xD => 1016,
    0xE => 2034,
    0xF => 4068,
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
  pub fn new() -> Self {
    Sequencer {
      sequence: 0b0000_0000_0000_0000_0000_0000_0000_0000,
      timer: 0x0000,
      reload: 0x0000,
      output: 0x00,
    }
  }

  pub fn clock(&mut self, enable: bool, manipulate_sequence: &mut dyn FnMut(u32) -> u32) -> u8 {
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
  // Only used by Sweep
  force_reload: bool,
}

impl Divider {
  pub fn new() -> Self {
    Divider {
      reload: 0x0000,
      counter: 0x0000,
      force_reload: false,
    }
  }

  pub fn clock(&mut self) -> bool {
    if self.counter == 0 || self.force_reload {
      self.counter = self.reload;
      self.force_reload = false;
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
      (self.divider.reload as f32) / 15.0
    } else {
      (self.decay_level as f32) / 15.0
    }
  }
}

pub struct Sweep {
  enabled: bool,
  divider: Divider,
  negate: bool,
  // We use the reload flag in our Divider:
  // reload: bool,
  shift_count: u8,
  muting: bool,
}

impl Sweep {
  pub fn new() -> Self {
    Sweep {
      enabled: false,
      divider: Divider::new(),
      negate: false,
      shift_count: 0,
      muting: false,
    }
  }

  /// Clocks the Sweep's internal divider and if conditions are right, returns a
  /// new value to set the Pulse's sequencer's period to.
  pub fn clock(&mut self, current_period: u16, is_pulse_2: bool) -> u16 {
    // https://www.nesdev.org/wiki/APU_Sweep#Updating_the_period
    //
    // When the frame counter sends a half-frame clock (at 120 or 96 Hz), two
    // things happen.
    //
    // 1. If the divider's counter is zero, the sweep is enabled, and the sweep
    //    unit is not muting the channel: The pulse's period is adjusted.
    // 2. If the divider's counter is zero or the reload flag is true: The
    //    counter is set to P and the reload flag is cleared. Otherwise, the
    //    counter is decremented.
    //
    // When the sweep unit is muting the channel, the channel's current period
    // remains unchanged, but the divider continues to count down and reload the
    // (unchanging) period as normal. Otherwise, if the enable flag is set and
    // the shift count is non-zero, when the divider outputs a clock, the
    // channel's period is updated.
    //
    // If the shift count is zero, the channel's period is never updated, but
    // muting logic still applies.
    // let change_amount = barrel_shift_11_bits(current_period, self.shift_count);
    let change_amount = current_period >> self.shift_count;
    let target_period = if self.negate {
      current_period.wrapping_sub(change_amount + if is_pulse_2 { 0 } else { 1 })
    } else {
      current_period.wrapping_add(change_amount)
    };

    self.muting = current_period < 8 || target_period > 0x7FF;

    if self.divider.clock() && self.enabled && !self.muting {
      // https://www.nesdev.org/wiki/APU_Sweep#Calculating_the_target_period
      //
      // The sweep unit continuously calculates each channel's target period in
      // this way:
      //
      // 1. A barrel shifter shifts the channel's 11-bit raw timer period
      //      ^^^^^^^^^^^^^^ - NO. THIS IS WRONG. USE ORDINARY RIGHT SHIFT.
      //      OTHERWISE YOU GET HUGE CHANGES WHEN NUMBERS WRAP AROUND.
      //
      //    right by the shift count, producing the change amount.
      // 2. If the negate flag is true, the change amount is made negative.
      // 3. The target period is the sum of the current period and the change
      //    amount.
      //
      // For example, if the negate flag is false and the shift amount is zero,
      // the change amount equals the current period, making the target period
      // equal to twice the current period.
      //
      // The two pulse channels have their adders' carry inputs wired
      // differently, which produces different results when each channel's
      // change amount is made negative:
      //
      // - Pulse 1 adds the ones' complement (−c − 1). Making 20 negative
      //   produces a change amount of −21.
      // - Pulse 2 adds the two's complement (−c). Making 20 negative produces a
      //   change amount of −20.
      //
      // Whenever the current period changes for any reason, whether by $400x
      // writes or by sweep, the target period also changes. println!( "s {}
      // chg_amt {}{:03X} {}{}; c {:03X} {} t {:03X} {}", self.shift_count, if
      //   self.negate { "-" } else { "" }, change_amount, if self.negate { "-"
      //   } else { "" }, period_to_frequency(change_amount), current_period,
      //   period_to_frequency(current_period), target_period,
      //   period_to_frequency(target_period) );

      target_period
    } else {
      current_period
    }
  }
}

pub struct Pulse {
  pub enable: bool,
  pub sample: f32,
  pub sequencer: Sequencer,
  pub osc: PulseOscillator,
  pub length_counter: u8,
  pub length_counter_halt: bool,
  pub envelope: Envelope,
  pub sweep: Sweep,
}

impl Pulse {
  fn new() -> Self {
    Pulse {
      enable: false,
      sample: 0.0,
      sequencer: Sequencer {
        // Not sure why this is 32 bits; seems we only care about the lower 8
        // bits:
        sequence: 0b0000_0000_0000_0000_0000_0000__0000_0000,
        timer: 0x0000,
        reload: 0x0000,
        output: 0x00,
      },
      osc: PulseOscillator::new(),
      length_counter: 0x00,
      length_counter_halt: false,
      envelope: Envelope::new(),
      sweep: Sweep::new(),
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
      amplitude: 1.0,
      harmonics: 60,
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

pub struct Triangle {
  enable: bool,
  sequencer: Sequencer,
  length_counter: u8,
  linear_counter: u8,
  linear_counter_reload_value: u8,
  linear_counter_reload: bool,
  control: bool,
  sample: f32,
}

#[rustfmt::skip]
const TRIANGLE_SEQUENCE: [f32; 32] = [
  15.0 / 15.0, 14.0 / 15.0, 13.0 / 15.0, 12.0 / 15.0, 11.0 / 15.0, 10.0 / 15.0, 9.0 / 15.0, 8.0 / 15.0, 7.0 / 15.0, 6.0 / 15.0, 5.0 / 15.0, 4.0 / 15.0, 3.0 / 15.0, 2.0 / 15.0, 1.0 / 15.0, 0.0 / 15.0,
  0.0 / 15.0, 1.0 / 15.0, 2.0 / 15.0, 3.0 / 15.0, 4.0 / 15.0, 5.0 / 15.0, 6.0 / 15.0, 7.0 / 15.0, 8.0 / 15.0, 9.0 / 15.0, 10.0 / 15.0, 11.0 / 15.0, 12.0 / 15.0, 13.0 / 15.0, 14.0 / 15.0, 15.0 / 15.0,
];

impl Triangle {
  pub fn new() -> Self {
    Triangle {
      enable: true,
      sequencer: Sequencer::new(),
      length_counter: 0x00,
      linear_counter: 0x00,
      linear_counter_reload_value: 0x00,
      linear_counter_reload: false,
      control: false,
      sample: 0.0,
    }
  }

  pub fn linear_counter_clock(&mut self) {
    // https://www.nesdev.org/wiki/APU_Triangle
    //
    // When the frame counter generates a linear counter clock, the following
    // actions occur in order:
    //
    // 1. If the linear counter reload flag is set, the linear counter is
    //    reloaded with the counter reload value, otherwise if the linear
    //    counter is non-zero, it is decremented.
    // 2. If the control flag is clear, the linear counter reload flag is
    //    cleared.

    if self.linear_counter_reload {
      self.linear_counter = self.linear_counter_reload_value;
    } else if self.linear_counter != 0 {
      self.linear_counter -= 1;
    }

    if !self.control {
      self.linear_counter_reload = false;
    }
  }

  pub fn get_sample(&mut self) -> f32 {
    // We (mis)use the sequencer's sequence value to loop through 32 steps.
    TRIANGLE_SEQUENCE[(self.sequencer.sequence % 32) as usize]
  }
}

/// https://www.nesdev.org/wiki/APU_Noise
pub struct Noise {
  enable: bool,
  sequencer: Sequencer,
  envelope: Envelope,

  mode_flag: bool,

  // TODO: Move length counter logic into a struct with methods
  length_counter_halt: bool,
  length_counter: u8,

  lfsr: LinearFeedbackShiftRegister,

  sample: f32,
}

impl Noise {
  fn new() -> Self {
    Noise {
      enable: true,
      sequencer: Sequencer::new(),
      envelope: Envelope::new(),

      mode_flag: false,

      length_counter_halt: false,
      length_counter: 0x00,

      // On power-up, the shift register is loaded with the value 1.
      lfsr: LinearFeedbackShiftRegister(0b0000_0000_0000_0001),

      sample: 0.0,
    }
  }
  fn clock(lfsr: &mut LinearFeedbackShiftRegister, mode_flag: bool) -> u16 {
    // The shift register is 15 bits wide, with bits numbered:
    // ```
    // 14 - 13 - 12 - 11 - 10 - 9 - 8 - 7 - 6 - 5 - 4 - 3 - 2 - 1 - 0
    // ```
    //
    // When the timer clocks the shift register, the following actions occur in
    // order:
    //
    // 1. Feedback is calculated as the exclusive-OR of bit 0 and one other bit:
    //    bit 6 if Mode flag is set, otherwise bit 1.
    // 2. The shift register is shifted right by one bit.
    // 3. Bit 14, the leftmost bit, is set to the feedback calculated earlier.
    let feedback = (0b0000_0000_0000_0001 & lfsr.0)
      ^ (if mode_flag {
        (0b0000_0000_0010_0000 & lfsr.0) >> 6
      } else {
        (0b0000_0000_0000_0010 & lfsr.0) >> 1
      });

    lfsr.0 >>= 1;
    lfsr.0 |= feedback << 14;
    lfsr.0 & 0b0000_0000_0000_0001
  }
  fn get_sample(&mut self) -> f32 {
    // The mixer receives the current envelope volume except when
    // - Bit 0 of the shift register is set, or
    // - The length counter is zero
    if (self.lfsr.0 & 0b0000_0000_0000_0001) != 0 || self.length_counter == 0 {
      0.0
    } else {
      self.envelope.volume_level()
    }
  }
}

struct LinearFeedbackShiftRegister(u16);
