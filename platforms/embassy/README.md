# pocket-embassy

Run a PocketJS JavaScript guest and RGB565 software renderer from an Embassy task.
The crate uses `no_std` and `alloc`. The application owns the allocator, executor,
stack, clock driver, and display peripherals. No ESP HAL or RMK types enter the API.

## Build

Provide the QuickJS-NG **0.16.2** sources through `POCKET_QUICKJS_DIR`. The build
uses `cc` and its target-specific `CC`, `AR`, and `CFLAGS` configuration. It compiles
the engine and the existing PocketJS guest binding; it does not fetch dependencies.
For bare metal, a build-directory copy excludes unused pthread helpers; the
source checkout stays unchanged. Private clock functions use Embassy time.
Bare-metal builds require the target C runtime and math library. The board owns
those link settings and any newlib syscall hooks. They are not selected by chip name.

```sh
export POCKET_QUICKJS_DIR=/absolute/path/to/quickjs
cargo test --manifest-path tests/fixtures/embassy-host/Cargo.toml --release --locked
cargo run --manifest-path tests/fixtures/embassy-host/Cargo.toml --release --locked
```

For the link-only check, install the Rust target and its C toolchain, then run:

```sh
python3 tests/fixtures/embassy-link/build.py thumbv7em-none-eabihf --cc arm-none-eabi-gcc
python3 tests/fixtures/embassy-link/build.py riscv32imafc-unknown-none-elf --cc /path/to/riscv32-esp-elf-gcc
```

The executable example runs the real JS guest and an async display on the same
Embassy executor as a heartbeat. `tests/fixtures/embassy-link` is a link-only
bare-metal harness; it is not board firmware.

## Application ownership

Create `QuickJs` with `RuntimeConfig`, a JS stack budget, JavaScript bytes and an
owned or static PAK. This embedded API takes a trusted, compiler-produced JS/PAK
pair with matching binding generation. It does not admit arbitrary `.pocket`
packages. The PAK stays alive until guest shutdown. `with_setup` attaches native
extensions before the guest mounts its bindings; raw pointers belong to that realm.

Pass the guest to `Ui::new`, provide an existing `InputSource`, and implement
`Display::present`. Call `ui.run(&mut input, &mut display).await` in the application's
task. The display receives RGB565 pixels, stride in pixels, and damage regions.
It must stop borrowing the pixels when the future returns or is cancelled.
A failed or cancelled submission forces a full repaint on the next frame.

The first backend supports one surface at density 1. Viewport, heap, framebuffer
budget and cadence come from `RuntimeConfig`; the JS stack limit is an argument.
The application initializes its global allocator before creating the guest.
No external RAM or second core is required by the API.

The runner samples standard PocketJS input at the selected cadence. `frame` is
available for an application-owned event loop. `wait_until` converts a selected
microsecond deadline into an Embassy wait. `TickSchedule` skips obsolete wall-clock
slots without replaying guest turns. Native-only animation skipping and
application-specific state protocols are outside this host.

JS evaluation and CPU rendering are synchronous. Interrupt-poll budgets abort
runaway JS; they do not provide preemption or a wall-clock latency guarantee.
The guest stays on its owner task. On bare metal, JS time reads use Embassy uptime,
not a wall-clock date. Ordinary clocks and C syscalls outside QuickJS are unchanged.

## Validation

The host fixture checks JS exceptions, interrupt budgets, immutable package bytes,
framebuffer limits, input transfer and cancellation recovery. Its executable checks
that the UI runner and another Embassy task both make progress.
The link fixture checks target C/Rust ABI and libc integration. Real display,
DMA, allocator sizing and interrupt priorities remain board responsibilities.
