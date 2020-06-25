use crate::bus_device::BusDevice;

pub struct Bus {
  pub devices: Vec<Box<dyn BusDevice>>,
}

impl Bus {
  pub fn new(devices: Vec<Box<dyn BusDevice>>) -> Self {
    Bus { devices }
  }

  // TODO: Extract this addressing logic into a reusable util function, maybe?

  pub fn write(&mut self, addr: u16, data: u8) {
    for device in &mut self.devices {
      match device.write(addr, data) {
        None => (),
        Some(_) => {
          break;
        }
      }
    }
  }

  pub fn read(&self, addr: u16) -> u8 {
    for device in &self.devices {
      match device.read(addr) {
        None => (),
        Some(data) => {
          return data;
        }
      }
    }

    0x00
  }

  pub fn read16(&self, addr: u16) -> u16 {
    let lo = self.read(addr) as u16;
    let hi = self.read(addr + 1) as u16;
    (hi << 8) | lo
  }

  pub fn write16(&mut self, addr: u16, data: u16) {
    let lo: u8 = (data << 8) as u8;
    let hi: u8 = (data >> 8) as u8;
    self.write(addr, lo);
    self.write(addr + 1, hi);
  }
}
