use rand::Rng;

use crate::bus_device::{BusDevice, BusDeviceRange};
use crate::palette::Palette;

pub const SCREEN_W: usize = 256;
pub const SCREEN_H: usize = 240;

pub struct Ppu {
  /// The current row number on the screen
  scanline: isize,
  /// The current pixel number on the current scanline
  cycle: isize,
  palette: Palette,
  pub frame_complete: bool,
  pub screen: [[u8; 4]; SCREEN_W * SCREEN_H],
}

impl Ppu {
  pub fn new(palette: Palette) -> Ppu {
    Ppu {
      scanline: 0,
      cycle: 0,
      frame_complete: false,
      palette,
      screen: [[0x00, 0x00, 0x00, 0xFF]; SCREEN_W * SCREEN_H],
    }
  }

  pub fn clock(&mut self) {
    if self.frame_complete {
      self.frame_complete = false;
    }

    let mut rng = rand::thread_rng();

    let screen_x = self.cycle - 1;
    let screen_y = self.scanline;
    if screen_x >= 0
      && screen_y >= 0
      && screen_x < (SCREEN_W as isize)
      && screen_y < (SCREEN_H as isize)
    {
      let idx = (screen_y as usize) * SCREEN_W + (screen_x as usize);
      let color = self.palette.colors[rng.gen_range(0, 64)];
      self.screen[idx][0] = color.r;
      self.screen[idx][1] = color.g;
      self.screen[idx][2] = color.b;
    }

    // Move right one pixel...
    self.cycle += 1;
    // ...and if we're at the end of the scanline...
    if self.cycle >= 341 {
      // ...increment the scanline and reset the cycle:
      self.scanline += 1;
      self.cycle = 0;

      // If our scanline is at the end of the screen...
      if self.scanline >= 261 {
        // ...reset the scanline, and mark this frame as complete
        self.scanline = -1;
        self.frame_complete = true;
      }
    }
  }

  pub fn render_pattern_table(&self) -> [[u8; 4]; 128 * 128] {
    [[0xFF, 0x00, 0xFF, 0xFF]; 128 * 128]
  }
}

// CPU can Read/Write to PPU registers, which are 8 bytes that start at 0x2000
impl BusDeviceRange for Ppu {
  fn start(&self) -> u16 {
    0x2000
  }
  fn size(&self) -> usize {
    8
  }
}

// Not sure how to implement this yet 😅
impl BusDevice for Ppu {
  fn read(&self, _addr: u16) -> std::option::Option<u8> {
    todo!()
  }
  fn write(&mut self, _addr: u16, _data: u8) -> std::option::Option<()> {
    todo!()
  }
}
