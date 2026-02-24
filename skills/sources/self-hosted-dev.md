# Self-Modifying / Self-Hosted Development Environments

Research into applications that can modify their own UI from within themselves,
with specific focus on what is achievable for bisque-computer: a native macOS app
built with Rust / vello / winit that already has an embedded PTY-backed terminal.

---

## 1. Historical Precedents for Self-Modifying UIs

### Smalltalk / Squeak / Pharo

The most complete realization of a self-modifying environment. In Smalltalk,
the entire IDE -- the text editor, the debugger, the class browser, the window
manager -- is written in Smalltalk and editable at runtime. There is no
distinction between "the language" and "the environment."

Key architectural features:

- **Image-based persistence**: the running state of the system (all objects,
  all code, the UI) is serialized as a single "image" file. There is no
  separate compile step; you modify live objects, and the image is saved.
- **Morphic UI framework**: all graphical objects are "morphs" -- tangible,
  interactively changeable objects. You can right-click any morph, inspect
  its properties, and modify them live. This promotes extremely short
  feedback loops.
- **Meta-circularity**: the VM is itself largely written in Smalltalk
  (Slang, a restricted subset). The Cog VM (used by Pharo and Squeak)
  JIT-compiles Smalltalk to native code.
- **No boundary between editing and running**: code modification happens
  in the running system. `doIt` evaluates a selected expression immediately.
  There is no "build" step.

Pharo (the modern fork of Squeak) continues this tradition. Its live
programming MOOC explicitly teaches modifying the running environment:
https://siemenbaader.github.io/PharoMooc-website/

The Squeak project homepage: https://squeak.org/

**Relevance to bisque-computer**: Smalltalk's lesson is that the
edit/run boundary is a design choice, not a technical necessity. The
cost is complexity -- maintaining coherent state across live edits.
For bisque-computer, we need a much simpler version of this idea:
edit visual constants or layout logic, see results immediately.

Sources:
- [Pharo Wikipedia](https://en.wikipedia.org/wiki/Pharo)
- [Pharo GitHub](https://github.com/pharo-project/pharo)
- [Smalltalk Wikipedia](https://en.wikipedia.org/wiki/Smalltalk)
- [Pharo MOOC](https://siemenbaader.github.io/PharoMooc-website/)

### Emacs

Emacs is self-modifying via Emacs Lisp. Every buffer, every mode, every
keybinding is defined in Elisp and can be redefined at runtime.

The mechanism:

1. **`eval-region` / `eval-buffer` / `eval-defun`**: evaluate Lisp code
   directly from the current buffer. If you change a function definition
   and press `C-M-x`, the new definition replaces the old one immediately.
2. **The `*scratch*` buffer**: a persistent REPL (lisp-interaction-mode)
   where `C-j` evaluates the expression before point and prints the result
   into the buffer.
3. **Buffer modification tracking**: Emacs tracks a "modified flag" per
   buffer, enabling the system to know which buffers have unsaved changes.
4. **Advice system**: you can wrap any function with `defadvice` or the
   modern `advice-add`, modifying behavior without touching the original
   source.

The architecture is: C core for rendering and Lisp evaluation +
Elisp for everything else. The boundary is clear: the C core never
changes at runtime; the Elisp layer is fully mutable.

**Relevance to bisque-computer**: Emacs shows that a "thin native core +
thick scriptable layer" is a proven architecture. The Rust/vello/winit
core would play the role of the C core; a scripting layer (Rhai or Lua)
would play the role of Elisp.

Sources:
- [GNU Emacs Lisp Eval](https://www.gnu.org/software/emacs/manual/html_node/emacs/Lisp-Eval.html)
- [Evaluating Elisp in Emacs](https://www.masteringemacs.org/article/evaluating-elisp-emacs)

### HyperCard

Released by Apple in 1987. HyperCard obliterated the boundary between
using software and creating software.

Key architectural features:

- **Stacks, cards, buttons, fields**: the UI is a hierarchy of
  objects, each of which can have HyperTalk scripts attached.
- **No separate developer mode**: browsing, editing, and programming
  happen within the same interface. You can open any button and see
  its code. Bill Atkinson said there would never be a HyperCard
  compiler because of the self-modifying script capability.
- **Event-driven messaging**: events bubble up through the object
  hierarchy (button -> card -> stack). This is the same pattern
  used by the DOM in web browsers.
- **Self-modifying scripts**: HyperCard scripts could modify
  themselves at runtime -- a feature that explicitly prevented
  ahead-of-time compilation.

HyperCard was withdrawn from sale in 2004 but its influence is
enormous: it inspired the web, wiki software, and modern no-code tools.

**Relevance to bisque-computer**: HyperCard's "click to inspect,
edit in place" model is directly applicable. An "inspector mode"
where clicking a UI element reveals its properties (font size,
color, position) and lets you edit them would be the simplest
possible self-modifying interface.

Sources:
- [HyperCard Wikipedia](https://en.wikipedia.org/wiki/HyperCard)
- [HyperCard's Legacy](https://medium.com/@avonliden/hypercards-legacy-the-revolution-we-lost-2819ca0b63ac)
- [User-Modifiable Software: HyperCard Properties](https://usermodifiable.codingitwrong.com/hypercard/properties)

### Max/MSP, Pure Data, TouchDesigner

Node-based visual programming environments where the program IS the
interface. In Max/MSP and Pure Data, you edit a "patch" (a directed
graph of objects connected by "patch cords") while it is running.

Key characteristics:

- **Dataflow architecture**: the program is modeled as data flowing
  between operations. There is no sequential control flow; the
  graph topology IS the program.
- **Edit-while-running**: in Max/MSP, you toggle between "edit mode"
  (drag objects, connect cords) and "run mode" (interact with UI
  objects). In practice, many operations work in both modes.
- **TouchDesigner**: built for real-time visual processing with
  multi-threaded GPU utilization. Operators are connected in a
  network, and changes propagate immediately.

Pure Data is open-source and is both an example of dataflow programming
and a platform for real-time audio/visual processing.

**Relevance to bisque-computer**: The dataflow model is less directly
applicable (bisque-computer is not a node graph), but the principle
of "edit-while-running" is key. The terminal pane could serve as
the "edit mode" interface for the dashboard.

Sources:
- [Pure Data Wikipedia](https://en.wikipedia.org/wiki/Pure_Data)
- [TouchDesigner vs Max/MSP](https://www.dlux.org.au/touchdesigner-vs-max-msp-choosing-your-real-time-visual-programming-platform/)

### Bret Victor: "Inventing on Principle"

Bret Victor's guiding principle: creators need an immediate connection
to what they are creating. When you make a change, you must see the
effect immediately.

His demonstrations include:

- **Live coding of a tree visualization**: changing parameters (branch
  length, blossom count) immediately updates the on-screen image.
  No compile step, no refresh. The code and the output are
  side-by-side, and changes propagate in real time.
- **Circuit simulation**: voltage and current flows visualized in
  real time as users manipulate components.
- **Animation system**: hand gestures directly control character
  movements with immediate playback.

The architecture underlying all demos prioritizes reducing the
feedback loop between code changes and visual output.

**Relevance to bisque-computer**: This is the philosophical
foundation. The terminal pane and the dashboard pane are already
side-by-side (Cmd+Left/Right). The goal is to make edits in the
terminal propagate visually to the dashboard within a single frame.

Sources:
- [Inventing on Principle transcript](https://jamesclear.com/great-speeches/inventing-on-principle-by-bret-victor)
- [Bret Victor Wikipedia](https://en.wikipedia.org/wiki/Bret_Victor)

### Observable Notebooks

Founded by Mike Bostock (creator of D3.js). Observable notebooks use
a reactive programming model where cells re-run automatically when
referenced variables change, like a spreadsheet.

Key architectural features:

- **Reactive dataflow**: changing a cell's value automatically
  re-evaluates all dependent cells. No explicit refresh needed.
- **Live execution**: code runs when you load the notebook, and
  re-runs on edit.
- **View inputs**: cells can define interactive inputs with `view()`,
  creating bidirectional bindings between UI controls and code.
- **Human-readable format**: Notebooks 2.0 uses an HTML-based file
  format that works in standard text editors.

**Relevance to bisque-computer**: The reactive model maps well to
a design-token system. Changing a token value (like `bg_color`)
would automatically re-render all UI elements that depend on it.

Sources:
- [Observable Notebooks](https://observablehq.com/platform/notebooks)
- [Observable Notebooks 2.0](https://observablehq.com/notebook-kit/)
- [Interesting Ideas in Observable Framework](https://simonwillison.net/2024/Mar/3/interesting-ideas-in-observable-framework/)

---

## 2. Live-Coding Systems for Graphics

### Processing / p5.js

Processing (Java) and p5.js (JavaScript) are the standard creative
coding toolkits. The workflow is: edit code in a text editor, press
"play," see the visual output. The feedback loop is fast but not
instant -- there is a compile/reload step.

p5.js in the browser has near-instant reload via the standard
browser hot-reload mechanism. Processing for Java requires a
recompile.

### Shadertoy / ISF

Shadertoy provides a browser-based editor for GLSL fragment shaders.
The shader recompiles on every keystroke, and the output is displayed
in real time. This is possible because:

1. Shaders compile in milliseconds (GPU compiler).
2. There is no application state to preserve -- each frame is
   computed from scratch from the shader code and uniforms.

This architecture is relevant: if bisque-computer's visual constants
are treated like "uniforms" (external parameters fed to the renderer),
changing them requires no recompilation.

### Nannou (Rust Creative Coding)

Nannou is a Rust creative coding framework that uses wgpu for
rendering. It does NOT have built-in hot reload, but the community
has built solutions:

- **nannou-hot-reload** (https://github.com/rksm/nannou-hot-reload):
  a `cargo-generate` template that splits the project into a
  main binary (windowing, event loop) and a dynamic library
  (the `model`/`update`/`view` functions). The library is compiled
  as a `.dylib` and reloaded at runtime when the source changes.
- **GLSL hot-reload**: the nannou organization provides a crate for
  hotloading GLSL shaders as SPIR-V.

The fundamental Rust limitation is that Rust compiles to native code
with no VM, so "hot reload" requires either:
1. Dynamic library reloading (complex, fragile ABI).
2. A scripting layer (Rhai, Lua, WASM) for the hot-reloadable parts.
3. File-based data reloading (TOML, JSON) for simple values.

Sources:
- [Nannou GitHub](https://github.com/nannou-org/nannou)
- [nannou-hot-reload](https://github.com/rksm/nannou-hot-reload)
- [Hot Reloading for Rust Gamedev](https://ryanisaacg.com/posts/hot-reloading-rust.html)

### Flutter Hot Reload

Flutter's hot reload is the gold standard for native-app live coding.
The mechanism:

1. **Dart JIT compiler** compiles modified code in memory and sends
   updated classes to the running Dart VM.
2. **Widget tree rebuilding**: Flutter re-runs the `build()` method
   of affected widgets, NOT the whole app.
3. **Element tree diffing**: the "reconciliation layer" between
   widgets and render objects. An efficient diffing algorithm
   handles widget identity and state preservation.
4. **State preservation**: `useState` and `useRef` equivalents
   (`StatefulWidget` + `State`) are preserved in memory across
   reloads.
5. **Retained-mode rendering**: only changed parts of the UI are
   re-laid-out and re-painted.

**Relevance to bisque-computer**: bisque-computer uses vello, which
is an immediate-mode renderer (the entire scene is rebuilt each
frame). This means there is no diffing needed -- changing a design
token simply changes the next frame's output. The challenge is not
rendering; it is getting the new values into the render loop.

Sources:
- [Flutter Hot Reload](https://docs.flutter.dev/tools/hot-reload)
- [Flutter Reload Under the Hood](https://medium.com/flutter-community/flutter-reload-whats-under-the-hood-978bce8af874)
- [Hot Reload Deep Dive](https://mailharshkhatri.medium.com/hot-reload-deep-dive-how-it-works-in-flutter-54c722c9dfc7)

### SwiftUI Previews

Xcode achieves near-instant SwiftUI preview updates through:

1. **Shared build artifacts**: Preview and Build-and-Run share the
   same compiled artifacts (since Xcode 16).
2. **Incremental strategies**: simple literal changes are patched
   into the running preview process without recompilation.
   Method-level changes regenerate only the affected `.o` files.
3. **XPC communication**: separate preview processes communicate
   with Xcode via XPC (Apple's inter-process communication).
4. **`__designTimeString()`**: a special function that returns the
   latest value of updated string literals, enabling instant
   reflection of literal changes.

**Relevance to bisque-computer**: the "literal patching" idea is
interesting. Design tokens are, in effect, literals. If they live
in an external file (TOML), "patching" is just re-reading the file.

Sources:
- [SwiftUI Previews Apple Docs](https://developer.apple.com/documentation/swiftui/previews-in-xcode)
- [How SwiftUI Preview Works Under the Hood](https://onee.me/en/blog/how-new-xcode-swiftui-preview-works-under-the-hood/)
- [Building Stable Preview Views](https://fatbobman.com/en/posts/how-swiftui-preview-works/)

### React Fast Refresh

React Fast Refresh preserves component state across code edits:

1. **Babel plugin**: detects all components and custom Hooks, inserts
   registration calls to collect Hook signatures.
2. **Function components**: their state (`useState`, `useRef`) is
   preserved as long as Hook call order doesn't change.
3. **Dependency arrays ignored**: `useEffect`, `useMemo`, etc.
   always re-run during Fast Refresh, ignoring dependency lists.
4. **Manual reset**: `// @refresh reset` directive forces remount.

**Relevance**: React Fast Refresh works because React owns the
component tree and can surgically update it. In bisque-computer,
vello owns nothing between frames -- the scene is rebuilt from
scratch each frame. This is actually simpler: there is no tree
to patch, just new values to feed into the next `render_dashboard()`.

Sources:
- [React Fast Refresh docs](https://reactnative.dev/docs/fast-refresh)
- [Next.js Fast Refresh Architecture](https://nextjs.org/docs/architecture/fast-refresh)
- [Beyond HMR: Understanding React's Fast Refresh](https://dev.to/leapcell/beyond-hmr-understanding-reacts-fast-refresh-13h8)

---

## 3. Architecture for "Edit From Within"

The core question: how does an edit in the terminal pane (Screen 2)
trigger a visual change in the dashboard pane (Screen 0)?

### 3.1 Data Flow in the Current Codebase

From reading the source:

```
main.rs:
  App struct holds:
    - scene: Scene (rebuilt every frame in RedrawRequested)
    - pane_tree: Option<PaneTree> (PTY-backed terminal)
    - app_mode_machine: StateMachine<AppModeMachine>
      - inner: instances: SharedInstances (Arc<Mutex<Vec<LobsterInstance>>>)

  On RedrawRequested:
    1. drain PTY output
    2. rebuild selectable regions
    3. render Screen 0 (dashboard): dashboard::render_dashboard()
    4. render Screen 1 (info): info_screen::render_info_screen()
    5. render Screen 2 (terminal): pane_tree.render_into_scene()
    6. composite with Affine::translate based on screen_offset
    7. submit to GPU

dashboard.rs:
  - All visual constants are Rust `const`:
    BG_COLOR, TEXT_PRIMARY, TEXT_SECONDARY, TITLE_SIZE, etc.
  - render_dashboard() reads SharedInstances and draws text

design.rs:
  - Comprehensive design system: type scale, spacing, margins, colors
  - All values are `const` (compile-time)
```

The key insight: **every frame is rendered from scratch**. There is
no retained scene graph to patch. If we can change the values that
`render_dashboard()` reads between frames, the visual update is
free -- it happens on the next `RedrawRequested`.

### 3.2 Pattern: File Watcher

```
Terminal pane: user edits design.toml (e.g., `vim design.toml`)
                    |
                    v
File system: design.toml saved
                    |
                    v
notify crate: detects file modification
                    |
                    v
Main thread: reloads design.toml, updates DesignTokens struct
                    |
                    v
Next RedrawRequested: render_dashboard() reads new tokens
                    |
                    v
User sees updated dashboard
```

Latency: ~16ms (one frame at 60fps) after file save.

The `notify` crate (https://github.com/notify-rs/notify) is the
standard Rust file watcher. It supports macOS FSEvents, Linux
inotify, and Windows ReadDirectoryChangesW.

### 3.3 Pattern: REPL

```
Terminal pane: user types `set bg #FAEBD7` in design REPL
                    |
                    v
REPL parser: parses command, validates values
                    |
                    v
DesignTokens: mutates in-place (no file I/O)
                    |
                    v
Next RedrawRequested: render_dashboard() reads new tokens
                    |
                    v
User sees updated dashboard instantly
```

Latency: sub-frame. The mutation happens in the same process, no
file I/O, no watcher delay.

### 3.4 Pattern: Inspector

```
User presses a hotkey (e.g., Cmd+I) to enter "inspect mode"
                    |
                    v
Mouse cursor changes to crosshair
                    |
                    v
User clicks on a text element (e.g., the dashboard title)
                    |
                    v
Inspector overlay shows properties:
  - font: Optima
  - size: 44.0px
  - color: #000000 (1.0 opacity)
  - position: (48.0, 56.0)
                    |
                    v
User edits a property inline (e.g., changes size to 52.0)
                    |
                    v
DesignTokens updated -> next frame reflects change
```

This requires hit-testing infrastructure. The existing
`text_selection.rs` already has `hit_test(x, y)` for selectable
regions -- this could be extended to an inspector.

### 3.5 Serialization / Deserialization

For incremental edits to persist, the design state must be
serializable. TOML is the natural choice:

```toml
[colors]
bg = [1.0, 0.894, 0.769, 1.0]
text_primary = [0.0, 0.0, 0.0, 1.0]
text_secondary = [0.0, 0.0, 0.0, 0.50]

[type_scale]
base = 18.0
ratio = 1.333

[spacing]
baseline = 28.0
margin_left_frac = 0.111
margin_right_frac = 0.222

[layout]
left_margin = 48.0
section_spacing = 32.0
```

This maps directly onto the existing `design.rs` constants.
Serde + toml crate handle the (de)serialization.

---

## 4. The Figma / Browser DevTools Model

### Figma's Architecture

Figma achieves instant visual updates through:

1. **Custom rendering pipeline**: originally WebGL, now WebGPU.
   Written in C++ (compiled to WASM), with the UI chrome in
   JavaScript. This is a hybrid architecture similar to what
   bisque-computer uses (Rust core + vello GPU rendering).
2. **CRDTs for conflict resolution**: edits are represented as
   operations on a CRDT (Conflict-Free Replicated Data Type),
   enabling multiplayer without locking.
3. **Local-first editing**: edits are applied locally and rendered
   immediately, then synchronized to the server. The rendering
   prioritizes local edits over remote changes.
4. **WebSocket communication**: clients communicate with servers
   over WebSockets (the same protocol bisque-computer already
   uses for Lobster data).

**Relevance**: Figma's "local edit -> immediate render -> sync later"
model maps to bisque-computer's potential workflow: edit a design
token locally, see it render immediately, optionally sync the
change to the Rust source code.

Sources:
- [How Figma's Multiplayer Works](https://www.figma.com/blog/how-figmas-multiplayer-technology-works/)
- [Keeping Figma Fast](https://www.figma.com/blog/keeping-figma-fast/)
- [Figma Rendering: WebGPU](https://www.figma.com/blog/figma-rendering-powered-by-webgpu/)
- [Made by Evan: Figma](https://madebyevan.com/figma/)

### Browser DevTools

Chrome/Firefox DevTools enable live CSS editing because:

1. **Separation of structure from style**: HTML defines structure,
   CSS defines appearance. You can change one without the other.
2. **Incremental re-layout**: the browser engine only re-lays-out
   the parts of the page affected by the CSS change.
3. **Style system as a key-value store**: CSS properties are
   essentially design tokens. Changing `font-size: 18px` to
   `font-size: 24px` is a single property mutation.
4. **No recompilation**: CSS is interpreted, not compiled. Changes
   take effect on the next paint.

**Relevance**: bisque-computer's design constants in `design.rs`
and `dashboard.rs` are functionally identical to CSS custom
properties. If they were loaded from a file instead of being
compile-time constants, they could be edited with DevTools-like
immediacy.

A "design token override" system would work like this:

```
1. App starts with compiled defaults from design.rs
2. On startup, check for ~/.config/bisque-computer/design.toml
3. If present, load and override matching tokens
4. Watch the file for changes (notify crate)
5. On change, re-read, merge, apply on next frame
```

Sources:
- [Live Editing HTML and CSS with Chrome DevTools](https://lucid.co/techblog/2018/05/01/live-editing-html-css-chrome-devtools)
- [Chrome DevTools CSS Reference](https://developer.chrome.com/docs/devtools/css/reference)

---

## 5. Concrete Architecture Proposals

### Architecture A: Script-Driven Rendering with Rhai

**Overview**: embed the Rhai scripting language. A `.rhai` script
defines the dashboard layout, colors, fonts, and spacing. The Rust
host handles the window, GPU, input, terminal, and WebSocket
connections. Editing the script in the terminal triggers a hot-reload.

**What goes in the script**:
- Color definitions (background, text colors, opacity levels)
- Type scale (base size, ratio, named steps)
- Layout parameters (margins, spacing, grid)
- Section ordering and visibility
- Conditional display logic (e.g., "hide Memory section if < 5 events")

**What stays in Rust**:
- Window creation, event loop (winit)
- GPU rendering (vello, wgpu)
- Terminal emulation (alacritty_terminal, portable-pty)
- WebSocket communication (tokio-tungstenite)
- Font loading and glyph layout (skrifa)
- Input handling (keyboard, mouse)
- State machines (statig)

**Implementation**:

```rust
// In Cargo.toml:
// rhai = { version = "1", features = ["sync"] }
// notify = "7"

use rhai::{Engine, AST, Scope};
use std::sync::{Arc, RwLock};

struct DesignTokens {
    bg_color: [f32; 4],
    text_primary: [f32; 4],
    title_size: f64,
    section_spacing: f64,
    // ... all tokens from design.rs
}

struct ScriptEngine {
    engine: Engine,
    ast: Arc<RwLock<AST>>,
    tokens: Arc<RwLock<DesignTokens>>,
}

impl ScriptEngine {
    fn reload(&self, script_path: &str) -> Result<()> {
        let new_ast = self.engine.compile_file(script_path.into())?;
        let mut scope = Scope::new();
        self.engine.run_ast_with_scope(&mut scope, &new_ast)?;

        // Extract design tokens from scope
        let mut tokens = self.tokens.write().unwrap();
        if let Some(bg) = scope.get_value::<rhai::Array>("bg_color") {
            tokens.bg_color = parse_color_array(&bg);
        }
        // ... extract other tokens

        *self.ast.write().unwrap() = new_ast;
        Ok(())
    }
}
```

**Rhai's built-in hot reload support**:

Rhai explicitly documents hot reloading as a supported pattern
(https://rhai.rs/book/patterns/hot-reload.html). The Engine is
re-entrant: compile a new script, replace the old AST with the
new one, and new behaviors are immediately active. For
multi-threaded use, replace `Rc` with `Arc` and `RefCell` with
`RwLock`, and enable the `sync` feature.

**File watcher integration**:

```rust
use notify::{Watcher, RecommendedWatcher, RecursiveMode};

fn spawn_script_watcher(
    script_path: PathBuf,
    engine: Arc<ScriptEngine>,
) -> notify::Result<RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |res| {
        if let Ok(event) = res {
            if event.kind.is_modify() {
                if let Err(e) = engine.reload(script_path.to_str().unwrap()) {
                    tracing::warn!("Script reload failed: {}", e);
                }
            }
        }
    })?;
    watcher.watch(&script_path, RecursiveMode::NonRecursive)?;
    Ok(watcher)
}
```

**Example Rhai script** (`~/.config/bisque-computer/layout.rhai`):

```rhai
// Design tokens
let bg_color = [1.0, 0.894, 0.769, 1.0];     // bisque
let text_primary = [0.0, 0.0, 0.0, 1.0];      // black
let text_secondary = [0.0, 0.0, 0.0, 0.50];   // 50% black

// Type scale
let type_base = 18.0;
let type_ratio = 1.333;
let title_size = 44.0;
let section_size = 18.0;

// Layout
let left_margin = 48.0;
let section_spacing = 32.0;

// Section visibility
let show_memory = true;
let show_agents = true;
let max_agent_rows = 4;
```

**Complexity**: Medium-high. Requires adding Rhai as a dependency,
building the bridge between Rhai scope and Rust rendering, handling
script errors gracefully, and maintaining the script API as the
Rust code evolves.

**Flexibility**: High. Scripts can contain conditional logic, loops,
computed values. A skilled user could completely rearrange the
dashboard layout.

**Developer experience**: Good for power users. Edit a script in
vim/nano in the terminal pane, save, see changes on the dashboard
pane within one frame. Errors are reported in the terminal.

**Compatibility with existing codebase**: Moderate. The current
`render_dashboard()` reads `const` values. These would need to
be replaced with reads from `Arc<RwLock<DesignTokens>>`. The
rendering code itself stays in Rust.

### Architecture B: Design Token Hot-Reload via TOML

**Overview**: all visual constants from `design.rs` and `dashboard.rs`
are loaded from a TOML file at startup. A file watcher reloads the
TOML on change. Editing the TOML from the terminal triggers an
instant visual update.

**Implementation**:

```rust
// In Cargo.toml:
// toml = "0.8"
// notify = "7"
// serde = { version = "1", features = ["derive"] }

use serde::Deserialize;

#[derive(Deserialize, Clone)]
struct DesignTokens {
    colors: Colors,
    type_scale: TypeScale,
    spacing: Spacing,
    layout: Layout,
}

#[derive(Deserialize, Clone)]
struct Colors {
    bg: [f32; 4],
    text_primary: [f32; 4],
    text_secondary: [f32; 4],
    text_section: [f32; 4],
    text_annotation: [f32; 4],
    rule: [f32; 4],
}

#[derive(Deserialize, Clone)]
struct TypeScale {
    base: f64,
    ratio: f64,
    title_size: f64,
    section_size: f64,
    data_primary_size: f64,
    data_secondary_size: f64,
    annotation_size: f64,
}

#[derive(Deserialize, Clone)]
struct Spacing {
    baseline: f64,
    section_spacing: f64,
    line_height_factor: f64,
}

#[derive(Deserialize, Clone)]
struct Layout {
    left_margin: f64,
    rule_thickness: f64,
    margin_left_frac: f64,
    margin_right_frac: f64,
}

impl DesignTokens {
    fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }

    fn defaults() -> Self {
        // Map from existing design.rs constants
        Self {
            colors: Colors {
                bg: [1.0, 0.894, 0.769, 1.0],
                text_primary: [0.0, 0.0, 0.0, 1.0],
                // ...
            },
            // ...
        }
    }
}
```

**TOML file** (`~/.config/bisque-computer/design.toml`):

```toml
[colors]
bg = [1.0, 0.894, 0.769, 1.0]
text_primary = [0.0, 0.0, 0.0, 1.0]
text_secondary = [0.0, 0.0, 0.0, 0.50]
text_section = [0.0, 0.0, 0.0, 0.80]
text_annotation = [0.0, 0.0, 0.0, 0.40]
rule = [0.0, 0.0, 0.0, 0.15]

[type_scale]
base = 18.0
ratio = 1.333
title_size = 44.0
section_size = 18.0
data_primary_size = 24.0
data_secondary_size = 20.0
annotation_size = 16.0

[spacing]
baseline = 28.0
section_spacing = 32.0
line_height_factor = 1.4

[layout]
left_margin = 48.0
rule_thickness = 0.5
margin_left_frac = 0.111
margin_right_frac = 0.222
```

**Integration with the render loop**:

The `App` struct gains a `tokens: Arc<RwLock<DesignTokens>>` field.
The file watcher runs on a background thread and updates the
`RwLock` on file change. On each `RedrawRequested`, the render
functions read the current tokens:

```rust
// In render_dashboard:
let tokens = self.tokens.read().unwrap();
let bg_color = Color::new(tokens.colors.bg);
let title_size = tokens.type_scale.title_size;
// ... use tokens instead of const values
```

**Complexity**: Low. TOML parsing with serde is straightforward.
`notify` is a single dependency. The main work is replacing `const`
references with `tokens.field` reads in `dashboard.rs`.

**Flexibility**: Limited to value changes. Cannot add new sections,
change rendering logic, or implement conditional display. But
covers the vast majority of visual customization needs.

**Developer experience**: Excellent. Edit a well-documented TOML
file with any text editor. The format is self-explanatory. Errors
are easily reported (invalid TOML, out-of-range values).

**Compatibility with existing codebase**: High. The existing `const`
values in `design.rs` become the defaults. The TOML file is an
optional override. No structural changes to the rendering code
beyond replacing const reads with token reads.

**New dependencies**: `toml` (already using `serde`), `notify`.

### Architecture C: Embedded Design REPL

**Overview**: the terminal pane can switch to a "design REPL" mode.
The user types commands like `set bg #FAEBD7` or `set type.base 20`
and sees results instantly. State persists in memory; `save` writes
changes back to the TOML design tokens file.

**Implementation**:

```rust
/// Commands recognized by the design REPL.
enum DesignCommand {
    Set { path: String, value: String },
    Get { path: String },
    List,
    Reset,
    Save,
    Help,
}

fn parse_design_command(input: &str) -> Option<DesignCommand> {
    let parts: Vec<&str> = input.trim().split_whitespace().collect();
    match parts.first()? {
        &"set" if parts.len() >= 3 => Some(DesignCommand::Set {
            path: parts[1].to_string(),
            value: parts[2..].join(" "),
        }),
        &"get" if parts.len() >= 2 => Some(DesignCommand::Get {
            path: parts[1].to_string(),
        }),
        &"list" => Some(DesignCommand::List),
        &"reset" => Some(DesignCommand::Reset),
        &"save" => Some(DesignCommand::Save),
        &"help" => Some(DesignCommand::Help),
        _ => None,
    }
}

fn execute_design_command(
    cmd: DesignCommand,
    tokens: &mut DesignTokens,
) -> String {
    match cmd {
        DesignCommand::Set { path, value } => {
            match path.as_str() {
                "bg" => {
                    if let Some(color) = parse_color(&value) {
                        tokens.colors.bg = color;
                        format!("bg = {:?}", color)
                    } else {
                        "Error: invalid color (use #RRGGBB or [r,g,b,a])".into()
                    }
                }
                "type.base" => {
                    if let Ok(v) = value.parse::<f64>() {
                        tokens.type_scale.base = v;
                        format!("type.base = {}", v)
                    } else {
                        "Error: expected a number".into()
                    }
                }
                // ... other paths
                _ => format!("Unknown token: {}", path),
            }
        }
        DesignCommand::List => {
            // Return formatted list of all tokens and current values
            format_all_tokens(tokens)
        }
        DesignCommand::Save => {
            // Serialize to TOML and write to disk
            match save_tokens_to_disk(tokens) {
                Ok(path) => format!("Saved to {}", path),
                Err(e) => format!("Save error: {}", e),
            }
        }
        // ...
    }
}
```

**Terminal integration**: The design REPL could be implemented as:

1. **A separate PTY mode**: when the user types `bisque-design` in
   the terminal, a custom REPL process starts (or a special mode
   in the terminal pane is activated).
2. **An in-process REPL**: the terminal pane intercepts input when
   in "design mode" (toggled with a hotkey like Cmd+Shift+I) and
   feeds it to the command parser instead of the PTY.
3. **A Unix socket**: a small REPL server listens on a socket; the
   user connects from the terminal with `nc` or a custom CLI tool.

Option 2 is the most integrated: no extra process, no socket, just
a mode toggle in the existing terminal pane.

**Example REPL session**:

```
bisque> list
colors.bg = [1.0, 0.894, 0.769, 1.0]
colors.text_primary = [0.0, 0.0, 0.0, 1.0]
type_scale.base = 18.0
type_scale.title_size = 44.0
spacing.section_spacing = 32.0
layout.left_margin = 48.0
...

bisque> set type.base 24
type.base = 24.0

bisque> set colors.bg #FFF8DC
colors.bg = [1.0, 0.973, 0.863, 1.0]

bisque> save
Saved to ~/.config/bisque-computer/design.toml
```

**Complexity**: Medium. Requires building a command parser and
dispatcher, but no external dependencies beyond what exists.
The REPL is simple string processing.

**Flexibility**: Moderate. Limited to predefined token paths.
But the immediate feedback and discoverability (`list`, `help`)
make it very approachable.

**Developer experience**: Best for quick experiments. Type a
command, see the result. No file to manage, no editor to open.
The `save` command persists changes when satisfied.

**Compatibility with existing codebase**: Good. The REPL reads
and writes the same `Arc<RwLock<DesignTokens>>` as Architecture B.
The terminal pane already routes keyboard input; adding a mode
toggle is straightforward.

---

## 6. Comparative Analysis

| Criterion                | A: Rhai Scripts     | B: TOML Hot-Reload  | C: Design REPL     |
|--------------------------|---------------------|---------------------|---------------------|
| Implementation effort    | 3-4 days            | 1-2 days            | 2-3 days            |
| New dependencies         | rhai, notify        | toml, notify        | (none / toml)       |
| Flexibility              | High (logic + data) | Low (data only)     | Medium (data only)  |
| Learning curve for user  | Must learn Rhai     | Must know TOML      | Self-documenting    |
| Feedback latency         | ~16ms (file save)   | ~16ms (file save)   | Sub-frame           |
| Error handling           | Rhai error messages | TOML parse errors   | Inline error msgs   |
| Persistence              | Script file         | TOML file           | In-memory + save    |
| Extensibility            | Can add functions   | Add TOML sections   | Add commands        |
| Risk of breaking app     | Moderate (bad script)| Low (bad values)   | Low (validated)     |

## 7. Recommended Approach: B + C (Layered)

The strongest architecture combines B and C:

1. **Start with Architecture B** (TOML hot-reload):
   - Create `DesignTokens` struct with serde derive.
   - Move all `const` values from `design.rs` and `dashboard.rs`
     into `DesignTokens::defaults()`.
   - Add `Arc<RwLock<DesignTokens>>` to `App`.
   - Replace `const` reads with `tokens.field` reads in render
     functions.
   - Add `notify` file watcher for `design.toml`.
   - Ship a default `design.toml` in the repo.

2. **Layer Architecture C on top** (Design REPL):
   - Add a "design mode" toggle to the terminal pane (Cmd+Shift+I).
   - In design mode, intercept terminal input, parse commands,
     mutate the shared `DesignTokens`.
   - The `save` command serializes to the same `design.toml` that
     the file watcher monitors.
   - The `list` and `help` commands make the system self-documenting.

3. **Optionally add Architecture A later** (Rhai scripting):
   - When layout logic needs to be customizable (section ordering,
     conditional display), add Rhai.
   - The Rhai script reads and writes the same `DesignTokens`.
   - This is additive -- it does not replace B or C.

This layered approach gives:
- **Lowest-effort first win**: TOML hot-reload works in 1-2 days.
- **Best interactive experience**: the REPL provides instant
  feedback without leaving the app.
- **Future extensibility**: Rhai can be added later without
  rearchitecting.
- **Compatibility**: the existing codebase changes minimally.
  `const` values become `tokens.read().unwrap().field` values.
  The rendering code stays in Rust.

---

## 8. Key Implementation Details

### Thread Safety

The `DesignTokens` struct must be shared between:
- The file watcher thread (writes on file change)
- The REPL (writes on command)
- The render loop (reads on each frame)

`Arc<RwLock<DesignTokens>>` is the correct synchronization
primitive. Reads are non-blocking (multiple readers OK). Writes
are exclusive but brief (deserialize TOML, swap struct).

### Error Recovery

If `design.toml` contains invalid TOML or out-of-range values:
1. Log the error via `tracing::warn!()`.
2. Keep the previous valid tokens.
3. Display an error indicator in the dashboard (e.g., a small
   "design error" annotation in the corner).

### Default Generation

On first run, if `design.toml` does not exist, generate it from
`DesignTokens::defaults()` so the user has a complete, commented
template to edit.

### Existing Constants to Migrate

From `dashboard.rs` (currently used for rendering):
- `BG_COLOR`, `TEXT_PRIMARY`, `TEXT_SECONDARY`, `TEXT_SECTION`,
  `TEXT_ANNOTATION`, `RULE_COLOR`
- `TITLE_SIZE`, `SECTION_SIZE`, `DATA_PRIMARY_SIZE`,
  `DATA_SECONDARY_SIZE`, `ANNOTATION_SIZE`
- `LEFT_MARGIN`, `SECTION_SPACING`, `LINE_HEIGHT_FACTOR`,
  `RULE_THICKNESS`

From `design.rs` (comprehensive but currently unused by dashboard):
- `BG`, `INK_PRIMARY` through `INK_GHOST`
- `TYPE_BASE`, `TYPE_RATIO`, all `TYPE_*` sizes
- `BASELINE`, `SPACE_*` constants
- `MARGIN_*` constants
- `GUTTER`, `RULE_THICKNESS`, `MEASURE_*`

The migration path: make `design.rs` the canonical source, update
`dashboard.rs` to read from `design.rs` (or from runtime tokens),
and retire the duplicate constants in `dashboard.rs`.

### File Paths

- Design tokens: `~/.config/bisque-computer/design.toml`
- Rhai scripts (future): `~/.config/bisque-computer/layout.rhai`
- Both paths follow the existing convention established by the
  server URL config at `~/.config/bisque-computer/server`.
