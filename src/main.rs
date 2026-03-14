mod bus;
mod cpu;
mod ppu;
mod apu;
mod cartridge;
mod controller;
mod emulator;

use emulator::Emulator;
use controller::buttons;
use ppu::{SCREEN_WIDTH, SCREEN_HEIGHT};

use sdl2::pixels::PixelFormatEnum;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::audio::{AudioQueue, AudioSpecDesired};
use sdl2::rect::Rect;
use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use std::time::Instant;
use std::path::Path;

const AUDIO_SAMPLE_RATE: u32 = 44100;

// NES pixels are 8:7 aspect — displayed width should be wider than 256 raw pixels.
// We render at 3x integer scale then use SDL logical size to correct aspect ratio.
const GAME_W: u32 = 292; // 256 * (8/7) ≈ 292 for correct aspect ratio
const GAME_H: u32 = 240;
const SCALE: u32 = 3;

const WIN_W: u32 = GAME_W * SCALE;
const WIN_H: u32 = GAME_H * SCALE + STATUS_H * SCALE;
const STATUS_H: u32 = 12; // logical height of the bottom status bar

// Color theme
const COL_BG:       Color = Color::RGB(10,  10,  20);   // window background
const COL_BAR_BG:   Color = Color::RGB(18,  18,  36);   // status bar background
const COL_BAR_LINE: Color = Color::RGB(60,  60, 100);   // separator line
const COL_SCANLINE: Color = Color::RGBA(0,   0,   0, 55); // scanline overlay

fn main() {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: nes <rom.nes>");
        std::process::exit(1);
    }

    let rom_path = &args[1];
    let rom_name = Path::new(rom_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    let rom_data = match std::fs::read(rom_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to read ROM '{}': {}", rom_path, e);
            std::process::exit(1);
        }
    };

    let mut emu = match Emulator::new(&rom_data, AUDIO_SAMPLE_RATE) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Failed to load ROM: {}", e);
            std::process::exit(1);
        }
    };

    // SDL2 init
    let sdl = sdl2::init().expect("SDL2 init failed");
    let video = sdl.video().expect("SDL2 video failed");
    let audio_sys = sdl.audio().expect("SDL2 audio failed");

    let window = video
        .window(&format!("NES — {}", rom_name), WIN_W, WIN_H)
        .position_centered()
        .resizable()
        .build()
        .expect("Window creation failed");

    let mut canvas = window
        .into_canvas()
        .accelerated()
        .present_vsync()
        .build()
        .expect("Canvas creation failed");

    // Logical size covers the full window including status bar
    canvas
        .set_logical_size(GAME_W + 0, GAME_H + STATUS_H)
        .expect("Logical size failed");

    canvas.set_blend_mode(BlendMode::Blend);

    let texture_creator = canvas.texture_creator();

    // Streaming texture for game output (raw 256×240, will be stretched to GAME_W×GAME_H)
    let mut game_tex = texture_creator
        .create_texture_streaming(PixelFormatEnum::ARGB8888, SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32)
        .expect("Game texture failed");

    // Render buffer — holds the processed (scanline-applied) frame
    let mut render_buf = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4];

    let desired_audio = AudioSpecDesired {
        freq: Some(AUDIO_SAMPLE_RATE as i32),
        channels: Some(1),
        samples: Some(512),
    };
    let audio_queue: AudioQueue<f32> = audio_sys
        .open_queue(None, &desired_audio)
        .expect("Audio queue failed");
    audio_queue.resume();

    let mut event_pump = sdl.event_pump().expect("Event pump failed");
    let mut audio_buf: Vec<f32> = Vec::with_capacity(4096);

    // FPS tracking
    let mut fps_timer = Instant::now();
    let mut frame_count = 0u32;
    let mut fps_display = 60.0f32;

    'main: loop {
        // ── Input ──────────────────────────────────────────────────────────
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    break 'main;
                }
                _ => {}
            }
        }

        let mut btn = 0u8;
        let ks = event_pump.keyboard_state();
        if ks.is_scancode_pressed(sdl2::keyboard::Scancode::Z)         { btn |= buttons::A; }
        if ks.is_scancode_pressed(sdl2::keyboard::Scancode::X)         { btn |= buttons::B; }
        if ks.is_scancode_pressed(sdl2::keyboard::Scancode::Return)    { btn |= buttons::START; }
        if ks.is_scancode_pressed(sdl2::keyboard::Scancode::RShift)    { btn |= buttons::SELECT; }
        if ks.is_scancode_pressed(sdl2::keyboard::Scancode::Up)        { btn |= buttons::UP; }
        if ks.is_scancode_pressed(sdl2::keyboard::Scancode::Down)      { btn |= buttons::DOWN; }
        if ks.is_scancode_pressed(sdl2::keyboard::Scancode::Left)      { btn |= buttons::LEFT; }
        if ks.is_scancode_pressed(sdl2::keyboard::Scancode::Right)     { btn |= buttons::RIGHT; }
        emu.bus.controller1.buttons = btn;

        // ── Emulate one frame ───────────────────────────────────────────────
        emu.run_frame();

        // ── Post-process frame: apply scanlines ─────────────────────────────
        {
            let src = emu.bus.ppu.frame_buffer.as_ref();
            for y in 0..SCREEN_HEIGHT {
                // Every other scanline is dimmed to create a CRT grid effect
                let brightness = if y % 2 == 0 { 210u32 } else { 255u32 };
                for x in 0..SCREEN_WIDTH {
                    let i = (y * SCREEN_WIDTH + x) * 4;
                    render_buf[i]     = (src[i]     as u32 * brightness / 255) as u8; // B
                    render_buf[i + 1] = (src[i + 1] as u32 * brightness / 255) as u8; // G
                    render_buf[i + 2] = (src[i + 2] as u32 * brightness / 255) as u8; // R
                    render_buf[i + 3] = 0xFF;
                }
            }
        }

        // ── Render ──────────────────────────────────────────────────────────
        // Dark window background
        canvas.set_draw_color(COL_BG);
        canvas.clear();

        // Upload processed frame to texture
        game_tex
            .update(None, &render_buf, SCREEN_WIDTH * 4)
            .expect("Texture update failed");

        // Draw game area stretched to correct 8:7 aspect ratio
        let game_rect = Rect::new(0, 0, GAME_W, GAME_H);
        canvas.copy(&game_tex, None, game_rect).expect("Canvas copy failed");

        // Subtle vignette: darken the very edges of the game area
        canvas.set_draw_color(Color::RGBA(0, 0, 0, 80));
        // Left edge
        canvas.fill_rect(Rect::new(0, 0, 4, GAME_H)).ok();
        // Right edge
        canvas.fill_rect(Rect::new((GAME_W - 4) as i32, 0, 4, GAME_H)).ok();
        // Top edge
        canvas.fill_rect(Rect::new(0, 0, GAME_W, 3)).ok();
        // Bottom edge (just above status bar)
        canvas.fill_rect(Rect::new(0, (GAME_H - 3) as i32, GAME_W, 3)).ok();

        // Status bar background
        let bar_y = GAME_H as i32;
        canvas.set_draw_color(COL_BAR_BG);
        canvas.fill_rect(Rect::new(0, bar_y, GAME_W, STATUS_H)).ok();

        // Separator line between game and status bar
        canvas.set_draw_color(COL_BAR_LINE);
        canvas.draw_line(
            sdl2::rect::Point::new(0, bar_y),
            sdl2::rect::Point::new(GAME_W as i32, bar_y),
        ).ok();

        canvas.present();

        // ── FPS tracking & window title ─────────────────────────────────────
        frame_count += 1;
        let elapsed = fps_timer.elapsed().as_secs_f32();
        if elapsed >= 1.0 {
            fps_display = frame_count as f32 / elapsed;
            fps_timer = Instant::now();
            frame_count = 0;
            let title = format!("NES — {}  |  {:.0} FPS", rom_name, fps_display);
            canvas.window_mut().set_title(&title).ok();
        }

        // ── Audio ───────────────────────────────────────────────────────────
        emu.bus.apu.drain_samples(&mut audio_buf);
        let max_queue_bytes = AUDIO_SAMPLE_RATE / 20 * 4; // ~50ms buffer cap
        if audio_queue.size() < max_queue_bytes {
            audio_queue.queue_audio(&audio_buf).ok();
        }
        audio_buf.clear();
    }
}
