use crate::bus_device::BusDevice;

/// A list of bus devices, in order of "priority". The order of devices does
/// **not** represent where the device lives in address space.
///
/// When performing a read or write, devices are accessed in the order supplied
/// in this list. When a device returns `Some` from a `read`/`write`, it now
/// owns that operation, and all devices after it in the list are ignored.
pub type DeviceList = Vec<Box<dyn BusDevice>>;

impl Bus for DeviceList {
  fn write(&mut self, addr: u16, data: u8) {
    for device in self {
      match device.write(addr, data) {
        None => (),
        Some(_) => {
          break;
        }
      }
    }
  }
  fn read(&self, addr: u16) -> u8 {
    for device in self {
      match device.read(addr) {
        None => (),
        Some(data) => {
          return data;
        }
      }
    }
    0x00
  }
  fn read16(&self, addr: u16) -> u16 {
    let lo = self.read(addr) as u16;
    let hi = self.read(addr + 1) as u16;
    (hi << 8) | lo
  }
  fn write16(&mut self, addr: u16, data: u16) {
    let lo: u8 = (data << 8) as u8;
    let hi: u8 = (data >> 8) as u8;
    self.write(addr, lo);
    self.write(addr + 1, hi);
  }
}

pub trait Bus {
  fn read(&self, addr: u16) -> u8;
  fn read16(&self, addr: u16) -> u16;
  fn write(&mut self, addr: u16, data: u8);
  fn write16(&mut self, addr: u16, data: u16);
}
