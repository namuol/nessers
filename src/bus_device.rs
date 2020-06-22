// TODO:
//
// - Change the design of `BusDevice` to always work with absolute addresses.
//   - Instead of returning `u8` for `read` and `()` for `write`, we should
//     return an `Option` or similar to indicate whether this device has
//     intercepted the read/write.
//   - If `None` is returned, we continue to the next device in the list and
//     attempt to read/write, and repeat until we encounter `Some`.
//   - This should simplify some logic and reduce some annoying requirements
//     like padding the address space with dummy devices and instead just allow
//     us to fall back to some default behaviors (i.e. returning `Some(0)` if no
//     device is read from)

pub trait BusDevice {
  fn size(&self) -> usize;
  fn write(&mut self, addr: u16, data: u8);
  fn read(&self, addr: u16 /*, read_only: bool*/) -> u8;
}
