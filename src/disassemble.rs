use crate::cpu6502::AddressingMode::*;
use crate::cpu6502::Instruction::*;
use crate::cpu6502::Operation;
use crate::nes::Nes;
use crate::trace::trace;

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
    let addr_ = pc;
    let trace = trace(nes, addr_);
    let operation: &Operation = nes.safe_cpu_read(pc).into();
    pc += 1;
    let instruction_name: String = match trace.instruction {
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

    let needs_suffix: bool = match trace.instruction {
      STA | STY | STX | LDY | LDX | LDA | ORA | AND | EOR | ADC | CMP | SBC | BIT | CPX | CPY
      | LSR | ASL | ROR | ROL | INC | DEC | NOP | LAX | SAX | DCP | ISB | SLO | RLA | SRE | RRA => {
        true
      }
      _ => false,
    };

    let params: String = match &trace.addressing_mode {
      IMP => "".into(),
      IMM => {
        format!("#${:02X}", trace.param)
      }
      ZP0 => {
        // Zero Page; read one byte:
        format!("${:02X} = {:02X}", trace.param, trace.data_at)
      }
      ZPX => {
        format!(
          "${:02X},X @ {:02X} = {:02X}",
          trace.param, trace.addr_abs, trace.data_at
        )
      }
      ZPY => {
        format!(
          "${:02X},Y @ {:02X} = {:02X}",
          trace.param, trace.addr_abs, trace.data_at
        )
      }
      ABS => {
        if needs_suffix {
          format!("${:04X} = {:02X}", trace.addr, trace.data_at)
        } else {
          format!("${:04X}", trace.addr)
        }
      }
      ABX => {
        format!(
          "${:04X},X @ {:04X} = {:02X}",
          trace.addr, trace.addr_abs, trace.data_at
        )
      }
      ABY => {
        format!(
          "${:04X},Y @ {:04X} = {:02X}",
          trace.addr, trace.addr_abs, trace.data_at
        )
      }
      IND => {
        format!("(${:04X}) = {:04X}", trace.addr, trace.addr_abs)
      }
      IZX => {
        if needs_suffix {
          format!(
            "(${:02X},X) @ {:02X} = {:04X} = {:02X}",
            trace.param, trace.param_expanded, trace.addr_abs, trace.data_at
          )
        } else {
          format!("(${:02X},X)", trace.param)
        }
      }
      IZY => {
        pc += 1;
        format!(
          "(${:02X}),Y = {:04X} @ {:04X} = {:02X}",
          trace.param, trace.addr, trace.addr_abs, trace.data_at
        )
      }
      ACC => {
        // Accumulator; nothing to read:
        "A".into()
      }
      REL => {
        format!("${:04X}", trace.addr_abs)
      }
    };

    output.push(DisassembledOperation {
      instruction_name,
      params,
      addr: addr_,
      data: trace.data,
      undocumented: operation.undocumented,
    });
  }

  output
}
