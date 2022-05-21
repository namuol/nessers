#[macro_use]
extern crate maplit;

use audio::AudioDevice;
use coffee::graphics::{Color, Frame, Gpu, Window, WindowSettings};
use coffee::input::{self, keyboard, Input};
use coffee::load::Task;
use coffee::ui::{
  Align, Column, Element, Image, Justify, Panel, Renderer, Row, Text, UserInterface,
};
use coffee::{Game, Result, Timer};
use cpu6502::{NMI_POINTER, STACK_START};
use docopt::Docopt;
use serde::Deserialize;
use std::collections::HashSet;
use winit::event::VirtualKeyCode::*;

pub mod apu;
pub mod audio;
pub mod bus;
pub mod bus_device;
pub mod cart;
pub mod cpu6502;
pub mod disassemble;
pub mod mapper;
pub mod mirror;
pub mod nes;
pub mod palette;
pub mod peripherals;
pub mod ppu;
pub mod ram;
pub mod trace;

use crate::cpu6502::{StatusFlag, STACK_SIZE};
use crate::disassemble::disassemble;
use crate::nes::Nes;
use crate::ppu::{SCREEN_H, SCREEN_W};

const USAGE: &'static str = "
Usage:

nessers <rom> [<breakpoints>]
";

const TXT_SIZE: u16 = 24;

#[derive(Deserialize)]
struct Args {
  arg_rom: String,
  arg_breakpoints: Vec<String>,
}

fn main() -> Result<()> {
  <NESDebugger as UserInterface>::run(WindowSettings {
    title: String::from("nessers"),
    size: ((SCREEN_W * 9) as u32, (SCREEN_H * 6) as u32),
    resizable: false,
    fullscreen: false,
    maximized: false,
  })
}

struct NESDebugger {
  audio_device: AudioDevice,
  screen_img: Option<coffee::graphics::Image>,
  pattern_table_0_img: Option<coffee::graphics::Image>,
  pattern_table_1_img: Option<coffee::graphics::Image>,
  name_table_0_img: Option<coffee::graphics::Image>,
  name_table_1_img: Option<coffee::graphics::Image>,
  debug_palette: u8,
  palettes_imgs: Option<[coffee::graphics::Image; 8]>,
  nes: Nes,
  running: bool,
  debug_with_pt1: bool,
}

// Fibonacci sequence program:
// let mut debugger_ui = NESDebugger {
//     screen: [0x00; SCREEN_W * SCREEN_H],
//     cpu: Processor::new(Bus::new(vec![Box::new(Ram::new(0x0000, 0xFFFF + 1))])),
// };

// let program_start: u16 = 0x8000;

// debugger_ui.nes.cpu.bus.write16(PC_INIT_ADDR, program_start);

// let program: Vec<u8> = vec![
//     // Initialize A to 0
//     0xA9, 0, // LDA #0
//     // Set X to 0x0000 + 0
//     0xA2, 0, // LDX #0
//     // [0, ...]
//     //  ^
//     0x95, 0x00, // STA #0
//     // Set A to 1
//     0xA9, 1, // LDA #1
//     // [0, 1, ...]
//     //     ^
//     0x95, 0x01, // STA #1
//     //
//     // LOOP:
//     //
//     // A = A + RAM[X]
//     0x75, 0x00, // ADC $00,X
//     // RAM[X + 2] = A
//     0x95, 0x02, // STA $02,X
//     // Increment X
//     0xE8, // INX
//     // JMP Loop
//     0x4C, 10, 0x80,
// ];
// let mut offset: u16 = 0;
// for byte in &program {
//     debugger_ui.nes.cpu.bus.write(program_start + offset, *byte);
//     offset += 1;
// }

// debugger_ui.nes.cpu.sig_reset();
// debugger_ui.nes.cpu.step();

impl Game for NESDebugger {
  type Input = CPUDebuggerInput; // No input data
  type LoadingScreen = (); // No loading screen

  fn load(_window: &Window) -> Task<NESDebugger> {
    Task::succeed(|| {
      let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

      let mut nes = match Nes::new(&args.arg_rom, "src/test_fixtures/ntscpalette.pal") {
        Ok(n) => n,
        Err(msg) => panic!("{}", msg),
      };
      nes.breakpoints = args
        .arg_breakpoints
        .iter()
        .map(|s| u16::from_str_radix(s, 16).unwrap())
        .collect();
      let audio_device = AudioDevice::init();
      let mut debugger_ui = NESDebugger {
        audio_device,
        screen_img: None,
        pattern_table_0_img: None,
        pattern_table_1_img: None,
        name_table_0_img: None,
        name_table_1_img: None,
        palettes_imgs: None,
        debug_palette: 0,
        nes,
        running: false,
        debug_with_pt1: true,
      };
      debugger_ui.nes.reset();
      debugger_ui.nes.step();

      debugger_ui
    })
  }

  fn draw(&mut self, frame: &mut Frame, _timer: &Timer) {
    // Clear the current frame
    frame.clear(Color {
      r: 0.6,
      g: 0.3,
      b: 0.6,
      a: 1.0,
    });
  }

  fn interact(&mut self, input: &mut CPUDebuggerInput, window: &mut Window) {
    for key in &input.keypresses {
      // Debugger controls:
      match key {
        // Start/Stop execution:
        Space => {
          self.running = !self.running;
        }

        Period => {
          self.nes.clock();
        }

        Slash => {
          self.nes.step();
        }

        // Step with trace:
        T => self
          .nes
          .step_with_callback(|nes| println!("{}", nes.trace())),

        // Reset signal:
        R => {
          self.nes.reset();
        }

        // Render a single frame:
        F => {
          self.nes.frame();
        }

        // List all addresses that have been executed:
        D => {
          println!(
            "{}",
            self
              .nes
              .addresses_hit
              .iter()
              .map(|addr| format!("{:04X}", addr))
              .collect::<Vec<String>>()
              .join("\n")
          );
        }

        // Change palette in pattern/nametable debug views:
        P => {
          self.debug_palette = (self.debug_palette + 1) % 8;
        }

        LBracket => {
          self.debug_with_pt1 = !self.debug_with_pt1;
        }

        N => {
          // Render nametables as text grid for now:
          let mut nametable_text = vec![String::new(); 30];
          for y in 0..30 {
            for x in 0..32 {
              nametable_text[y] += &format!("{:02X}", self.nes.ppu.name_tables[0][y * 32 + x]);
            }
          }

          println!("nametable 0:\n{}", nametable_text.join("\n"));

          for y in 0..30 {
            nametable_text[y] = String::new();
            for x in 0..32 {
              nametable_text[y] += &format!("{:02X}", self.nes.ppu.name_tables[1][y * 32 + x]);
            }
          }
          println!("nametable 1:\n{}", nametable_text.join("\n"));
        }

        O => {
          // Output debug info about OAM (Sprites)
          println!("{}", self.nes.ppu.oam_trace());
        }
        Tab => {
          // Clear the screen; useful for debugging drawing step by step.
          for n in 0..self.nes.ppu.screen.len() {
            self.nes.ppu.screen[n] = [0xFF, 0x00, 0xFF, 0xFF];
          }
        }

        L => {
          // Continue stepping until our PPU cycle is at the end of the current scanline
          loop {
            self.nes.clock();
            if self.nes.ppu.cycle == 0 {
              break;
            }
          }
        }

        S => {
          std::thread::spawn(|| {
            AudioDevice::init().play();
            std::thread::sleep(std::time::Duration::from_millis(1000));
          });
        }

        _ => {}
      }
    }

    self.nes.peripherals.controllers[0].a = input.held.contains(&X);
    self.nes.peripherals.controllers[0].b = input.held.contains(&Z);
    self.nes.peripherals.controllers[0].select = input.held.contains(&RShift);
    self.nes.peripherals.controllers[0].start = input.held.contains(&Return);
    self.nes.peripherals.controllers[0].up = input.held.contains(&Up);
    self.nes.peripherals.controllers[0].down = input.held.contains(&Down);
    self.nes.peripherals.controllers[0].left = input.held.contains(&Left);
    self.nes.peripherals.controllers[0].right = input.held.contains(&Right);

    input.keypresses.clear();

    // Update the screen image:
    self.screen_img = Some(from_screen(window.gpu(), &self.nes.ppu.screen).unwrap());

    let pt0 = self
      .nes
      .ppu
      .render_pattern_table(0, self.debug_palette, &self.nes.cart);
    // Get the pattern table image:
    self.pattern_table_0_img = Some(from_pattern_table(window.gpu(), &pt0).unwrap());
    let pt1 = self
      .nes
      .ppu
      .render_pattern_table(1, self.debug_palette, &self.nes.cart);
    self.pattern_table_1_img = Some(from_pattern_table(window.gpu(), &pt1).unwrap());

    let pt = if self.debug_with_pt1 { &pt1 } else { &pt0 };

    self.name_table_0_img =
      Some(from_name_table(window.gpu(), &self.nes.ppu.render_name_table(&pt, 0)).unwrap());
    self.name_table_1_img =
      Some(from_name_table(window.gpu(), &self.nes.ppu.render_name_table(&pt, 1)).unwrap());

    // lol
    let palettes = self.nes.ppu.get_palettes(&self.nes.cart);
    self.palettes_imgs = Some([
      from_palette(window.gpu(), &palettes[0]).unwrap(),
      from_palette(window.gpu(), &palettes[1]).unwrap(),
      from_palette(window.gpu(), &palettes[2]).unwrap(),
      from_palette(window.gpu(), &palettes[3]).unwrap(),
      from_palette(window.gpu(), &palettes[4]).unwrap(),
      from_palette(window.gpu(), &palettes[5]).unwrap(),
      from_palette(window.gpu(), &palettes[6]).unwrap(),
      from_palette(window.gpu(), &palettes[7]).unwrap(),
    ]);
  }
}

enum Message {}

const ACTIVE_COLOR: Color = Color {
  r: 1.0,
  g: 1.0,
  b: 1.0,
  a: 1.0,
};

const INACTIVE_COLOR: Color = Color {
  r: 1.0,
  g: 1.0,
  b: 1.0,
  a: 0.5,
};

impl UserInterface for NESDebugger {
  type Message = Message;
  type Renderer = Renderer;
  fn react(&mut self, _event: Message, _window: &mut Window) {
    // Does nothing
  }
  fn layout(&mut self, window: &Window) -> Element<Message> {
    if self.running {
      if self.nes.frame() {
        println!("Broke at {:04X}", self.nes.cpu.pc);
        self.running = false;
      }
    }
    let mut stack_str = String::new();
    let start: u16 = 0;
    for page in start..=(start + (STACK_SIZE as u16 / 16) * 4) {
      let addr = (STACK_START * 0) as u16 + (page as u16) * 16;
      stack_str.push_str(&format!("{:04X}:", addr));
      for offset in 0..16 {
        // NOTE: Using nbsp in the string below to prevent line breaks:
        stack_str.push_str(&format!("{:02X} ", self.nes.cpu_read(addr + offset)));
      }
      stack_str.push_str("\n");
    }

    // let first_pc_page = (self.nes.cpu.pc / 16) as u16;
    // let mut ram_str = String::new();
    // for page in 0..(STACK_SIZE / 16) {
    //   let addr = ((first_pc_page + page as u16) as u16).wrapping_mul(16);
    //   ram_str.push_str(&format!("{:04X}: ", addr));
    //   for offset in 0..16 {
    //     let addr = addr + offset;
    //     if addr == self.nes.cpu.pc {
    //       ram_str.push_str(&format!("{:02X}<", self.nes.cpu_read(addr)));
    //     } else {
    //       ram_str.push_str(&format!("{:02X} ", self.nes.cpu_read(addr)));
    //     }
    //   }
    //   ram_str.push_str("\n");
    // }

    let disassembled = disassemble(&self.nes, self.nes.cpu.pc, 128);
    let mut disassembled_output: Vec<String> = vec![];
    let mut pc_idx: i32 = 0;
    let mut idx: i32 = 0;
    for o in disassembled {
      let current = self.nes.cpu.pc == o.addr;
      if current {
        pc_idx = idx;
      }
      disassembled_output.push(format!(
        "{} ${:04X}: {} {}",
        if current { ">" } else { " " },
        o.addr,
        o.instruction_name,
        o.params
      ));
      idx += 1;
    }
    let start = (pc_idx - 8).max(0).min(disassembled_output.len() as i32) as usize;
    let end = ((start as i32) + 32)
      .max(0)
      .min(disassembled_output.len() as i32) as usize;
    let disassembled_output = &disassembled_output[start..end];
    let status = Row::new()
      .push(Text::new("Status:").size(TXT_SIZE))
      .push(Text::new("C").size(TXT_SIZE).color(
        if self.nes.cpu.get_status(StatusFlag::Carry) != 0x00 {
          ACTIVE_COLOR
        } else {
          INACTIVE_COLOR
        },
      ))
      .push(Text::new("Z").size(TXT_SIZE).color(
        if self.nes.cpu.get_status(StatusFlag::Zero) != 0x00 {
          ACTIVE_COLOR
        } else {
          INACTIVE_COLOR
        },
      ))
      .push(Text::new("I").size(TXT_SIZE).color(
        if self.nes.cpu.get_status(StatusFlag::DisableInterrupts) != 0x00 {
          ACTIVE_COLOR
        } else {
          INACTIVE_COLOR
        },
      ))
      .push(Text::new("B").size(TXT_SIZE).color(
        if self.nes.cpu.get_status(StatusFlag::Break) != 0x00 {
          ACTIVE_COLOR
        } else {
          INACTIVE_COLOR
        },
      ))
      .push(Text::new("O").size(TXT_SIZE).color(
        if self.nes.cpu.get_status(StatusFlag::Overflow) != 0x00 {
          ACTIVE_COLOR
        } else {
          INACTIVE_COLOR
        },
      ))
      .push(Text::new("N").size(TXT_SIZE).color(
        if self.nes.cpu.get_status(StatusFlag::Negative) != 0x00 {
          ACTIVE_COLOR
        } else {
          INACTIVE_COLOR
        },
      ));

    let status_pane = Column::new()
      .push(status)
      .push(
        Text::new(&format!(
          "PC: {:04X}        PPU: {:02X} {:08b}",
          self.nes.cpu.pc, self.nes.ppu.status, self.nes.ppu.status
        ))
        .size(TXT_SIZE),
      )
      .push(
        Text::new(&format!(
          " A: {:02X} ({:03})   CTRL: {:02X} {:08b}",
          self.nes.cpu.a, self.nes.cpu.a, self.nes.ppu.control, self.nes.ppu.control
        ))
        .size(TXT_SIZE),
      )
      .push(
        Text::new(&format!(
          " X: {:02X} ({:03})   MASK: {:02X} {:08b}",
          self.nes.cpu.x, self.nes.cpu.x, self.nes.ppu.mask, self.nes.ppu.mask
        ))
        .size(TXT_SIZE),
      )
      .push(
        Text::new(&format!(
          " Y: {:02X} ({:03})    NMI: {:04X}",
          self.nes.cpu.y,
          self.nes.cpu.y,
          self.nes.safe_cpu_read16(NMI_POINTER)
        ))
        .size(TXT_SIZE),
      )
      .push(
        Text::new(&format!(
          "SP: {:02X} ({:03})   ADDR: {:04X}",
          self.nes.cpu.s, self.nes.cpu.s, self.nes.ppu.vram_addr
        ))
        .size(TXT_SIZE),
      );

    let mut pattern_tables = Row::new().width(1024).height(512);
    pattern_tables = match &self.pattern_table_0_img {
      Some(img) => pattern_tables.push(Image::new(&img).width(256).height(256)),
      None => pattern_tables,
    };
    pattern_tables = match &self.pattern_table_1_img {
      Some(img) => pattern_tables.push(Image::new(&img).width(256).height(256)),
      None => pattern_tables,
    };

    let mut palettes = Row::new().width(64 * 8).height(16);

    palettes = match &self.palettes_imgs {
      Some(imgs) => {
        for i in 0..8 {
          palettes = palettes.push(Image::new(&imgs[i]).width(64).height(16));
        }
        palettes
      }
      None => palettes,
    };

    let mut name_tables = Column::new();

    name_tables = match (&self.name_table_0_img, &self.name_table_1_img) {
      (Some(a), Some(b)) => match self.nes.cart.mirroring {
        cart::Mirroring::Horizontal => name_tables
          .push(
            Row::new()
              .push(Image::new(&a).width(256).height(240))
              .push(Image::new(&a).width(256).height(240)),
          )
          .push(
            Row::new()
              .push(Image::new(&b).width(256).height(240))
              .push(Image::new(&b).width(256).height(240)),
          ),
        cart::Mirroring::Vertical => name_tables
          .push(
            Row::new()
              .push(Image::new(&a).width(256).height(240))
              .push(Image::new(&b).width(256).height(240)),
          )
          .push(
            Row::new()
              .push(Image::new(&a).width(256).height(240))
              .push(Image::new(&b).width(256).height(240)),
          ),
        cart::Mirroring::OneScreenLo => todo!(),
        cart::Mirroring::OneScreenHi => todo!(),
      },
      _ => name_tables,
    };

    // let debug_visuals = Column::new()
    //   .push(name_tables)
    //   .push(palettes)
    //   .push(pattern_tables);

    let mut screen = Column::new();
    screen = match &self.screen_img {
      Some(img) => screen.push(Image::new(&img).width(256 * 4).height(240 * 4)),
      None => screen,
    };

    Row::new()
      .width(window.width() as u32)
      .push(
        Panel::new(
          Row::new()
            .spacing(8)
            .push(
              Row::new()
                // .width((SCREEN_W * 3) as u32)
                .push(Text::new(&stack_str).size(TXT_SIZE)),
            )
            .push(
              Column::new()
                // .width((SCREEN_W * 2) as u32)
                .height(window.height() as u32)
                .push(status_pane)
                .push(
                  Text::new(&make_non_breakable(&disassembled_output.join("\n"))).size(TXT_SIZE),
                ),
            ),
        )
        .max_width((SCREEN_W * 5) as u32)
        .width((SCREEN_W * 5) as u32),
      )
      .push(
        Column::new()
          .width((SCREEN_W * 4) as u32)
          .push(screen)
          .push(
            Row::new()
              .height(256 * 4)
              .push(
                Column::new()
                  .height((SCREEN_H * 2) as u32 + 16)
                  .width((SCREEN_W * 2) as u32)
                  .push(palettes)
                  .push(pattern_tables),
              )
              .push(name_tables),
          ),
      )
      .align_items(Align::Start)
      .justify_content(Justify::Start)
      .into()
  }
}

fn make_non_breakable(string: &str) -> String {
  string.replace(" ", " ")
}

struct CPUDebuggerInput {
  keypresses: HashSet<keyboard::KeyCode>,
  held: HashSet<keyboard::KeyCode>,
}

impl Input for CPUDebuggerInput {
  fn new() -> Self {
    CPUDebuggerInput {
      keypresses: HashSet::new(),
      held: HashSet::new(),
    }
  }
  fn update(&mut self, event: input::Event) {
    match event {
      input::Event::Keyboard(keyboard::Event::Input {
        key_code,
        state: input::ButtonState::Pressed,
      }) => {
        self.keypresses.insert(key_code);
        self.held.insert(key_code);
      }
      input::Event::Keyboard(keyboard::Event::Input {
        key_code,
        state: input::ButtonState::Released,
      }) => {
        self.held.remove(&key_code);
      }
      _ => {}
    }
  }
  fn clear(&mut self) {}
}

fn from_screen(
  gpu: &mut Gpu,
  screen: &[[u8; 4]; SCREEN_W * SCREEN_H],
) -> Result<coffee::graphics::Image> {
  let colors: Vec<[u8; 4]> = screen
    .iter()
    // For now, we just plop the pixel
    .map(|color| *color)
    .collect();

  coffee::graphics::Image::from_image(
    gpu,
    &image::DynamicImage::ImageRgba8(
      image::RgbaImage::from_raw(
        SCREEN_W as u32,
        SCREEN_H as u32,
        colors.iter().flatten().cloned().collect(),
      )
      .unwrap(),
    ),
  )
}

fn from_pattern_table(
  gpu: &mut Gpu,
  pattern_table: &[[u8; 4]; 128 * 128],
) -> Result<coffee::graphics::Image> {
  let colors: Vec<[u8; 4]> = pattern_table
    .iter()
    // For now, we just plop the pixel
    .map(|color| *color)
    .collect();

  coffee::graphics::Image::from_image(
    gpu,
    &image::DynamicImage::ImageRgba8(
      image::RgbaImage::from_raw(
        128 as u32,
        128 as u32,
        colors.iter().flatten().cloned().collect(),
      )
      .unwrap(),
    ),
  )
}

fn from_name_table(
  gpu: &mut Gpu,
  pattern_table: &[[u8; 4]; 256 * 240],
) -> Result<coffee::graphics::Image> {
  let colors: Vec<[u8; 4]> = pattern_table
    .iter()
    // For now, we just plop the pixel
    .map(|color| *color)
    .collect();

  coffee::graphics::Image::from_image(
    gpu,
    &image::DynamicImage::ImageRgba8(
      image::RgbaImage::from_raw(
        256 as u32,
        240 as u32,
        colors.iter().flatten().cloned().collect(),
      )
      .unwrap(),
    ),
  )
}

fn from_palette(gpu: &mut Gpu, palettes: &[[u8; 4]; 4]) -> Result<coffee::graphics::Image> {
  let colors: Vec<[u8; 4]> = palettes
    .iter()
    // For now, we just plop the pixel
    .map(|color| *color)
    .collect();

  coffee::graphics::Image::from_image(
    gpu,
    &image::DynamicImage::ImageRgba8(
      image::RgbaImage::from_raw(
        4 as u32,
        1 as u32,
        colors.iter().flatten().cloned().collect(),
      )
      .unwrap(),
    ),
  )
}
