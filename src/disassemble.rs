use crate::bus::Bus;
use crate::cpu6502::AddressingMode::*;
use crate::cpu6502::Cpu;
use crate::cpu6502::Instruction::*;
use crate::cpu6502::Operation;
use crate::nes::Nes;

pub struct DisassembledOperation {
  pub instruction_name: String,
  pub params: String,
  pub addr: u16,
  pub data: Vec<u8>,
  pub undocumented: bool,
}

pub fn disassemble(nes: &Nes, start: u16, length: u16) -> Vec<DisassembledOperation> {
  let mut output: Vec<DisassembledOperation> = vec![];
  let mut pc = start;
  while pc < start + length {
    let mut data = vec![];
    let addr = pc;
    let operation: &Operation = nes.safe_cpu_read(pc).into();
    let pc_start = pc;
    pc += 1;
    let instruction_name: String = match operation.instruction {
      ADC => "ADC",
      AND => "AND",
      ASL => "ASL",
      BCC => "BCC",
      BCS => "BCS",
      BEQ => "BEQ",
      BIT => "BIT",
      BMI => "BMI",
      BNE => "BNE",
      BPL => "BPL",
      BRK => "BRK",
      BVC => "BVC",
      BVS => "BVS",
      CLC => "CLC",
      CLD => "CLD",
      CLI => "CLI",
      CLV => "CLV",
      CMP => "CMP",
      CPX => "CPX",
      CPY => "CPY",
      DEC => "DEC",
      DEX => "DEX",
      DEY => "DEY",
      EOR => "EOR",
      INC => "INC",
      INX => "INX",
      INY => "INY",
      JMP => "JMP",
      JSR => "JSR",
      LDA => "LDA",
      LDX => "LDX",
      LDY => "LDY",
      LSR => "LSR",
      NOP => "NOP",
      ORA => "ORA",
      PHA => "PHA",
      PHP => "PHP",
      PLA => "PLA",
      PLP => "PLP",
      ROL => "ROL",
      ROR => "ROR",
      RTI => "RTI",
      RTS => "RTS",
      SBC => "SBC",
      SEC => "SEC",
      SED => "SED",
      SEI => "SEI",
      STA => "STA",
      STX => "STX",
      STY => "STY",
      TAX => "TAX",
      TAY => "TAY",
      TSX => "TSX",
      TXA => "TXA",
      TXS => "TXS",
      TYA => "TYA",

      LAX => "LAX",
      SAX => "SAX",
      DCP => "DCP",
      ISB => "ISB",
      SLO => "SLO",
      RLA => "RLA",
      SRE => "SRE",
      RRA => "RRA",
    }
    .into();

    let needs_suffix: bool = match operation.instruction {
      STA | STY | STX | LDY | LDX | LDA | ORA | AND | EOR | ADC | CMP | SBC | BIT | CPX | CPY
      | LSR | ASL | ROR | ROL | INC | DEC | NOP | LAX | SAX | DCP | ISB | SLO | RLA | SRE | RRA => {
        true
      }
      _ => false,
    };

    let params: String = match &operation.addressing_mode {
      IMP => {
        // Implied; nothing to read:
        "".into()
      }
      IMM => {
        // Immediate; read one byte:
        let param = nes.safe_cpu_read(pc);
        pc += 1;
        format!("#${:02X}", param)
      }
      ZP0 => {
        // Zero Page; read one byte:
        let param = nes.safe_cpu_read(pc);
        let data_at = nes.safe_cpu_read(param as u16);
        pc += 1;
        format!("${:02X} = {:02X}", param, data_at)
      }
      ZPX => {
        // Zero Page with X offset; read one byte:
        let param = nes.safe_cpu_read(pc);
        pc += 1;
        let addr_abs = (param.wrapping_add(nes.cpu.x)) as u16 & 0x00FF;
        format!(
          "${:02X},X @ {:02X} = {:02X}",
          param,
          addr_abs,
          nes.safe_cpu_read(addr_abs)
        )
      }
      ZPY => {
        // Zero Page with Y offset; read one byte:
        let param = nes.safe_cpu_read(pc);
        pc += 1;
        let addr_abs = (param.wrapping_add(nes.cpu.y)) as u16 & 0x00FF;
        format!(
          "${:02X},Y @ {:02X} = {:02X}",
          param,
          addr_abs,
          nes.safe_cpu_read(addr_abs)
        )
      }
      ABS => {
        // Absolute; read two bytes:
        let lo = nes.safe_cpu_read(pc) as u16;
        pc += 1;
        let hi = nes.safe_cpu_read(pc) as u16;
        pc += 1;
        let addr = (hi << 8) | lo;
        if needs_suffix {
          let data = nes.safe_cpu_read(addr);
          format!("${:04X} = {:02X}", addr, data)
        } else {
          format!("${:04X}", addr)
        }
      }
      ABX => {
        // Absolute, X; read two bytes:
        let lo = nes.safe_cpu_read(pc) as u16;
        pc += 1;
        let hi = nes.safe_cpu_read(pc) as u16;
        pc += 1;
        let addr = (hi << 8) | lo;
        let addr_abs = addr.wrapping_add(nes.cpu.x as u16);
        format!(
          "${:04X},X @ {:04X} = {:02X}",
          addr,
          addr_abs,
          nes.safe_cpu_read(addr_abs)
        )
      }
      ABY => {
        // Absolute, Y; read two bytes:
        let lo = nes.safe_cpu_read(pc) as u16;
        pc += 1;
        let hi = nes.safe_cpu_read(pc) as u16;
        pc += 1;
        let addr = (hi << 8) | lo;
        let addr_abs = addr.wrapping_add(nes.cpu.y as u16);
        format!(
          "${:04X},Y @ {:04X} = {:02X}",
          addr,
          addr_abs,
          nes.safe_cpu_read(addr_abs)
        )
      }
      IND => {
        // Indirect, Y; read four bytes:
        let lo = nes.safe_cpu_read(pc) as u16;
        pc += 1;
        let hi = nes.safe_cpu_read(pc) as u16;
        pc += 1;
        let ptr = (hi << 8) | lo;
        // The 6502 has a hardware bug where if you happen to have a pointer address
        // in memory that spans across pages (remember, pointers are 2 bytes, and
        // therefore it is possible for this to happen), it will not actually read the
        // hi byte of the address properly
        let addr_abs = if lo == 0x00FF {
          ((nes.safe_cpu_read(ptr & 0xFF00) as u16) << 8) | nes.safe_cpu_read(ptr) as u16
        } else {
          nes.safe_cpu_read16(ptr)
        };
        format!("(${:04X}) = {:04X}", ptr, addr_abs)
      }
      IZX => {
        // Indexed Indirect; read one byte:
        let param = nes.safe_cpu_read(pc);
        pc += 1;
        if needs_suffix {
          // We read X offset from this pointer
          let lo = nes.safe_cpu_read(param.wrapping_add(nes.cpu.x) as u16 & 0x00FF) as u16;
          let hi = nes.safe_cpu_read(param.wrapping_add(nes.cpu.x + 1) as u16 & 0x00FF) as u16;
          let addr_abs = (hi << 8) | lo;
          let data_at = nes.safe_cpu_read(addr_abs);
          format!(
            "(${:02X},X) @ {:02X} = {:04X} = {:02X}",
            param,
            param.wrapping_add(nes.cpu.x) & 0x00FF,
            addr_abs,
            data_at
          )
        } else {
          format!("(${:02X},X)", param)
        }
      }
      IZY => {
        // Indirect Indexed; read one byte:
        let param = nes.safe_cpu_read(pc);
        // Our pointer lives in the zeroth page, so we only need to read one byte
        let ptr = param as u16 & 0x00FF;
        let lo = nes.safe_cpu_read(ptr as u16 & 0x00FF) as u16;
        let hi = nes.safe_cpu_read(ptr.wrapping_add(1) as u16 & 0x00FF) as u16;
        let addr = (hi << 8) | lo;
        let addr_abs = addr.wrapping_add(nes.cpu.y as u16);

        let data_at = nes.safe_cpu_read(addr_abs);

        pc += 1;
        format!(
          "(${:02X}),Y = {:04X} @ {:04X} = {:02X}",
          param,
          // FIXME: What should this really be?
          addr,
          // FIXME: What should this really be?
          addr_abs,
          data_at
        )
      }
      ACC => {
        // Accumulator; nothing to read:
        "A".into()
      }
      REL => {
        let addr = pc;
        // Relative; read one byte:
        let param = nes.safe_cpu_read(addr);

        pc += 1;

        if param & 0x80 != 0 {
          // Get the inverted version of the offset by applying two's complement:
          let neg_offset = !(param as u16) + 1 & 0x00FF;
          format!("${:04X}", pc - neg_offset)
        } else {
          format!("${:04X}", pc + param as u16)
        }
      }
    };

    for pc_ in pc_start..pc {
      data.push(nes.safe_cpu_read(pc_));
    }

    output.push(DisassembledOperation {
      instruction_name,
      params,
      addr,
      data,
      undocumented: operation.undocumented,
    });
  }

  output
}
