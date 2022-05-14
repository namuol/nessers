use crate::bus_device::{BusDevice, BusDeviceRange};
use crate::cart::{Cart, Mirroring};
use crate::palette::{Color, Palette};

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

  address_latch: bool,

  /// Reading from the PPU usually takes two cycles to fully read our data, so
  /// we need to store the data that was read so that we can return it the next
  /// time we read data:
  data_buffer: u8,

  pub vram_addr: u16,
  pub tram_addr: u16,
  pub fine_x: u8,

  pub status: u8,
  pub mask: u8,
  pub control: u8,

  /// Whether a non-maskable interrupt has been triggered
  pub nmi: bool,

  // Internal state for rendering 8-pixels at a time
  bg_next_tile_id: u8,
  bg_next_tile_attribute: u8,
  bg_next_tile_addr_lsb: u8,
  bg_next_tile_addr_msb: u8,
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

impl SetFlag for u16 {
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
  fn pattern_bg_table(self) -> bool;
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
  fn pattern_bg_table(self) -> bool { (1 << 4) & self != 0 }
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

#[rustfmt::skip]
pub trait LoopyRegister {
  fn coarse_x(self)     -> u8;
  fn coarse_y(self)     -> u8;
  fn nametable_x(self)  -> bool;
  fn nametable_y(self)  -> bool;
  fn fine_y(self)       -> u8;
  fn unused(self)       -> bool;

  fn set_coarse_x(self, v: u8)      -> Self;
  fn set_coarse_y(self, v: u8)      -> Self;
  fn set_nametable_x(self, v: bool) -> Self;
  fn set_nametable_y(self, v: bool) -> Self;
  fn set_fine_y(self, v: u8)        -> Self;
  fn set_unused(self, v: bool)      -> Self;
}

#[rustfmt::skip]
impl LoopyRegister for u16 {
  fn coarse_x(self)     -> u8   { ((0b0_000_0_0_00000_11111 & self) >> 0) as u8 }
  fn coarse_y(self)     -> u8   { ((0b0_000_0_0_11111_00000 & self) >> 5) as u8 }
  fn nametable_x(self)  -> bool { (0b0_000_0_1_00000_00000 & self) != 0 }
  fn nametable_y(self)  -> bool { (0b0_000_1_0_00000_00000 & self) != 0 }
  fn fine_y(self)       -> u8   { ((0b0_111_0_0_00000_00000 & self) >> 12) as u8 }
  fn unused(self)       -> bool { (0b1_000_0_0_00000_00000 & self) != 0 }


  fn set_coarse_x(self, v: u8)     -> Self   { (self & (0b1_111_1_1_11111_00000)) | ((v as u16) << 0) }
  fn set_coarse_y(self, v: u8)     -> Self   { (self & (0b1_111_1_1_00000_11111)) | ((v as u16) << 5) }
  fn set_nametable_x(self, v: bool)  -> Self { self.set(10, v) }
  fn set_nametable_y(self, v: bool)  -> Self { self.set(11, v) }
  fn set_fine_y(self, v: u8)       -> Self   { (self & (0b1_000_1_1_11111_11111)) | ((v as u16) << 12) }
  fn set_unused(self, v: bool)       -> Self { self.set(12, v) }
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
      address_latch: false,

      data_buffer: 0x00,
      vram_addr: 0x0000,
      tram_addr: 0x0000,
      fine_x: 0x00,

      // Registers
      status: 0x00,
      mask: 0x00,
      control: 0x00,

      nmi: false,

      bg_next_tile_id: 0x00,
      bg_next_tile_attribute: 0x00,
      bg_next_tile_addr_lsb: 0x00,
      bg_next_tile_addr_msb: 0x00,
    }
  }

  pub fn clock(&mut self, cart: &Cart) {
    if self.frame_complete {
      self.frame_complete = false;
    }

    // Following this diagram:
    // https://www.nesdev.org/w/images/default/4/4f/Ppu.svg
    //
    // Note: The 0th scanline corresponds to the -1th scanline in our code.
    //
    // Does the 0th "dot" correspond to our -1th cycle or is this also 0?

    let scanline = self.scanline;
    let cycle = self.cycle;
    let cycle_in_tile = (cycle - 1) % 8;
    match (scanline, cycle_in_tile, cycle) {
      (0, _, 0) => {
        // Skipped on BG+odd (what does this mean?)
      }
      (_, _, 0) => {
        // Idle
      }
      (240, _, _) => {
        // Post-render scanline; do nothing!
      }
      (241, _, 1) => {
        // Set VBlank flag
        self.status = self.status.set_vblank(true);
        if self.control.enable_nmi() {
          self.nmi = true;
        }
        if self.mask.render_background() || self.mask.render_sprites() {
          println!("");
        }
      }
      (-1..=239, 1, 1..=256 | 321) => {
        // NT byte
        let tile_addr = 0x2000 | (self.vram_addr & 0x0FFF);

        self.bg_next_tile_id = self.ppu_read(tile_addr, cart);
        if (self.mask.render_background() || self.mask.render_sprites())
          && self.vram_addr.fine_y() == 0
        {
          print!("{:02X} ", self.bg_next_tile_id);
        }
      }
      (-1..=239, 3, _) => {
        // AT byte

        // One day I will break this down into parts that I understand:
        //
        // https://www.nesdev.org/wiki/PPU_scrolling#Tile_and_attribute_fetching
        let attribute_addr = 0x23C0
          | (self.vram_addr & 0x0C00)
          | ((self.vram_addr >> 4) & 0x38)
          | ((self.vram_addr >> 2) & 0x07);
        self.bg_next_tile_attribute = self.ppu_read(attribute_addr, cart)
      }
      (-1..=239, 5 | 7, _) => {
        // Low/High BG tile byte
        let pattern_table_base_addr = if self.control.pattern_bg_table() {
          0x1000
        } else {
          0x0000
        };
        // Offset determines low vs high:
        let offset = if cycle_in_tile == 5 { 0 } else { 8 };
        let addr = pattern_table_base_addr
          + ((self.bg_next_tile_id as u16) << 4)
          + (self.vram_addr.fine_y() as u16)
          + offset;
        let tile_sliver = self.ppu_read(addr, cart);

        if offset == 0 {
          self.bg_next_tile_addr_msb = tile_sliver;
        } else {
          self.bg_next_tile_addr_lsb = tile_sliver;
        }
      }
      (-1..=239, 0, _) => {
        if self.mask.render_background() || self.mask.render_sprites() {
          if self.vram_addr.coarse_x() == 31 {
            self.vram_addr = self
              .vram_addr
              .set_coarse_x(0)
              .set_nametable_x(!self.vram_addr.nametable_x());
          } else {
            self.vram_addr = self.vram_addr.set_coarse_x(self.vram_addr.coarse_x() + 1);
          }
        }
      }
      _ => {}
    }

    if scanline == -1 && cycle == 1 {
      // Clear:
      // - VBlank
      self.status = self.status.set_vblank(false);
      // - Sprite 0
      // - Overflow
    }

    if self.mask.render_background() || self.mask.render_sprites() {
      // Something here causes corruption in donkey kong nametable; looks like
      // tiles are getting shifted somehow:

      if cycle == 256 {
        self.vram_addr = self.vram_addr.set_fine_y((self.vram_addr.fine_y() + 1) % 8);
        if self.vram_addr.fine_y() == 0 {
          println!("");
          self.vram_addr = self
            .vram_addr
            .set_coarse_y((self.vram_addr.coarse_y() + 1) % 30);
        }
      }

      if cycle == 257 {
        self.vram_addr = self
          .vram_addr
          .set_nametable_x(self.tram_addr.nametable_x())
          .set_coarse_x(self.tram_addr.coarse_x());
      }

      if scanline == -1 && cycle >= 280 && cycle <= 304 {
        self.vram_addr = self
          .vram_addr
          .set_nametable_y(self.tram_addr.nametable_y())
          .set_coarse_y(self.tram_addr.coarse_y());
      }
    }

    self.cycle += 1;

    if self.cycle >= 341 {
      self.scanline += 1;
      self.cycle = 0;
    }

    if self.scanline >= 261 {
      self.scanline = -1;
      self.frame_complete = true;
    }

    // Finally, let's draw our pixel at (scanline, cycle)

    match (scanline, cycle) {
      (0..=239, 1..=256) => {
        let screen_x = cycle - 1;
        let screen_y = scanline;
        let idx = (screen_y as usize) * SCREEN_W + (screen_x as usize);
        // println!("{} {}", self.bg_next_tile_addr_msb, self.bg_next_tile_addr_lsb);
        let pixel_color_index = ((self.bg_next_tile_addr_lsb & (1 << cycle_in_tile))
          >> cycle_in_tile)
          + ((self.bg_next_tile_addr_msb & (1 << cycle_in_tile)) >> cycle_in_tile);
        let color = self.get_color_from_palette_ram(0, pixel_color_index, cart);
        self.screen[idx][0] = color.r;
        self.screen[idx][1] = color.g;
        self.screen[idx][2] = color.b;
      }
      _ => {}
    }
  }

  fn get_color_from_palette_ram(&self, palette: u8, pixel: u8, cart: &Cart) -> Color {
    let idx = self.ppu_read(0x3F00 as u16 + ((palette << 2) + pixel) as u16, cart);
    self.palette.colors[idx as usize]
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
        // https://www.nesdev.org/wiki/PPU_scrolling#$2002_read
        //
        // ```
        // w:                  <- 0
        // ```

        // Reading from the status register resets the address latch:
        self.address_latch = false;

        // Reading from the status register, we only care about the top 3 bits,
        // however according to NES lore, the lower 5 bits apparently contain
        // the contents from whatever data was last read from the PPU (which we
        // store as `self.data_buffer`):
        let data = (self.status & 0b111_00000) | (self.data_buffer & 0b000_11111);

        // Reading from the status register clears the vblank flag 🤷‍♂️
        self.status = self.status.set_vblank(false);

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
        self.data_buffer = self.ppu_read(self.vram_addr, cart);

        // Addresses above 0x3F00 are part of the palette memory which can be
        // read right away rather than taking an extra cycle:
        if self.vram_addr > 0x3F00 {
          return Some(self.data_buffer);
        }

        // Auto-increment our address for the next operation if the developer
        // so-chooses:
        self.vram_addr += if self.control.increment_mode() { 32 } else { 1 };

        Some(data)
      }
      _ => Some(0x00),
    }
  }

  // From `cpuWrite` in https://www.youtube.com/watch?v=xdzOvpYPmGE&list=PLrOv9FMX8xJHqMvSGB_9G9nZZ_4IgteYf&index=4
  fn write(&mut self, addr: u16, data: u8, cart: &Cart) -> Option<()> {
    if !self.in_range(addr) {
      return None;
    }

    match addr % 8 {
      // https://www.nesdev.org/wiki/PPU_scrolling#$2000_write
      //
      // ```
      // t: ...GH.. ........ <- d: ......GH
      // <used elsewhere> <- d: ABCDEF..
      // ```
      //
      // The nametable_x/y bits from the control register get copied into the
      // nametable bits of the t-register.
      0x0000 => {
        self.control = data;
        self.tram_addr = self
          .tram_addr
          .set_nametable_x(self.control.nametable_x())
          .set_nametable_y(self.control.nametable_y());
      }
      0x0001 => {
        self.mask = data;
      }
      // 0x0002 => {} // Status
      // 0x0003 => {} // OAM Address
      // 0x0004 => {} // OAM Data
      0x0005 => {
        if self.address_latch == false {
          // https://www.nesdev.org/wiki/PPU_scrolling#$2005_first_write_(w_is_0)
          //
          // ```
          // t: ....... ...ABCDE <- d: ABCDE...
          // x:              FGH <- d: .....FGH
          // w:                  <- 1
          // ```
          self.fine_x = data & 0b0000_0111;
          self.tram_addr.set_coarse_x(data >> 3);
          self.address_latch = true;
        } else {
          // https://www.nesdev.org/wiki/PPU_scrolling#$2005_second_write_(w_is_1)
          //
          // ```
          // t: FGH..AB CDE..... <- d: ABCDEFGH
          // w:                  <- 0
          // ```
          self.tram_addr.set_fine_y(data & 0b0000_0111);
          self.tram_addr.set_coarse_y(data >> 3);
          self.address_latch = false;
        }
      }
      0x0006 => {
        if self.address_latch == false {
          // https://www.nesdev.org/wiki/PPU_scrolling#$2006_first_write_(w_is_0)
          //
          // ```
          // t: .CDEFGH ........ <- d: ..CDEFGH
          //        <unused>     <- d: AB......
          // t: Z...... ........ <- 0 (bit Z is cleared)
          // w:                  <- 1
          // ```
          //
          // Note: `w` here is our `address_latch`

          // Write the high byte of address:
          self.tram_addr = (self.tram_addr & 0x00FF) | ((data as u16) << 8);
          self.address_latch = true;
        } else {
          // https://www.nesdev.org/wiki/PPU_scrolling#$2006_second_write_(w_is_1)
          //
          // ```
          // t: ....... ABCDEFGH <- d: ABCDEFGH
          // v: <...all bits...> <- t: <...all bits...>
          // w:                  <- 0
          // ```
          //
          // Note: `w` here is our `address_latch`
          // Write the low byte of address:
          self.tram_addr = (self.tram_addr & 0xFF00) | data as u16;
          // ...and copy the full address from `t` into `v`:
          self.vram_addr = self.tram_addr;
          self.address_latch = false;
        }
      }
      0x0007 => {
        // https://www.nesdev.org/wiki/PPU_scrolling#$2007_reads_and_writes

        self.ppu_write(self.vram_addr, data, cart);

        // Auto-increment our address for the next operation if the developer
        // so-chooses:
        self.vram_addr += if self.control.increment_mode() { 32 } else { 1 };
      }
      _ => {}
    }

    Some(())
  }

  fn safe_read(&self, _addr: u16, cart: &Cart) -> Option<u8> {
    todo!()
  }
}

#[cfg(test)]
mod tests {
  use crate::ppu::LoopyRegister;
  use pretty_assertions::assert_eq;

  fn assert_eq_binary(left: u8, right: u8, msg: &str) {
    assert_eq!(format!("{:08b}", left), format!("{:08b}", right), "{}", msg);
  }

  #[rustfmt::skip]
  #[test]
  fn loopy() {
    assert_eq_binary((0b0000_0000_0000_0000 as u16).coarse_x(), 0b000_00000, "coarse_x");
    assert_eq_binary((0b0000_0000_0000_0000 as u16).coarse_y(), 0b000_00000, "coarse_y");
    assert_eq!((0b0000_0000_0000_0000 as u16).nametable_x(), false, "nametable_x");
    assert_eq!((0b0000_0000_0000_0000 as u16).nametable_y(), false, "nametable_y");
    assert_eq_binary((0b0000_0000_0000_0000 as u16).fine_y(), 0b00000_000, "fine_y");
    assert_eq!((0b0000_0000_0000_0000 as u16).unused(), false, "unused");

    assert_eq_binary((0b1_111_1_1_11111_01010 as u16).coarse_x(), 0b000_01010, "coarse_x with stuff");
    assert_eq_binary((0b1_111_1_1_10101_11111 as u16).coarse_y(), 0b000_10101, "coarse_y with stuff");
    assert_eq!((0b1_111_1_1_11111_11111 as u16).nametable_x(), true, "nametable_x with stuff");
    assert_eq!((0b1_111_1_0_11111_11111 as u16).nametable_x(), false, "nametable_x with stuff");
    assert_eq!((0b1_111_1_1_11111_11111 as u16).nametable_y(), true, "nametable_y with stuff");
    assert_eq!((0b1_111_0_1_11111_11111 as u16).nametable_y(), false, "nametable_y with stuff");
    assert_eq_binary((0b1_101_1_1_11111_11111 as u16).fine_y(), 0b00000_101, "fine_y with stuff");
    assert_eq!((0b1_111_1_1_11111_11111 as u16).unused(), true, "unused with stuff");
    assert_eq!((0b0_111_1_1_11111_11111 as u16).unused(), false, "unused with stuff");

    assert_eq_binary((0b1_111_1_1_11111_11111 as u16).set_coarse_x(0b111_10101).coarse_x(), 0b000_10101, "coarse_x with stuff 2");
    assert_eq_binary((0b1_111_1_1_11111_11111 as u16).set_coarse_y(0b111_10101).coarse_y(), 0b000_10101, "coarse_y with stuff 2");
    assert_eq!((0b1_111_1_1_11111_11111 as u16).set_nametable_x(true).nametable_x(), true, "nametable_x with stuff 2");
    assert_eq!((0b1_111_1_1_11111_11111 as u16).set_nametable_x(false).nametable_x(), false, "nametable_x with stuff 2");
    assert_eq!((0b1_111_1_1_11111_11111 as u16).set_nametable_y(true).nametable_y(), true, "nametable_y with stuff 2");
    assert_eq!((0b1_111_1_1_11111_11111 as u16).set_nametable_y(false).nametable_y(), false, "nametable_y with stuff 2");
    assert_eq_binary((0b1_111_1_1_11111_11111 as u16).set_fine_y(0b0000_0000).fine_y(), 0b0000_0000, "fine_y with stuff 2");
    assert_eq!((0b1_111_1_1_11111_11111 as u16).set_unused(true).unused(), true, "unused with stuff 2");
    assert_eq!((0b0_111_1_1_11111_11111 as u16).set_unused(false).unused(), false, "unused with stuff 2");
  }
}
