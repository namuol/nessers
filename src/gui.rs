use crate::nes::Nes;

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
  memory_editor: MemoryEditor,
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
    let memory_editor = MemoryEditor::new()
      .with_window_title("Bus editor")
      .with_options(opts)
      .with_address_range("All", 0..0xFFFF);
    Self {
      bus_open: false,
      memory_editor,
    }
  }

  /// Create the UI using egui.
  ///
  /// Returns `true` if any egui widget has focus.
  fn ui(&mut self, ctx: &Context, nes: &mut Nes) -> bool {
    egui::TopBottomPanel::top("menubar_container").show(ctx, |ui| {
      egui::menu::bar(ui, |ui| {
        ui.menu_button("File", |ui| {
          if ui.button("Bus editor").clicked() {
            self.bus_open = true;
            ui.close_menu();
          }
        })
      });
    });

    self.memory_editor.window_ui(
      ctx,
      &mut self.bus_open,
      nes,
      |nes, address| Some(nes.safe_cpu_read(address as u16)),
      |nes, address, value| nes.cpu_write(address as u16, value),
    );

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
