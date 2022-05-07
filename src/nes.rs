use crate::bus::Bus;
use crate::bus_device::BusDevice;
use crate::cart::Cart;
use crate::cpu6502::Cpu;
use crate::mirror::Mirror;
use crate::palette::{Color, Palette};
use crate::ppu::Ppu;
use crate::ram::Ram;

pub struct Nes {
  pub cpu: Cpu,
  pub ppu: Ppu,
  tick: u64,
  ram: Ram,
  ram_mirror: Mirror,
  ppu_registers_mirror: Mirror,
  cart: Cart,
}

impl Nes {
  pub fn new(cart_filename: &str, palette_filename: &str) -> Result<Nes, &'static str> {
    let cpu = Cpu::new();

    // 2K internal RAM, mirrored to 8K
    let ram = Ram::new(0x0000, 2 * 1024);
    let ram_mirror = Mirror::new(0x0000, 8 * 1024);

    // PPU Registers, mirrored for 8K
    let ppu = Ppu::new(Palette::from_file(palette_filename)?);
    let ppu_registers_mirror = Mirror::new(0x2000, 8 * 1024);

    let cart = Cart::from_file(cart_filename)?;

    Ok(Nes {
      tick: 0,
      cpu,
      ppu,
      cart,
      ram_mirror,
      ram,
      ppu_registers_mirror,
    })
  }

  pub fn clock(&mut self) {
    self.ppu.clock();
    if self.tick % 3 == 0 {
      // Is there a shorthand way to run a method on a field by cloning it and
      // replacing its value with the cloned object?
      let cpu = &mut self.cpu.clone();
      cpu.clock(self);
      self.cpu = *cpu;
    }

    if self.ppu.nmi {
      self.ppu.nmi = false;
      let cpu = &mut self.cpu.clone();
      cpu.sig_nmi(self);
      self.cpu = *cpu;
    }

    self.tick += 1;
  }

  pub fn step(&mut self) {
    loop {
      self.clock();
      if self.tick % 3 == 1 && self.cpu.cycles_left == 0 {
        return;
      }
    }
  }

  pub fn frame(&mut self) {
    loop {
      self.clock();
      if self.ppu.frame_complete == true {
        return;
      }
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
  pub fn render_pattern_table(&mut self, table_number: u16, palette: u8) -> [[u8; 4]; 128 * 128] {
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
          let mut tile_lsb = self.ppu_read(table_number * 0x1000 + offset + row + 0);
          // Least-significant bit starts at our offset + 8 bytes; one byte per
          // row of pixels in the tile, to skip over the LSB plane:
          let mut tile_msb = self.ppu_read(table_number * 0x1000 + offset + row + 8);

          for col in 0..8 {
            // A 2 bit number; 0, 1, or 2
            //
            // To compute this, we can actually just add these two bits
            // together, since the highest the value can be is 2.
            let pixel_color_index = (tile_lsb & 0x01) + (tile_msb & 0x01);
            let color = self.get_color_from_palette_ram(palette, pixel_color_index);

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

  pub fn get_color_from_palette_ram(&mut self, palette: u8, pixel: u8) -> Color {
    let idx = self.ppu_read(0x3F00 as u16 + ((palette << 2) + pixel) as u16);
    self.ppu.palette.colors[idx as usize]
  }

  pub fn get_palettes(&mut self) -> [[[u8; 4]; 4]; 8] {
    let mut result = [[[0x00, 0x00, 0x00, 0xFF]; 4]; 8];

    for palette_num in 0..8 {
      for color_num in 0..4 {
        let color = self.get_color_from_palette_ram(palette_num, color_num);
        result[palette_num as usize][color_num as usize][0] = color.r;
        result[palette_num as usize][color_num as usize][1] = color.g;
        result[palette_num as usize][color_num as usize][2] = color.b;
        result[palette_num as usize][color_num as usize][3] = 0xFF;
      }
    }

    result
  }

  pub fn reset(&mut self) {
    let cpu = &mut self.cpu.clone();
    cpu.sig_reset(self);
    self.cpu = *cpu;
  }

  // BEGIN ------ Hacky? Helper functions to avoid ugly manual dyn cast -------

  pub fn cpu_read(&mut self, addr: u16) -> u8 {
    (self as &mut dyn Bus<Cpu>).read(addr)
  }

  pub fn cpu_read16(&mut self, addr: u16) -> u16 {
    (self as &mut dyn Bus<Cpu>).read16(addr)
  }

  pub fn ppu_read(&mut self, addr: u16) -> u8 {
    (self as &mut dyn Bus<Ppu>).read(addr)
  }

  pub fn ppu_read16(&mut self, addr: u16) -> u16 {
    (self as &mut dyn Bus<Ppu>).read16(addr)
  }

  // END -------- Hacky? Helper functions to avoid ugly manual dyn cast -------
}

/// The CPU's Bus
impl Bus<Cpu> for Nes {
  fn read(&mut self, addr: u16) -> u8 {
    match None // Hehe, using None here just for formatting purposes:
      .or(self.cart.cpu_mapper.read(addr))
      .or(self.ram_mirror.read(&mut self.ram, addr))
      .or(self.ppu_registers_mirror.read(&mut self.ppu, addr))
    {
      Some(data) => data,
      None => 0x00,
    }
  }

  fn write(&mut self, addr: u16, data: u8) {
    None // Hehe, using None here just for formatting purposes:
      .or_else(|| self.cart.cpu_mapper.write(addr, data))
      .or_else(|| self.ram_mirror.write(&mut self.ram, addr, data))
      .or_else(|| self.ppu_registers_mirror.write(&mut self.ppu, addr, data));
  }
}

/// The PPU's Bus
impl Bus<Ppu> for Nes {
  fn read(&mut self, addr_: u16) -> u8 {
    let addr = addr_ & 0x3FFF;
    match None // Hehe, using None here just for formatting purposes:
      .or(self.cart.ppu_mapper.read(addr))
      .or(Some(self.ppu.ppu_read(addr)))
    {
      Some(data) => data,
      None => 0x00,
    }
  }

  fn write(&mut self, addr_: u16, data: u8) {
    let addr = addr_ & 0x3FFF;

    None // Hehe, using None here just for formatting purposes:
      .or_else(|| self.cart.ppu_mapper.write(addr, data))
      .or_else(|| Some(self.ppu.ppu_write(addr, data)));
  }
}
