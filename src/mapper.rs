#![allow(unused_comparisons)]

type MapperFn = fn(u16, usize) -> Option<u16>;

#[derive(Clone)]
pub struct Mapper {
  pub cpu_read: MapperFn,
  pub cpu_write: MapperFn,
  pub ppu_read: MapperFn,
  pub ppu_write: MapperFn,
}

const MXXX: Mapper = Mapper {
  cpu_read: |_, _| todo!(),
  cpu_write: |_, _| todo!(),
  ppu_read: |_, _| todo!(),
  ppu_write: |_, _| todo!(),
};

const NOP: MapperFn = |_, _| None;

const M000_CPU: MapperFn = |addr, num_banks| {
  if addr >= 0x8000 && addr <= 0xFFFF {
    // - num_banks > 1 => 32k rom => map 0x8000 to 0x0000
    // - else, this is a 16k rom => mirror 0x8000 thru the full addr range
    Some(addr & if num_banks > 1 { 0x7FFF } else { 0x3FFF })
  } else {
    None
  }
};

const M000: Mapper = Mapper {
  cpu_read: M000_CPU,
  cpu_write: M000_CPU,
  ppu_read: |addr, _| {
    if addr >= 0x0000 && addr <= 0x1FFF {
      Some(addr)
    } else {
      None
    }
  },
  ppu_write: |addr, _| {
    if addr >= 0x0000 && addr <= 0x1FFF {
      Some(addr)
    } else {
      None
    }
  },
};

pub const MAPPERS: [Mapper; 256] = [
  M000, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX,
  MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX,
  MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX,
  MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX,
  MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX,
  MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX,
  MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX,
  MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX,
  MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX,
  MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX,
  MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX,
  MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX,
  MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX,
  MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX,
  MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX,
  MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX, MXXX,
];
