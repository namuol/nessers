use crate::bus::{Bus, DeviceList};
use crate::cart::Cart;
use crate::cpu6502::Processor;
use crate::mirror::Mirror;
use crate::palette::{Color, Palette};
use crate::ppu::Ppu;
use crate::ram::Ram;

pub struct Nes {
  tick: u64,
  pub cpu: Processor,
  pub ppu: Ppu,
  pub cpu_devices: DeviceList,
  pub ppu_devices: DeviceList,
}

impl Nes {
  pub fn new(cart_filename: &str, palette_filename: &str) -> Result<Nes, &'static str> {
    let cart = match Cart::from_file(cart_filename) {
      Ok(c) => c,
      Err(msg) => return Err(msg),
    };

    let ppu = match Palette::from_file(palette_filename) {
      Ok(palette) => Ppu::new(palette),
      Err(msg) => return Err(msg),
    };

    let ppu_registers = Box::new(Ram::new(0x2000, 8));

    let cpu_devices: DeviceList = vec![
      // Cartridge
      Box::new(cart),
      // 2K internal RAM, mirrored to 8K
      Box::new(Mirror::new(
        0x0000,
        Box::new(Ram::new(0x0000, 2 * 1024)),
        8 * 1024,
      )),
      // PPU Registers, mirrored for 8K
      Box::new(Mirror::new(0x2000, ppu_registers, 8 * 1024)),
      // APU & I/O Registers
      Box::new(Ram::new(0x4000, 0x18)),
      // APU & I/O functionality that is normally disabled
      Box::new(Ram::new(0x4018, 0x08)),
    ];

    let ppu_devices: DeviceList = vec![];

    Ok(Nes {
      tick: 0,
      ppu,
      cpu: Processor::new(),
      cpu_devices,
      ppu_devices,
    })
  }

  pub fn clock(&mut self) {
    self.ppu.clock();
    if self.tick % 3 == 0 {
      self.cpu.clock(&mut self.cpu_devices);
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

  pub fn render_pattern_table(&self, table_number: u16, palette: u8) -> [[u8; 4]; 128 * 128] {
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
          let mut tile_lsb = self
            .ppu_devices
            .read(table_number * 0x1000 + offset + row + 0);
          let mut tile_msb = self
            .ppu_devices
            .read(table_number * 0x1000 + offset + row + 8);
          let pixel_y = (tile_y * 8) + row;

          for col in 0..8 {
            // A 2 bit number; 0, 1, or 2
            //
            // To compute this, we can actually just add these two bits
            // together, since the highest the value can be is 2.
            let pixel_color_index = (tile_lsb & 0x01) + (tile_msb & 0x01);
            let color = self.get_color_from_palette_ram(palette, pixel_color_index);

            // For our next column, we just need to look at the _next_ bit in
            // our least/most significant bytes. To achieve this, all we need to
            // do is shift them right one bit:
            tile_lsb >>= 1;
            tile_msb >>= 1;
            // Our pixels are laid out right-to-left in terms of
            // bit-significance, so we _subtract_ our col number from the
            // right-most edge of our tile:
            let pixel_x = (tile_x * 8) + (7 - col);

            let pixel_idx = (pixel_y * 128 + pixel_x) as usize;
            result[pixel_idx][0] = color.r;
            result[pixel_idx][1] = color.g;
            result[pixel_idx][2] = color.b;
          }
        }
      }
    }

    result
  }

  pub fn get_color_from_palette_ram(&self, palette: u8, pixel: u8) -> Color {
    let idx = self
      .ppu_devices
      .read(0x3F00 as u16 + ((palette << 2) + pixel) as u16);
    self.ppu.palette.colors[idx as usize]
  }
}
