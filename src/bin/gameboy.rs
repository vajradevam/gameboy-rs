//! SDL2 front-end. The core has no dependencies; this binary links directly
//! against the system SDL2 library so the crate builds without network access.

use gameboy::Emulator;
use std::ffi::CString;
use std::os::raw::{c_char, c_void};

const SDL_INIT_VIDEO: u32 = 0x0000_0020;
const SDL_WINDOW_SHOWN: u32 = 0x0000_0004;
const SDL_RENDERER_ACCELERATED: u32 = 0x0000_0002;
const SDL_TEXTUREACCESS_STREAMING: i32 = 1;
const SDL_PIXELFORMAT_RGB24: u32 = 386_930_691;
const SDL_QUIT: u32 = 0x0000_0100;

const FRAME_INTERVAL_MS: u32 = 17;

#[link(name = "SDL2")]
extern "C" {
    fn SDL_Init(flags: u32) -> i32;
    fn SDL_Quit();
    fn SDL_CreateWindow(
        title: *const c_char,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        flags: u32,
    ) -> *mut c_void;
    fn SDL_DestroyWindow(window: *mut c_void);
    fn SDL_CreateRenderer(window: *mut c_void, index: i32, flags: u32) -> *mut c_void;
    fn SDL_DestroyRenderer(renderer: *mut c_void);
    fn SDL_CreateTexture(
        renderer: *mut c_void,
        format: u32,
        access: i32,
        w: i32,
        h: i32,
    ) -> *mut c_void;
    fn SDL_DestroyTexture(texture: *mut c_void);
    fn SDL_UpdateTexture(
        texture: *mut c_void,
        rect: *const c_void,
        pixels: *const c_void,
        pitch: i32,
    ) -> i32;
    fn SDL_RenderClear(renderer: *mut c_void) -> i32;
    fn SDL_RenderCopy(
        renderer: *mut c_void,
        texture: *mut c_void,
        src: *const c_void,
        dst: *const c_void,
    ) -> i32;
    fn SDL_RenderPresent(renderer: *mut c_void);
    fn SDL_PollEvent(event: *mut c_void) -> i32;
    fn SDL_GetKeyboardState(num_keys: *mut i32) -> *const u8;
    fn SDL_GetTicks() -> u32;
    fn SDL_Delay(ms: u32);
}

/// Scancode -> joypad bit, matching the original key bindings.
const KEY_MAP: [(u8, u8); 8] = [
    (4, 4),  // A key
    (22, 5), // S key
    (44, 6), // space
    (40, 7), // enter
    (79, 0), // right
    (80, 1), // left
    (82, 2), // up
    (81, 3), // down
];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: {} [--headless] <rom> [frames]", args[0]);
        std::process::exit(1);
    }

    let headless = args[1] == "--headless";
    let rom_path = if headless {
        if args.len() < 3 {
            eprintln!("usage: {} --headless <rom> [frames]", args[0]);
            std::process::exit(1);
        }
        &args[2]
    } else {
        &args[1]
    };

    let mut emulator = Box::new(Emulator::new());
    if !emulator.load_rom(rom_path) || !emulator.init_game() {
        eprintln!("could not initialise {}", rom_path);
        std::process::exit(1);
    }

    if headless {
        let frames: u32 = args
            .get(3)
            .and_then(|s| s.parse().ok())
            .unwrap_or(60);
        for _ in 0..frames {
            emulator.update();
        }
        let checksum = emulator
            .screen()
            .iter()
            .flatten()
            .flatten()
            .enumerate()
            .fold(0u32, |acc, (i, channel)| {
                acc.wrapping_add((*channel as u32).wrapping_mul((i % 251) as u32 + 1))
            });
        println!(
            "frames={} opcodes={} pc={:#06x} checksum={:#010x}",
            frames,
            emulator.total_opcodes(),
            emulator.program_counter(),
            checksum
        );
        return;
    }

    unsafe { run_window(&mut emulator) };
}

unsafe fn run_window(emulator: &mut Emulator) {
    if SDL_Init(SDL_INIT_VIDEO) < 0 {
        eprintln!("SDL_Init failed");
        std::process::exit(1);
    }

    let title = CString::new("Game Boy").unwrap();
    let window = SDL_CreateWindow(title.as_ptr(), 100, 100, 480, 432, SDL_WINDOW_SHOWN);
    if window.is_null() {
        eprintln!("SDL_CreateWindow failed");
        SDL_Quit();
        std::process::exit(1);
    }

    let renderer = SDL_CreateRenderer(window, -1, SDL_RENDERER_ACCELERATED);
    if renderer.is_null() {
        SDL_DestroyWindow(window);
        SDL_Quit();
        eprintln!("SDL_CreateRenderer failed");
        std::process::exit(1);
    }

    let texture = SDL_CreateTexture(
        renderer,
        SDL_PIXELFORMAT_RGB24,
        SDL_TEXTUREACCESS_STREAMING,
        gameboy::WIDTH as i32,
        gameboy::HEIGHT as i32,
    );
    if texture.is_null() {
        SDL_DestroyRenderer(renderer);
        SDL_DestroyWindow(window);
        SDL_Quit();
        eprintln!("SDL_CreateTexture failed");
        std::process::exit(1);
    }

    let mut held = [false; 8];
    let mut next_frame = SDL_GetTicks();

    'outer: loop {
        let mut event = [0u8; 128];
        while SDL_PollEvent(event.as_mut_ptr() as *mut c_void) != 0 {
            let kind = u32::from_ne_bytes([event[0], event[1], event[2], event[3]]);
            if kind == SDL_QUIT {
                break 'outer;
            }
        }

        let mut num_keys = 0i32;
        let keyboard = SDL_GetKeyboardState(&mut num_keys);
        for &(scancode, key) in KEY_MAP.iter() {
            let pressed = (scancode as usize) < num_keys as usize
                && *keyboard.add(scancode as usize) != 0;
            if pressed && !held[key as usize] {
                emulator.key_pressed(key);
            } else if !pressed && held[key as usize] {
                emulator.key_released(key);
            }
            held[key as usize] = pressed;
        }

        let now = SDL_GetTicks();
        if now >= next_frame {
            emulator.update();
            let screen = emulator.screen();
            SDL_UpdateTexture(
                texture,
                std::ptr::null(),
                screen.as_ptr() as *const c_void,
                (gameboy::WIDTH * 3) as i32,
            );
            SDL_RenderClear(renderer);
            SDL_RenderCopy(renderer, texture, std::ptr::null(), std::ptr::null());
            SDL_RenderPresent(renderer);
            next_frame = now.saturating_add(FRAME_INTERVAL_MS);
        }

        SDL_Delay(1);
    }

    SDL_DestroyTexture(texture);
    SDL_DestroyRenderer(renderer);
    SDL_DestroyWindow(window);
    SDL_Quit();
}
