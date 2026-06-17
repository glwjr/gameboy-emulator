# Game Boy Emulator

A Game Boy (DMG) emulator written in Rust. It boots and plays
commercial cartridges. The primary test target has been _The Legend of Zelda:
Link's Awakening_.

## Status

The emulator is playable; it runs the full boot sequence, renders background and
sprite graphics, accepts joypad input, and drives the game's main loop through
an accurate interrupt and timer system.

| Subsystem            | Status                                             |
| -------------------- | -------------------------------------------------- |
| CPU (SM83 core)      | Complete - all opcodes, passes Blargg `cpu_instrs` |
| Memory + MBC1 mapper | Complete (512KB / 32-bank cartridges)              |
| Interrupts           | Complete - VBlank, STAT, timer, with EI delay      |
| Timer (DIV/TIMA)     | Complete                                           |
| PPU — background     | Complete (scroll, palettes, tile-data addressing)  |
| PPU — sprites        | Functional (8×8); see Refinements for known gaps   |
| OAM DMA              | Functional (instant copy)                          |
| Joypad input         | Complete (polled)                                  |
| Sound (APU)          | Not implemented                                    |

## Running

The emulator expects a cartridge ROM. Place it under `roms/` and set the path
in `main.rs`:

```rust
let path = "roms/your-game.gb";
```

Then:

```sh
cargo run --release
```

`--release` is strongly recommended — debug builds run the emulation loop far
slower.

### Controls

| Game Boy | Keyboard   |
| -------- | ---------- |
| D-pad    | Arrow keys |
| A        | Z          |
| B        | X          |
| Start    | Enter      |
| Select   | Backspace  |
| (quit)   | Escape     |

## Architecture

The emulator is organized around the real hardware's component boundaries.

```
main.rs    GameBoy struct (CPU + Bus), the frame loop, minifb window, input
cpu.rs     SM83 CPU core — registers, opcode execution, interrupt dispatch
bus.rs     Memory map and routing, MBC1 mapper, DIV/TIMA timer, OAM DMA
ppu.rs     Picture Processing Unit — VRAM, OAM, registers, scanline rendering
joypad.rs  Button state and the multiplexed 0xFF00 register
```

### Design notes

- **The Bus owns the hardware and routes access.** The CPU sees memory only
  through `Bus::read_byte` / `write_byte`, which dispatch each address to the
  right component (ROM, RAM, PPU, joypad, timer registers).

- **The PPU owns its own state.** VRAM, OAM, the LCD registers, the scanline
  counter, and the framebuffer all live in `Ppu`. The Bus routes the relevant
  address ranges (0x8000–0x9FFF, 0xFE00–0xFE9F, 0xFF40–0xFF4B) to it. The PPU
  never reaches back into the Bus — instead, `Ppu::tick` returns an
  `InterruptRequest` describing which interrupts it wants raised, and the Bus
  applies them. This keeps ownership clean and avoids cyclic borrows.

- **Rendering is scanline-based.** Each visible line is drawn when `LY`
  advances: the background row first, then sprites on top. By the time `LY`
  reaches 144 (VBlank), the full frame is in the framebuffer, and the frame
  loop pushes it to the window.

- **Timing is instruction-stepped.** The CPU executes one instruction, reports
  its cycle cost, and the Bus advances the PPU and timer by that many cycles.
  The frame loop runs ~70224 cycles (one frame) per displayed frame, paced to
  ~60 FPS.

## Testing

The CPU has a unit-test suite (75 tests) covering opcodes with real logic —
flag behavior, stack operations, branching, and the subtle cases (DAA, the
SP-relative arithmetic flag quirk, the carry-fold in ADC/SBC, the rotate
zero-flag behaviors).

```sh
cargo test
```

For end-to-end CPU validation, point the ROM path at Blargg's `cpu_instrs.gb`.
The test ROM reports its results over the serial port, which the emulator
prints to stdout. A correct CPU prints `Passed all tests`.

## Refinements (planned)

These are known simplifications — the emulator works without them, but they're
on the roadmap toward full accuracy:

- **8×16 sprite mode** — sprites are currently rendered as 8×8. Games that use
  tall sprites (including parts of Link's Awakening) will show only the top
  half of affected objects until this is added.
- **Sprite priority** — the "behind background" priority bit is not yet
  honored; sprites always draw on top.
- **10-sprites-per-line limit** — the hardware drops sprites beyond 10 per
  scanline; this limit is not enforced.
- **The window layer** — the third rendering layer (used for status bars and
  dialogue boxes) is not yet implemented.
- **OAM DMA timing** — the DMA copy is instantaneous rather than taking its
  real ~160 cycles. Games that depend on DMA timing are unaffected in practice.
- **EI delay precision / STAT timing** — interrupt timing is accurate enough to
  pass `cpu_instrs`, but some cycle-exact edge cases the Mooneye test suite
  probes are not yet handled.
- **Sound (APU)** — not implemented.

## License

This emulator — the code in this repository — is released under the MIT
License. See [LICENSE](LICENSE) for the full text.

The MIT License covers **the emulator code only**. It does not cover, and
cannot grant any rights to, copyrighted game ROMs. Commercial Game Boy games
remain the property of their respective copyright holders; you must supply your
own legally-obtained ROMs to run them. No commercial ROMs are included in this
repository, and they should not be committed to it.

Blargg's test ROMs, used for CPU validation, are freely redistributable
homebrew and may be included.

## Acknowledgements

- Blargg's test ROMs, for rigorous CPU validation.
- The Pan Docs, the community's reference for Game Boy hardware behavior.
