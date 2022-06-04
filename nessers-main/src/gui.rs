use crate::{cpu6502::NMI_POINTER, disassemble::disassemble, nes::Nes};

use egui::{ClippedMesh, Context, TexturesDelta};
use egui_memory_editor::{option_data::MemoryEditorOptions, MemoryEditor};
use egui_wgpu_backend::{BackendError, RenderPass, ScreenDescriptor};
use pixels::{wgpu, PixelsContext};
use winit::window::Window;

/// Manages all state required for rendering egui over `Pixels`.
pub(crate) struct Framework {
  // State for egui.
  egui_ctx: Context,
  egui_state: egui_winit::State,
  screen_descriptor: ScreenDescriptor,
  rpass: RenderPass,
  paint_jobs: Vec<ClippedMesh>,
  textures: TexturesDelta,

  // State for the GUI
  gui: Gui,
}

/// Example application state. A real application will need a lot more state than this.
struct Gui {
  bus_open: bool,
  bus_editor: MemoryEditor,
  debugger_open: bool,
  search_string: String,
  search_pattern: Option<Vec<u8>>,
}

impl Framework {
  /// Create egui.
  pub(crate) fn new(width: u32, height: u32, scale_factor: f32, pixels: &pixels::Pixels) -> Self {
    let max_texture_size = pixels.device().limits().max_texture_dimension_2d as usize;

    let egui_ctx = Context::default();
    let egui_state = egui_winit::State::from_pixels_per_point(max_texture_size, scale_factor);
    let screen_descriptor = ScreenDescriptor {
      physical_width: width,
      physical_height: height,
      scale_factor,
    };
    let rpass = RenderPass::new(pixels.device(), pixels.render_texture_format(), 1);
    let textures = TexturesDelta::default();
    let gui = Gui::new();

    Self {
      egui_ctx,
      egui_state,
      screen_descriptor,
      rpass,
      paint_jobs: Vec::new(),
      textures,
      gui,
    }
  }

  /// Handle input events from the window manager.
  pub(crate) fn handle_event(&mut self, event: &winit::event::WindowEvent) {
    self.egui_state.on_event(&self.egui_ctx, event);
  }

  /// Resize egui.
  pub(crate) fn resize(&mut self, width: u32, height: u32) {
    if width > 0 && height > 0 {
      self.screen_descriptor.physical_width = width;
      self.screen_descriptor.physical_height = height;
    }
  }

  /// Update scaling factor.
  pub(crate) fn scale_factor(&mut self, scale_factor: f64) {
    self.screen_descriptor.scale_factor = scale_factor as f32;
  }

  /// Prepare egui.
  pub(crate) fn prepare(&mut self, window: &Window, nes: &mut Nes, egui_has_focus: &mut bool) {
    // Run the egui frame and create all paint jobs to prepare for rendering.
    let raw_input = self.egui_state.take_egui_input(window);
    let mut result = false;
    let output = self.egui_ctx.run(raw_input, |egui_ctx| {
      // Draw the demo application.
      *egui_has_focus = self.gui.ui(egui_ctx, nes);
    });

    self.textures.append(output.textures_delta);
    self
      .egui_state
      .handle_platform_output(window, &self.egui_ctx, output.platform_output);
    self.paint_jobs = self.egui_ctx.tessellate(output.shapes);
  }

  /// Render egui.
  pub(crate) fn render(
    &mut self,
    encoder: &mut wgpu::CommandEncoder,
    render_target: &wgpu::TextureView,
    context: &PixelsContext,
  ) -> Result<(), BackendError> {
    // Upload all resources to the GPU.
    self
      .rpass
      .add_textures(&context.device, &context.queue, &self.textures)?;
    self.rpass.update_buffers(
      &context.device,
      &context.queue,
      &self.paint_jobs,
      &self.screen_descriptor,
    );

    // Record all render passes.
    self.rpass.execute(
      encoder,
      render_target,
      &self.paint_jobs,
      &self.screen_descriptor,
      None,
    )?;

    // Cleanup
    let textures = std::mem::take(&mut self.textures);
    self.rpass.remove_textures(textures)
  }
}

impl Gui {
  /// Create a `Gui`.
  fn new() -> Self {
    let mut opts = MemoryEditorOptions::default();
    opts.is_options_collapsed = true;
    opts.show_ascii = false;
    let bus_editor = MemoryEditor::new()
      .with_window_title("Bus editor")
      .with_options(opts)
      .with_address_range("All", 0..0xFFFF);
    Self {
      bus_open: false,
      debugger_open: false,
      bus_editor,
      search_string: String::new(),
      search_pattern: None,
    }
  }

  /// Create the UI using egui.
  ///
  /// Returns `true` if any egui widget has focus.
  fn ui(&mut self, ctx: &Context, nes: &mut Nes) -> bool {
    egui::TopBottomPanel::top("menubar_container").show(ctx, |ui| {
      egui::menu::bar(ui, |ui| {
        ui.menu_button("Debug", |ui| {
          if ui.button("Bus editor").clicked() {
            self.bus_open = true;
            ui.close_menu();
          }

          if ui.button("Debugger").clicked() {
            self.debugger_open = true;
            ui.close_menu();
          }
        })
      });
    });

    let mut bytes: Vec<u8> = vec![];

    if self.search_string.len() > 0 {
      let mut nybble_idx: usize = 0;
      for i in 0..self.search_string.len() {
        let maybe_nybble: Option<u8> = match self.search_string.chars().nth(i).unwrap() {
          '0' => Some(0x00),
          '1' => Some(0x01),
          '2' => Some(0x02),
          '3' => Some(0x03),
          '4' => Some(0x04),
          '5' => Some(0x05),
          '6' => Some(0x06),
          '7' => Some(0x07),
          '8' => Some(0x08),
          '9' => Some(0x09),
          'a' | 'A' => Some(0x0A),
          'b' | 'B' => Some(0x0B),
          'c' | 'C' => Some(0x0C),
          'd' | 'D' => Some(0x0D),
          'e' | 'E' => Some(0x0E),
          'f' | 'F' => Some(0x0F),
          _ => None,
        };

        if let Some(nybble) = maybe_nybble {
          let byte_idx = nybble_idx / 2;
          if byte_idx >= bytes.len() {
            bytes.push(0x00);
          }
          let hi_nybble = nybble_idx % 2 == 0;
          if hi_nybble {
            bytes[byte_idx] |= nybble << 4;
          } else {
            bytes[byte_idx] |= nybble;
          }
          nybble_idx += 1;
        }
      }
    }

    if bytes.len() > 0 {
      self.search_pattern = Some(bytes);
    } else {
      self.search_pattern = None;
    }

    egui::Window::new("Bus editor")
      .open(&mut self.bus_open)
      .show(ctx, |ui| {
        ui.label("Search:");
        ui.text_edit_singleline(&mut self.search_string);
        if let Some(search_pattern) = self.search_pattern.clone() {
          ui.label(
            search_pattern
              .iter()
              .map(|b| format!("{:02X}", b))
              .collect::<Vec<String>>()
              .join(" "),
          );
        }

        self.bus_editor.draw_editor_contents(
          ui,
          // &mut self.bus_open,
          nes,
          // Read
          |nes, addr| Some(nes.safe_cpu_read(addr as u16)),
          // Write
          |nes, addr, value| nes.cpu_write(addr as u16, value),
          // Highlight
          |nes, addr| match &self.search_pattern {
            Some(pattern) => {
              // Read ahead until we hit something that isn't in our pattern
              for i in 0..pattern.len() {
                let byte = nes.safe_cpu_read((addr + i) as u16);
                if byte != pattern[i] {
                  return None;
                }
              }

              // ...if we get here then we know our pattern matches the next N
              // bytes:
              Some((
                pattern.len(),
                egui::Color32::LIGHT_RED,
                egui::Color32::BLACK,
              ))
            }
            None => None,
          },
        )
      });

    egui::Window::new("Debugger")
      .open(&mut self.debugger_open)
      .show(ctx, |ui| {
        let disassembled = disassemble(nes, nes.cpu.pc, 128);
        let mut disassembled_output: Vec<String> = vec![];
        let mut pc_idx: i32 = 0;
        let mut idx: i32 = 0;
        for o in disassembled {
          let current = nes.cpu.pc == o.addr;
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
        ui.code(format!(
          "PC: {:04X}        PPU: {:02X} {:08b}",
          nes.cpu.pc, nes.ppu.status, nes.ppu.status
        ));
        ui.code(format!(
          " A: {:02X} ({:03})   CTRL: {:02X} {:08b}",
          nes.cpu.a, nes.cpu.a, nes.ppu.control, nes.ppu.control
        ));
        ui.code(format!(
          " X: {:02X} ({:03})   MASK: {:02X} {:08b}",
          nes.cpu.x, nes.cpu.x, nes.ppu.mask, nes.ppu.mask
        ));
        ui.code(format!(
          " Y: {:02X} ({:03})    NMI: {:04X}",
          nes.cpu.y,
          nes.cpu.y,
          nes.safe_cpu_read16(NMI_POINTER)
        ));
        ui.code(format!(
          "SP: {:02X} ({:03})   ADDR: {:04X}",
          nes.cpu.s, nes.cpu.s, nes.ppu.vram_addr
        ));
        ui.code(disassembled_output.join("\n"));
      });

    // It's not obvious at all but this checks to see if any UI has focus, and
    // if it does, returns `Some(...)`.
    //
    // We only want to capture keyboard input while no UI has focus, in case
    // we're using the keyboard to enter data or navigate the UI.
    match ctx.memory().focus() {
      None => false,
      Some(_) => true,
    }
  }
}
