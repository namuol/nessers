use crate::bus_device::BusDevice;

#[derive(Copy, Clone)]
pub struct Controller {
  pub a: bool,
  pub b: bool,
  pub select: bool,
  pub start: bool,
  pub up: bool,
  pub down: bool,
  pub left: bool,
  pub right: bool,
}

impl Controller {
  pub fn new() -> Controller {
    Controller {
      a: false,
      b: false,
      select: false,
      start: false,
      up: false,
      down: false,
      left: false,
      right: false,
    }
  }
}

impl From<Controller> for u8 {
  fn from(controller: Controller) -> u8 {
    let mut result: u8 = 0b00000000;
    result = (result | controller.a as u8) << 1;
    result = (result | controller.b as u8) << 1;
    result = (result | controller.select as u8) << 1;
    result = (result | controller.start as u8) << 1;
    result = (result | controller.up as u8) << 1;
    result = (result | controller.down as u8) << 1;
    result = (result | controller.left as u8) << 1;
    result = result | controller.right as u8;
    return result;
  }
}

pub struct Peripherals {
  pub controllers: [Controller; 2],
  controller_shifts: [u8; 2],
}

impl Peripherals {
  pub fn new() -> Peripherals {
    Peripherals {
      controllers: [Controller::new(), Controller::new()],
      controller_shifts: [0x00; 2],
    }
  }
}

impl BusDevice for Peripherals {
  fn write(&mut self, addr: u16, _data: u8, _cart: &mut crate::cart::Cart) -> Option<()> {
    if addr == 0x4016 {
      // Store our controller state into the bit shift register buffer:
      self.controller_shifts[0] = self.controllers[0].into();
      self.controller_shifts[1] = self.controllers[1].into();
      return Some(());
    }

    None
  }

  fn read(&mut self, addr: u16, _cart: &crate::cart::Cart) -> Option<u8> {
    match addr {
      0x4016 | 0x4017 => {
        let num = if addr == 0x4016 { 0 } else { 1 };
        // Read the high bit from the controller shift register:
        let bit = (0b1000_0000 & self.controller_shifts[num]) != 0;
        // Shift the controller shift register:
        self.controller_shifts[num] <<= 1;
        // xx xx xx D4 D3 D2 D1 D0
        //                      ^^ - bit 0 is for controller button status
        Some(bit as u8)
      }
      _ => None,
    }
  }

  fn safe_read(&self, _addr: u16, _cart: &crate::cart::Cart) -> Option<u8> {
    None
  }
}
