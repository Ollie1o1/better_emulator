mod bus;
mod cpu;
mod ppu;
mod apu;
mod cartridge;
mod controller;
mod emulator;
mod ui;

use emulator::Emulator;
use controller::buttons;
use ppu::{SCREEN_WIDTH, SCREEN_HEIGHT};
use ui::{BAR_W, BAR_H};

use sdl2::pixels::PixelFormatEnum;
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Scancode};
use sdl2::audio::{AudioQueue, AudioSpecDesired};
use sdl2::rect::Rect;
use sdl2::render::BlendMode;

use std::time::Instant;
use std::path::Path;

const AUDIO_SAMPLE_RATE: u32 = 44100;

// NES pixels are 8:7 — displayed width must be stretched for correct aspect ratio.
// 256 * (8/7) ≈ 292 wide, 240 tall.
const GAME_W: u32 = BAR_W as u32; // 292
const GAME_H: u32 = 240;
const SCALE:  u32 = 3;

const WIN_W: u32 = GAME_W * SCALE;
const WIN_H: u32 = (GAME_H + BAR_H as u32) * SCALE;

fn main() {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: nes <rom.nes>");
        eprintln!();
        eprintln!("Controls:");
        eprintln!("  Z / Alt       = A button");
        eprintln!("  X / Ctrl      = B button");
        eprintln!("  Enter / Space = Start");
        eprintln!("  Shift / Tab   = Select");
        eprintln!("  Arrow keys / WASD = D-pad");
        eprintln!("  Escape        = Quit");
        std::process::exit(1);
    }

    let rom_path = &args[1];
    let rom_name = Path::new(rom_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .replace('_', " ")
        .replace('-', " ");

    let rom_data = match std::fs::read(rom_path) {
        Ok(d) => d,
        Err(e) => { eprintln!("Failed to read '{}': {}", rom_path, e); std::process::exit(1); }
    };

    let mut emu = match Emulator::new(&rom_data, AUDIO_SAMPLE_RATE) {
        Ok(e) => e,
        Err(e) => { eprintln!("Failed to load ROM: {}", e); std::process::exit(1); }
    };

    // ── SDL2 init ──────────────────────────────────────────────────────────────
    let sdl       = sdl2::init().expect("SDL2 init");
    let video     = sdl.video().expect("SDL2 video");
    let audio_sys = sdl.audio().expect("SDL2 audio");

    let window = video
        .window(&format!("NES  —  {}", rom_name), WIN_W, WIN_H)
        .position_centered()
        .resizable()
        .build()
        .expect("Window");

    let mut canvas = window
        .into_canvas()
        .accelerated()
        .present_vsync()
        .build()
        .expect("Canvas");

    // Logical size: the whole window including status bar, in NES-logical pixels.
    canvas.set_logical_size(GAME_W, GAME_H + BAR_H as u32).expect("Logical size");
    canvas.set_blend_mode(BlendMode::None);

    let tc = canvas.texture_creator();

    // Game texture: raw 256×240 NES output.
    let mut game_tex = tc
        .create_texture_streaming(PixelFormatEnum::ARGB8888, SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32)
        .expect("Game texture");

    // Status bar texture: 292×18, updated every frame.
    let mut bar_tex = tc
        .create_texture_streaming(PixelFormatEnum::ARGB8888, BAR_W as u32, BAR_H as u32)
        .expect("Bar texture");

    // CPU-side pixel buffers
    let mut game_buf = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4];
    let mut bar_buf  = vec![0u8; BAR_W * BAR_H * 4];

    // Audio
    let audio_queue: AudioQueue<f32> = audio_sys
        .open_queue(None, &AudioSpecDesired {
            freq: Some(AUDIO_SAMPLE_RATE as i32),
            channels: Some(1),
            samples: Some(512),
        })
        .expect("Audio queue");
    audio_queue.resume();

    let mut event_pump = sdl.event_pump().expect("Event pump");
    let mut audio_buf: Vec<f32> = Vec::with_capacity(4096);

    let mut fps_timer  = Instant::now();
    let mut fps_frames = 0u32;
    let mut fps        = 60.0f32;

    'main: loop {
        // ── Events ────────────────────────────────────────────────────────────
        for event in event_pump.poll_iter() {
            if let Event::Quit { .. } = event { break 'main; }
            if let Event::KeyDown { keycode: Some(Keycode::Escape), .. } = event { break 'main; }
        }

        // ── Controller input ──────────────────────────────────────────────────
        // Multiple keys mapped to each button so the emulator feels natural.
        let ks = event_pump.keyboard_state();
        let mut btn = 0u8;

        if ks.is_scancode_pressed(Scancode::Z)   || ks.is_scancode_pressed(Scancode::LAlt)
                                                 || ks.is_scancode_pressed(Scancode::RAlt)
        { btn |= buttons::A; }

        if ks.is_scancode_pressed(Scancode::X)   || ks.is_scancode_pressed(Scancode::LCtrl)
                                                 || ks.is_scancode_pressed(Scancode::RCtrl)
        { btn |= buttons::B; }

        if ks.is_scancode_pressed(Scancode::Return)
            || ks.is_scancode_pressed(Scancode::KpEnter)
            || ks.is_scancode_pressed(Scancode::Space)
        { btn |= buttons::START; }

        if ks.is_scancode_pressed(Scancode::RShift)
            || ks.is_scancode_pressed(Scancode::LShift)
            || ks.is_scancode_pressed(Scancode::Tab)
            || ks.is_scancode_pressed(Scancode::Backspace)
        { btn |= buttons::SELECT; }

        if ks.is_scancode_pressed(Scancode::Up)    || ks.is_scancode_pressed(Scancode::W) { btn |= buttons::UP; }
        if ks.is_scancode_pressed(Scancode::Down)  || ks.is_scancode_pressed(Scancode::S) { btn |= buttons::DOWN; }
        if ks.is_scancode_pressed(Scancode::Left)  || ks.is_scancode_pressed(Scancode::A) { btn |= buttons::LEFT; }
        if ks.is_scancode_pressed(Scancode::Right) || ks.is_scancode_pressed(Scancode::D) { btn |= buttons::RIGHT; }

        emu.bus.controller1.buttons = btn;

        // ── Emulate one frame ─────────────────────────────────────────────────
        emu.run_frame();

        // ── Post-process game frame: scanlines + contrast boost ────────────────
        {
            let src = emu.bus.ppu.frame_buffer.as_ref();
            for y in 0..SCREEN_HEIGHT {
                // Alternating scanlines darkened 20% — classic CRT grid look.
                let dim: u32 = if y % 2 == 0 { 205 } else { 255 };
                for x in 0..SCREEN_WIDTH {
                    let i = (y * SCREEN_WIDTH + x) * 4;
                    game_buf[i]     = (src[i]     as u32 * dim / 255) as u8;
                    game_buf[i + 1] = (src[i + 1] as u32 * dim / 255) as u8;
                    game_buf[i + 2] = (src[i + 2] as u32 * dim / 255) as u8;
                    game_buf[i + 3] = 0xFF;
                }
            }
        }

        // ── Render status bar into cpu buffer ──────────────────────────────────
        ui::render_status(&mut bar_buf, btn, fps);

        // ── Upload textures ────────────────────────────────────────────────────
        game_tex.update(None, &game_buf, SCREEN_WIDTH * 4).expect("game tex update");
        bar_tex.update(None, &bar_buf, BAR_W * 4).expect("bar tex update");

        // ── Draw ───────────────────────────────────────────────────────────────
        canvas.set_draw_color(sdl2::pixels::Color::RGB(8, 8, 18)); // deep dark background
        canvas.clear();

        // Game — stretched to 292×240 for correct 8:7 pixel aspect ratio.
        canvas.copy(&game_tex, None, Rect::new(0, 0, GAME_W, GAME_H)).expect("game copy");

        // Subtle vignette: darken the outermost pixels of the game area.
        canvas.set_blend_mode(BlendMode::Blend);
        canvas.set_draw_color(sdl2::pixels::Color::RGBA(0, 0, 0, 90));
        canvas.fill_rect(Rect::new(0, 0, GAME_W, 2)).ok();
        canvas.fill_rect(Rect::new(0, (GAME_H - 2) as i32, GAME_W, 2)).ok();
        canvas.fill_rect(Rect::new(0, 0, 2, GAME_H)).ok();
        canvas.fill_rect(Rect::new((GAME_W - 2) as i32, 0, 2, GAME_H)).ok();
        canvas.set_blend_mode(BlendMode::None);

        // Status bar below game.
        canvas.copy(&bar_tex, None, Rect::new(0, GAME_H as i32, GAME_W, BAR_H as u32)).expect("bar copy");

        canvas.present();

        // ── FPS ────────────────────────────────────────────────────────────────
        fps_frames += 1;
        let elapsed = fps_timer.elapsed().as_secs_f32();
        if elapsed >= 1.0 {
            fps = fps_frames as f32 / elapsed;
            fps_timer  = Instant::now();
            fps_frames = 0;
            canvas.window_mut()
                .set_title(&format!("NES  —  {}  |  {:.0} FPS", rom_name, fps))
                .ok();
        }

        // ── Audio ──────────────────────────────────────────────────────────────
        emu.bus.apu.drain_samples(&mut audio_buf);
        // Cap queue to ~50ms to avoid audio lag building up.
        if audio_queue.size() < (AUDIO_SAMPLE_RATE / 20) * 4 {
            audio_queue.queue_audio(&audio_buf).ok();
        }
        audio_buf.clear();
    }
}
