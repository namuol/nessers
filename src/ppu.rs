use crate::bus_device::{BusDevice, BusDeviceRange};

pub struct Ppu {
  /// The current row number on the screen
  scanline: i16,
  /// The current pixel number on the current scanline
  cycle: u16,
  frame_complete: bool,
}

impl Ppu {
  pub fn new() -> Ppu {
    Ppu {
      scanline: -1,
      cycle: 0,
      frame_complete: false,
    }
  }

  pub fn clock(&mut self) {
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
