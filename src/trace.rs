use crate::cpu6502::AddressingMode::*;
use crate::cpu6502::{AddressingMode, Cpu, Instruction, Operation};
use crate::nes::Nes;

#[derive(Clone, Debug, PartialEq)]
pub struct Trace {
  pub cpu: Cpu,
  pub instruction: Instruction,
  pub addressing_mode: AddressingMode,
  pub undocumented: bool,
  pub data: Vec<u8>,
  pub param: u8,
  pub param_expanded: u8,
  pub addr: u16,
  pub addr_abs: u16,
  pub data_at: u8,
}

pub fn trace(nes: &Nes, pc_: u16) -> Trace {
  let mut pc = pc_;
  let mut data = vec![];
  let operation: &Operation = nes.safe_cpu_read(pc).into();
  let pc_start = pc;
  pc += 1;

  let mut param: u8 = 0x00;
  let mut param_expanded: u8 = 0x00;
  let mut addr: u16 = 0x0000;
  let mut addr_abs: u16 = 0x0000;
  let mut data_at: u8 = 0x00;

  match &operation.addressing_mode {
    IMP => {}
    IMM => {
      // Immediate; read one byte:
      param = nes.safe_cpu_read(pc);
      pc += 1;
    }
    ZP0 => {
      // Zero Page; read one byte:
      param = nes.safe_cpu_read(pc);
      data_at = nes.safe_cpu_read(param as u16);
      pc += 1;
    }
    ZPX => {
      // Zero Page with X offset; read one byte:
      param = nes.safe_cpu_read(pc);
      pc += 1;
      addr_abs = (param.wrapping_add(nes.cpu.x)) as u16 & 0x00FF;
      data_at = nes.safe_cpu_read(addr_abs);
    }
    ZPY => {
      // Zero Page with Y offset; read one byte:
      param = nes.safe_cpu_read(pc);
      pc += 1;
      addr_abs = (param.wrapping_add(nes.cpu.y)) as u16 & 0x00FF;
      data_at = nes.safe_cpu_read(addr_abs);
    }
    ABS => {
      // Absolute; read two bytes:
      let lo = nes.safe_cpu_read(pc) as u16;
      pc += 1;
      let hi = nes.safe_cpu_read(pc) as u16;
      pc += 1;
      addr = (hi << 8) | lo;
      data_at = nes.safe_cpu_read(addr);
    }
    ABX => {
      // Absolute, X; read two bytes:
      let lo = nes.safe_cpu_read(pc) as u16;
      pc += 1;
      let hi = nes.safe_cpu_read(pc) as u16;
      pc += 1;
      addr = (hi << 8) | lo;
      addr_abs = addr.wrapping_add(nes.cpu.x as u16);
      data_at = nes.safe_cpu_read(addr_abs);
    }
    ABY => {
      // Absolute, Y; read two bytes:
      let lo = nes.safe_cpu_read(pc) as u16;
      pc += 1;
      let hi = nes.safe_cpu_read(pc) as u16;
      pc += 1;
      addr = (hi << 8) | lo;
      addr_abs = addr.wrapping_add(nes.cpu.y as u16);
      data_at = nes.safe_cpu_read(addr_abs);
    }
    IND => {
      // Indirect, Y; read four bytes:
      let lo = nes.safe_cpu_read(pc) as u16;
      pc += 1;
      let hi = nes.safe_cpu_read(pc) as u16;
      pc += 1;
      addr = (hi << 8) | lo;
      // The 6502 has a hardware bug where if you happen to have a pointer address
      // in memory that spans across pages (remember, pointers are 2 bytes, and
      // therefore it is possible for this to happen), it will not actually read the
      // hi byte of the address properly
      addr_abs = if lo == 0x00FF {
        ((nes.safe_cpu_read(addr & 0xFF00) as u16) << 8) | nes.safe_cpu_read(addr) as u16
      } else {
        nes.safe_cpu_read16(addr)
      };
    }
    IZX => {
      // Indexed Indirect; read one byte:
      param = nes.safe_cpu_read(pc);
      pc += 1;
      // We read X offset from this pointer
      let lo = nes.safe_cpu_read(param.wrapping_add(nes.cpu.x) as u16 & 0x00FF) as u16;
      let hi =
        nes.safe_cpu_read(param.wrapping_add(nes.cpu.x.wrapping_add(1)) as u16 & 0x00FF) as u16;
      addr_abs = (hi << 8) | lo;
      data_at = nes.safe_cpu_read(addr_abs);
      param_expanded = param.wrapping_add(nes.cpu.x) & 0x00FF;
    }
    IZY => {
      // Indirect Indexed; read one byte:
      param = nes.safe_cpu_read(pc);
      // Our pointer lives in the zeroth page, so we only need to read one byte
      let ptr = param as u16 & 0x00FF;
      let lo = nes.safe_cpu_read(ptr as u16 & 0x00FF) as u16;
      let hi = nes.safe_cpu_read(ptr.wrapping_add(1) as u16 & 0x00FF) as u16;
      addr = (hi << 8) | lo;
      addr_abs = addr.wrapping_add(nes.cpu.y as u16);

      data_at = nes.safe_cpu_read(addr_abs);

      pc += 1;
    }
    ACC => {}
    REL => {
      let addr = pc;
      // Relative; read one byte:
      let param = nes.safe_cpu_read(addr);

      pc += 1;

      addr_abs = if param & 0x80 != 0 {
        // Get the inverted version of the offset by applying two's complement:
        let neg_offset = !(param as u16) + 1 & 0x00FF;
        pc - neg_offset
      } else {
        pc + param as u16
      };
    }
  };

  for pc_ in pc_start..pc {
    data.push(nes.safe_cpu_read(pc_));
  }

  let mut cpu = nes.cpu.clone();
  cpu.pc = pc_;

  Trace {
    cpu,
    instruction: operation.instruction,
    addressing_mode: operation.addressing_mode,
    undocumented: operation.undocumented,
    data,

    param,
    param_expanded,
    addr,
    addr_abs,
    data_at,
  }
}
