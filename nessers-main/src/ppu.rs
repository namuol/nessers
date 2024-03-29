use crate::bus_device::{BusDevice, BusDeviceRange};
use crate::cart::{Cart, Mirroring};
use crate::palette::{Color, Palette};

pub const SCREEN_W: usize = 256;
pub const SCREEN_H: usize = 240;

/// 0b0100_0001 -> 0b1000_0010
fn flip(bits: u8) -> u8 {
  0x00
    | (bits & 0b1000_0000) >> 7
    | (bits & 0b0100_0000) >> 5
    | (bits & 0b0010_0000) >> 3
    | (bits & 0b0001_0000) >> 1
    | (bits & 0b0000_1000) << 1
    | (bits & 0b0000_0100) << 3
    | (bits & 0b0000_0010) << 5
    | (bits & 0b0000_0001) << 7
}

#[derive(Clone)]
pub struct Ppu {
  /// The current row number on the screen
  pub scanline: isize,
  /// The current pixel number on the current scanline
  pub cycle: isize,
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

  // Internal state for rendering 8-pixels of background at a time
  bg_next_tile_id: u8,
  bg_next_tile_attribute: u8,
  bg_next_tile_addr_lsb: u8,
  bg_next_tile_addr_msb: u8,

  bg_shifter_pattern_lo: u16,
  bg_shifter_pattern_hi: u16,
  bg_shifter_attrib_lo: u16,
  bg_shifter_attrib_hi: u16,

  pub oam: [ObjectAttributeEntry; 64],

  oam_addr: u8,

  // For rendering sprites:
  sprites_on_scanline: Vec<ObjectAttributeEntry>,
  sprites_on_scanline_contains_sprite_0: bool,
}

/// A Sprite, basically
#[derive(Clone, Copy)]
pub struct ObjectAttributeEntry {
  /// Y position of the sprite
  pub y: u8,
  /// ID ofthe  tile in pattern memory
  pub tile_id: u8,
  /// Flags that determine how the sprite should be rendered
  pub attribute: u8,
  /// X position of the sprite
  pub x: u8,
}

impl ObjectAttributeEntry {
  /// Whether or not this sprite should be flipped horizontally
  fn flip_x(self) -> bool {
    (self.attribute & 0b0100_0000) != 0
  }

  /// Whether or not this sprite should be flipped vertically
  fn flip_y(self) -> bool {
    (self.attribute & 0b1000_0000) != 0
  }

  /// Palette for the sprite, in absolute terms (always in the range 4 to 7)
  fn palette(self) -> u8 {
    // NES has 8 total palettes; first 4 are for BG rendering, and last 4 are
    // for FG rendering.
    //
    // The palette index is 2 bits (0-4) so we can add 4 to get the absolute
    // palette index:
    4 + (self.attribute & 0b0000_0011)
  }

  /// Rendering priority; true = render sprites above background; false = render
  /// sprites behind.
  fn priority(self) -> bool {
    // The default behavior (bit unset) is to render sprites in front of
    // background, so we compare against zero to determine if sprites are
    // rendered in front of background:
    (self.attribute & 0b0010_0000) == 0
  }
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
  fn grayscale(self)              -> bool { (1 << 0) & self != 0 }
  fn render_background_left(self) -> bool { (1 << 1) & self != 0 }
  fn render_sprites_left(self)    -> bool { (1 << 2) & self != 0 }
  fn render_background(self)      -> bool { (1 << 3) & self != 0 }
  fn render_sprites(self)         -> bool { (1 << 4) & self != 0 }
  fn enhance_red(self)            -> bool { (1 << 5) & self != 0 }
  fn enhance_green(self)          -> bool { (1 << 6) & self != 0 }
  fn enhance_blue(self)           -> bool { (1 << 7) & self != 0 }

  fn set_grayscale(self, v: bool)              -> u8 { self.set(0, v) }
  fn set_render_background_left(self, v: bool) -> u8 { self.set(1, v) }
  fn set_render_sprites_left(self, v: bool)    -> u8 { self.set(2, v) }
  fn set_render_background(self, v: bool)      -> u8 { self.set(3, v) }
  fn set_render_sprites(self, v: bool)         -> u8 { self.set(4, v) }
  fn set_enhance_red(self, v: bool)            -> u8 { self.set(5, v) }
  fn set_enhance_green(self, v: bool)          -> u8 { self.set(6, v) }
  fn set_enhance_blue(self, v: bool)           -> u8 { self.set(7, v) }
}

pub trait ControlRegister {
  fn nametable_x(self) -> bool;
  fn nametable_y(self) -> bool;
  fn increment_mode(self) -> bool;
  fn pattern_fg_table(self) -> bool;
  fn pattern_bg_table(self) -> bool;
  fn tall_sprites(self) -> bool;
  fn slave_mode(self) -> bool;
  fn enable_nmi(self) -> bool;

  fn set_nametable_x(self, v: bool) -> Self;
  fn set_nametable_y(self, v: bool) -> Self;
  fn set_increment_mode(self, v: bool) -> Self;
  fn set_pattern_fg_table(self, v: bool) -> Self;
  fn set_pattern_background(self, v: bool) -> Self;
  fn set_tall_sprites(self, v: bool) -> Self;
  fn set_slave_mode(self, v: bool) -> Self;
  fn set_enable_nmi(self, v: bool) -> Self;
}

#[rustfmt::skip]
impl ControlRegister for u8 {
  fn nametable_x(self)        -> bool { (1 << 0) & self != 0 }
  fn nametable_y(self)        -> bool { (1 << 1) & self != 0 }
  fn increment_mode(self)     -> bool { (1 << 2) & self != 0 }
  fn pattern_fg_table(self)     -> bool { (1 << 3) & self != 0 }
  fn pattern_bg_table(self)   -> bool { (1 << 4) & self != 0 }
  fn tall_sprites(self)        -> bool { (1 << 5) & self != 0 }
  fn slave_mode(self)         -> bool { (1 << 6) & self != 0 }
  fn enable_nmi(self)         -> bool { (1 << 7) & self != 0 }

  fn set_nametable_x(self, v: bool)         -> u8 { self.set(0, v) }
  fn set_nametable_y(self, v: bool)         -> u8 { self.set(1, v) }
  fn set_increment_mode(self, v: bool)      -> u8 { self.set(2, v) }
  fn set_pattern_fg_table(self, v: bool)      -> u8 { self.set(3, v) }
  fn set_pattern_background(self, v: bool)  -> u8 { self.set(4, v) }
  fn set_tall_sprites(self, v: bool)         -> u8 { self.set(5, v) }
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
  fn set_unused(self, v: bool)       -> Self { self.set(15, v) }
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
      screen: [[0xFF, 0x00, 0xFF, 0xFF]; SCREEN_W * SCREEN_H],

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

      bg_shifter_pattern_lo: 0x0000,
      bg_shifter_pattern_hi: 0x0000,
      bg_shifter_attrib_lo: 0x0000,
      bg_shifter_attrib_hi: 0x0000,

      oam: [ObjectAttributeEntry {
        y: 0x00,
        tile_id: 0x00,
        attribute: 0x00,
        x: 0x00,
      }; 64],

      oam_addr: 0x00,

      sprites_on_scanline: vec![],
      sprites_on_scanline_contains_sprite_0: false,
    }
  }

  fn load_shift_registers(&mut self) {
    // Load shift registers:
    self.bg_shifter_pattern_lo =
      (self.bg_shifter_pattern_lo & 0xFF00) | (self.bg_next_tile_addr_lsb as u16);
    self.bg_shifter_pattern_hi =
      (self.bg_shifter_pattern_hi & 0xFF00) | (self.bg_next_tile_addr_msb as u16);

    self.bg_shifter_attrib_lo = (self.bg_shifter_attrib_lo & 0xFF00)
      | (if (self.bg_next_tile_attribute & 0b01) != 0 {
        0xFF
      } else {
        0x00
      });
    self.bg_shifter_attrib_hi = (self.bg_shifter_attrib_hi & 0xFF00)
      | (if (self.bg_next_tile_attribute & 0b10) != 0 {
        0xFF
      } else {
        0x00
      });
  }

  fn increment_scroll_y(&mut self) {
    if !(self.mask.render_background() || self.mask.render_sprites()) {
      return;
    }

    if self.vram_addr.fine_y() < 7 {
      self.vram_addr = self.vram_addr.set_fine_y(self.vram_addr.fine_y() + 1);
    } else {
      // If we cross 8 scanlines we're entering the next tile vertically, so
      // reset the fine_y offset:
      self.vram_addr = self.vram_addr.set_fine_y(0);

      // ...and determine what our coarse y should be, accounting for the
      // need to swap nametable space and to avoid entering the attribute
      // rows of our nametable (rows 30 and 31; zero-indexed):
      match self.vram_addr.coarse_y() {
        29 => {
          // Our nametables have a height of 30 (0 thru 29), so if coarse y
          // is 29 it means we need to swap the vertical nametables and
          // reset coarse y:
          self.vram_addr = self
            .vram_addr
            .set_coarse_y(0)
            .set_nametable_y(!self.vram_addr.nametable_y());
        }
        31 => {
          // If rendering is enabled and we're entering attribute memory
          // space, wrap around to the top:
          self.vram_addr = self.vram_addr.set_coarse_y(0);
        }
        _ => {
          // Otherwise just increment coarse y as usual:
          self.vram_addr = self.vram_addr.set_coarse_y(self.vram_addr.coarse_y() + 1);
        }
      }
    }
  }

  fn transfer_address_x(&mut self) {
    if !(self.mask.render_background() || self.mask.render_sprites()) {
      return;
    }

    self.vram_addr = self
      .vram_addr
      .set_nametable_x(self.tram_addr.nametable_x())
      .set_coarse_x(self.tram_addr.coarse_x());
  }

  fn transfer_address_y(&mut self) {
    if !(self.mask.render_background() || self.mask.render_sprites()) {
      return;
    }

    self.vram_addr = self
      .vram_addr
      .set_nametable_y(self.tram_addr.nametable_y())
      .set_coarse_y(self.tram_addr.coarse_y())
      .set_fine_y(self.tram_addr.fine_y());
  }

  pub fn clock(&mut self, cart: &mut Cart) {
    if self.frame_complete {
      self.frame_complete = false;
    }

    if self.scanline >= -1 && self.scanline < 240 {
      if self.scanline == 0 && self.cycle == 0 {
        // "Odd frame"
        self.cycle = 1;
      }

      // Following this diagram:
      // https://www.nesdev.org/w/images/default/4/4f/Ppu.svg
      //
      // Note: The 0th scanline corresponds to the -1th scanline in our code.
      //
      // Does the 0th "dot" correspond to our -1th cycle or is this also 0?
      let cycle_in_tile = (self.cycle - 1).rem_euclid(8);

      if self.scanline == -1 && self.cycle == 1 {
        // Clear:
        // - VBlank
        // - Sprite 0
        // - Overflow
        self.status = self
          .status
          .set_vblank(false)
          .set_sprite_zero_hit(false)
          .set_sprite_overflow(false);
      }

      if (self.cycle >= 2 && self.cycle < 258) || (self.cycle >= 321 && self.cycle < 338) {
        if self.mask.render_background() {
          // Shifting background tile pattern row
          self.bg_shifter_pattern_lo <<= 1;
          self.bg_shifter_pattern_hi <<= 1;

          self.bg_shifter_attrib_lo <<= 1;
          self.bg_shifter_attrib_hi <<= 1;
        }

        match cycle_in_tile {
          // (0, _, 0) => {
          //   // Skipped on BG+odd (what does this mean?)
          //   self.cycle = 1;
          // }
          // (240, _, _) => {
          //   // Post-render scanline; do nothing!
          // }
          0 => {
            self.load_shift_registers();

            // NT byte
            let tile_addr = 0x2000 | (self.vram_addr & 0x0FFF);
            self.bg_next_tile_id = self.ppu_read(tile_addr, cart);
          }
          2 => {
            // AT byte

            // One day I will break this down into parts that I understand:
            //
            // https://www.nesdev.org/wiki/PPU_scrolling#Tile_and_attribute_fetching
            let attribute_addr = 0x23C0
              | (self.vram_addr & 0x0C00)
              | ((self.vram_addr >> 4) & 0x38)
              | ((self.vram_addr >> 2) & 0x07);
            self.bg_next_tile_attribute = self.ppu_read(attribute_addr, cart);

            if (self.vram_addr.coarse_y() & 0x02) != 0 {
              self.bg_next_tile_attribute >>= 4;
            }
            if (self.vram_addr.coarse_x() & 0x02) != 0 {
              self.bg_next_tile_attribute >>= 2;
            }
            self.bg_next_tile_attribute &= 0x03;
          }
          4 | 6 => {
            // Low/High BG tile byte
            let base_addr = ((self.control.pattern_bg_table() as u16) * 0x1000)
              + ((self.bg_next_tile_id as u16) << 4)
              + (self.vram_addr.fine_y() as u16);

            if cycle_in_tile == 4 {
              self.bg_next_tile_addr_lsb = self.ppu_read(base_addr + 0, cart);
            } else {
              self.bg_next_tile_addr_msb = self.ppu_read(base_addr + 8, cart);
            }
          }
          7 => {
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
      }

      if self.cycle == 256 {
        self.increment_scroll_y();
      }

      if self.cycle == 257 {
        self.load_shift_registers();
        self.transfer_address_x();
      }

      // Superfluous reads of tile id at end of scanline
      if self.cycle == 338 || self.cycle == 340 {
        let tile_addr = 0x2000 | (self.vram_addr & 0x0FFF);
        self.bg_next_tile_id = self.ppu_read(tile_addr, cart);
      }

      if self.scanline == -1 && self.cycle >= 280 && self.cycle <= 304 {
        self.transfer_address_y();
      }

      // Foreground sprite "evaluation"
      if self.cycle == 257 && self.scanline >= 0 {
        let scanline = self.scanline as i16;
        let sprite_height = if self.control.tall_sprites() { 16 } else { 8 };

        self.sprites_on_scanline.clear();
        self.sprites_on_scanline_contains_sprite_0 = false;
        // Determine which sprites will be visible on our scanline; we only draw
        // the first 8 that appear in the order of our OAM.
        for i in 0..self.oam.len() {
          let sprite = self.oam[i];
          let y_diff = scanline - (sprite.y as i16);

          // First determine whether the sprite is within our Y range:
          if !(y_diff >= 0 && y_diff < sprite_height) {
            continue;
          }

          if self.sprites_on_scanline.len() < 8 {
            if i == 0 {
              self.sprites_on_scanline_contains_sprite_0 = true;
            }
            self.sprites_on_scanline.push(sprite);
          }

          if self.sprites_on_scanline.len() >= 8 {
            self.status = self.status.set_sprite_overflow(true);
            break;
          }
        }
      }
    }

    if self.scanline == 240 {
      // Post-render scanline; do nothing
    }

    // VBlank period:
    if self.scanline >= 241 && self.scanline < 261 {
      // Start of VBlank:
      if self.scanline == 241 && self.cycle == 1 {
        self.status = self.status.set_vblank(true);
        if self.control.enable_nmi() {
          self.nmi = true;
        }
      }
    }

    if self.cycle >= 1 && self.cycle <= 256 && self.scanline >= 0 && self.scanline <= 239 {
      let mut fg_pixel: u8 = 0x00;
      let mut fg_palette: u8 = 0x00;
      let mut fg_priority: bool = false;
      let mut fg_sprite_0_hit: bool = false;
      if self.mask.render_sprites()
        && !(self.mask.render_sprites_left() == false && self.cycle >= 1 && self.cycle <= 8)
      {
        // First determine which sprite pixel (if any) we need to render:

        for i in 0..self.sprites_on_scanline.len() {
          let sprite = self.sprites_on_scanline[i];
          let x_diff = (self.cycle as i16) - (sprite.x as i16) - 1;
          if !(x_diff >= 0 && x_diff < 8) {
            continue;
          }
          let y_diff = (self.scanline as i16) - (sprite.y as i16) - 1;

          // Table number to get our sprite graphics from.
          // false = 0; true = 1
          let table: bool;
          let tile_id: u8;
          match self.control.tall_sprites() {
            // When we're working with tall sprites, each sprite takes up 2x the
            // space, so we can only refer to 128 sprites per table instead of
            // the usual 256.
            //
            // Rather than being limited to a single pattern table for sprites,
            // we can use the unused bit in our tile ID byte to select which
            // pattern table we want our sprite to be from.
            //
            // The NES designers use the least significant bit of our tile ID
            // byte for this purpose.
            true => {
              table = (sprite.tile_id & 0b0000_0001) != 0;
              // The tile_id
              tile_id = if y_diff < 8 {
                // Top 8x8 of the 8x16 sprite:
                (sprite.tile_id & 0b1111_1110) + if sprite.flip_y() { 1 } else { 0 }
              } else {
                // Bottom 8x8 of the 8x16 sprite; effectively one full row down:
                (sprite.tile_id & 0b1111_1110) + if sprite.flip_y() { 0 } else { 1 }
              };
            }
            // Otherwise all sprites share the same table, controlled with a
            // flag in the control register:
            false => {
              table = self.control.pattern_fg_table();
              tile_id = sprite.tile_id;
            }
          };

          // Low/High tile byte
          let base_addr = ((table as u16) << 12)
            | ((tile_id as u16) << 4)
            | (((if sprite.flip_y() { 7 - y_diff } else { y_diff }) as u16) & 0x0007);

          // We can shift our bit-fields by our x-diff so the most significant
          // bit is the current pixel value:
          let mut tile_lsb = self.ppu_read(base_addr + 0, cart);
          let mut tile_msb = self.ppu_read(base_addr + 8, cart);
          if sprite.flip_x() {
            tile_lsb = flip(tile_lsb);
            tile_msb = flip(tile_msb);
          }
          tile_lsb <<= x_diff;
          tile_msb <<= x_diff;
          let p0_pixel = ((tile_lsb & 0b1000_0000) > 0) as u8;
          let p1_pixel = ((tile_msb & 0b1000_0000) > 0) as u8;
          let pixel = (p1_pixel << 1) | p0_pixel;
          if pixel != 0x00 {
            fg_pixel = pixel;
            fg_palette = sprite.palette();
            fg_priority = sprite.priority();
            if i == 0 && self.sprites_on_scanline_contains_sprite_0 {
              fg_sprite_0_hit = true;
            }
            break;
          }
        }
      }

      let mut bg_pixel: u8 = 0x00;
      let mut bg_palette: u8 = 0x00;
      if self.mask.render_background()
        && !(self.mask.render_background_left() == false && self.cycle >= 0 && self.cycle <= 8)
      {
        let bit_mux: u16 = 0x8000 >> self.fine_x;
        let p0_pixel = ((self.bg_shifter_pattern_lo & bit_mux) > 0) as u8;
        let p1_pixel = ((self.bg_shifter_pattern_hi & bit_mux) > 0) as u8;
        bg_pixel = (p1_pixel << 1) | p0_pixel;

        let bg_pal0 = ((self.bg_shifter_attrib_lo & bit_mux) > 0) as u8;
        let bg_pal1 = ((self.bg_shifter_attrib_hi & bit_mux) > 0) as u8;
        bg_palette = (bg_pal1 << 1) | bg_pal0;
      }

      let mut pixel = 0x00;
      let mut palette = 0x00;
      match (bg_pixel, fg_pixel) {
        (0, 0) => {} // Nothing to render
        // Only foreground to render:
        (0, 1..) => {
          pixel = fg_pixel;
          palette = fg_palette;
        }
        // Only background to render:
        (1.., 0) => {
          pixel = bg_pixel;
          palette = bg_palette;
        }
        // Could render either...
        (1.., 1..) => {
          // Here is where we need to do sprite zero hit detection.
          //
          // https://www.nesdev.org/wiki/PPU_OAM#Sprite_zero_hits
          //
          // Sprite 0 hit does not happen:
          // - If background or sprite rendering is disabled in PPUMASK ($2001)
          // - At x=0 to x=7 if the left-side clipping window is enabled (if bit
          //   2 or bit 1 of PPUMASK is 0).
          // - At x=255, for an obscure reason related to the pixel pipeline.
          // - At any pixel where the background or sprite pixel is transparent
          //   (2-bit color index from the CHR pattern is %00).
          // - If sprite 0 hit has already occurred this frame. Bit 6 of
          //   PPUSTATUS ($2002) is cleared to 0 at dot 1 of the pre-render
          //   line. This means only the first sprite 0 hit in a frame can be
          //   detected.

          if fg_sprite_0_hit && self.cycle != 256 {
            self.status = self.status.set_sprite_zero_hit(true);
          }

          // ...so determine this by our fg_priority flag:
          if fg_priority {
            pixel = fg_pixel;
            palette = fg_palette;
          } else {
            pixel = bg_pixel;
            palette = bg_palette;
          }
        }
      };

      // Finally, let's draw our pixel at (scanline, cycle)
      let screen_x = self.cycle - 1;
      let screen_y = self.scanline;
      let idx = (screen_y as usize) * SCREEN_W + (screen_x as usize);
      let color = self.get_color_from_palette_ram(palette, pixel, cart);
      self.screen[idx][0] = color.r;
      self.screen[idx][1] = color.g;
      self.screen[idx][2] = color.b;
    }

    self.cycle += 1;

    if (self.mask.render_background() || self.mask.render_sprites())
      && self.cycle == 260
      && self.scanline < 240
    {
      cart.mapper.scanline_complete();
    }

    if self.cycle >= 341 {
      self.cycle = 0;
      self.scanline += 1;
      if self.scanline >= 261 {
        self.scanline = -1;
        self.frame_complete = true;
      }
    }
  }

  fn get_color_from_palette_ram(&self, palette: u8, pixel: u8, cart: &mut Cart) -> Color {
    let idx = self.ppu_read(0x3F00 as u16 + ((palette << 2) + pixel) as u16, cart);
    self.palette.colors[(idx % 64) as usize]
  }

  fn get_oam_data(&self) -> u8 {
    // Each OAM entry is 4 bytes long, so our OAM address needs to be divided by
    // four to determine which index into our OAM array we need to read from.
    let oam_entry = self.oam[(self.oam_addr as usize) / 4];

    // The remainder determines which piece of data we need to read from our OAM
    // entry:
    match self.oam_addr % 4 {
      0 => oam_entry.y,
      1 => oam_entry.tile_id,
      2 => oam_entry.attribute,
      3 => oam_entry.x,
      _ => 0x00, // Unreachable
    }
  }

  pub fn set_oam_data(&mut self, oam_addr: u8, data: u8) {
    // Each OAM entry is 4 bytes long, so our OAM address needs to be divided by
    // four to determine which index into our OAM array we need to read from.
    let idx = (oam_addr as usize) / 4;

    // The remainder determines which piece of data we need to read from our OAM
    // entry:
    match oam_addr % 4 {
      0 => self.oam[idx].y = data,
      1 => self.oam[idx].tile_id = data,
      2 => self.oam[idx].attribute = data,
      3 => self.oam[idx].x = data,
      _ => {} // Unreachable
    };
  }

  pub fn oam_trace(&self) -> String {
    let mut i = 0;
    let mut string = String::new();
    for sprite in self.oam {
      string.push_str(&format!(
        "{:02} {:02X} {:03},{:03} {:02X}\n",
        i, sprite.tile_id, sprite.x, sprite.y, sprite.attribute
      ));
      i += 1;
    }
    string
  }

  #[allow(unused_comparisons)]
  pub fn ppu_read(&self, addr_: u16, cart: &mut Cart) -> u8 {
    let mut addr = addr_ & 0x3FFF;

    match cart.ppu_read(addr) {
      Some(data) => {
        return data;
      }
      None => {}
    };

    if addr >= 0x0000 && addr <= 0x1FFF {
      // 0x0000 -> 0x1FFF = pattern memory
      return self.pattern_tables[((addr & 0x1000) >> 12) as usize][(addr & 0x0FFF) as usize];
    } else if addr >= 0x2000 && addr <= 0x3EFF {
      // 0x2000 -> 0x3EFF = nametable memory
      addr &= 0x0FFF;
      let table = match cart.mirroring() {
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
        Mirroring::OneScreenLo => 0,
        Mirroring::OneScreenHi => 1,
      };
      let idx = (addr & 0x03FF) as usize;

      return self.name_tables[table][idx];
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
  pub fn ppu_write(&mut self, addr_: u16, data: u8, cart: &mut Cart) {
    let mut addr = addr_ & 0x3FFF;

    match cart.ppu_write(addr, data) {
      Some(()) => {
        return;
      }
      None => {}
    };

    if addr >= 0x0000 && addr <= 0x1FFF {
      // 0x0000 -> 0x1FFF = pattern memory
      self.pattern_tables[((addr & 0x1000) >> 12) as usize][(addr & 0x0FFF) as usize] = data;
      return;
    } else if addr >= 0x2000 && addr <= 0x3EFF {
      // 0x2000 -> 0x3EFF = nametable memory
      addr &= 0x0FFF;
      let table = match cart.mirroring() {
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
        Mirroring::OneScreenLo => 0,
        Mirroring::OneScreenHi => 1,
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

  /// In order to render the pattern table, we need to understand how pattern
  /// data is structured in memory.
  ///
  /// On the NES, each pixel of a sprite is 2 bits, allowing up to 4 unique
  /// colors (including 0 which is always "transparent").
  ///
  /// A tile consists of an 8x8 grid of 2-bit pixels.
  ///
  /// It can help to picture a tile as something like this:
  ///
  /// ```
  /// 0, 1, 2, 3, 3, 2, 1, 0
  /// ...7 more rows like this...
  /// ```
  ///
  /// You might at first assume that these pixels are stored in the following
  /// way in memory (it'll be clear why I'm using binary notation here later):
  ///
  /// ```
  /// 0,    1,    2,    3,    3,    2,    1,    0
  /// 0b00, 0b01, 0b10, 0b11, 0b11, 0b10, 0b01, 0b00
  /// ...7 more rows like this...
  /// ```
  ///
  /// Written in _bytes_ (the unit we're used to reading one at a time) this
  /// would look like this:
  ///
  /// ```
  ///   0,1,2,3,    3,2,1,0
  /// 0b00011011, 0b11100100
  /// ...7 more rows like this...
  /// ```
  ///
  /// So in this form, a tile would be a sequence of 64 * 2-bit pixels, or 128
  /// bits = 16 bytes.
  ///
  /// This might seem fine and intuitive, until you actually go to _read_ the
  /// pixel at a specific coordinate within the data.
  ///
  /// For instance, let's say we wanted to get the pixel at x=3, y=3.
  ///
  /// We would first need to determine which _byte_ to read since I can only
  /// read one byte at a time.
  ///
  /// Then we'd need to perform some bit-operations on the byte to mask out the
  /// bits that aren't important to us, and then _finally_ we'd need to _shift_
  /// the bits such that only the 2-bit pixel we care about is selected.
  ///
  /// There's a better way: Bit-planes!
  ///
  /// Since our pixels are 2 bits each, we can _split_ our 8x8 2-bit grid in
  /// half such that the 8 bytes correspond to the _least significant bit_ of
  /// each of the 8x8=64 bits in the tile, and the next 8x8=64 bits correspond
  /// to the _most significant bit_ of each pixel in the tile.
  ///
  /// Concretely, the first 8 pixels (`0, 1, 2, 3, 3, 2, 1, 0`) could be
  /// represented like this in the pattern table memory:
  ///
  /// ```
  ///       2-bit number:  0   1   2   3   3   2   1   0
  ///      binary number: 00  01  10  11  11  10  01  00
  /// lsb (offset by  0):  0,  1,  0,  1,  1,  0,  1,  0
  ///                     ...rest of the lsb tile rows...
  /// msb (offset by 64): 0 , 0 , 1 , 1 , 1 , 1 , 0 , 0
  ///                     ...rest of the msb tile rows...
  /// ```
  ///
  /// So now if we want to render a tile, we can simply read two bytes at a time
  /// for each 8-pixel wide row of pixels, and to determine the 2-bit color of
  /// each column, we can mask all but the last bit from each byte we read and
  /// add them together appropriately (0b0<lsb> & 0b<msb>0, or more easily
  /// 0x0<lsb> + 0x0<msb>) to get our 2-bit color palette index.
  ///
  /// Whew!
  pub fn render_pattern_table(
    &mut self,
    table_number: u16,
    palette: u8,
    cart: &mut Cart,
  ) -> [[u8; 4]; 128 * 128] {
    let mut result = [[0x00, 0x00, 0x00, 0xFF]; 128 * 128];
    // We want to render 16x16 tiles
    for tile_y in 0..16 {
      for tile_x in 0..16 {
        // Even though a tile is 8x8, each tile is actually 16 bits "wide",
        // because each pixel takes up 2 bits. Our tile sheet is 16 tiles wide,
        // hence the y * (16*16).
        let offset = tile_y * (16 * 16) + tile_x * 16;

        // Each tile is 8x8 pixels
        for row in 0..8 {
          // A full pattern table is 4KB, or 0x1000

          // Least-significant bit starts at our offset + 0 bytes:
          let mut tile_lsb = self.ppu_read(table_number * 0x1000 + offset + row + 0, cart);
          // Least-significant bit starts at our offset + 8 bytes; one byte per
          // row of pixels in the tile, to skip over the LSB plane:
          let mut tile_msb = self.ppu_read(table_number * 0x1000 + offset + row + 8, cart);

          for col in 0..8 {
            // A 2 bit number; 0, 1, or 2
            //
            // To compute this, we can actually just add these two bits
            // together, since the highest the value can be is 2.
            let pixel_color_index = (tile_lsb & 0x01) + (tile_msb & 0x01);
            let color = self.get_color_from_palette_ram(palette, pixel_color_index, cart);

            // Our pixels are laid out right-to-left in terms of
            // bit-significance, so we _subtract_ our col number from the
            // right-most edge of our tile:
            let pixel_x = (tile_x * 8) + (7 - col);
            let pixel_y = (tile_y * 8) + row;
            let pixel_idx = (pixel_y * 128 + pixel_x) as usize;
            result[pixel_idx][0] = color.r;
            result[pixel_idx][1] = color.g;
            result[pixel_idx][2] = color.b;

            // For our next column, we just need to look at the _next_ bit in
            // our least/most significant bytes. To achieve this, all we need to
            // do is shift them right one bit:
            tile_lsb >>= 1;
            tile_msb >>= 1;
          }
        }
      }
    }

    result
  }

  pub fn render_name_table(
    &mut self,
    pattern_table: &[[u8; 4]; 128 * 128],
    name_table_idx: usize,
  ) -> [[u8; 4]; 256 * 240] {
    let mut result = [[0x00, 0x00, 0x00, 0xFF]; 256 * 240];
    for y in 0..30 {
      for x in 0..32 {
        let tile = self.name_tables[name_table_idx][y * 32 + x];
        // 0x00 => tile_y = 0, tile_x = 0
        // 0x01 => tile_y = 0, tile_x = 1
        // 0xA5 => tile_y = A, tile_x = 5
        let tile_y = ((tile & 0xF0) >> 4) as usize;
        let tile_x = (tile & 0x0F) as usize;
        for row in 0..8 {
          for col in 0..8 {
            let pt_pixel_x = (tile_x * 8) + (7 - col);
            let pt_pixel_y = (tile_y * 8) + row;
            let pt_pixel_idx = (pt_pixel_y * 128 + pt_pixel_x) as usize;

            let pixel_x = (x * 8) + (7 - col);
            let pixel_y = (y * 8) + row;
            let pixel_idx = (pixel_y * 256 + pixel_x) as usize;
            result[pixel_idx] = pattern_table[pt_pixel_idx];
          }
        }
      }
    }

    result
  }

  pub fn get_palettes(&mut self, cart: &mut Cart) -> [[[u8; 4]; 4]; 8] {
    let mut result = [[[0x00, 0x00, 0x00, 0xFF]; 4]; 8];

    for palette_num in 0..8 {
      for color_num in 0..4 {
        let color = self.get_color_from_palette_ram(palette_num, color_num, cart);
        result[palette_num as usize][color_num as usize][0] = color.r;
        result[palette_num as usize][color_num as usize][1] = color.g;
        result[palette_num as usize][color_num as usize][2] = color.b;
        result[palette_num as usize][color_num as usize][3] = 0xFF;
      }
    }

    result
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
  fn read(&mut self, addr: u16, cart: &mut Cart) -> Option<u8> {
    if !self.in_range(addr) {
      return None;
    }

    match addr % 8 {
      // 0x0000 => Control register is not readable
      // 0x0001 => Mask register is not readable
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
      // 0x0003 => {}, // OAM Address
      0x0004 => Some(self.get_oam_data()),
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
  fn write(&mut self, addr: u16, data: u8, cart: &mut Cart) -> Option<()> {
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
      0x0003 => self.oam_addr = data,
      0x0004 => self.set_oam_data(self.oam_addr, data),
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
          self.tram_addr = self.tram_addr.set_coarse_x(data >> 3);
          self.address_latch = true;
        } else {
          // https://www.nesdev.org/wiki/PPU_scrolling#$2005_second_write_(w_is_1)
          //
          // ```
          // t: FGH..AB CDE..... <- d: ABCDEFGH
          // w:                  <- 0
          // ```
          self.tram_addr = self
            .tram_addr
            .set_fine_y(data & 0b0000_0111)
            .set_coarse_y(data >> 3);
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

  fn safe_read(&self, _addr: u16, _cart: &Cart) -> Option<u8> {
    todo!()
  }
}

#[cfg(test)]
mod tests {
  use crate::{palette::Palette, ppu::LoopyRegister};
  use pretty_assertions::assert_eq;

  use super::{ObjectAttributeEntry, Ppu};

  fn assert_eq_binary<T: std::fmt::Binary>(left: T, right: T, msg: &str) {
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

    assert_eq_binary((0b1_111_1_1_11111_11111 as u16).set_coarse_x(0b111_10101), 0b1_111_1_1_11111_10101, "coarse_x with stuff 2");
    assert_eq_binary((0b1_111_1_1_11111_11111 as u16).set_coarse_y(0b111_10101), 0b1_111_1_1_10101_11111, "coarse_y with stuff 2");
    assert_eq_binary((0b1_111_1_0_11111_11111 as u16).set_nametable_x(true), 0b1_111_1_1_11111_11111, "nametable_x with stuff 2");
    assert_eq_binary((0b1_111_1_1_11111_11111 as u16).set_nametable_x(false), 0b1_111_1_0_11111_11111, "nametable_x with stuff 2");
    assert_eq_binary((0b1_111_0_1_11111_11111 as u16).set_nametable_y(true), 0b1_111_1_1_11111_11111, "nametable_y with stuff 2");
    assert_eq_binary((0b1_111_1_1_11111_11111 as u16).set_nametable_y(false), 0b1_111_0_1_11111_11111, "nametable_y with stuff 2");
    assert_eq_binary((0b1_111_1_1_11111_11111 as u16).set_fine_y(0b00000_101), 0b1_101_1_1_11111_11111, "fine_y with stuff 2");
    assert_eq_binary((0b1_111_1_1_11111_11111 as u16).set_unused(true), 0b1_111_1_1_11111_11111, "unused with stuff 2");
    assert_eq_binary((0b0_111_1_1_11111_11111 as u16).set_unused(false), 0b0_111_1_1_11111_11111, "unused with stuff 2");
  }

  #[test]
  fn get_oam_data() {
    let mut ppu = Ppu::new(Palette::new());
    let oam_entry = ObjectAttributeEntry {
      y: 42,
      tile_id: 43,
      attribute: 44,
      x: 45,
    };

    let idx = 42;
    ppu.oam[idx as usize] = oam_entry;

    // Before; should be all zeroes
    ppu.oam_addr = idx * 4 - 1;
    assert_eq!(ppu.get_oam_data(), 0x00);

    ppu.oam_addr = idx * 4 + 0;
    assert_eq!(ppu.get_oam_data(), oam_entry.y);

    ppu.oam_addr = idx * 4 + 1;
    assert_eq!(ppu.get_oam_data(), oam_entry.tile_id);

    ppu.oam_addr = idx * 4 + 2;
    assert_eq!(ppu.get_oam_data(), oam_entry.attribute);

    ppu.oam_addr = idx * 4 + 3;
    assert_eq!(ppu.get_oam_data(), oam_entry.x);

    // After; should be all zeroes
    ppu.oam_addr = idx * 4 + 4;
    assert_eq!(ppu.get_oam_data(), 0x00);
  }

  #[test]
  fn set_oam_data() {
    let mut ppu = Ppu::new(Palette::new());

    let idx = 42;

    ppu.set_oam_data(idx * 4 + 0, 42);
    assert_eq!(ppu.oam[idx as usize].y, 42);

    ppu.set_oam_data(idx * 4 + 1, 43);
    assert_eq!(ppu.oam[idx as usize].tile_id, 43);

    ppu.set_oam_data(idx * 4 + 2, 44);
    assert_eq!(ppu.oam[idx as usize].attribute, 44);

    ppu.set_oam_data(idx * 4 + 3, 45);
    assert_eq!(ppu.oam[idx as usize].x, 45);

    // Looping over to the next item...
    ppu.set_oam_data(idx * 4 + 4, 46);
    assert_eq!(ppu.oam[idx as usize + 1].y, 46);

    ppu.set_oam_data(idx * 4 + 5, 47);
    assert_eq!(ppu.oam[idx as usize + 1].tile_id, 47);
  }
}
