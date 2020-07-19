use crate::bus::DeviceList;
use crate::cart::Cart;
use crate::cpu6502::Processor;
use crate::mirror::Mirror;
use crate::palette::Palette;
use crate::ppu::Ppu;
use crate::ram::Ram;

pub struct Nes {
  tick: u64,
  pub cpu: Processor,
  pub ppu: Ppu,
  pub devices: DeviceList,
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

    let devices: DeviceList = vec![
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

    Ok(Nes {
      tick: 0,
      ppu,
      cpu: Processor::new(),
      devices,
    })
  }

  pub fn clock(&mut self) {
    self.ppu.clock();
    if self.tick % 3 == 0 {
      self.cpu.clock(&mut self.devices);
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
}
