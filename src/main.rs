#[macro_use]
extern crate maplit;

use coffee::graphics::{Color, Frame, Window, WindowSettings};
use coffee::input::{self, keyboard, Input};
use coffee::load::Task;
use coffee::ui::{Column, Element, Renderer, Row, Text, UserInterface};
use coffee::{Game, Result, Timer};
use std::collections::HashSet;

pub mod bus;
pub mod bus_device;
pub mod cpu6502;
pub mod mirror;
pub mod ram;

use crate::bus::Bus;
use crate::cpu6502::{Processor, StatusFlag, PC_INIT_ADDR, STACK_SIZE};
use crate::ram::Ram;

fn main() -> Result<()> {
    <CPUDebugger as UserInterface>::run(WindowSettings {
        title: String::from("A caffeinated game"),
        size: (1920, 1080),
        resizable: false,
        fullscreen: false,
        maximized: false,
    })
}

struct CPUDebugger {
    cpu: Processor,
}

impl Game for CPUDebugger {
    type Input = CPUDebuggerInput; // No input data
    type LoadingScreen = (); // No loading screen

    fn load(_window: &Window) -> Task<CPUDebugger> {
        // Load your game assets here. Check out the `load` module!
        Task::succeed(|| {
            let mut debugger_ui = CPUDebugger {
                cpu: Processor::new(Bus::new(vec![Box::new(Ram::new(64 * 1024))])),
            };

            let program_start: u16 = 0x8000;

            debugger_ui.cpu.bus.write16(PC_INIT_ADDR, program_start);

            let program: Vec<u8> = vec![
                // Initialize A to 0
                0xA9, 0, // LDA #0
                // Set X to 0x0000 + 0
                0xA2, 0, // LDX #0
                // [0, ...]
                //  ^
                0x95, 0x00, // STA #0
                // Set A to 1
                0xA9, 1, // LDA #1
                // [0, 1, ...]
                //     ^
                0x95, 0x01, // STA #1
                //
                // LOOP:
                //
                // A = RAM[X]
                0xB5, 0x00, // LDA (X + 0)
                // A = A + RAM[X + 1]
                0x75, 0x01, // ADC (X + 1)
                // RAM[X + 2] = A
                0x95, 0x02, // STA (X + 2)
                // Increment X
                0xE8, // INX
                // JMP Loop
                0x4C, 10, 0x80,
            ];
            let mut offset: u16 = 0;
            for byte in program {
                debugger_ui.cpu.bus.write(program_start + offset, byte);
                offset += 1;
            }
            debugger_ui.cpu.sig_reset();
            debugger_ui.cpu.a = 0x01;
            debugger_ui.cpu.step();

            debugger_ui
        })
    }

    fn draw(&mut self, frame: &mut Frame, _timer: &Timer) {
        // Clear the current frame
        frame.clear(Color {
            r: 0.3,
            g: 0.3,
            b: 0.6,
            a: 1.0,
        });

        // Draw your game here. Check out the `graphics` module!
    }

    fn interact(&mut self, input: &mut CPUDebuggerInput, _window: &mut Window) {
        for keypress in &input.keypresses {
            let key = format!("{:?}", keypress);
            if key == "Space" {
                self.cpu.step();
            } else if key == "R" {
                self.cpu.sig_reset();
            }
        }

        input.keypresses.clear();
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

impl UserInterface for CPUDebugger {
    type Message = Message;
    type Renderer = Renderer;
    fn react(&mut self, _event: Message, _window: &mut Window) {
        // Does nothing
    }
    fn layout(&mut self, window: &Window) -> Element<Message> {
        let mut stack_str = String::new();
        for page in 0..(STACK_SIZE / 16) {
            let addr = 0 as u16 + (page as u16) * 16;
            stack_str.push_str(&format!("{:04X}: ", addr));
            for offset in 0..16 {
                stack_str.push_str(&format!("{:02X} ", self.cpu.bus.read(addr + offset)));
            }
            stack_str.push_str("\n");
        }

        let first_pc_page = (self.cpu.pc / 16) as u16;
        let mut ram_str = String::new();
        for page in 0..(STACK_SIZE / 16) {
            let addr = ((first_pc_page + page as u16) as u16) * 16;
            ram_str.push_str(&format!("{:04X}: ", addr));
            for offset in 0..16 {
                let addr = addr + offset;
                if addr == self.cpu.pc {
                    ram_str.push_str(&format!("{:02X}<", self.cpu.bus.read(addr)));
                } else {
                    ram_str.push_str(&format!("{:02X} ", self.cpu.bus.read(addr)));
                }
            }
            ram_str.push_str("\n");
        }

        let left_pane = Column::new()
            .width((window.width() * 0.75) as u32)
            .push(Text::new("---").size(32))
            .push(Text::new(&stack_str).size(32))
            .push(Text::new("---").size(32))
            .push(Text::new(&ram_str).size(32));

        let right_pane = Column::new()
            .push(
                Row::new()
                    .push(Text::new("Status:").size(32))
                    .push(Text::new("C").size(32).color(
                        if self.cpu.get_status(StatusFlag::Carry) != 0x00 {
                            ACTIVE_COLOR
                        } else {
                            INACTIVE_COLOR
                        },
                    ))
                    .push(Text::new("Z").size(32).color(
                        if self.cpu.get_status(StatusFlag::Zero) != 0x00 {
                            ACTIVE_COLOR
                        } else {
                            INACTIVE_COLOR
                        },
                    ))
                    .push(Text::new("I").size(32).color(
                        if self.cpu.get_status(StatusFlag::DisableInterrupts) != 0x00 {
                            ACTIVE_COLOR
                        } else {
                            INACTIVE_COLOR
                        },
                    ))
                    .push(Text::new("B").size(32).color(
                        if self.cpu.get_status(StatusFlag::Break) != 0x00 {
                            ACTIVE_COLOR
                        } else {
                            INACTIVE_COLOR
                        },
                    ))
                    .push(Text::new("O").size(32).color(
                        if self.cpu.get_status(StatusFlag::Overflow) != 0x00 {
                            ACTIVE_COLOR
                        } else {
                            INACTIVE_COLOR
                        },
                    ))
                    .push(Text::new("N").size(32).color(
                        if self.cpu.get_status(StatusFlag::Negative) != 0x00 {
                            ACTIVE_COLOR
                        } else {
                            INACTIVE_COLOR
                        },
                    )),
            )
            .push(Text::new(&format!("PC: {:04X} -", self.cpu.pc)).size(32))
            .push(Text::new(&format!(" A: {:02X} ({})", self.cpu.a, self.cpu.a)).size(32))
            .push(Text::new(&format!(" X: {:02X} ({})", self.cpu.x, self.cpu.x)).size(32))
            .push(Text::new(&format!(" Y: {:02X} ({})", self.cpu.y, self.cpu.y)).size(32));

        Row::new()
            .padding(16)
            .spacing(16)
            .width(window.width() as u32)
            .height(window.height() as u32)
            .push(left_pane)
            .push(right_pane)
            .into()
    }
}

struct CPUDebuggerInput {
    keypresses: HashSet<keyboard::KeyCode>,
}

impl Input for CPUDebuggerInput {
    fn new() -> Self {
        CPUDebuggerInput {
            keypresses: HashSet::new(),
        }
    }
    fn update(&mut self, event: input::Event) {
        match event {
            input::Event::Keyboard(keyboard::Event::Input {
                key_code,
                state: input::ButtonState::Pressed,
            }) => {
                println!("{:?}", key_code);
                self.keypresses.insert(key_code);
            }
            _ => {}
        }
    }
    fn clear(&mut self) {}
}
