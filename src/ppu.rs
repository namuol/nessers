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
  pub palette: Palette,
  pub name_tables: [[u8; 1024]; 2],
  pub pattern_tables: [[u8; 4096]; 2],
  pub frame_complete: bool,
  pub screen: [[u8; 4]; SCREEN_W * SCREEN_H],

  address_latch: u8,
  data_buffer: u8,
  address: u16,

  status: u8,
  mask: u8,
  control: u8,
}

trait Register {
  fn from_u8(u: u8) -> Self;
}

#[derive(Debug)]
pub struct StatusRegister {
  pub sprite_overflow: bool,
  pub sprite_zero_hit: bool,
  pub vblank: bool,
}

impl Register for StatusRegister {
  fn from_u8(u: u8) -> StatusRegister {
    StatusRegister {
      sprite_overflow: (1 << 2) & u != 0,
      sprite_zero_hit: (1 << 1) & u != 0,
      vblank: (1 << 0) & u != 0,
    }
  }
}

#[derive(Debug)]
pub struct MaskRegister {
  pub grayscale: bool,
  pub render_background_left: bool,
  pub render_sprites_left: bool,
  pub render_background: bool,
  pub render_sprites: bool,
  pub enhance_red: bool,
  pub enhance_green: bool,
  pub enhance_blue: bool,
}

impl Register for MaskRegister {
  fn from_u8(u: u8) -> MaskRegister {
    MaskRegister {
      grayscale: (1 << 7) & u != 0,
      render_background_left: (1 << 6) & u != 0,
      render_sprites_left: (1 << 5) & u != 0,
      render_background: (1 << 4) & u != 0,
      render_sprites: (1 << 3) & u != 0,
      enhance_red: (1 << 2) & u != 0,
      enhance_green: (1 << 1) & u != 0,
      enhance_blue: (1 << 0) & u != 0,
    }
  }
}

#[derive(Debug)]
pub struct ControlRegister {
  pub nametable_x: bool,
  pub nametable_y: bool,
  pub increment_mode: bool,
  pub pattern_sprite: bool,
  pub pattern_background: bool,
  pub sprite_size: bool,
  pub slave_mode: bool,
  pub enable_nmi: bool,
}

impl Register for ControlRegister {
  fn from_u8(u: u8) -> ControlRegister {
    ControlRegister {
      nametable_x: (1 << 7) & u != 0,
      nametable_y: (1 << 6) & u != 0,
      increment_mode: (1 << 5) & u != 0,
      pattern_sprite: (1 << 4) & u != 0,
      pattern_background: (1 << 3) & u != 0,
      sprite_size: (1 << 2) & u != 0,
      slave_mode: (1 << 1) & u != 0,
      enable_nmi: (1 << 0) & u != 0,
    }
  }
}

impl Ppu {
  pub fn new(palette: Palette) -> Ppu {
    Ppu {
      scanline: 0,
      cycle: 0,
      frame_complete: false,
      palette,
      name_tables: [[0x00; 1024]; 2],
      pattern_tables: [[0x00; 4096]; 2],
      screen: [[0x00, 0x00, 0x00, 0xFF]; SCREEN_W * SCREEN_H],

      // Misc internal state
      address_latch: 0x00,
      data_buffer: 0x00,
      address: 0x0000,

      // Registers
      status: 0x00,
      mask: 0x00,
      control: 0x00,
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

  pub fn ppu_read(&self, addr: u16) -> u8 {
    // 0x0000 -> 0x1FFF = pattern memory
    // 0x2000 -> 0x3EFF = nametable memory
    // 0x3F00 -> 0x3FFF = palette memory
    0x00
  }

  pub fn ppu_write(&mut self, addr: u16, data: u8) {
    // 0x0000 -> 0x1FFF = pattern memory
    // 0x2000 -> 0x3EFF = nametable memory
    // 0x3F00 -> 0x3FFF = palette memory
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
  // From `cpuRead` in
  // https://www.youtube.com/watch?v=xdzOvpYPmGE&list=PLrOv9FMX8xJHqMvSGB_9G9nZZ_4IgteYf&index=4
  fn read(&mut self, addr: u16) -> Option<u8> {
    if !self.in_range(addr) {
      return None;
    }

    match addr % 8 {
      0x0000 => Some(self.control),
      0x0001 => Some(self.mask),
      0x0002 => Some(self.status),
      // 0x0003 => {} // OAM Address
      // 0x0004 => {} // OAM Data
      // 0x0005 => {} // Scroll
      // 0x0006 => {} // PPU Address
      0x0007 => {
        let data = self.data_buffer;
        // NEED TO MAKE READS MUTABLE UGH:
        self.data_buffer = self.ppu_read(self.address);
        Some(data)
      }
      _ => Some(0x00),
    }
  }

  // From `cpuWrite` in https://www.youtube.com/watch?v=xdzOvpYPmGE&list=PLrOv9FMX8xJHqMvSGB_9G9nZZ_4IgteYf&index=4
  fn write(&mut self, addr: u16, data: u8) -> Option<()> {
    if !self.in_range(addr) {
      return None;
    }

    match addr % 8 {
      0x0000 => {
        self.control = data;
      } // Control
      0x0001 => {
        self.mask = data;
      } // Mask
      0x0002 => {} // Status
      0x0003 => {} // OAM Address
      0x0004 => {} // OAM Data
      0x0005 => {} // Scroll
      0x0006 => {
        if self.address_latch == 0 {
          // Write the low byte of address:
          self.address = (self.address & 0xFF00) | data as u16;
          self.address_latch = 1;
        } else {
          // Write the high byte of address:
          self.address = (self.address & 0x00FF) | ((data as u16) << 8);
          self.address_latch = 0;
        }
      }
      0x0007 => {
        self.ppu_write(self.address, data);
      }
      _ => {}
    }

    Some(())
  }
}
