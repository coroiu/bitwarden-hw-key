use bitwarden_hw_key::desktop::{DesktopInput, SyncServer};
use bitwarden_hw_key::{simple_gui, simple_view};
use minifb::{Window, WindowOptions};
use std::time::{Duration, Instant};

const WIDTH: usize = 128;
const HEIGHT: usize = 32;
const SCALE: usize = 8;
const WINDOW_WIDTH: usize = WIDTH * SCALE;
const WINDOW_HEIGHT: usize = HEIGHT * SCALE;

fn main() {
    println!("Starting desktop emulator...");
    println!("Controls: Arrow Up/Down, Space (Middle button)");
    println!("Window size: {}x{} ({}x scale)", WINDOW_WIDTH, WINDOW_HEIGHT, SCALE);

    // Start HTTP server in background thread
    let server = SyncServer::new("127.0.0.1:8080").expect("Failed to start HTTP server");
    let credentials = server.get_credentials_ref();

    std::thread::spawn(move || {
        println!("HTTP server running on http://127.0.0.1:8080");
        println!("Endpoints:");
        println!("  POST /api/sync - Sync credentials (CBOR)");
        println!("  GET  /api/status - Get server status");
        println!("  POST /api/clear - Clear credentials");
        loop {
            if let Err(e) = server.handle_request() {
                eprintln!("HTTP server error: {}", e);
            }
        }
    });

    // Create window
    let mut window = Window::new(
        "Bitwarden HW Key - Desktop Emulator",
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        WindowOptions::default(),
    )
    .unwrap_or_else(|e| {
        panic!("Unable to create window: {}", e);
    });

    // Limit to 60 fps
    window.set_target_fps(60);

    // Create input handler
    let mut input = DesktopInput::new();

    // Create document and canvas
    let mut document = simple_view::create_view(WIDTH as u32, HEIGHT as u32);
    let mut canvas = simple_gui::Canvas::new(WIDTH, HEIGHT);

    // Initialize focus on first focusable component
    document.initialize_focus();

    // Track credential changes
    let mut last_cred_count = 0;

    // Timing
    let mut last_update = Instant::now();
    let mut last_draw = Instant::now();
    let update_interval = Duration::from_millis(25); // 40 fps update rate
    let draw_interval = Duration::from_millis(16); // ~60 fps draw rate

    // Initial render
    document.update();
    document.layout();
    document.draw(&mut canvas);

    // Frame buffer for minifb (scaled)
    let mut frame_buffer: Vec<u32> = vec![0; WINDOW_WIDTH * WINDOW_HEIGHT];

    println!("Emulator started!");

    while window.is_open() {
        let now = Instant::now();

        // Check for new credentials from HTTP server
        {
            let creds = credentials.lock().unwrap();
            if creds.len() != last_cred_count {
                println!(
                    "Credentials updated: {} → {} credentials",
                    last_cred_count,
                    creds.len()
                );
                last_cred_count = creds.len();
                // TODO: Update document with new credentials
                // This will be implemented in Phase 1.3 when we create the credential list view
            }
        }

        // Process input - update input state
        input.process_window(&window);

        // Handle input through document
        document.handle_input(&mut input);

        // Update at ~40 fps
        if now.duration_since(last_update) >= update_interval {
            document.update();
            document.layout();
            last_update = now;
        }

        // Draw at ~60 fps
        if now.duration_since(last_draw) >= draw_interval {
            canvas.clear();
            document.draw(&mut canvas);

            // Convert canvas to frame buffer with 8x scaling
            convert_canvas_to_framebuffer(&canvas, &mut frame_buffer);

            // Update window
            window
                .update_with_buffer(&frame_buffer, WINDOW_WIDTH, WINDOW_HEIGHT)
                .unwrap();

            last_draw = now;
        }
    }

    println!("Emulator closed.");
}

fn convert_canvas_to_framebuffer(canvas: &simple_gui::Canvas, frame_buffer: &mut [u32]) {
    let pixels = &canvas.image_buffer.pixels;

    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let pixel_index = x + y * WIDTH;
            let color = pixels[pixel_index];

            // Convert RGB to u32 format (0xRRGGBB)
            let rgb = ((color.r as u32) << 16) | ((color.g as u32) << 8) | (color.b as u32);

            // Write scaled pixel (8x8 block)
            for sy in 0..SCALE {
                for sx in 0..SCALE {
                    let scaled_x = x * SCALE + sx;
                    let scaled_y = y * SCALE + sy;
                    let scaled_index = scaled_x + scaled_y * WINDOW_WIDTH;
                    frame_buffer[scaled_index] = rgb;
                }
            }
        }
    }
}
