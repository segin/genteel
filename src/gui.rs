use crate::audio;
use crate::frontend::{self, InputMapping};
use crate::input::InputScript;
use crate::Emulator;
#[cfg(feature = "gui")]
use pixels::{wgpu, Pixels, SurfaceTexture};
#[cfg(feature = "gui")]
use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

#[cfg(feature = "gui")]
pub struct GuiState {
    pub show_settings: bool,
    pub show_debug: bool,
    pub input_mapping: InputMapping,
    pub integer_scaling: bool,
    pub force_red: bool,
}

#[cfg(feature = "gui")]
pub struct DebugInfo {
    pub m68k_pc: u32,
    pub z80_pc: u16,
    pub frame_count: u64,
    pub vdp_status: u16,
    pub display_enabled: bool,
    pub bg_color_index: u8,
    pub cram0: u16,
}

#[cfg(feature = "gui")]
pub struct Framework {
    pub egui_ctx: egui::Context,
    pub egui_state: egui_winit::State,
    pub screen_descriptor: egui_wgpu::ScreenDescriptor,
    pub renderer: egui_wgpu::Renderer,
    pub gui_state: GuiState,
}

#[cfg(feature = "gui")]
impl Framework {
    pub fn new(
        event_loop: &winit::event_loop::EventLoopWindowTarget<()>,
        width: u32,
        height: u32,
        scale_factor: f32,
        pixels: &pixels::Pixels,
        input_mapping: InputMapping,
    ) -> Self {
        let egui_ctx = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::viewport::ViewportId::ROOT,
            &event_loop,
            Some(scale_factor),
            None,
        );
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: scale_factor,
        };
        let renderer =
            egui_wgpu::Renderer::new(pixels.device(), pixels.render_texture_format(), None, 1);
        let gui_state = GuiState {
            show_settings: false,
            show_debug: false,
            input_mapping,
            integer_scaling: true,
            force_red: false,
        };
        Self {
            egui_ctx,
            egui_state,
            screen_descriptor,
            renderer,
            gui_state,
        }
    }
    pub fn handle_event(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
    ) {
        let _ = self.egui_state.on_window_event(window, event);
    }
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.screen_descriptor.size_in_pixels = [width, height];
        }
    }
    pub fn scale_factor(&mut self, scale_factor: f32) {
        self.screen_descriptor.pixels_per_point = scale_factor;
    }
    pub fn prepare(&mut self, window: &winit::window::Window, debug_info: &DebugInfo) {
        let raw_input = self.egui_state.take_egui_input(window);
        self.egui_ctx.begin_frame(raw_input);
        // Draw the GUI
        egui::TopBottomPanel::top("menubar_container").show(&self.egui_ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        std::process::exit(0);
                    }
                });
                ui.menu_button("Settings", |ui| {
                    if ui.button("Video").clicked() {
                        self.gui_state.show_settings = true;
                        ui.close_menu();
                    }
                    if ui.button("Input Mapping").clicked() {
                        self.gui_state.show_settings = true;
                        ui.close_menu();
                    }
                    ui.checkbox(&mut self.gui_state.show_debug, "Show Performance & Debug");
                });
            });
        });

        if self.gui_state.show_debug {
            egui::Window::new("Performance & Debug").show(&self.egui_ctx, |ui| {
                let dt = self.egui_ctx.input(|i| i.stable_dt);
                let fps = if dt > 0.0 { 1.0 / dt } else { 0.0 };
                ui.label(format!("Frontend FPS: {:.1}", fps));
                ui.label(format!("Frame Time: {:.2}ms", dt * 1000.0));
                ui.separator();
                ui.label(format!("Internal Frames: {}", debug_info.frame_count));
                ui.label(format!("M68k PC: {:06X}", debug_info.m68k_pc));
                ui.label(format!("Z80 PC: {:04X}", debug_info.z80_pc));
                ui.separator();
                ui.label(format!(
                    "VDP Display: {}",
                    if debug_info.display_enabled {
                        "ENABLED"
                    } else {
                        "DISABLED"
                    }
                ));
                ui.label(format!("VDP Status: {:04X}", debug_info.vdp_status));
                ui.label(format!("BG Color Index: {}", debug_info.bg_color_index));
                ui.label(format!("CRAM[0] (RGB565): {:04X}", debug_info.cram0));
                ui.checkbox(&mut self.gui_state.force_red, "Force Red BG (Debug)");
            });
        }

        if self.gui_state.show_settings {
            egui::Window::new("Settings").show(&self.egui_ctx, |ui| {
                ui.heading("Video");
                ui.checkbox(&mut self.gui_state.integer_scaling, "Integer Pixel Scaling");
                ui.separator();
                ui.heading("Input");
                ui.label("Input Mapping:");
                ui.radio_value(
                    &mut self.gui_state.input_mapping,
                    InputMapping::Original,
                    "Original",
                );
                ui.radio_value(
                    &mut self.gui_state.input_mapping,
                    InputMapping::Ergonomic,
                    "Ergonomic",
                );
                ui.separator();
                if ui.button("Close").clicked() {
                    self.gui_state.show_settings = false;
                }
            });
        }
    }
    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        render_target: &wgpu::TextureView,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        let full_output = self.egui_ctx.end_frame();
        let paint_jobs = self
            .egui_ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);
        // Update textures
        for (id, image_delta) in full_output.textures_delta.set {
            self.renderer
                .update_texture(device, queue, id, &image_delta);
        }
        // Prepare renderer
        self.renderer
            .update_buffers(device, queue, encoder, &paint_jobs, &self.screen_descriptor);
        // Render GUI
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: render_target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            self.renderer
                .render(&mut rpass, &paint_jobs, &self.screen_descriptor);
        }
        // Clean up textures
        for id in full_output.textures_delta.free {
            self.renderer.free_texture(&id);
        }
    }
}

#[cfg(feature = "gui")]
pub fn run(mut emulator: Emulator, record_path: Option<String>) -> Result<(), String> {
    if emulator.input_mapping == InputMapping::Original {
        println!("Controls: Arrow keys=D-pad, Z=A, X=B, C=C, Enter=Start");
    } else {
        println!("Controls: WASD/Arrows=D-pad, J/Z=A, K/X=B, L/C=C, U=X, I=Y, O=Z, Enter=Start, Space=Mode");
    }
    println!("Press Escape to quit.");
    if let Some(path) = &record_path {
        println!("Recording inputs to: {}", path);
        emulator.input.start_recording();
    }
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let size = winit::dpi::LogicalSize::new(
        frontend::GENESIS_WIDTH as f64 * 3.0,
        frontend::GENESIS_HEIGHT as f64 * 3.0,
    );
    let window = WindowBuilder::new()
        .with_title("Genteel - Sega Genesis Emulator")
        .with_inner_size(size)
        .with_min_inner_size(winit::dpi::LogicalSize::new(
            frontend::GENESIS_WIDTH as f64,
            frontend::GENESIS_HEIGHT as f64,
        ))
        .build(&event_loop)
        .map_err(|e| e.to_string())?;
    // Leak the window to get a &'static Window, simplifying lifetime management
    let window: &'static winit::window::Window = Box::leak(Box::new(window));
    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, window);
        Pixels::new(320, 240, surface_texture).map_err(|e| e.to_string())?
    };
    // Initialize egui framework
    let mut framework = Framework::new(
        &event_loop,
        window.inner_size().width,
        window.inner_size().height,
        window.scale_factor() as f32,
        &pixels,
        emulator.input_mapping,
    );
    // Audio setup
    let audio_buffer = audio::create_audio_buffer();
    let audio_output = match audio::AudioOutput::new(audio_buffer.clone()) {
        Ok(output) => {
            emulator.bus.borrow_mut().sample_rate = output.sample_rate;
            Some(output)
        }
        Err(e) => {
            eprintln!("Warning: Failed to initialize audio: {}", e);
            None
        }
    };
    let _audio_output = audio_output;
    // Input and Timing state
    let mut input = crate::input::FrameInput::default();
    let mut frame_count: u64 = 0;
    let mut last_frame_inst = std::time::Instant::now();
    let mut fps_timer = std::time::Instant::now();
    let mut fps_count = 0;
    let frame_duration = std::time::Duration::from_nanos(16_666_667); // 60.0 fps
    println!("Starting event loop...");
    event_loop
        .run(move |event, target| {
            match event {
                Event::WindowEvent { event, .. } => {
                    // Handle GUI events
                    framework.handle_event(window, &event);
                    match event {
                        WindowEvent::CloseRequested => {
                            println!("Using CloseRequested to exit");
                            if let Some(path) = &record_path {
                                let script: InputScript = emulator.input.stop_recording();
                                if let Err(e) = script.save(path) {
                                    eprintln!("Failed to save recorded script: {}", e);
                                } else {
                                    println!("Recorded script saved to: {}", path);
                                }
                            }
                            target.exit();
                        }
                        WindowEvent::KeyboardInput {
                            event: key_event, ..
                        } => {
                            // If egui wants focus, don't process game input
                            if framework.egui_ctx.wants_keyboard_input() {
                                return;
                            }
                            let pressed = key_event.state == ElementState::Pressed;
                            // 1. Try physical key first
                            let mut handled = false;
                            if let PhysicalKey::Code(keycode) = key_event.physical_key {
                                if keycode == KeyCode::Escape && pressed {
                                    println!("Escape pressed, exiting");
                                    if let Some(path) = &record_path {
                                        let script: InputScript = emulator.input.stop_recording();
                                        if let Err(e) = script.save(path) {
                                            eprintln!("Failed to save recorded script: {}", e);
                                        } else {
                                            println!("Recorded script saved to: {}", path);
                                        }
                                    }
                                    target.exit();
                                    return;
                                }
                                if let Some((button, _)) =
                                    frontend::keycode_to_button(keycode, emulator.input_mapping)
                                {
                                    input.p1.set_button(button, pressed);
                                    handled = true;
                                }
                            }
                            // 2. Fallback to logical key
                            if !handled {
                                use winit::keyboard::Key;
                                if let Key::Named(named) = key_event.logical_key {
                                    let button = match named {
                                        winit::keyboard::NamedKey::ArrowUp => Some("up"),
                                        winit::keyboard::NamedKey::ArrowDown => Some("down"),
                                        winit::keyboard::NamedKey::ArrowLeft => Some("left"),
                                        winit::keyboard::NamedKey::ArrowRight => Some("right"),
                                        winit::keyboard::NamedKey::Enter => Some("start"),
                                        winit::keyboard::NamedKey::Space => Some("mode"),
                                        _ => None,
                                    };
                                    if let Some(btn) = button {
                                        input.p1.set_button(btn, pressed);
                                    }
                                }
                            }
                        }
                        WindowEvent::Resized(size) => {
                            if size.width > 0 && size.height > 0 {
                                pixels.resize_surface(size.width, size.height).ok();
                                framework.resize(size.width, size.height);
                            }
                        }
                        WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                            framework.scale_factor(scale_factor as f32);
                        }
                        WindowEvent::RedrawRequested => {
                            // Sync settings from GUI
                            emulator.input_mapping = framework.gui_state.input_mapping;
                            let force_red = framework.gui_state.force_red;

                            frame_count += 1;
                            fps_count += 1;
                            // Update FPS in title bar every second
                            if fps_timer.elapsed() >= std::time::Duration::from_secs(1) {
                                window.set_title(&format!(
                                    "Genteel - Sega Genesis Emulator | FPS: {}",
                                    fps_count
                                ));
                                fps_count = 0;
                                fps_timer = std::time::Instant::now();
                            }
                            // Debug: Print every 60 frames
                            if emulator.debug && frame_count % 60 == 1 {
                                emulator.log_debug(frame_count);
                            }
                            // Run one frame of emulation
                            emulator.step_frame(Some(&input));
                            // Process audio
                            if let Ok(mut buf) = audio_buffer.lock() {
                                buf.push(&emulator.audio_buffer);
                            }
                            emulator.audio_buffer.clear();

                            // Collect debug info and render
                            let debug_info = {
                                let mut bus = emulator.bus.borrow_mut();
                                if force_red {
                                    bus.vdp.framebuffer.fill(0xF800); // Red in RGB565
                                }
                                let info = DebugInfo {
                                    m68k_pc: emulator.cpu.pc,
                                    z80_pc: emulator.z80.pc,
                                    frame_count: emulator.internal_frame_count,
                                    vdp_status: bus.vdp.read_status(),
                                    display_enabled: bus.vdp.display_enabled(),
                                    bg_color_index: bus.vdp.registers[7],
                                    cram0: bus.vdp.cram_cache[0],
                                };
                                frontend::rgb565_to_rgba8(&bus.vdp.framebuffer, pixels.frame_mut());
                                info
                            };

                            // Update egui
                            framework.prepare(window, &debug_info);
                            if let Err(e) = pixels.render_with(|encoder, render_target, context| {
                                // Render the board
                                context.scaling_renderer.render(encoder, render_target);
                                // Render GUI
                                framework.render(
                                    encoder,
                                    render_target,
                                    &context.device,
                                    &context.queue,
                                );
                                Ok(())
                            }) {
                                eprintln!("Render error: {}", e);
                                target.exit();
                            }
                        }
                        _ => {}
                    }
                }
                Event::AboutToWait => {
                    let now = std::time::Instant::now();
                    let next_frame = last_frame_inst + frame_duration;
                    if now >= next_frame {
                        last_frame_inst = now;
                        window.request_redraw();
                    }
                    target.set_control_flow(winit::event_loop::ControlFlow::Poll);
                }
                _ => {}
            }
        })
        .map_err(|e| e.to_string())
}
