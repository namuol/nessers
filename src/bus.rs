use crate::bus_device::BusDevice;

pub struct Bus {
  devices: Vec<Box<dyn BusDevice>>,
}

impl Bus {
  pub fn new(devices: Vec<Box<dyn BusDevice>>) -> Self {
    Bus { devices }
  }

  // TODO: Extract this addressing logic into a reusable util function, maybe?

  pub fn write(&mut self, addr: u16, data: u8) {
    let mut device_start = 0x0000;
    let mut idx = 0;
    let len = self.devices.len();

    while idx < len {
      let current_device = &mut self.devices[idx];
      let size = current_device.size();
      let device_end = device_start + size;
      if (addr as usize) >= device_start && (addr as usize) < device_end {
        current_device.write(device_start as u16 + addr, data);
        break;
      }
      idx += 1;
      device_start += current_device.size();
    }
  }

  pub fn write16(&mut self, addr: u16, data: u16) {
    let mut device_start = 0x0000;
    let mut idx = 0;
    let len = self.devices.len();

    while idx < len {
      let current_device = &mut self.devices[idx];
      let size = current_device.size();
      let device_end = device_start + size;
      if (addr as usize) >= device_start && (addr as usize) < device_end {
        current_device.write16(device_start as u16 + addr, data);
        break;
      }
      idx += 1;
      device_start += current_device.size();
    }
  }

  pub fn read(&self, addr: u16) -> u8 {
    let mut device_start = 0x0000;
    let mut idx = 0;
    let len = self.devices.len();

    while idx < len {
      let current_device = &self.devices[idx];
      let size = current_device.size();
      let device_end = device_start + size;
      if (addr as usize) >= device_start && (addr as usize) < device_end {
        return current_device.read(device_start as u16 + addr);
      }
      idx += 1;
      device_start += current_device.size();
    }

    0x00
  }

  pub fn read16(&self, addr: u16) -> u16 {
    let mut device_start = 0x0000;
    let mut idx = 0;
    let len = self.devices.len();

    while idx < len {
      let current_device = &self.devices[idx];
      let size = current_device.size();
      let device_end = device_start + size;
      if (addr as usize) >= device_start && (addr as usize) < device_end {
        return current_device.read16(device_start as u16 + addr);
      }
      idx += 1;
      device_start += current_device.size();
    }

    0x00
  }
}
