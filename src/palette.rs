use std::fs;

use crate::bus_device::{BusDevice, BusDeviceRange};

/// 24-bit sRGB color
#[derive(Clone, Copy)]
pub struct Color {
  pub r: u8,
  pub g: u8,
  pub b: u8,
}

/// NES color palette
#[derive(Clone)]
pub struct Palette {
  // The SRGB colors that the NES is capable of displaying.
  pub colors: [Color; 64],
  // The actual "live" palette of colors; each `u8` in the array is an index
  // into the `colors` array.
  pub map: [u8; 32],
}

impl Palette {
  pub fn from_file(filename: &str) -> Result<Palette, &'static str> {
    let contents = fs::read(filename).expect(&format!("Failure reading {}", filename));
    if contents.len() != 192 {
      return Err("File had size other than 192 (3 * 64) bytes");
    }

    let mut palette = Palette {
      colors: [Color { r: 0, g: 0, b: 0 }; 64],
      map: [0x00; 32],
    };
    let mut index = 0;
    while index < 192 {
      palette.colors[index / 3].r = contents[index + 0];
      palette.colors[index / 3].g = contents[index + 1];
      palette.colors[index / 3].b = contents[index + 2];
      index += 3;
    }

    // for i in 0..32 {
    //   palette.map[i] = i as u8;
    // }

    Ok(palette)
  }
}

impl BusDeviceRange for Palette {
  fn start(&self) -> u16 {
    0x3F00
  }

  fn size(&self) -> usize {
    (0x3FFF - 0x3F00) + 1
  }
}

impl BusDevice for Palette {
  fn safe_read(&self, addr: u16) -> Option<u8> {
    if !self.in_range(addr) {
      return None;
    }

    Some(self.map[addr_to_palette_map_index(addr)])
  }

  fn write(&mut self, addr: u16, data: u8) -> Option<()> {
    if !self.in_range(addr) {
      return None;
    }

    self.map[addr_to_palette_map_index(addr)] = data;

    Some(())
  }
}

fn addr_to_palette_map_index(addr_: u16) -> usize {
  // We just want the index within our palette of 32 colors, so we can strip
  // everything but the lower 5 bits (aka 0-31):
  let mut addr = addr_ & 0x001F;

  // Copied from olc2C02 implementation; what mirroring is this? Is this
  // background color mirroring? Why does this feel off to me? It seems like
  // each of these should map directly back to 0x0000, but they don't.
  //
  // TODO: Document this once I understand wtf it's doing.
  if addr == 0x0010 {
    addr = 0x0000;
  }
  if addr == 0x0014 {
    addr = 0x0004;
  }
  if addr == 0x0018 {
    addr = 0x0008;
  }
  if addr == 0x001C {
    addr = 0x000C;
  }

  addr as usize
}
