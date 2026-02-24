# Fast Iteration Cycles for Rust GUI/vello/winit Apps

Research into how Rust GUI and game developers achieve fast edit-compile-run cycles
without full rebuilds. Covers hot-reloading, dynamic linking, alternative codegen
backends, framework-level hot-reload patterns, and linker optimization.

---

## 1. Rust Hot-Reloading

### cargo-watch + Incremental Compilation

[cargo-watch](https://crates.io/crates/cargo-watch) monitors your project source
and re-runs a cargo command on file changes. By default it runs `cargo check`, but
can be configured to run `cargo build` or `cargo run`.

Combined with Rust's incremental compilation (enabled by default for dev builds),
this gives a basic watch-rebuild loop. Typical incremental rebuild times for a
medium Rust GUI project (~20k LOC) in 2025:

- **Single-file change, incremental build:** 2-8 seconds (varies with dependency graph depth)
- **No-change rebuild:** ~0.5-1.5 seconds (overhead from rustc and linker)
- **With optimized linker + cranelift:** can drop to 1-3 seconds for single-file changes

The [2025 Rust Compiler Performance Survey](https://blog.rust-lang.org/2025/09/10/rust-compiler-performance-survey-2025-results/) found that 55% of respondents wait more than 10 seconds for an incremental rebuild, making it the most common complaint. The linking phase is always performed from scratch, which is a significant bottleneck.

**Setup:**
```bash
cargo install cargo-watch
cargo watch -x run          # rebuild and run on changes
cargo watch -x 'run -- --dev-mode'  # with custom args
```

**Limitations:**
- Full process restart on every change (state is lost)
- Linking is not incremental -- it happens from scratch every time
- For a vello/winit app, the GPU context, window, and all state must be recreated

### hot-lib-reloader

**Crate:** [hot-lib-reloader](https://crates.io/crates/hot-lib-reloader) (v0.8.x, actively maintained)
**Repository:** [rksm/hot-lib-reloader-rs](https://github.com/rksm/hot-lib-reloader-rs)
**Author:** Robert Krahn

This is the most battle-tested approach for Rust hot-reloading. It dynamically
reloads a `.dylib` at runtime without restarting the host process.

**How it works:**

1. You split your app into a **host binary** and a **library crate** (crate-type = `["dylib"]`)
2. Functions you want to hot-reload are marked `#[unsafe(no_mangle)]` and made `pub`
3. The `hot_lib_reloader` macro generates a wrapper that uses `libloading` to dlopen the dylib
4. A file watcher (via `notify` crate) detects when the dylib is recompiled
5. On change: unload the old dylib, load the new one, resolve function symbols
6. The host calls into the library through the reloaded function pointers

**Example structure:**

```
my-app/
  Cargo.toml          # workspace
  host/
    Cargo.toml         # [[bin]]
    src/main.rs        # creates window, event loop, calls into lib
  ui-lib/
    Cargo.toml         # [lib] crate-type = ["dylib"]
    src/lib.rs          # #[unsafe(no_mangle)] pub fn render(scene: &mut Scene) { ... }
```

**Host side:**
```rust
#[hot_lib_reloader::hot_module(dylib = "ui_lib")]
mod hot_lib {
    #[hot_function]
    pub fn render(scene: &mut vello::Scene) {}

    #[lib_change_subscription]
    pub fn subscribe() -> hot_lib_reloader::LibReloadObserver {}
}

fn main() {
    // ... winit event loop ...
    hot_lib::render(&mut scene);
}
```

**Constraints and caveats:**

- **No generics:** `#[no_mangle]` does not support generic functions. Monomorphize before exporting.
- **Type layout must match:** Structs/enums used across the boundary must have identical layout in both host and lib. Changing a struct's fields without restarting the host causes UB/crashes.
- **No stable Rust ABI:** This works because both sides are compiled with the same rustc version and flags, producing the same (unstable) ABI. This is a *development-only* technique, not suitable for plugin distribution.
- **macOS codesigning:** On macOS, the reloaded dylib must be codesigned. hot-lib-reloader automatically invokes `codesign` from Xcode CLI tools. Ensure they are installed.
- **Workaround for type changes:** Use serialization (e.g., `serde_json::Value`) to pass data across the boundary when types are evolving. This avoids layout mismatches at the cost of serialization overhead.

**Relevance to vello/winit:**

This is the most directly applicable approach for a vello/winit app. The host owns the winit event loop, wgpu device, and vello renderer. The dylib exports a `render` function that takes a `&mut Scene` and builds the scene graph. On reload, only the rendering logic changes; the window, GPU context, and application state survive.

**Critical consideration for vello types across the dylib boundary:**
- `vello::Scene`, `peniko::Color`, `kurbo::Rect` etc. are Rust structs without `#[repr(C)]`.
- They work across the dylib boundary *only* because both sides are compiled with the same rustc and the same version of vello. Updating vello in the lib without rebuilding the host will crash.
- For maximum safety, pass only primitives (`f64`, `u32`, etc.) or `#[repr(C)]` types across the boundary, and reconstruct vello types inside the lib.

### dexterous_developer

**Crate:** [dexterous_developer](https://crates.io/crates/dexterous_developer) (v0.3.x)
**Repository:** [lee-orr/dexterous_developer](https://github.com/lee-orr/dexterous_developer)

A hot-reload system designed for Bevy but with a modular architecture.

**How it works:**

1. A CLI tool builds and watches your project, producing reloadable dylib artifacts
2. The runtime loads dylibs and can serialize/deserialize ECS components across reloads
3. You explicitly mark "reloadable areas" of your game: systems, components, events, resources
4. Schema evolution is supported -- component structures can change over time through serde
5. Hot-reload capacity is only included when explicitly enabled via feature flag

**Could the approach generalize to vello/winit?**

The core idea -- serializing application state, unloading the old dylib, loading the new one, deserializing state -- is generalizable. However:
- dexterous_developer is deeply tied to Bevy's ECS architecture
- For a vello/winit app, you would need to build your own state serialization layer
- The CLI tooling is Bevy-specific
- The serialization approach adds overhead but solves the type-layout problem elegantly

**Key innovation:** Schema evolution across reloads. If your `DashboardState` struct gains a new field, the old serialized state can be deserialized into the new struct with defaults for missing fields. This is much more robust than raw dylib reloading.

### hot_module / hot_function Attribute Macros

The `hot-lib-reloader` crate provides `#[hot_module]` and `#[hot_function]` attribute macros that generate the dlopen/dlsym boilerplate. These are the primary way to use the crate.

There is no widely-used standalone `hot_reload` attribute macro crate outside of the hot-lib-reloader ecosystem. The macro approach is:

```rust
#[hot_lib_reloader::hot_module(dylib = "my_lib")]
mod hot {
    #[hot_function]
    pub fn update(state: &mut AppState) -> bool {}
}
```

This expands to code that:
- Watches the dylib file for changes
- Uses `libloading::Library` to load/unload
- Resolves symbols by name (the `#[no_mangle]` names)
- Calls through function pointers

### Typical Rebuild Times

For a medium Rust GUI app (vello/winit, ~10-20k LOC of your own code, plus heavy dependencies like wgpu/vello):

| Scenario | Time |
|----------|------|
| Clean build (LLVM) | 60-120+ seconds |
| Clean build (Cranelift) | 40-90 seconds |
| Incremental, single-file change (LLVM) | 3-10 seconds |
| Incremental, single-file change (Cranelift) | 2-5 seconds |
| hot-lib-reloader dylib rebuild (small lib) | 1-3 seconds |
| Dioxus subsecond hotpatch | 100-500 ms |

---

## 2. Dynamic Linking for Dev Builds

### The Host + Dylib Pattern

The fundamental pattern for fast iteration in Rust GUI apps:

```
[host binary]  -- thin, rarely changes, owns window/GPU/state
    |
    dlopen()
    |
[UI dylib]     -- all rendering/layout code, frequently changes
```

**Implementation with feature flags:**

```toml
# Cargo.toml of the UI library
[lib]
# In dev: compile as dylib for hot-reload
# In release: compile as rlib for static linking
crate-type = ["dylib", "rlib"]

[features]
hotreload = []  # enables hot-reload codepaths in the host
```

```rust
// host/src/main.rs
#[cfg(feature = "hotreload")]
#[hot_lib_reloader::hot_module(dylib = "ui_lib")]
mod ui {
    #[hot_function]
    pub fn render(scene: &mut Scene, state: &AppState) {}
}

#[cfg(not(feature = "hotreload"))]
mod ui {
    pub fn render(scene: &mut Scene, state: &AppState) {
        ui_lib::render(scene, state);
    }
}
```

### How Bevy Handles Dynamic Linking

**Crate:** [bevy_dylib](https://crates.io/crates/bevy_dylib)

Bevy's `dynamic_linking` feature compiles most of Bevy as a single large dylib instead
of statically linking it into your binary.

**How it works:**

1. Enable with `--features bevy/dynamic_linking`
2. Bevy is compiled once as `libbevy_dylib.dylib`
3. Your game binary links to it dynamically
4. On incremental rebuilds, only *your* code is recompiled and relinked -- Bevy's dylib is already built
5. The linker just needs to resolve symbols against the existing dylib, which is much faster than statically linking all of Bevy's code

**Impact:** Bevy users report incremental rebuild times dropping from 10-15 seconds to 2-5 seconds with dynamic linking enabled, because the linker no longer needs to process Bevy's ~1M lines of code.

**For a vello/winit app:** You could apply the same principle by compiling vello, wgpu, and other heavy dependencies as dylibs. However, these crates do not provide a `dynamic_linking` feature out of the box. You would need to create a "deps dylib" crate that re-exports the dependencies:

```toml
# deps-dylib/Cargo.toml
[lib]
crate-type = ["dylib"]

[dependencies]
vello = "0.3"
wgpu = "24"
peniko = "0.3"
```

### The cdylib + dlopen Approach

For maximum control, compile UI code as a `cdylib`:

```toml
[lib]
crate-type = ["cdylib"]
```

**What must be `extern "C"` vs what can stay Rust:**

- **Must be `extern "C"`:** All functions exported across the dylib boundary if you use `cdylib`. This gives a stable C ABI.
- **Can stay Rust:** If you use `dylib` (not `cdylib`) crate-type, Rust functions can be exported with `#[no_mangle]` using the Rust calling convention. This is what hot-lib-reloader does. It relies on the same rustc version producing the same (unstable) ABI on both sides.
- **Practical recommendation for dev hot-reload:** Use `dylib` + `#[no_mangle]`, not `cdylib` + `extern "C"`. The Rust ABI is stable enough across compilations with the same compiler. The `cdylib` approach forces you to flatten all types to C-compatible representations, which is painful with complex types like `vello::Scene`.

**How this interacts with vello's Scene/peniko types:**

- `vello::Scene` is a complex Rust struct containing `Vec<u8>` encoding buffers. It has no `#[repr(C)]`.
- Passing `&mut Scene` across a `dylib` boundary works because the memory layout is identical when compiled with the same rustc.
- Passing `Scene` across a `cdylib` boundary is NOT safe -- you would need to serialize it or use opaque pointers.
- `peniko::Color`, `kurbo::Point`, `kurbo::Rect`, etc. are simpler but still `repr(Rust)`.
- **Best practice:** Have the host create the `Scene`, pass a `&mut Scene` reference to the dylib. The dylib calls `scene.fill()`, `scene.stroke()`, etc. This works with `dylib` crate-type because vello is a shared dependency compiled once.

---

## 3. Cranelift Backend for Debug Builds

### Overview

[rustc_codegen_cranelift](https://github.com/rust-lang/rustc_codegen_cranelift) (cg_clif)
is an alternative codegen backend for rustc that uses [Cranelift](https://cranelift.dev/)
instead of LLVM. Cranelift is designed for fast compilation rather than optimal runtime
code generation.

**Usage:**
```bash
cargo +nightly -Z codegen-backend=cranelift build
```

Or in `.cargo/config.toml`:
```toml
[unstable]
codegen-backend = true

[profile.dev]
codegen-backend = "cranelift"
```

### Benchmarks and Speedups

From the [Rust Project Goals for 2025h2](https://rust-lang.github.io/rust-project-goals/2025h2/production-ready-cranelift.html)
and community benchmarks:

| Metric | LLVM | Cranelift | Improvement |
|--------|------|-----------|-------------|
| Full debug build (cranelift-codegen itself) | 37.5s wall | 29.6s wall | ~20% wall-clock |
| CPU time for same build | 211 CPU-sec | 125 CPU-sec | ~40% CPU time |
| Code generation phase specifically | - | - | ~20% reduction |
| Total compilation (large project, clean) | - | - | ~5% total (codegen is one phase) |
| Incremental build with Cranelift + mold | - | - | ~75% reduction reported |

The key insight: Cranelift's biggest wins are in the **codegen phase**, which is only part of the total compile time. Front-end parsing, macro expansion, type checking, and borrow checking are unaffected. For incremental builds where only codegen re-runs, the percentage improvement is larger.

### Current Status (2025-2026)

- **Distribution:** Included in nightly builds on Linux, macOS (x86_64 and aarch64), and x86_64 Windows
- **Stability:** Not yet stable enough for daily use on all projects. Most large projects hit one or two missing features or subtle codegen bugs.
- **2025h2 goal:** The Rust project aims to make cg_clif "production-ready" and eventually recommend it as the default backend for `cargo test` and `cargo run` in development
- **Unwinding:** Support for unwinding on Linux and macOS is a current focus area
- **SIMD:** Partial SIMD support; some intrinsics may not be available
- **For vello/winit:** Likely to work for the host application, but wgpu's shader compilation and GPU driver interactions could surface edge cases. Worth testing.

### Practical Recommendation

For a vello/winit app in 2025-2026:
- **Try it:** `cargo +nightly -Z codegen-backend=cranelift run` and see if your app compiles and runs correctly
- **Expected improvement:** 20-40% faster codegen, translating to ~15-30% faster incremental builds
- **Combine with fast linker:** Cranelift + mold (Linux) yields the best results. On macOS, combine with the system linker improvements.
- **Fallback:** If cranelift hits an issue with a specific dependency, you can use it selectively via per-crate codegen settings (not yet ergonomic)

---

## 4. Framework Hot-Reload Patterns (Dioxus / Leptos)

### Dioxus RSX Hot-Reload

[Dioxus](https://dioxuslabs.com/) (v0.7) has two levels of hot-reload:

**Level 1: RSX interpretation (no recompilation)**

The RSX parser runs both at compile time and in the dev tools. When you change static content inside an `rsx!{}` block (text, styles, element structure), the CLI:

1. Parses the changed RSX at the source level
2. Diffs against the previous RSX tree
3. Sends a patch over a websocket to the running app
4. The app applies the patch to the virtual DOM without any Rust compilation

This works for: element structure changes, static text, Tailwind classes, styling.
Does NOT work for: Rust logic, hook changes, component signatures, complex expressions.

**Level 2: Subsecond Hot-Patching (Dioxus 0.7, experimental)**

[subsecond](https://crates.io/crates/subsecond) is a runtime hotpatching engine.

```bash
dx serve --hotpatch
```

**How it works internally:**

1. **Jump table:** All `subsecond::call` sites are compiled to indirect calls through a jump table. Instead of `call my_function`, the generated code does `call [jump_table + offset]`.

2. **Incremental recompilation:** When you change a file, `dx` drives `rustc` directly (intercepting the normal linking phase). Only changed functions are recompiled.

3. **Assembly diffing:** The tool compares the assembly output between the old and new compilation. Only changed functions need to be patched.

4. **Symbol patching:** The new function code is loaded into the running process's memory. The jump table entries are updated to point to the new function addresses. ASLR is handled by the running app communicating the address of `main` to the patcher.

5. **Stack rewinding:** If a hot function is currently on the call stack, subsecond rewinds to the nearest clean entry point and re-enters.

**Performance:** Sub-200ms patches in many cases. Sub-500ms with ThinLink.

**Limitations:**
- Cannot hot-reload struct layout changes (codegen assumes specific layout/alignment)
- Only tracks the "tip" crate (your binary crate), not dependency crates
- Globals: new globals can be added but destructors won't be called; renames create new globals
- Experimental; developed for about a year before the 0.7 release

### Leptos Hot-Reload

[Leptos](https://leptos.dev/) (via [cargo-leptos](https://github.com/leptos-rs/cargo-leptos)):

- Sends HTML/CSS patches to the browser over a websocket while waiting for the full Rust recompilation
- CSS changes are applied without page reload
- HTML structure changes are patched into the DOM
- For Rust logic changes, a full recompile + WASM rebuild is required

The key insight: Leptos separates "template changes" (instant, no compile) from "logic changes" (require rebuild). The template is essentially an HTML DSL that can be interpreted.

### Could These Patterns Work for a vello Scene Renderer?

**RSX-style interpretation:** Partially applicable. You could create a declarative scene description format (JSON, RON, or a custom DSL) that the renderer interprets at runtime. Changes to the scene description file would be picked up by a file watcher and re-rendered without recompilation. This works for layout/styling but not for custom rendering logic.

**Subsecond-style patching:** Directly applicable in theory. The `subsecond` crate is designed to be framework-agnostic. You could annotate your render function with `subsecond::call` and use `dx serve --hotpatch` even outside of Dioxus. However, this is bleeding-edge and not yet well-documented for non-Dioxus use.

**Practical approach for vello/winit:**
A scene description file (e.g., RON format) watched at runtime + hot-lib-reloader for Rust logic changes provides two tiers of hot-reload:
- **Tier 1 (instant):** Change layout/colors/text in a data file, re-render immediately
- **Tier 2 (~2-3s):** Change Rust rendering logic, dylib recompiles, hot-lib-reloader picks it up

---

## 5. Linker Optimization

The linker is invoked on every incremental build and is often the bottleneck. Linking
is always done from scratch (not incrementally), so a faster linker directly reduces
iteration time.

### Linker Options by Platform

#### macOS (most relevant for this project)

| Linker | Status | Speed | Notes |
|--------|--------|-------|-------|
| **ld64** (Apple default) | Active, ships with Xcode | Baseline | Significantly improved in Xcode 14+ (2022). Now reasonably fast. |
| **zld** | **Deprecated/Archived** | ~40% faster than old ld64 | Fork of Apple's linker. Author now recommends lld instead. [repo](https://github.com/michaeleisel/zld) |
| **lld** (LLVM) | Available but not well-maintained for macOS Mach-O | 20-50% faster than ld64 | Not recommended on macOS due to maintenance gaps. Works well on Linux. |
| **sold** (commercial mold for macOS) | Available, commercial | Fastest on macOS | Fork of mold. Was commercial ($); maintainers considering open-sourcing as Apple's ld64 has caught up. [mold repo](https://github.com/rui314/mold) |

#### Linux

| Linker | Status | Speed | Notes |
|--------|--------|-------|-------|
| **ld** (GNU) | Default | Baseline (slow) | |
| **gold** | Legacy | Faster than ld | Superseded by lld and mold |
| **lld** | **Default in Rust 1.90+ on x86_64-linux** | 30%+ faster than GNU ld | Now the recommended choice. |
| **mold** | Active, open source | Fastest on Linux | [rui314/mold](https://github.com/rui314/mold). Up to 5-10x faster than GNU ld on large projects. |

### macOS Configuration

For macOS, the current best option in 2025-2026 is Apple's own ld64 (which has been
optimized significantly) or sold if you need the absolute fastest times.

**To use lld on macOS** (if you want to try it despite caveats):
```toml
# .cargo/config.toml
[target.aarch64-apple-darwin]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=lld"]
```

**To use sold on macOS:**
```toml
# .cargo/config.toml
[target.aarch64-apple-darwin]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=/path/to/sold"]
```

### Impact on Rebuild Times

Linker choice primarily affects:
- **Incremental builds** (linking happens every time)
- **Large binaries** (more symbols to resolve)

Reported improvements from switching linkers:

| Scenario | Default ld64 | Optimized linker | Improvement |
|----------|-------------|-----------------|-------------|
| Small Rust binary | 0.3s | 0.2s | Marginal |
| Medium GUI app (vello/winit) | 1-3s | 0.5-1.5s | 30-50% |
| Large project (Bevy game) | 5-10s | 1-3s | 50-70% |

### Current Best Linker for macOS Rust Dev Builds

**Recommendation for this project (vello/winit on macOS aarch64):**

1. **Start with Apple's default ld64** -- it has been significantly improved in recent Xcode versions and requires zero configuration.
2. **If linking is a bottleneck (>2s):** Try `sold` for the fastest macOS linking.
3. **Do NOT use lld on macOS** -- it is not well-maintained for Mach-O and can produce subtle issues.
4. **Do NOT use zld** -- it is archived/deprecated.

---

## Recommendation for bisque-computer (vello/winit macOS app)

### Most Proven Approach: hot-lib-reloader

For a vello/winit app, the **hot-lib-reloader** approach is the most proven and
directly applicable:

1. **Split into host + UI dylib:**
   - Host: owns `winit` event loop, `wgpu` device, `vello` renderer, application state
   - UI dylib: exports `render(&mut Scene, &AppState)` and `update(&mut AppState, &Event)`

2. **Use `#[hot_module]` macro** in the host to auto-reload the dylib

3. **Combine with:**
   - Cranelift backend for faster codegen: `cargo +nightly -Z codegen-backend=cranelift`
   - Apple's ld64 (default, already good on macOS)
   - `cargo-watch` to auto-trigger rebuilds of the dylib

4. **Data-driven scene descriptions** for instant (no-compile) iteration on layout/colors

### Expected Iteration Cycle

| Change type | Feedback time |
|------------|---------------|
| Layout/color/text in data file | <100ms (file watch + re-render) |
| Rust rendering logic in UI dylib | 1-3s (incremental compile + dylib reload) |
| App state/event handling changes | 1-3s (same as above if in dylib) |
| Host binary changes (window, GPU setup) | 5-15s (full restart needed) |

### Alternative: Subsecond (Bleeding Edge)

If you want the fastest possible Rust code hot-reload (~200ms), the
[subsecond](https://crates.io/crates/subsecond) crate from the Dioxus team is worth
watching. It is framework-agnostic in principle, but tooling support outside Dioxus
is limited as of early 2026. A Bevy integration PR exists
([bevyengine/bevy#19309](https://github.com/bevyengine/bevy/pull/19309)), suggesting
the approach is generalizing beyond Dioxus.

---

## Sources

- [hot-lib-reloader crate](https://crates.io/crates/hot-lib-reloader)
- [hot-lib-reloader GitHub](https://github.com/rksm/hot-lib-reloader-rs)
- [Robert Krahn: Hot Reloading Rust](https://robert.kra.hn/posts/hot-reloading-rust/)
- [Robert Krahn: Speeding up incremental Rust compilation with dylibs](https://robert.kra.hn/posts/2022-09-09-speeding-up-incremental-rust-compilation-with-dylibs/)
- [Kampffrosch: I hotreload Rust and so can you](https://kampffrosch94.github.io/posts/hotreloading_rust/)
- [fasterthanli.me: So you want to live-reload Rust](https://fasterthanli.me/articles/so-you-want-to-live-reload-rust)
- [dexterous_developer GitHub](https://github.com/lee-orr/dexterous_developer)
- [bevy_dylib docs](https://docs.rs/bevy_dylib)
- [rustc_codegen_cranelift GitHub](https://github.com/rust-lang/rustc_codegen_cranelift)
- [Rust Project Goals 2025h2: Production-ready Cranelift](https://rust-lang.github.io/rust-project-goals/2025h2/production-ready-cranelift.html)
- [Nicholas Nethercote: How to speed up the Rust compiler (May 2025)](https://nnethercote.github.io/2025/05/22/how-to-speed-up-the-rust-compiler-in-may-2025.html)
- [Rust Compiler Performance Survey 2025](https://blog.rust-lang.org/2025/09/10/rust-compiler-performance-survey-2025-results/)
- [Dioxus 0.7 Hot-Reload docs](https://dioxuslabs.com/learn/0.7/essentials/ui/hotreload/)
- [subsecond crate](https://crates.io/crates/subsecond)
- [Dioxus v0.7.0 release notes](https://github.com/DioxusLabs/dioxus/releases/tag/v0.7.0)
- [subsecond binary patching PR](https://github.com/DioxusLabs/dioxus/pull/3797)
- [Bevy subsecond integration PR](https://github.com/bevyengine/bevy/pull/19309)
- [cargo-leptos GitHub](https://github.com/leptos-rs/cargo-leptos)
- [mold linker GitHub](https://github.com/rui314/mold)
- [zld GitHub (archived)](https://github.com/michaeleisel/zld)
- [Michael Eisel: Faster Apple Builds with lld](https://eisel.me/lld)
- [Rust 1.90.0: LLD on Linux by default](https://blog.rust-lang.org/2025/09/01/rust-lld-on-1.90.0-stable/)
- [Speeding up Rust compilations on macOS](https://naz.io/posts/speeding-up-rust-compile-times/)
- [corrode: Tips for Faster Rust Compile Times](https://corrode.dev/blog/tips-for-faster-rust-compile-times/)
- [Ryan Isaac: Hot Reloading for Rust Gamedev](https://ryanisaacg.com/posts/hot-reloading-rust.html)
- [Rust ABI stability discussion](https://users.rust-lang.org/t/abi-stability-guarantee-of-dylib-vs-cdylib/50879)
- [Rust Linkage Reference](https://doc.rust-lang.org/reference/linkage.html)
