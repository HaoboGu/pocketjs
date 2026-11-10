# QuickJS guest

The C binding in `src/guest.c` implements the canonical PocketJS guest ABI. Existing
ESP-IDF, Zephyr, SiFli and native programs compile it against their chosen QuickJS
build. It owns UI bindings and guest lifecycle; Runtime owns the UI and package.

## Rust owner

`pocket-guest-quickjs` adds a `no_std + alloc` owner for a QuickJS-NG realm and the
existing `CGuest` adapter. **It implements `pocket_runtime::Guest`.** It owns no
Runtime, package, display, renderer, input encoder or scheduler.

```rust
let handle = Handle::new(config)?;
// The boxed Handle stays alive through guest shutdown and destruction.
let guest = unsafe { QuickJsGuest::new(handle.as_ptr(), limits)? };
handle.with_runtime(|runtime| runtime.set_guest(Box::new(guest)))?;
```

The program provides `Limits { heap_bytes, stack_bytes, interrupt_polls }` and
registers the package and drivers before `Runtime::boot`. Runtime supplies the
`GuestEnvironment`, keeps package bytes alive, samples input, and calls shutdown
before releasing the package. Source and bytecode fallback use the existing C
binding contract. Without a registered engine ABI, source is selected.

The constructor is unsafe because the C callbacks retain the supplied runtime
handle. Use the matching handle's OpContext for a program-owned lower-level
embedding, and keep that handle and its package bytes alive through shutdown.
`context_ptr` and `binding_ptr` support native extensions and the binding's existing
log/phase hooks. Configure hooks before boot; the pointers expire at guest Drop.
The guest is neither Send nor Sync.

The realm allocator routes VM allocations to the program's Rust global
allocator. It does not install an allocator or override the process's C allocator.
Interpreter poll budgets abort runaway scripts; they do not yield a running script
or bound synchronous native calls by wall time. Runtime reports guest failures as
`RuntimeError::Guest`; `last_error` exposes the C binding's error text.

## Link from a program

**This Cargo crate neither fetches nor builds QuickJS.** A program links
`src/guest.c`, `src/realm.c` and its engine, and defines the runtime C exports once,
as the SDK archive does. The Rust fixtures show the two operation exports needed
by this binding. C programs keep using the generated guest with the SDK archive;
the Rust adapter is for a source-built Rust program.

`build/sources.rs` is an optional `cc` build-script helper for the Rust owner. It
adds the C sources and routes engine/binding temporary allocations to the same
Rust allocator in that build; the
caller selects compiler flags, optimization, clocks and libc. It does not select a
chip or rewrite source. Other platform bindings retain their existing build paths.

For a bare-metal QuickJS-NG **0.16.2** build, explicitly prepare a build-directory
copy before invoking Cargo:

```sh
python3 tools/packaging/quickjs/prepare-baremetal.py \
  --source /path/to/quickjs --output .pocket-build/quickjs-baremetal
export POCKET_QUICKJS_DIR="$PWD/.pocket-build/quickjs-baremetal"
python3 tests/fixtures/embassy-link/build.py thumbv7em-none-eabihf --cc arm-none-eabi-gcc
```

Preparation checks the pinned `cutils.h` identity and excludes unused pthread
helpers when `POCKET_QJS_NO_THREADS` is defined. The source tree is unchanged.
The program supplies the private monotonic and Unix clock hooks declared in
`include/pocket_quickjs_clock.h`, and the target libc/newlib hooks. Clock policy
belongs to the program; the guest library does not treat uptime as a Unix date.

The owned Rust adapter is verified against QuickJS-NG 0.16.2. The generated C
binding keeps its older ArrayBuffer API path for existing platform consumers.
