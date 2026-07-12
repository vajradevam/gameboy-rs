# Test ROMs

Game Boy hardware test ROMs by Shay Green (blargg), collected in
[retrio/gb-test-roms](https://github.com/retrio/gb-test-roms). They are freely
distributed for emulator development and testing.

| ROM | What it exercises |
| --- | --- |
| `cpu_instrs.gb` | Full CPU instruction validation |
| `individual/01`-`11` | Individual CPU instruction groups |
| `instr_timing.gb` | Instruction cycle timing |
| `mem_timing.gb` | Memory access timing |
| `halt_bug.gb` | HALT instruction bug behaviour |
| `interrupt_time.gb` | Interrupt timing |
| `oam_bug.gb` | OAM/Sprite timing quirks |

Run one with:

```sh
cargo run --release -- ROMS/cpu_instrs.gb
```

Or headlessly (no window). The headless mode prints anything a test ROM sends
over the serial link, which is where blargg's suites report their results:

```sh
cargo run --release -- --headless ROMS/cpu_instrs.gb 30000
```

`cpu_instrs.gb`, `instr_timing.gb`, `mem_timing.gb` and `halt_bug.gb` all
report `Passed`. Memory writes and reads are timed at the correct cycle within
each instruction.

Some ROMs only print results on screen. Set `GB_FRAME_DUMP` to a path in
headless mode to write the final framebuffer as a PPM image:

```sh
GB_FRAME_DUMP=frame.ppm cargo run --release -- --headless ROMS/halt_bug.gb 8000
```
