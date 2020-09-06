#![deny(clippy::all)]
#![forbid(unsafe_code)]

use pixels::{Error, Pixels, SurfaceTexture};
use std::time::Instant;
use the_sprawl::constants::*;
use the_sprawl::data;
use the_sprawl::world::World;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalSize};
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit_input_helper::WinitInputHelper;

fn main() -> Result<(), Error> {
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    let (window, p_width, p_height, mut _hidpi_factor) = create_window("The Sprawl", &event_loop);

    let surface_texture = SurfaceTexture::new(p_width, p_height, &window);

    let all_stations = data::load_stations();
    let connections = data::load_connections();
    let mut time = Instant::now();
    let mut world = World::new(all_stations, connections);

    world.init();

    let mut pixels = Pixels::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32, surface_texture)?;

    event_loop.run(move |event, _, control_flow| {
        // Draw the current frame
        if let Event::RedrawRequested(_) = event {
            world.draw(pixels.get_frame());
            if pixels
                .render()
                .map_err(|e| eprintln!("pixels.render() failed: {}", e))
                .is_err()
            {
                *control_flow = ControlFlow::Exit;
                return;
            }
        }

        window.request_redraw();

        // Handle input events
        if input.update(&event) {
            // Close events
            if input.key_pressed(VirtualKeyCode::Escape) || input.quit() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                pixels.resize(size.width, size.height);
            }

            let (mouse_cell, mouse_prev_cell) = input
                .mouse()
                .map(|(mx, my)| {
                    let (dx, dy) = input.mouse_diff();
                    let prev_x = mx - dx;
                    let prev_y = my - dy;

                    let (mx_i, my_i) = pixels
                        .window_pos_to_pixel((mx, my))
                        .unwrap_or_else(|pos| pixels.clamp_pixel_pos(pos));

                    let (px_i, py_i) = pixels
                        .window_pos_to_pixel((prev_x, prev_y))
                        .unwrap_or_else(|pos| pixels.clamp_pixel_pos(pos));

                    (
                        (mx_i as isize, my_i as isize),
                        (px_i as isize, py_i as isize),
                    )
                })
                .unwrap_or_default();

            // if input.mouse_pressed(0) {
            //     world.inspect(mouse_cell);
            // }

            if input.mouse_held(0) {
                let diff_x = mouse_cell.0 - mouse_prev_cell.0;
                let diff_y = mouse_cell.1 - mouse_prev_cell.1;
                world.pan(diff_x, diff_y);
            }

            let scroll_diff = input.scroll_diff();
            if (scroll_diff - 0.0).abs() > f32::EPSILON {
                world.zoom(mouse_cell, scroll_diff);
            }

            let now = Instant::now();
            let dt = now.duration_since(time);
            time = now;

            world.update(&dt);
        }
    });
}

// This is taken from the Pixels example at https://github.com/parasyte/pixels/tree/master/examples/conway
fn create_window(
    title: &str,
    event_loop: &EventLoop<()>,
) -> (winit::window::Window, u32, u32, f64) {
    // Create a hidden window so we can estimate a good default window size
    let window = winit::window::WindowBuilder::new()
        .with_visible(false)
        .with_title(title)
        .build(&event_loop)
        .unwrap();
    let hidpi_factor = window.scale_factor();

    // Get dimensions
    let width = SCREEN_WIDTH as f64;
    let height = SCREEN_HEIGHT as f64;
    let (monitor_width, monitor_height) = {
        let size = window.current_monitor().size();
        (
            size.width as f64 / hidpi_factor,
            size.height as f64 / hidpi_factor,
        )
    };
    let scale = (monitor_height / height * 2.0 / 3.0).round();

    // Resize, center, and display the window
    let min_size: winit::dpi::LogicalSize<f64> =
        PhysicalSize::new(width, height).to_logical(hidpi_factor);
    let default_size = LogicalSize::new(width * scale, height * scale);
    let center = LogicalPosition::new(
        (monitor_width - width * scale) / 2.0,
        (monitor_height - height * scale) / 2.0,
    );
    window.set_inner_size(default_size);
    window.set_min_inner_size(Some(min_size));
    window.set_outer_position(center);
    window.set_visible(true);

    let size = default_size.to_physical::<f64>(hidpi_factor);

    (
        window,
        size.width.round() as u32,
        size.height.round() as u32,
        hidpi_factor,
    )
}
