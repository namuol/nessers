use crate::bus::Bus;
use crate::mirror::Mirror;
use crate::ram::Ram;

// WIP
pub fn make_nes_bus() -> Bus {
  Bus::new(vec![
    // $0000-$1FFF: 2k RAM, mirrored 4x:
    Box::new(Mirror::new(Box::new(Ram::new(2 * 1024)), 8 * 1024)),

    // $2000-$3FFF: 8 PPU register bytes, mirrored for 2k
    // TODO: Using plain RAM for now:
    Box::new(Mirror::new(Box::new(Ram::new(8)), 2 * 1024)),
  ])
}
