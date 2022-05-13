use rand::Rng;

use crate::bus_device::{BusDevice, BusDeviceRange};
use crate::cart::{Cart, Mirroring};
use crate::palette::Palette;

pub const SCREEN_W: usize = 256;
pub const SCREEN_H: usize = 240;

#[derive(Clone)]
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

  /// Reading from the PPU usually takes two cycles to fully read our data, so
  /// we need to store the data that was read so that we can return it the next
  /// time we read data:
  data_buffer: u8,

  /// The address which will be written to or read from
  pub address: u16,

  pub status: u8,
  pub mask: u8,
  pub control: u8,

  /// Whether a non-maskable interrupt has been triggered
  pub nmi: bool,
}

pub trait StatusRegister {
  fn sprite_overflow(self) -> bool;
  fn sprite_zero_hit(self) -> bool;
  fn vblank(self) -> bool;

  fn set_sprite_overflow(self, v: bool) -> Self;
  fn set_sprite_zero_hit(self, v: bool) -> Self;
  fn set_vblank(self, v: bool) -> Self;
}

trait SetFlag {
  fn set(self, pos: u8, v: bool) -> Self;
}

impl SetFlag for u8 {
  fn set(self, pos: u8, v: bool) -> Self {
    let x = 1 << pos;
    if v {
      self | x
    } else {
      self & !x
    }
  }
}

#[rustfmt::skip]
impl StatusRegister for u8 {
  fn vblank(self)          -> bool { (1 << 7) & self != 0 }
  fn sprite_zero_hit(self) -> bool { (1 << 6) & self != 0 }
  fn sprite_overflow(self) -> bool { (1 << 5) & self != 0 }

  fn set_vblank(self, v: bool)          -> u8 { self.set(7, v) }
  fn set_sprite_zero_hit(self, v: bool) -> u8 { self.set(6, v) }
  fn set_sprite_overflow(self, v: bool) -> u8 { self.set(5, v) }
}

pub trait MaskRegister {
  fn grayscale(self) -> bool;
  fn render_background_left(self) -> bool;
  fn render_sprites_left(self) -> bool;
  fn render_background(self) -> bool;
  fn render_sprites(self) -> bool;
  fn enhance_red(self) -> bool;
  fn enhance_green(self) -> bool;
  fn enhance_blue(self) -> bool;

  fn set_grayscale(self, v: bool) -> Self;
  fn set_render_background_left(self, v: bool) -> Self;
  fn set_render_sprites_left(self, v: bool) -> Self;
  fn set_render_background(self, v: bool) -> Self;
  fn set_render_sprites(self, v: bool) -> Self;
  fn set_enhance_red(self, v: bool) -> Self;
  fn set_enhance_green(self, v: bool) -> Self;
  fn set_enhance_blue(self, v: bool) -> Self;
}

#[rustfmt::skip]
impl MaskRegister for u8 {
  fn grayscale(self)              -> bool { (1 << 7) & self != 0 }
  fn render_background_left(self) -> bool { (1 << 6) & self != 0 }
  fn render_sprites_left(self)    -> bool { (1 << 5) & self != 0 }
  fn render_background(self)      -> bool { (1 << 4) & self != 0 }
  fn render_sprites(self)         -> bool { (1 << 3) & self != 0 }
  fn enhance_red(self)            -> bool { (1 << 2) & self != 0 }
  fn enhance_green(self)          -> bool { (1 << 1) & self != 0 }
  fn enhance_blue(self)           -> bool { (1 << 0) & self != 0 }

  fn set_grayscale(self, v: bool)              -> u8 { self.set(7, v) }
  fn set_render_background_left(self, v: bool) -> u8 { self.set(6, v) }
  fn set_render_sprites_left(self, v: bool)    -> u8 { self.set(5, v) }
  fn set_render_background(self, v: bool)      -> u8 { self.set(4, v) }
  fn set_render_sprites(self, v: bool)         -> u8 { self.set(3, v) }
  fn set_enhance_red(self, v: bool)            -> u8 { self.set(2, v) }
  fn set_enhance_green(self, v: bool)          -> u8 { self.set(1, v) }
  fn set_enhance_blue(self, v: bool)           -> u8 { self.set(0, v) }
}

pub trait ControlRegister {
  fn nametable_x(self) -> bool;
  fn nametable_y(self) -> bool;
  fn increment_mode(self) -> bool;
  fn pattern_sprite(self) -> bool;
  fn pattern_background(self) -> bool;
  fn sprite_size(self) -> bool;
  fn slave_mode(self) -> bool;
  fn enable_nmi(self) -> bool;

  fn set_nametable_x(self, v: bool) -> Self;
  fn set_nametable_y(self, v: bool) -> Self;
  fn set_increment_mode(self, v: bool) -> Self;
  fn set_pattern_sprite(self, v: bool) -> Self;
  fn set_pattern_background(self, v: bool) -> Self;
  fn set_sprite_size(self, v: bool) -> Self;
  fn set_slave_mode(self, v: bool) -> Self;
  fn set_enable_nmi(self, v: bool) -> Self;
}

#[rustfmt::skip]
impl ControlRegister for u8 {
  fn nametable_x(self)        -> bool { (1 << 0) & self != 0 }
  fn nametable_y(self)        -> bool { (1 << 1) & self != 0 }
  fn increment_mode(self)     -> bool { (1 << 2) & self != 0 }
  fn pattern_sprite(self)     -> bool { (1 << 3) & self != 0 }
  fn pattern_background(self) -> bool { (1 << 4) & self != 0 }
  fn sprite_size(self)        -> bool { (1 << 5) & self != 0 }
  fn slave_mode(self)         -> bool { (1 << 6) & self != 0 }
  fn enable_nmi(self)         -> bool { (1 << 7) & self != 0 }

  fn set_nametable_x(self, v: bool)         -> u8 { self.set(0, v) }
  fn set_nametable_y(self, v: bool)         -> u8 { self.set(1, v) }
  fn set_increment_mode(self, v: bool)      -> u8 { self.set(2, v) }
  fn set_pattern_sprite(self, v: bool)      -> u8 { self.set(3, v) }
  fn set_pattern_background(self, v: bool)  -> u8 { self.set(4, v) }
  fn set_sprite_size(self, v: bool)         -> u8 { self.set(5, v) }
  fn set_slave_mode(self, v: bool)          -> u8 { self.set(6, v) }
  fn set_enable_nmi(self, v: bool)          -> u8 { self.set(7, v) }
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

      nmi: false,
    }
  }

  pub fn clock(&mut self) {
    if self.scanline == -1 && self.cycle == 1 {
      self.status = self.status.set_vblank(false);
    }

    if self.scanline == 241 && self.cycle == 1 {
      self.status = self.status.set_vblank(true);
      if self.control.enable_nmi() {
        self.nmi = true;
      }
    }

    if self.frame_complete {
      self.frame_complete = false;
    }

    // Random noise:
    //
    // let mut rng = rand::thread_rng();

    // let screen_x = self.cycle - 1;
    // let screen_y = self.scanline;
    // if screen_x >= 0
    //   && screen_y >= 0
    //   && screen_x < (SCREEN_W as isize)
    //   && screen_y < (SCREEN_H as isize)
    // {
    //   let idx = (screen_y as usize) * SCREEN_W + (screen_x as usize);
    //   let color = self.palette.colors[rng.gen_range(0, 64)];
    //   self.screen[idx][0] = color.r;
    //   self.screen[idx][1] = color.g;
    //   self.screen[idx][2] = color.b;
    // }

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

  #[allow(unused_comparisons)]
  pub fn ppu_read(&self, addr: u16, cart: &Cart) -> u8 {
    if addr >= 0x0000 && addr <= 0x1FFF {
      // 0x0000 -> 0x1FFF = pattern memory
      return self.pattern_tables[((addr & 0x1000) >> 12) as usize][(addr & 0x0FFF) as usize];
    } else if addr >= 0x2000 && addr <= 0x3EFF {
      // 0x2000 -> 0x3EFF = nametable memory

      let table = match cart.mirroring {
        Mirroring::Vertical => match addr {
          0x0000..=0x03FF => 0,
          0x0400..=0x07FF => 1,
          0x0800..=0x0BFF => 0,
          0x0C00..=0x0FFF => 1,
          _ => 0x00,
        },
        Mirroring::Horizontal => match addr {
          0x0000..=0x03FF => 0,
          0x0400..=0x07FF => 0,
          0x0800..=0x0BFF => 1,
          0x0C00..=0x0FFF => 1,
          _ => 0x00,
        },
        Mirroring::OneScreenLo => todo!(),
        Mirroring::OneScreenHi => todo!(),
      };

      return self.name_tables[table][(addr & 0x03FF) as usize];
    } else if addr >= 0x3F00 && addr <= 0x3FFF {
      // 0x3F00 -> 0x3FFF = palette memory
      let addr = match addr & 0x001F {
        0x0010 => 0x0000,
        0x0014 => 0x0004,
        0x0018 => 0x0008,
        0x001C => 0x000C,
        _ => addr & 0x001F,
      };

      return self.palette.map[addr as usize];
    }

    0x00
  }

  #[allow(unused_comparisons)]
  pub fn ppu_write(&mut self, addr: u16, data: u8, cart: &Cart) {
    if addr >= 0x0000 && addr <= 0x1FFF {
      // 0x0000 -> 0x1FFF = pattern memory
      self.pattern_tables[((addr & 0x1000) >> 12) as usize][(addr & 0x0FFF) as usize] = data;
      return;
    } else if addr >= 0x2000 && addr <= 0x3EFF {
      // 0x2000 -> 0x3EFF = nametable memory
      let table = match cart.mirroring {
        Mirroring::Vertical => match addr {
          0x0000..=0x03FF => 0,
          0x0400..=0x07FF => 1,
          0x0800..=0x0BFF => 0,
          0x0C00..=0x0FFF => 1,
          _ => 0x00,
        },
        Mirroring::Horizontal => match addr {
          0x0000..=0x03FF => 0,
          0x0400..=0x07FF => 0,
          0x0800..=0x0BFF => 1,
          0x0C00..=0x0FFF => 1,
          _ => 0x00,
        },
        Mirroring::OneScreenLo => todo!(),
        Mirroring::OneScreenHi => todo!(),
      };
      let idx = (addr & 0x03FF) as usize;

      self.name_tables[table][idx] = data;
      return;
    } else if addr >= 0x3F00 && addr <= 0x3FFF {
      // 0x3F00 -> 0x3FFF = palette memory

      let addr = match addr & 0x001F {
        0x0010 => 0x0000,
        0x0014 => 0x0004,
        0x0018 => 0x0008,
        0x001C => 0x000C,
        _ => addr & 0x001F,
      };

      self.palette.map[addr as usize] = data;
      return;
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
  // From `cpuRead` in
  // https://www.youtube.com/watch?v=xdzOvpYPmGE&list=PLrOv9FMX8xJHqMvSGB_9G9nZZ_4IgteYf&index=4
  fn read(&mut self, addr: u16, cart: &Cart) -> Option<u8> {
    if !self.in_range(addr) {
      return None;
    }

    match addr % 8 {
      // 0x0000 => Some(self.control),
      // 0x0001 => Some(self.mask),
      0x0002 => {
        // Reading from the status register, we only care about the top 3 bits,
        // however according to NES lore, the lower 5 bits apparently contain
        // the contents from whatever data was last read from the PPU (which we
        // store as `self.data_buffer`):
        let data = (self.status & 0b111_00000) | (self.data_buffer & 0b000_11111);

        // Reading from the status register clears the vblank flag 🤷‍♂️
        self.status = self.status.set_vblank(false);

        // Reading from the status register also resets the address latch:
        self.address_latch = 0;

        Some(data)
      }
      // 0x0003 => {} // OAM Address
      // 0x0004 => {} // OAM Data
      // 0x0005 => {} // Scroll
      // 0x0006 => {} // PPU Address
      0x0007 => {
        // We don't actually return the data at the address from this read
        // operation; we instead return whatever was previously read - this is
        // basically a simulation of a read operation that takes more than one
        // cycle to complete.
        let data = self.data_buffer;
        self.data_buffer = self.ppu_read(self.address, cart);

        // Addresses above 0x3F00 are part of the palette memory which can be
        // read right away rather than taking an extra cycle:
        if self.address > 0x3F00 {
          return Some(self.data_buffer);
        }

        // Auto-increment our address for the next operation if the developer
        // so-chooses:
        self.address += if self.control.increment_mode() { 32 } else { 1 };
        // self.address += 1;

        Some(data)
      }
      _ => Some(0x00),
    }
  }

  fn safe_read(&self, _addr: u16, cart: &Cart) -> Option<u8> {
    todo!()
  }

  // From `cpuWrite` in https://www.youtube.com/watch?v=xdzOvpYPmGE&list=PLrOv9FMX8xJHqMvSGB_9G9nZZ_4IgteYf&index=4
  fn write(&mut self, addr: u16, data: u8, cart: &Cart) -> Option<()> {
    if !self.in_range(addr) {
      return None;
    }

    match addr % 8 {
      0x0000 => {
        self.control = data;
      }
      0x0001 => {
        self.mask = data;
      }
      // 0x0002 => {} // Status
      // 0x0003 => {} // OAM Address
      // 0x0004 => {} // OAM Data
      // 0x0005 => {} // Scroll
      0x0006 => {
        if self.address_latch == 0 {
          // Write the high byte of address:
          self.address = (self.address & 0x00FF) | ((data as u16) << 8);
          self.address_latch = 1;
        } else {
          // Write the low byte of address:
          self.address = (self.address & 0xFF00) | data as u16;
          self.address_latch = 0;
        }
      }
      0x0007 => {
        self.ppu_write(self.address, data, cart);

        // Auto-increment our address for the next operation if the developer
        // so-chooses:
        self.address += if self.control.increment_mode() { 32 } else { 1 };
      }
      _ => {}
    }

    Some(())
  }
}
