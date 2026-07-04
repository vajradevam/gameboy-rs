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

Or headlessly (no window):

```sh
cargo run --release -- --headless ROMS/cpu_instrs.gb 60
```
