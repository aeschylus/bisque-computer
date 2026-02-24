# Embedded Scripting in a Rust/Vello/Winit App

**Goal:** Edit a script file, see the visual change immediately (sub-second) — no recompilation of the host binary.

**Research date:** 2026-02-23

---

## Executive Summary

For a Vello/Winit dashboard app, the most practical choices ranked by fit:

1. **Lua via `mlua` + LuaJIT** — mature, fastest interpreted option, well-proven in games (Defold, Roblox), minimal binary overhead (~500 KB), exposes Rust types cleanly as userdata.
2. **Rhai** — pure Rust, Rust-like syntax, easiest Rust integration, adequate for UI scripts, but slower than Lua and has closure overhead issues.
3. **QuickJS via `rquickjs`** — lightweight JS, surprisingly fast startup (~300 µs per runtime), familiar syntax, no JIT, ~500 KB overhead.
4. **WASM via `wasmtime`** — best for complex sandboxed plugins, hot-swap is possible but adds compilation step; AssemblyScript makes it faster.
5. **V8/`deno_core`** — full-fat JS/TS with JIT; ~10–15 MB binary overhead; overkill for UI scripting.
6. **Starlark** — good for config/build files, not designed for stateful per-frame rendering loops.

---

## 1. Lua via `mlua`

### Maturity

`mlua` is the canonical Lua-in-Rust library as of 2025. It started as a fork of `rlua`, then diverged significantly. `rlua` is now officially deprecated and archived (September 2025), and is now just a thin transitional wrapper over `mlua`. `mlua` has ~3M+ crate downloads, supports Lua 5.1/5.2/5.3/5.4/5.5, LuaJIT, and Roblox's Luau.

- crates.io: https://crates.io/crates/mlua
- GitHub: https://github.com/mlua-rs/mlua
- rlua deprecation notice: https://github.com/mlua-rs/rlua/issues/294

### Performance

- **LuaJIT** (feature flag `luajit`) gives by far the best performance — comparable to compiled languages for numeric loops.
- Standard Lua 5.4 is roughly 2–5× slower than LuaJIT for CPU-bound work.
- In the `script-bench-rs` benchmark (https://github.com/khvzak/script-bench-rs), which covers not just script speed but Rust interop overhead, mlua/Luau and LuaJIT consistently top the table. Rhai runs at ~1/5th to 1/10th the speed of LuaJIT.
- Per-frame overhead at 60 fps: a Lua `draw()` callback that calls into Rust through mlua userdata takes on the order of **1–5 µs** per primitive call on modern hardware. A frame budget of 16 ms leaves ample room for hundreds of draw calls through Lua before you approach a bottleneck. The real ceiling is the Vello Scene encoding on the CPU side, not Lua dispatch.

### Exposing Vello Primitives to Lua

`mlua` uses the `UserData` trait. A `SceneProxy` userdata wraps a `&mut vello::Scene` (held via `Arc<Mutex<>>` or passed through a callback):

```rust
use mlua::prelude::*;
use vello::{Scene, kurbo::{Rect, Affine}, peniko::{Color, Fill}};

struct SceneProxy(Arc<Mutex<Scene>>);

impl LuaUserData for SceneProxy {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("fill_rect", |_, this, (x, y, w, h, r, g, b): (f64, f64, f64, f64, u8, u8, u8)| {
            let mut scene = this.0.lock().unwrap();
            scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                Color::rgb8(r, g, b),
                None,
                &Rect::new(x, y, x + w, y + h),
            );
            Ok(())
        });
        // add_method "draw_text", "fill_circle", etc.
    }
}

// In your frame loop:
let proxy = lua.create_userdata(SceneProxy(scene_arc.clone()))?;
lua.globals().set("scene", proxy)?;
lua.load(script_source).exec()?;
```

The Lua script then looks like:
```lua
scene:fill_rect(10, 10, 200, 40, 240, 210, 170)  -- bisque rect
scene:draw_text("Hello", 20, 30, 18)
```

**Key constraint:** `vello::Scene` is not `Send + Sync`, so you must coordinate access carefully. The cleanest pattern is to pass the scene only within the frame callback, not storing it in the Lua VM between frames.

### LuaJIT availability

Enable with `features = ["luajit", "vendored"]` in `Cargo.toml`. The `vendored` flag builds LuaJIT from source via the `luajit-src` crate — no system dependency required. LuaJIT is x86/ARM64 only (no WASM target). On Apple Silicon, LuaJIT runs in ARM64 mode natively.

### How Games Use Lua for UI Scripting

**Defold** (https://defold.com) runs on LuaJIT. Its render pipeline is entirely scriptable: a `render_script` file receives a `update()` callback every frame where it calls `render.draw()`, `render.set_view()`, etc. — exactly the pattern proposed here. Defold uses a C++ host that owns the GPU context and exposes draw commands to Lua via registered functions. GUI components have their own `gui_script` files that define `init()`, `update(dt)`, `on_message()` callbacks.

**Roblox** uses Luau (a Roblox fork of Lua 5.1 with type annotations) for all UI scripting. `ScreenGui` objects are created and manipulated via Lua; the engine polls Lua state each frame before compositing. Updating many GUI objects in one ScreenGui can hurt frame rate — Roblox engineers recommend splitting static and animated content across separate containers. (https://devforum.roblox.com/t/screengui-performance-one-vs-many/346193)

**LÖVE2D** (https://love2d.org) is C++/OpenGL with a Lua API. Developers write `love.draw()` in Lua; the engine calls it at 60 fps. The entire UI and game logic is Lua — the host provides windowing, audio, and low-level draw primitives. This is the closest structural analogy to the bisque-computer use case.

### `rlua` comparison

`rlua` is archived and no longer maintained. Do not use for new projects. Any existing `rlua` code should migrate to `mlua` directly.

### `piccolo` (pure Rust Lua)

- GitHub: https://github.com/kyren/piccolo
- Blog: https://kyju.org/blog/piccolo-a-stackless-lua-interpreter/
- **Status:** Experimental, pre-1.0, frequent breaking API changes. Resumed development April 2023 after years of hiatus.
- Uses a "stackless" trampoline VM which enables safe Rust ↔ Lua nesting without unsafe C stack manipulation.
- Implements a real cycle-detecting incremental GC with zero-cost `Gc<T>` pointers usable from safe Rust.
- **Not production-ready.** COMPATIBILITY.md lists many missing standard library functions. Do not use unless you specifically need a pure-Rust Lua with no C dependency.

---

## 2. JavaScript / TypeScript via Embedded Runtimes

### V8 / `rusty_v8` / `deno_core`

- rusty_v8 stabilized: https://deno.com/blog/rusty-v8-stabilized
- deno_core crate: https://crates.io/crates/deno_core

**Binary size:** V8 itself compiles to **~10–15 MB** of `.text` section. In Deno's own analysis, `rusty_v8` accounts for 9.7 MiB (13.5%) of the release binary. For a small dashboard app this is a significant bloat.

**Startup time:** V8 Isolate creation takes ~2–10 ms depending on snapshot availability. With a pre-serialized snapshot (which `deno_core` supports) this drops to ~1 ms. Still noticeable for hot-reloading individual frames but fine for module-level reload.

**Complexity:** Embedding `deno_core` requires managing `JsRuntime`, ops (the Rust→JS callback mechanism), module resolution, and the event loop. The API is designed for building full JS runtimes, not lightweight scripting. For a UI scripting layer this is significant over-engineering.

**Verdict for bisque-computer:** Only justifiable if you want full TypeScript support with type checking and npm packages. The 10–15 MB binary hit and setup complexity are high prices.

### `boa_engine` (Pure Rust JS)

- GitHub: https://github.com/boa-dev/boa
- v0.20 release (Dec 2024): https://boajs.dev/blog/2024/12/05/boa-release-020
- v0.21 (2025): 94.12% ECMAScript Test262 conformance, NaN-boxing for reduced memory footprint.

**Performance:** Boa v0.21 has overtaken Rhai in synthetic benchmarks, but still 10–50× slower than V8/LuaJIT for CPU-bound work. For IO-bound UI scripts (building a draw list) this gap matters less.

**Binary size:** Pure Rust, no C dependencies — adds ~2–4 MB to the binary vs V8's 10–15 MB.

**Suitability:** A reasonable middle ground if you want JavaScript syntax without V8. No JIT. Async support in progress. Not production-ready for performance-critical paths.

### `rquickjs` / QuickJS

- GitHub: https://github.com/DelSkayn/rquickjs
- crates.io: https://crates.io/crates/rquickjs

**QuickJS facts:** Written in C by Fabrice Bellard. Supports ES2020 including modules, async generators, proxies, and BigInt. Embeds as just a few C files. A complete runtime instance lifecycle completes in **under 300 µs**. Passes 75,000 ECMAScript tests in ~100 seconds on one core.

**Binary size:** ~210 KB x86 code for hello-world. Much smaller than V8.

**Performance:** No JIT — interpreted bytecode only. For UI scripts that build draw lists, this is generally fast enough. QuickJS benchmarks at roughly 3–5× slower than V8 for CPU-heavy code; for frame-building logic the gap is less relevant.

**Rust integration:** `rquickjs` provides a clean high-level API with `Class<T>` for exposing Rust types (analogous to `mlua`'s `UserData`). Futures support via `AsyncRuntime`/`AsyncContext`.

**Verdict:** Best JS option for low overhead. If developers prefer JS syntax over Lua, `rquickjs` is the right choice. Pairs well with TypeScript compiled to plain JS offline.

### Bun as External Process

- Bun IPC docs: https://bun.com/docs/guides/process/ipc
- Embedding Bun discussion: https://github.com/oven-sh/bun/discussions/7841

**Feasibility:** Bun does not provide an embeddable C API or Rust crate. The only integration path is spawning a `bun` child process and communicating via IPC (JSON or binary over stdin/stdout or a Unix socket). The script sends draw commands as messages; Rust receives and executes them against the Scene.

**Latency:** A round-trip through a Unix socket adds ~50–200 µs. For non-interactive scripts (rebuild on file change, paint one frame) this is acceptable. For reactive scripts that need per-frame feedback it is not viable.

**Startup time:** Bun starts in ~5–10 ms. Acceptable for reload-on-change but not for instantiation every frame.

**Verdict:** Interesting for running TypeScript UI scripts with full npm access, but fundamentally a sidecar architecture — not embedded scripting. Adds external process dependency.

### JSX/TSX → Vello Scene

Compiling JSX to draw commands requires a custom JSX renderer (analogous to React Native's Fabric). The transform chain: `.tsx` → Bun/esbuild transpile (JSX→JS) → QuickJS or V8 evaluates → registered `scene.fillRect()` etc. callbacks build the vello `Scene`. This is architecturally sound but requires writing the JSX runtime layer (the `h()` function that accumulates draw ops rather than DOM nodes). Feasible as a medium-effort project on top of `rquickjs`.

---

## 3. WASM as Plugin System

### `wasmtime`

- GitHub: https://github.com/bytecodealliance/wasmtime
- Performance post: https://bytecodealliance.org/articles/wasmtime-10-performance
- Fast instantiation docs: https://docs.wasmtime.dev/examples-fast-instantiation.html

**Instantiation performance:** After optimization, SpiderMonkey.wasm went from ~2 ms to **5 µs** instantiation time. Per-call overhead is **a few nanoseconds** for the wasm→host function boundary itself, though per-thread setup on first call can be ~100–300 µs.

**Hot-swap pattern:**
```rust
// On file change, reload the .wasm module:
let new_module = Module::from_file(&engine, "ui.wasm")?;
let new_instance = linker.instantiate(&mut store, &new_module)?;
// Swap the instance; next frame uses new_instance.render(&mut store, ...);
```
The key insight: `Module::from_file` + linking takes ~1–10 ms (AOT compilation). For sub-second reload this is within budget if the .wasm file is small. Pre-compiled `.cwasm` files (wasmtime's cache format) can reduce this to ~50 µs.

**Writing the WASM plugin:**

Any language compiling to WASM can implement the UI render function. The host exposes draw primitives via WASM imports:
```wit
// ui.wit (WIT interface)
interface renderer {
  fill-rect: func(x: f32, y: f32, w: f32, h: f32, r: u8, g: u8, b: u8);
  draw-text: func(text: string, x: f32, y: f32, size: f32);
}
```

**AssemblyScript** (TypeScript-like, compiles to WASM) is the best language for fast iteration here. Compilation takes ~100–500 ms for small files — acceptable for hot-reload. Binary output is small. The `rust-wasm-hotreload` PoC demonstrates the exact pattern: https://github.com/shekohex/rust-wasm-hotreload

**Rust → WASM:** `cargo build --target wasm32-wasi` for a small UI crate takes 5–30 seconds even in incremental mode. Too slow for sub-second hot reload. Only viable with pre-compiled caching or if using a dedicated small crate.

**Zig → WASM:** Zig's compiler is fast (~1–3 seconds for small files). A Zig UI module could hot-reload meaningfully faster than Rust.

### `wasmer`

- `wasmer` supports AOT compilation to native `.dylib` that can be serialized and reloaded without re-JIT — interesting for repeated hot-swaps of the same or similar modules.
- Less mature Component Model support than `wasmtime` as of 2024.

### WASM vs Scripting for UI

WASM wins when:
- You need sandboxing / untrusted user code
- Plugin authors use diverse languages
- Compute-intensive rendering logic

WASM loses when:
- You want to edit a small script and see a change in <100 ms (Lua reload: <1 ms; WASM compile: 100 ms–30 s)
- You need rich two-way interaction with host types (WASM's interface is value-typed at the boundary)

---

## 4. Rhai

- Website: https://rhai.rs/
- GitHub: https://github.com/rhaiscript/rhai
- Hot reload pattern: https://rhai.rs/book/patterns/hot-reload.html
- Benchmarks: https://rhai.rs/book/about/benchmarks.html
- Performance discussion (HN, Jan 2025): https://news.ycombinator.com/item?id=42738753

### Overview

Rhai is a pure-Rust scripting engine designed explicitly for embedding in Rust. Syntax is Rust-adjacent (C-family with Rust idioms). No C dependencies. Compiles on all targets including `no_std` and WASM.

### Performance

- 1 million iterations in ~0.14 s on a 2.6 GHz single-core Linux VM.
- Significantly slower than LuaJIT; roughly comparable to PUC-Rio Lua 5.4 or slightly slower.
- Boa v0.21 has overtaken Rhai in the `script-bench-rs` benchmark.
- Closure overhead: every variable access must check if it's a shared (captured) value and take a read lock. The `no_closure` feature flag disables this to improve hot-path performance.
- Community performance concerns surfaced January 2025: https://biggo.com/news/202501201142_rhai-scripting-performance-debate

### Exposing Vello Types

Rhai uses a registration API rather than a trait-based approach:

```rust
let mut engine = Engine::new();
engine.register_type_with_name::<SceneProxy>("Scene")
      .register_fn("fill_rect", SceneProxy::fill_rect)
      .register_fn("draw_text", SceneProxy::draw_text);

engine.set_max_operations(50_000); // safety limit per frame

let ast = engine.compile_file("ui.rhai".into())?;
// Per frame:
let mut scope = Scope::new();
scope.push("scene", scene_proxy.clone());
engine.run_ast_with_scope(&mut scope, &ast)?;
```

### Hot Reload

Rhai documents two patterns (https://rhai.rs/book/patterns/hot-reload.html):

1. **Full reload:** recompile the AST on file change and replace the stored `Arc<RwLock<AST>>`.
2. **Selective reload:** retain only function definitions, hot-swap individual functions using `ast +=` operator.

Both work well with `notify` or `hotwatch` for file watching.

### Limitations vs Lua/JS

- No JIT — purely interpreted tree-walking VM in early versions, bytecode in newer versions.
- Closure capture adds lock overhead (mitigated with `no_closure`).
- No coroutines or cooperative multitasking.
- Not suitable for compute-heavy script logic.
- No standard library of graphics utilities — you build it all yourself (same as Lua, but at least Lua has a larger game-dev ecosystem of existing libraries).
- "What Rhai Isn't": https://rhai.rs/book/about/non-design.html — explicitly not a general-purpose language, not targeting performance, not a replacement for Lua.

### Verdict

Rhai is the **lowest-friction** option for a Rust developer. Registering Rust functions takes 2–3 lines with no FFI. The syntax will feel familiar. For a dashboard with dozens of draw calls per frame it will be fast enough. If you later need LuaJIT-level performance you'll need to migrate.

---

## 5. Starlark

- GitHub (Facebook/Meta): https://github.com/facebook/starlark-rust
- Docs: https://docs.rs/starlark/latest/starlark/

### Overview

Starlark is a deterministic, hermetic, Python-like language designed for configuration (Bazel, Buck2, Buck). Meta maintains `starlark-rust` and depends on it in Buck2.

### Suitability for UI Scripting

**Strengths:**
- Python-like syntax — readable for non-Rust developers.
- Deterministic execution (no randomness, no I/O by default) — easy to sandbox.
- Good IDE support (LSP integration in `starlark-rust`).
- Rust-friendly interop with `StarlarkValue` trait.

**Weaknesses:**
- Designed for *configuration*, not *imperative rendering loops*. There is no mutable shared state model between evaluations — each script evaluation is meant to be self-contained and side-effect-free.
- No concept of `update(dt)` callbacks, coroutines, or per-frame state accumulation.
- No numeric JIT.
- The ecosystem of graphics/UI helpers is nonexistent.

**Verdict:** Not suitable for per-frame UI scripting. Would work for static layout declarations (describe the layout as data; Rust renders it), but Lua or Rhai are better fits for imperative draw scripts.

---

## 6. The "Self-Modifying" / Hot-Reload Pattern

### Architecture

```
┌─────────────────────────────────────────────────────┐
│                  Rust Host (compiled)               │
│  winit event loop → script reload → vello render   │
│                                                     │
│  ┌──────────────┐    ┌──────────────────────────┐   │
│  │ File Watcher │───▶│  Script Engine           │   │
│  │ (notify)     │    │  (mlua / rhai / rquickjs) │   │
│  └──────────────┘    └──────────────────────────┘   │
│         │                        │                  │
│         │ reload on change       │ calls into       │
│         ▼                        ▼                  │
│  ┌──────────────┐    ┌──────────────────────────┐   │
│  │  script file │    │  SceneProxy (UserData)   │   │
│  │  ui.lua      │    │  fill_rect, draw_text    │   │
│  └──────────────┘    └──────────────────────────┘   │
└─────────────────────────────────────────────────────┘
```

### File Watcher Crates

**`notify`** (https://crates.io/crates/notify):
- Cross-platform, uses native OS APIs (FSEvents on macOS, inotify on Linux, ReadDirectoryChangesW on Windows).
- Used by: cargo-watch, deno, rust-analyzer, alacritty, mdBook.
- `notify` 6.x uses a channel-based API.
- Sub-millisecond event delivery on macOS via FSEvents.

**`hotwatch`** (https://crates.io/crates/hotwatch):
- Thin convenience wrapper over `notify`.
- Callback-based API; watches happen on a background thread.
- Good for simple "watch one file and call a closure" patterns.
- Suitable for bisque-computer's use case.

### Recommended pattern

```rust
use hotwatch::{Hotwatch, Event, EventKind};
use std::sync::{Arc, RwLock};

let script_source: Arc<RwLock<String>> = Arc::new(RwLock::new(
    std::fs::read_to_string("ui.lua").unwrap()
));
let script_clone = script_source.clone();

let mut hw = Hotwatch::new().unwrap();
hw.watch("ui.lua", move |event: Event| {
    if matches!(event.kind, EventKind::Modify(_)) {
        if let Ok(src) = std::fs::read_to_string("ui.lua") {
            *script_clone.write().unwrap() = src;
        }
    }
}).unwrap();

// In winit event loop, each frame:
let src = script_source.read().unwrap().clone();
lua.load(&src).exec().unwrap_or_else(|e| eprintln!("Script error: {e}"));
```

For Lua with mlua, re-parsing on every frame is wasteful. Better: store a compiled `Function` or `Chunk`, invalidate it on file change:

```rust
// On change: recompile
let new_chunk = lua.load(&new_source).into_function()?;
*draw_fn.write().unwrap() = new_chunk;

// Each frame:
let f = draw_fn.read().unwrap();
f.call::<()>(scene_proxy.clone())?;
```

### Host/Guest Boundary Design

The boundary should be:

**Host owns (Rust, compiled):**
- Window creation and event loop (winit)
- GPU device, surface, wgpu context
- Vello `Renderer` and frame submission
- Font loading and text shaping primitives
- File watching and script reload
- Application state (panels, data, sessions)
- Input routing

**Guest defines (script, interpreted):**
- Layout logic: where panels go, sizes, colors
- Content rendering: what text/shapes to draw where
- Animations: time-based transforms
- Conditional visibility, theming

The guest receives a `Scene` proxy and optionally a `State` table (dimensions, current data snapshot). It writes draw commands. It never owns GPU resources.

---

## Comparative Summary Table

| Criterion | mlua + LuaJIT | Rhai | rquickjs | wasmtime (WASM) | deno_core (V8) |
|---|---|---|---|---|---|
| **Maturity** | Production (5+ yrs) | Stable | Stable | Production | Production |
| **Binary overhead** | ~500 KB (vendored) | ~300 KB | ~210 KB | ~5 MB | ~10–15 MB |
| **Script startup** | <1 ms | <1 ms | <300 µs | 5 µs (pre-compiled) | 1–10 ms |
| **Per-call overhead** | ~1–5 µs | ~5–20 µs | ~2–10 µs | ~1–10 ns | ~100 ns |
| **Hot reload latency** | <1 ms (re-parse) | <1 ms | <1 ms | 100ms–30s (compile) | 1–5 ms |
| **Vello integration** | UserData trait | register_fn | Class<T> | WIT/extern "C" | V8 bindings |
| **Syntax** | Lua | Rust-like | JavaScript | Any→WASM | JS/TS |
| **JIT available** | Yes (LuaJIT) | No | No | Yes (Cranelift) | Yes (V8) |
| **Closures/state** | Yes | Limited | Yes | Depends on lang | Yes |
| **Pure Rust** | No (C Lua) | Yes | No (C QuickJS) | Yes | No (C++ V8) |
| **Game precedents** | Defold, Roblox, LÖVE2D | None | None | Unity IL2CPP-ish | None |

---

## Recommendation for Bisque-Computer

**Primary choice: `mlua` with LuaJIT.**

Rationale:
- LuaJIT gives the fastest interpreted execution available in Rust.
- Defold's architecture (C++ host + LuaJIT scripts for rendering) is a proven template for exactly this pattern.
- `mlua`'s `UserData` trait maps cleanly onto vello's `Scene` — you can expose `fill_rect`, `draw_text`, `stroke_path`, `set_clip` as methods on a `SceneProxy` userdata.
- `notify` or `hotwatch` provides sub-millisecond file change detection on macOS (FSEvents).
- The entire cycle: save `ui.lua` → FSEvents fires → re-parse Lua chunk (<0.5 ms) → next frame renders it. Sub-second is trivially achievable; **sub-frame** is achievable.
- Binary overhead is ~500 KB with vendored LuaJIT — acceptable.

**If Lua syntax is undesirable: `rhai`.**

Rhai is more ergonomic to set up from Rust (no `unsafe`, no C dependency, simpler API), and the Rust-like syntax may feel more natural for developers already in this codebase. Performance will be adequate for a dashboard with O(100) draw calls per frame. The hot-reload pattern is documented and straightforward.

**If TypeScript is required: `rquickjs`.**

Compile `.ts` files to `.js` with `esbuild` or `bun build --target=node` as an offline step (or as a tiny subprocess), then evaluate the `.js` output in `rquickjs`. This gives TypeScript authoring with lightweight runtime overhead.

---

## Sources

- [mlua GitHub](https://github.com/mlua-rs/mlua)
- [mlua crates.io](https://crates.io/crates/mlua)
- [rlua deprecation issue](https://github.com/mlua-rs/rlua/issues/294)
- [piccolo GitHub](https://github.com/kyren/piccolo)
- [piccolo blog post](https://kyju.org/blog/piccolo-a-stackless-lua-interpreter/)
- [script-bench-rs benchmark](https://github.com/khvzak/script-bench-rs)
- [Survey of Rust embeddable scripting languages (2020, still useful)](https://www.boringcactus.com/2020/09/16/survey-of-rust-embeddable-scripting-languages.html)
- [Rhai website](https://rhai.rs/)
- [Rhai GitHub](https://github.com/rhaiscript/rhai)
- [Rhai hot reload pattern](https://rhai.rs/book/patterns/hot-reload.html)
- [Rhai performance discussion (HN 2025)](https://news.ycombinator.com/item?id=42738753)
- [rquickjs GitHub](https://github.com/DelSkayn/rquickjs)
- [rquickjs crates.io](https://crates.io/crates/rquickjs)
- [boa_engine GitHub](https://github.com/boa-dev/boa)
- [Boa v0.20 release](https://boajs.dev/blog/2024/12/05/boa-release-020)
- [rusty_v8 stabilization](https://deno.com/blog/rusty-v8-stabilized)
- [deno_core crates.io](https://crates.io/crates/deno_core)
- [Bun IPC docs](https://bun.com/docs/guides/process/ipc)
- [Embedding Bun in Rust discussion](https://github.com/oven-sh/bun/discussions/7841)
- [wasmtime GitHub](https://github.com/bytecodealliance/wasmtime)
- [Wasmtime 1.0 performance post](https://bytecodealliance.org/articles/wasmtime-10-performance)
- [Wasmtime fast instantiation docs](https://docs.wasmtime.dev/examples-fast-instantiation.html)
- [rust-wasm-hotreload PoC](https://github.com/shekohex/rust-wasm-hotreload)
- [starlark-rust GitHub](https://github.com/facebook/starlark-rust)
- [notify GitHub](https://github.com/notify-rs/notify)
- [hotwatch GitHub](https://github.com/francesca64/hotwatch)
- [vello GitHub](https://github.com/linebender/vello)
- [vello Scene docs](https://docs.rs/vello/latest/vello/struct.Scene.html)
- [Defold engine scripting overview](https://defold.com/2020/12/27/engine-overview-pt1/)
- [Defold render script manual](https://defold.com/manuals/script/)
- [LÖVE2D](https://love2d.org/)
- [Roblox ScreenGui performance thread](https://devforum.roblox.com/t/screengui-performance-one-vs-many/346193)
- [Vello for video games (simbleau blog)](https://simbleau.github.io/rust/graphics/2023/11/20/using-vello-for-video-games.html)
- [Patterns of use of Vello crate (poignardazur)](https://poignardazur.github.io/2025/01/18/vello-analysis/)
- [Hot Reloading for Rust Gamedev (ryanisaacg)](https://ryanisaacg.com/posts/hot-reloading-rust.html)
