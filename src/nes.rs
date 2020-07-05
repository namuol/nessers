use std::rc::Rc;

use crate::bus::Bus;
use crate::cart::Cart;
use crate::cpu6502::Processor;
use crate::mirror::Mirror;
use crate::ppu::Ppu;
use crate::ram::Ram;

pub struct Nes {
  tick: u64,
  pub cpu: Processor,
  pub ppu: Rc<Ppu>,
}

impl Nes {
  pub fn new(cart_filename: &str) -> Result<Nes, &'static str> {
    let cart = match Cart::from_file(cart_filename) {
      Ok(c) => c,
      Err(msg) => return Err(msg),
    };

    let ppu = Rc::new(Ppu::new());
    // let bus_ppu = Rc::clone(&ppu);
    let bus_ppu = Rc::new(Ram::new(0x2000, 8));

    Ok(Nes {
      tick: 0,
      ppu,
      cpu: Processor::new(Bus::new(vec![
        // Cartridge
        Rc::new(cart),
        // 2K internal RAM, mirrored to 8K
        Rc::new(Mirror::new(
          0x0000,
          Rc::new(Ram::new(0x0000, 2 * 1024)),
          8 * 1024,
        )),
        // PPU Registers, mirrored for 8K
        Rc::new(Mirror::new(0x2000, bus_ppu, 8 * 1024)),
        // APU & I/O Registers
        Rc::new(Ram::new(0x4000, 0x18)),
        // APU & I/O functionality that is normally disabled
        Rc::new(Ram::new(0x4018, 0x08)),
      ])),
    })
  }

  pub fn clock(&mut self) {
    Rc::get_mut(&mut self.ppu).unwrap().clock();
    if self.tick % 3 == 0 {
      self.cpu.clock();
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
