use std::rc::Rc;

use crate::bus_device::BusDevice;

/// A list of bus devices, in order of "priority". The order of devices does
/// **not** represent where the device lives in address space.
///
/// When performing a read or write, devices are accessed in the order supplied
/// in this list. When a device returns `Some` from a `read`/`write`, it now
/// owns that operation, and all devices after it in the list are ignored.
type DeviceList = Vec<Rc<dyn BusDevice>>;

pub struct Bus {
  pub devices: DeviceList,
}

impl Bus {
  pub fn new(devices: DeviceList) -> Self {
    Bus { devices }
  }

  // TODO: Extract this addressing logic into a reusable util function, maybe?

  pub fn write(&mut self, addr: u16, data: u8) {
    for device in &mut self.devices {
      // FIXME: We probably want to use RefCell here:
      match Rc::get_mut(device).unwrap().write(addr, data) {
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
