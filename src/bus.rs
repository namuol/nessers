use crate::bus_device::BusDevice;

/// A list of bus devices, in order of "priority". The order of devices does
/// **not** represent where the device lives in address space.
///
/// When performing a read or write, devices are accessed in the order supplied
/// in this list. When a device returns `Some` from a `read`/`write`, it now
/// owns that operation, and all devices after it in the list are ignored.
pub type DeviceList = Vec<Box<dyn BusDevice>>;

pub fn write(devices: &mut DeviceList, addr: u16, data: u8) {
  for device in devices {
    match device.write(addr, data) {
      None => (),
      Some(_) => {
        break;
      }
    }
  }
}

pub fn read(devices: &DeviceList, addr: u16) -> u8 {
  for device in devices {
    match device.read(addr) {
      None => (),
      Some(data) => {
        return data;
      }
    }
  }

  0x00
}

pub fn read16(devices: &DeviceList, addr: u16) -> u16 {
  let lo = read(devices, addr) as u16;
  let hi = read(devices, addr + 1) as u16;
  (hi << 8) | lo
}

pub fn write16(devices: &mut DeviceList, addr: u16, data: u16) {
  let lo: u8 = (data << 8) as u8;
  let hi: u8 = (data >> 8) as u8;
  write(devices, addr, lo);
  write(devices, addr + 1, hi);
}
