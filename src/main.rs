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

const SCALE: u32 = 3;
const AUDIO_SAMPLE_RATE: u32 = 44100;

fn main() {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: nes <rom.nes>");
        std::process::exit(1);
    }

    let rom_data = match std::fs::read(&args[1]) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to read ROM '{}': {}", args[1], e);
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

    // SDL2 setup
    let sdl = sdl2::init().expect("SDL2 init failed");
    let video = sdl.video().expect("SDL2 video failed");
    let audio_sys = sdl.audio().expect("SDL2 audio failed");

    let window = video
        .window(
            "NES Emulator",
            SCREEN_WIDTH as u32 * SCALE,
            SCREEN_HEIGHT as u32 * SCALE,
        )
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

    canvas
        .set_logical_size(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32)
        .expect("Logical size failed");

    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(
            PixelFormatEnum::ARGB8888,
            SCREEN_WIDTH as u32,
            SCREEN_HEIGHT as u32,
        )
        .expect("Texture creation failed");

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

    'main: loop {
        // --- Input ---
        let mut btn = 0u8;
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'main,
                Event::KeyDown { keycode: Some(Keycode::Escape), .. } => break 'main,
                _ => {}
            }
        }

        // Continuous key state
        let keys: std::collections::HashSet<Keycode> = event_pump
            .keyboard_state()
            .pressed_scancodes()
            .filter_map(Keycode::from_scancode)
            .collect();

        if keys.contains(&Keycode::Z)      { btn |= buttons::A; }
        if keys.contains(&Keycode::X)      { btn |= buttons::B; }
        if keys.contains(&Keycode::Return) { btn |= buttons::START; }
        if keys.contains(&Keycode::RShift) { btn |= buttons::SELECT; }
        if keys.contains(&Keycode::Up)     { btn |= buttons::UP; }
        if keys.contains(&Keycode::Down)   { btn |= buttons::DOWN; }
        if keys.contains(&Keycode::Left)   { btn |= buttons::LEFT; }
        if keys.contains(&Keycode::Right)  { btn |= buttons::RIGHT; }

        emu.bus.controller1.buttons = btn;

        // --- Emulate one frame ---
        emu.run_frame();

        // --- Render ---
        texture
            .update(None, emu.bus.ppu.frame_buffer.as_ref(), SCREEN_WIDTH * 4)
            .expect("Texture update failed");
        canvas.copy(&texture, None, None).expect("Canvas copy failed");
        canvas.present();

        // --- Audio ---
        emu.bus.apu.drain_samples(&mut audio_buf);
        // Keep audio queue from growing too large (buffer about 2 frames)
        let max_queue = AUDIO_SAMPLE_RATE / 30;
        if audio_queue.size() < max_queue * 4 {
            audio_queue.queue_audio(&audio_buf).ok();
        }
        audio_buf.clear();
    }
}
