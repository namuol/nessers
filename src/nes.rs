use crate::bus::Bus;
use crate::cart::Cart;
use crate::cpu6502::Processor;
use crate::mirror::Mirror;
use crate::ppu::Ppu;
use crate::ram::Ram;

pub struct Nes {
  pub cpu: Processor,
  pub ppu: Ppu,
}

impl Nes {
  pub fn new(cart_filename: &str) -> Result<Nes, &'static str> {
    let cart = match Cart::from_file(cart_filename) {
      Ok(c) => c,
      Err(msg) => return Err(msg),
    };

    Ok(Nes {
      ppu: Ppu::new(),
      cpu: Processor::new(Bus::new(vec![
        // Cartridge
        Box::new(cart),
        // 2K internal RAM, mirrored to 8K
        Box::new(Mirror::new(
          0x0000,
          Box::new(Ram::new(0x0000, 2 * 1024)),
          8 * 1024,
        )),
        // PPU Registers, mirrored for 8K
        Box::new(Mirror::new(0x2000, Box::new(Ram::new(0x2000, 8)), 8 * 1024)),
        // APU & I/O Registers
        Box::new(Ram::new(0x4000, 0x18)),
        // APU & I/O functionality that is normally disabled
        Box::new(Ram::new(0x4018, 0x08)),
      ])),
    })
  }
}
