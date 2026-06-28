# Game Boy emulator

A Game Boy (DMG) emulator written in Rust. The core crate has no external
dependencies; the executable links against the system SDL2 library for display
and input.

## Build and run

```sh
cargo build --release
cargo run --release -- path/to/rom.gb
```

Controls: arrow keys for the D-pad, `A`/`S` for A/B, space for Select and
Enter for Start.

## Headless mode

Run a number of frames without opening a window and print a framebuffer
checksum. Useful for smoke tests and benchmarking.

```sh
cargo run --release -- --headless path/to/rom.gb 60
```

## Tests

```sh
cargo test
```
