# Understanding Rustty's Window System

**A friendly guide to how windows, rendering, and keyboard input actually work**

## Table of Contents

1. [Let's Start with the Window](#lets-start-with-the-window)
2. [How Does the Terminal Grid Fit Inside?](#how-does-the-terminal-grid-fit-inside)
3. [What Happens When You Resize?](#what-happens-when-you-resize)
4. [How Does Rendering Actually Work?](#how-does-rendering-actually-work)
5. [The CPU Renderer (Raqote)](#the-cpu-renderer-raqote)
6. [The GPU Renderer (wgpu)](#the-gpu-renderer-wgpu)
7. [What Happens When You Press a Key?](#what-happens-when-you-press-a-key)
8. [How Does the Event Loop Work?](#how-does-the-event-loop-work)
9. [Coordinate Systems Explained](#coordinate-systems-explained)
10. [Quick Reference](#quick-reference)

---

## Let's Start with the Window

When you run Rustty, the first thing that happens is a window appears on your screen. Pretty obvious, right?

But here's what's actually going on:

Rustty uses a library called **winit** to create cross-platform windows. Here's the code that makes your window:

```rust
let window_attrs = Window::default_attributes()
    .with_title("Rustty Terminal")
    .with_inner_size(winit::dpi::LogicalSize::new(800, 600));

let window = event_loop.create_window(window_attrs)?;
```

**What does this do?**
- Creates a window titled "Rustty Terminal"
- Sets the initial size to 800×600 pixels
- Gives you a window handle that you can use to draw stuff

The window is stored in an `Arc<Window>` (an atomic reference-counted pointer). Why? Because we might need to access the same window from multiple places in our code, and `Arc` makes that safe.

**The window is just a blank canvas.** It doesn't know anything about terminals, characters, or shell processes. It's just a rectangle on your screen waiting for us to draw something.

---

## How Does the Terminal Grid Fit Inside?

So you have an 800×600 pixel window. How do you decide how many terminal rows and columns fit inside?

### Step 1: Character Dimensions

First, we need to know how big each character cell is:

```rust
const CHAR_WIDTH: f32 = 9.0;   // pixels
const CHAR_HEIGHT: f32 = 20.0; // pixels
```

These numbers come from the font we use (CaskaydiaCove Nerd Font at 16pt). Each character takes up a 9×20 pixel rectangle.

### Step 2: Account for Padding

We don't draw right up to the edges. There's some padding:

```rust
const OFFSET_X: f32 = 10.0;  // 10 pixels from left/right edges
const OFFSET_Y: f32 = 20.0;  // 20 pixels from top/bottom edges
```

So the **usable space** is actually:
- Width: 800 - 20 = 780 pixels
- Height: 600 - 40 = 560 pixels

### Step 3: Calculate Grid Size

Now we just divide:

```rust
let cols = (780.0 / 9.0).floor() as usize;   // = 86 columns
let rows = (560.0 / 20.0).floor() as usize;  // = 28 rows
```

**That's it!** An 800×600 window gives you an 86×28 terminal grid.

Here's the visual:

```
┌────────────────────────────────────────┐
│ ← 10px → Terminal Content ← 10px →     │  ← 20px top
│                                        │
│  ┌──────────────────────────────────┐  │
│  │ [86 columns × 28 rows]           │  │
│  │ Each cell: 9×20 pixels           │  │
│  │                                  │  │
│  └──────────────────────────────────┘  │
│                                        │  ← 20px bottom
└────────────────────────────────────────┘
     800 pixels total
```

**What if the result is a fraction?** We use `.floor()` to round down. You can't have half a character, so we just ignore the extra pixels.

**Minimum size:** To prevent unusably small terminals, we enforce a minimum of 10 columns × 3 rows.

---

## What Happens When You Resize?

You grab the window corner and drag. The window gets bigger. Now what?

### The Resize Event

Your window manager sends a `WindowEvent::Resized` event with the new size:

```rust
WindowEvent::Resized(new_size) => {
    // new_size.width = 1200
    // new_size.height = 800
}
```

### Recalculate Everything

We immediately recalculate the grid size:

```rust
let (cols, rows) = calculate_grid_size(new_size.width, new_size.height);
// With 1200×800: cols = 131, rows = 38
```

### Update Three Things

1. **Terminal grid** - Resize the internal grid data structure
2. **PTY size** - Tell the shell the new size (sends SIGWINCH signal)
3. **Renderer** - Update the renderer's viewport

```rust
self.session.resize(cols, rows);  // Updates grid and PTY
self.renderer.resize(width, height);  // GPU only: reconfigure surface
window.request_redraw();  // Trigger a redraw
```

### What About the Content?

**The content is preserved!** When the grid resizes:
- If it gets bigger: New cells are filled with blanks
- If it gets smaller: Content scrolls into the scrollback buffer
- The scrollback buffer holds up to 10,000 lines

Your shell (bash, zsh, etc.) gets notified via SIGWINCH and redraws its prompt at the new size.

---

## How Does Rendering Actually Work?

Let's talk about how those characters actually appear on your screen.

### The Renderer Trait

Both CPU and GPU renderers implement the same trait:

```rust
pub trait Renderer {
    fn char_dimensions(&self) -> (f32, f32);
    fn resize(&mut self, width: u32, height: u32) -> Result<()>;
    fn render(&mut self, state: &TerminalState) -> Result<()>;
    fn render_with_blink(&mut self, state: &TerminalState,
                         cursor_visible: bool) -> Result<()>;
}
```

**What does this give us?**
- We can swap renderers at compile time (CPU vs GPU)
- Both renderers have the same interface
- The main application code doesn't need to know which renderer it's using

### The Rendering Loop

Here's what happens every frame:

1. **Get terminal state** - Current grid, cursor position, colors
2. **Call renderer** - Pass the state to the renderer
3. **Renderer draws everything** - Characters, colors, cursor
4. **Present to screen** - Show the result in the window

```
Terminal State → Renderer → Window
  (what to draw)  (how to draw)  (where to show)
```

Simple, right? Now let's look at how each renderer actually does the drawing.

---

## The CPU Renderer (Raqote)

The CPU renderer uses software rendering with [Raqote](https://github.com/jrmuizel/raqote) (a 2D graphics library).

### How It Works

**Step 1: Get a pixel buffer**

```rust
let mut buffer = surface.buffer_mut()?;
```

This gives you a chunk of memory representing the window's pixels. For an 800×600 window, that's 480,000 pixels (each pixel is 4 bytes: RGBA).

**Step 2: Clear to black**

```rust
for pixel in buffer.iter_mut() {
    *pixel = 0xFF000000;  // Solid black (ARGB format)
}
```

**Step 3: Draw backgrounds**

For each visible cell in the terminal grid:

```rust
for (row, col, cell) in viewport {
    let x = 10.0 + col as f32 * 9.0;
    let y = 20.0 + row as f32 * 20.0;

    // If background isn't black, draw a rectangle
    if cell.bg != BLACK {
        draw_rect(x, y, 9.0, 20.0, cell.bg);
    }
}
```

**Step 4: Draw text**

For each character:

```rust
if cell.ch != ' ' {
    // Rasterize the glyph (convert font outline to pixels)
    let glyph = font.rasterize_glyph(cell.ch);

    // Draw it at the correct position
    draw_glyph(x, y, glyph, cell.fg);
}
```

**Step 5: Apply text attributes**

- **Bold:** Brighten the color by 50%
- **Italic:** Add a cyan tint
- **Underline:** Draw a 2-pixel line below the text

**Step 6: Draw cursor**

```rust
match cursor.style {
    CursorStyle::Block => draw_rect(...),
    CursorStyle::Underline => draw_line(...),
    CursorStyle::Bar => draw_thin_rect(...),
}
```

**Step 7: Present**

```rust
buffer.present()?;
```

This copies the pixel buffer to the window and displays it.

### Performance

- **Frame time:** 1-2ms on a typical 80×24 terminal
- **CPU usage when idle:** < 0.1%
- **Memory:** One pixel buffer (width × height × 4 bytes)

**When to use:**
- Default choice, works on all systems
- Lower memory usage
- Better compatibility

---

## The GPU Renderer (wgpu)

The GPU renderer uses hardware acceleration with [wgpu](https://wgpu.rs/) (WebGPU).

### The Big Idea

Instead of drawing each character individually on the CPU, we:
1. Build a list of "quads" (rectangles) representing all characters
2. Upload that list to the GPU once
3. Let the GPU draw everything in parallel

**Result:** Much faster, especially for large terminals.

### The Glyph Atlas

The GPU renderer uses a clever trick called a **glyph atlas**.

**What is it?** A big texture (2048×2048 pixels) containing all the character glyphs we've seen so far:

```
┌─────────────────────────────────────┐
│ A B C D E F G ... ←─ Row 1          │
│ a b c d e f g ... ←─ Row 2          │
│ 0 1 2 3 4 5 6 ... ←─ Row 3          │
│ ! @ # $ % ^ & ... ←─ Row 4          │
│ ...                                  │
└─────────────────────────────────────┘
  Each glyph: 9×20 pixels
  Total capacity: ~11,377 glyphs
```

**How it works:**

1. **First time seeing 'A'?** Rasterize it and stick it in the atlas at position (0, 0)
2. **Need to draw 'A' again?** Look it up in the cache: "Oh, 'A' is at (0, 0)"
3. **Draw all the 'A's** using that one cached glyph

We keep a HashMap to remember where each character is:

```rust
HashMap<char, AtlasPosition>
// 'A' → (x: 0, y: 0, width: 9, height: 20)
// 'B' → (x: 9, y: 0, width: 9, height: 20)
// ...
```

### The Rendering Process

**Step 1: Build vertex buffer**

For each cell in the viewport:

```rust
for (row, col, cell) in viewport {
    // Convert pixel coordinates to NDC (-1.0 to +1.0)
    let x_ndc = (x / window_width) * 2.0 - 1.0;
    let y_ndc = 1.0 - (y / window_height) * 2.0;

    // Look up glyph in atlas (or rasterize if new)
    let atlas_pos = glyph_atlas.get_or_rasterize(cell.ch)?;

    // Create 6 vertices (2 triangles = 1 quad)
    vertices.extend(create_quad(
        position: [x_ndc, y_ndc],
        tex_coords: [u0, v0, u1, v1],  // Where in atlas
        fg_color: cell.fg,
        bg_color: cell.bg,
    ));
}
```

**Step 2: Upload to GPU**

```rust
queue.write_buffer(&vertex_buffer, 0, &vertices);
```

This sends all the vertex data to GPU memory.

**Step 3: Single draw call**

```rust
render_pass.set_pipeline(&pipeline);
render_pass.set_bind_group(0, &glyph_atlas.bind_group, &[]);
render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
render_pass.draw(0..vertices.len(), 0..1);
```

**That's it!** One draw call renders the entire terminal. The GPU processes all characters in parallel.

### The Shader

The vertex shader positions each quad:

```wgsl
@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    out.position = vec4<f32>(in.position, 0.0, 1.0);
    out.tex_coords = in.tex_coords;
    out.fg_color = in.fg_color;
    out.bg_color = in.bg_color;
    return out;
}
```

The fragment shader samples the glyph from the atlas and composites it:

```wgsl
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample glyph alpha from atlas
    let alpha = textureSample(atlas, sampler, in.tex_coords).r;

    // Mix background and foreground based on alpha
    let color = mix(in.bg_color.rgb, in.fg_color.rgb, alpha);
    return vec4<f32>(color, 1.0);
}
```

### Performance

- **Frame time:** < 1ms on large terminals (200×60)
- **Memory:** Glyph atlas (2048×2048 = 4MB) + vertex buffer
- **GPU usage:** Minimal (terminal rendering is not demanding)

**When to use:**
- Large terminals or high refresh rates
- When smooth scrolling is needed (future feature)
- When you have GPU drivers

### Dynamic Buffer Resizing

If you resize the window to be huge, we might need more vertices than we allocated. No problem:

```rust
if vertex_data.len() > vertex_buffer.size() {
    println!("Buffer too small, reallocating!");
    vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        size: vertex_data.len() as u64,
        ...
    });
}
```

The buffer automatically grows as needed.

---

## What Happens When You Press a Key?

Let's say you press the **up arrow** key. What happens?

### Step 1: OS sends event

Your operating system's window manager detects the key press and sends an event to winit:

```rust
WindowEvent::KeyboardInput { event, .. }
```

### Step 2: Check if it's pressed (not released)

```rust
if event.state == ElementState::Pressed {
    // Process the key
}
```

We ignore key release events—terminals only care about presses.

### Step 3: Convert to ANSI sequence

The key gets converted to an ANSI escape sequence:

```rust
match event.logical_key {
    Key::Named(NamedKey::ArrowUp) => {
        if state.application_cursor_keys {
            shell.write(b"\x1bOA")?;  // Application mode
        } else {
            shell.write(b"\x1b[A")?;  // Normal mode
        }
    }
    Key::Named(NamedKey::Enter) => shell.write(b"\r")?,
    Key::Named(NamedKey::Backspace) => shell.write(b"\x7f")?,
    Key::Character(ch) => shell.write(ch.as_bytes())?,
    ...
}
```

### Step 4: Write to shell

```rust
shell.write(bytes)?;
```

This writes the bytes to the PTY master, which sends them to the shell as if you typed them.

### The Complete Key Table

Here's what various keys send:

| Key | Normal Mode | App Cursor Mode | Notes |
|-----|-------------|-----------------|-------|
| ↑ | `\x1b[A` | `\x1bOA` | Arrow up |
| ↓ | `\x1b[B` | `\x1bOB` | Arrow down |
| → | `\x1b[C` | `\x1bOC` | Arrow right |
| ← | `\x1b[D` | `\x1bOD` | Arrow left |
| Home | `\x1b[H` | - | Beginning of line |
| End | `\x1b[F` | - | End of line |
| Enter | `\r` | - | Carriage return |
| Backspace | `\x7f` | - | Delete previous |
| Tab | `\t` | - | Tab character |
| F1 | `\x1bOP` | - | Function key |
| F2 | `\x1bOQ` | - | Function key |

**Control keys** generate control codes:

```rust
if modifiers.control_key() {
    // Ctrl+A = 1, Ctrl+B = 2, ..., Ctrl+Z = 26
    let code = (ch.to_ascii_uppercase() as u8 - b'A' + 1);
    shell.write(&[code])?;
}
```

So Ctrl+C sends byte `0x03` (which the shell interprets as SIGINT).

### Special: Paste (Ctrl+V)

When you press Ctrl+V:

1. **Read clipboard**
   ```rust
   let text = clipboard.get_text()?;
   ```

2. **Check bracketed paste mode**
   ```rust
   if state.bracketed_paste {
       shell.write(b"\x1b[200~")?;  // Start marker
       shell.write(text.as_bytes())?;
       shell.write(b"\x1b[201~")?;  // End marker
   } else {
       shell.write(text.as_bytes())?;
   }
   ```

**Why the markers?** They prevent paste injection attacks. The shell knows the text was pasted, not typed, so it won't execute it immediately.

### Special: Mouse Events

If mouse tracking is enabled (`ESC[?1000h`):

```rust
WindowEvent::MouseInput { state, button, .. } => {
    // Convert window coords to grid coords
    let (col, row) = window_to_grid_coords(x, y)?;

    // Generate SGR mouse sequence
    let sequence = if pressed {
        format!("\x1b[<{};{};{}M", button, col + 1, row + 1)
    } else {
        format!("\x1b[<{};{};{}m", button, col + 1, row + 1)
    };

    shell.write(sequence.as_bytes())?;
}
```

Applications like vim use this to handle mouse clicks.

---

## How Does the Event Loop Work?

Rustty uses winit's event loop system. Here's the basic structure:

```
Start
  ↓
resumed() ─→ Create window, renderer, shell
  ↓
Loop:
  ├─→ about_to_wait() ─→ Check for shell output
  │                      Check cursor blink timer
  │                      Set next wake time (16ms)
  ↓
  ├─→ window_event(KeyboardInput) ─→ Process key → Send to shell
  ├─→ window_event(MouseInput) ─→ Process mouse → Send to shell
  ├─→ window_event(Resized) ─→ Recalculate grid → Resize PTY
  ├─→ window_event(RedrawRequested) ─→ Call renderer
  └─→ window_event(CloseRequested) ─→ Exit
```

### The resumed() Method

**Called once** when the event loop starts:

```rust
fn resumed(&mut self, event_loop: &ActiveEventLoop) {
    // Create window
    let window = event_loop.create_window(...)?;

    // Initialize renderer (CPU or GPU)
    let renderer = CpuRenderer::new(...)?;
    // or
    let renderer = pollster::block_on(GpuRenderer::new(...))?;

    // Start shell process
    let (cols, rows) = calculate_grid_size(...);
    self.session.resize(cols, rows);
}
```

This is where everything gets initialized.

### The about_to_wait() Method

**Called** right before the event loop goes to sleep:

```rust
fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
    // Check for shell output (non-blocking)
    if !self.process_shell_output() {
        // Shell died, exit
        event_loop.exit();
        return;
    }

    // Handle cursor blink
    if self.session.state().cursor_blink {
        if self.last_blink_toggle.elapsed() >= Duration::from_millis(530) {
            self.cursor_visible_phase = !self.cursor_visible_phase;
            window.request_redraw();
        }
    }

    // Wake up in 16ms (for cursor blink checks)
    event_loop.set_control_flow(ControlFlow::WaitUntil(
        Instant::now() + Duration::from_millis(16)
    ));
}
```

**Why 16ms?** That's ~60 FPS. We need to check the cursor blink timer regularly.

**Why non-blocking?** We use `try_recv()` instead of `recv()`:

```rust
match shell.receiver.try_recv() {
    Ok(data) => {
        terminal.process_bytes(&data);
        window.request_redraw();
    }
    Err(TryRecvError::Empty) => {
        // No data, that's fine
    }
    Err(TryRecvError::Disconnected) => {
        // Shell died
        return false;
    }
}
```

If there's no data, we don't block. We just continue.

### The window_event() Method

**Called** for every window event:

```rust
fn window_event(&mut self, event_loop: &ActiveEventLoop,
                window_id: WindowId, event: WindowEvent) {
    match event {
        WindowEvent::RedrawRequested => {
            self.render()?;
        }
        WindowEvent::KeyboardInput { event, .. } => {
            self.handle_keyboard_input(&event.logical_key, event.text);
        }
        WindowEvent::Resized(new_size) => {
            let (cols, rows) = self.calculate_grid_size(...);
            self.session.resize(cols, rows);
        }
        WindowEvent::CloseRequested => {
            event_loop.exit();
        }
        // ... more events
    }
}
```

### The Full Flow

Let's trace what happens when you type 'A':

1. **Key pressed** → OS sends event
2. **window_event(KeyboardInput)** called
3. **handle_keyboard_input('A')** called
4. **shell.write(b"A")** writes to PTY master
5. **Shell receives 'A'** via PTY slave
6. **Shell echoes 'A' back** (writes to PTY slave)
7. **PTY reader thread** (blocking read) receives 'A'
8. **Channel.send('A')** sends to main thread
9. **about_to_wait()** calls `try_recv()`, gets 'A'
10. **terminal.process_bytes('A')** updates grid
11. **window.request_redraw()** triggers redraw
12. **window_event(RedrawRequested)** called
13. **renderer.render()** draws 'A' on screen

All of this happens in **less than 5 milliseconds**!

### Performance Characteristics

**Idle CPU usage:** < 0.1%
- Event loop sleeps when there are no events
- PTY reader thread blocks on read (no polling)
- No busy waiting anywhere

**Wake frequency:** Every 16ms
- To check cursor blink timer
- Can be optimized to only wake when cursor is blinking

**Input latency:** < 5ms
- Events processed immediately
- No input buffering or debouncing

---

## Coordinate Systems Explained

There are three coordinate systems in play. Let's understand each one.

### 1. Window Coordinates (Pixels)

This is what you're used to:

```
(0, 0) ─────────────────────────→ x
  │                            (800, 0)
  │
  │        Window
  │      800×600 pixels
  │
  ↓ y                       (800, 600)
```

- **Origin:** Top-left corner
- **Units:** Pixels
- **X:** 0 (left) to window_width (right)
- **Y:** 0 (top) to window_height (bottom)

**Used for:** Mouse events, window resize

### 2. Grid Coordinates (Cells)

This is how the terminal thinks:

```
(0, 0) ──────────────────────────→ col
  │                            (85, 0)
  │
  │       Terminal Grid
  │        86×28 cells
  │
  ↓ row                        (85, 27)
```

- **Origin:** Top-left cell
- **Units:** Character cells
- **Col:** 0 to cols-1
- **Row:** 0 to rows-1

**Used for:** Cursor position, terminal state

### 3. NDC (Normalized Device Coordinates) - GPU Only

This is what the GPU uses:

```
       (-1, 1) ─────────────────────────→ x (1, 1)
          │                                  │
          │                                  │
          │          (0, 0)                  │
          │            Center                │
          │                                  │
          ↓ y                                ↓
    (-1, -1)                             (1, -1)
```

- **Origin:** Center
- **Units:** -1.0 to +1.0
- **X:** -1.0 (left) to +1.0 (right)
- **Y:** -1.0 (bottom) to +1.0 (top)

**Used for:** GPU vertex positions

### Converting Between Them

**Window pixels → Grid cells:**

```rust
fn window_to_grid_coords(&self, x: f64, y: f64) -> Option<(usize, usize)> {
    // Subtract offsets
    let adjusted_x = x - 10.0;
    let adjusted_y = y - 20.0;

    // Divide by cell size
    let col = (adjusted_x / 9.0).floor() as usize;
    let row = (adjusted_y / 20.0).floor() as usize;

    // Check bounds
    if col >= cols || row >= rows {
        return None;
    }

    Some((col, row))
}
```

**Window pixels → NDC (GPU):**

```rust
fn pixels_to_ndc(x: f32, y: f32, width: f32, height: f32) -> (f32, f32) {
    let x_ndc = (x / width) * 2.0 - 1.0;
    let y_ndc = 1.0 - (y / height) * 2.0;
    (x_ndc, y_ndc)
}
```

**Example:**
- Window: 400, 300 (middle of 800×600)
- NDC: 0.0, 0.0 (center)

**Grid cells → Window pixels:**

```rust
let x = 10.0 + col as f32 * 9.0;
let y = 20.0 + row as f32 * 20.0;
```

---

## Quick Reference

### Grid Size Formula

```rust
cols = ((window_width - 20) / char_width).floor()
rows = ((window_height - 40) / char_height).floor()
```

### Default Sizes

- Window: 800×600 pixels
- Character cell: 9×20 pixels
- Offsets: 10px horizontal, 20px vertical
- Typical grid: 86×28 cells
- Minimum grid: 10×3 cells

### Key Sequences

| Key | Sequence |
|-----|----------|
| ↑ | `\x1b[A` |
| ↓ | `\x1b[B` |
| → | `\x1b[C` |
| ← | `\x1b[D` |
| Enter | `\r` |
| Backspace | `\x7f` |
| Ctrl+C | `\x03` |

### Event Loop Methods

- `resumed()` - Initialize (once)
- `about_to_wait()` - Check shell output (every loop)
- `window_event()` - Handle events (on event)

### Performance

- CPU renderer: 1-2ms per frame
- GPU renderer: <1ms per frame
- Idle CPU: <0.1%
- Input latency: <5ms

---

**Want to dive deeper?**
- See **[Terminal Fundamentals](terminal-fundamentals.md)** for PTY and shell process details
- See **[ANSI Implementation](ansi-implementation.md)** for escape sequence handling
- Check out the code in `src/renderer/` and `src/bin/ui/`
