# Migrating from winit (0.30) to tpt-window

This guide maps the winit concepts you use onto tpt-window, and lists what tpt-window does
not provide yet. Check the gaps section before you start.

## The main change: you own the loop

winit 0.30 calls your code. You implement `ApplicationHandler` and hand control to
`EventLoop::run_app`. tpt-window is the other way round: you call it once per frame and
drain the events yourself.

```rust
// winit 0.30
struct App { window: Option<Window> }
impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) { /* create window */ }
    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, ev: WindowEvent) { /* ... */ }
    fn about_to_wait(&mut self, el: &ActiveEventLoop) { /* render */ }
}
EventLoop::new()?.run_app(&mut App { window: None })?;
```

```rust
// tpt-window
let mut event_loop = tpt_window::new_event_loop()?;
let window = WindowBuilder::new().with_title("app").build(&event_loop)?;
let mut queue = EventQueue::with_capacity(DEFAULT_CAPACITY);

'main: loop {
    let status = event_loop.step(&mut EventSink::new(&mut queue), Duration::from_millis(16));
    for ev in queue.drain() {
        match ev.event {
            WindowEvent::CloseRequested => break 'main,
            _ => { /* update state */ }
        }
    }
    // render here, once per frame
    if status == PollStatus::Exit { break; }
}
```

Things that change because of this:

- There is no `resumed` callback. Create windows before the loop, or at any point in your
  own code, with `WindowBuilder::build(&event_loop)`.
- Windows are `Arc` handles. Dropping the last handle closes the window. You do not need
  to keep the window alive in your app state unless you want it open.
- `event_loop.request_exit()` stops the loop from any thread. The next `step` returns
  `PollStatus::Exit`.
- `YieldPoint` is emitted once per frame, after the OS events are drained. Rendering and AI
  agents can run there without changing display timing. winit has no equivalent.

## Event mapping

| winit 0.30 `WindowEvent`                    | tpt-window `WindowEvent`                              | Notes |
|---------------------------------------------|-------------------------------------------------------|-------|
| `CloseRequested`                            | `CloseRequested`                                      | The app decides whether to close. Drop the window to close it. |
| `Destroyed`                                 | `Destroyed`                                           | |
| `Resized(PhysicalSize)`                     | `Resized(PhysicalSize<u32>)`                          | |
| `Moved(PhysicalPosition)`                   | `Moved(PhysicalPosition<i32>)`                        | |
| `ScaleFactorChanged { scale_factor, .. }`   | `ScaleFactorChanged { scale_factor, new_inner_size }` | No `InnerSizeWriter`. Set the size yourself with `set_size`. |
| `Focused(bool)`                             | `Focused(bool)`                                       | |
| `Occluded(bool)`                            | `Occluded(bool)`                                      | |
| `KeyboardInput { event, .. }`               | `KeyboardInput { state, key_code, physical_key }`     | See "Keys and text" below. |
| `Ime(Ime::Commit / Preedit / Enabled / Disabled)` | `Ime(ImeEvent::Commit / Preedit / Enabled / Disabled)` | Text is in chunks of up to 32 bytes. See "Keys and text". |
| `CursorMoved { position, .. }`              | `CursorMoved { position, pressure }`                  | `pressure` is 0.0 for a mouse. |
| `CursorEntered` / `CursorLeft`              | `CursorEntered` / `CursorLeft`                        | macOS: not run yet (see gaps). |
| `MouseInput { state, button, .. }`          | `MouseInput { state, button }`                        | |
| `MouseWheel { delta, phase, .. }`           | `MouseWheel { delta, phase }`                         | |
| `Touch(Touch)`                              | `Touch(TouchEvent)`                                   | `force` is `f32`. Touch lift is not verified on Windows. |
| `RedrawRequested`                           | `RedrawRequested`                                     | Call `window.request_redraw()` to get one. |
| `ModifiersChanged`                          | **none**                                              | Track modifier keys yourself from `KeyboardInput` (see below). |
| `DroppedFile` / `HoveredFile` / `HoveredFileCancelled` | **none**                                  | Not provided. |
| `ActivationTokenDone`, `ThemeChanged`, `TouchpadMagnify`, `SmartMagnify`, `TouchpadRotate`, `TouchpadPressure`, `AxisMotion` | **none** | Not provided. |
| (device events) `DeviceEvent::MouseMotion`  | `RawMouseMotion { dx, dy }`                           | Only the window that registered for raw input gets it (Windows). |
| (no winit equivalent)                       | `YieldPoint { frame_number, timestamp_ns }`           | Once per frame. |
| (no winit equivalent)                       | `GamepadConnected` / `GamepadDisconnected`            | Gamepad input is in the separate `tpt-window-gamepad` crate. |

## Keys and text

winit gives one `KeyEvent` with `logical_key`, `text` and `repeat`. tpt-window splits this:

- `KeyboardInput` reports the key press or release. `key_code` is the layout-translated key
  and `physical_key` is the position on the keyboard. Shortcuts that should follow the
  keyboard position (for example WASD) use `physical_key`.
- Text comes from `Ime(ImeEvent::Commit)`, not from the key event. A typed character
  therefore arrives after its key press. Commits longer than 32 bytes arrive in chunks with
  `continued: true` on every chunk except the last.
- Key repeat has no `repeat` flag. A held key sends repeated `Pressed` events, and text is
  committed for each repeat. Win32, X11 and Wayland do this. macOS is not verified. The
  event carries no flag to tell repeats apart, so if you need that, track the key state
  yourself.
- Modifier state is not an event. Keep a `bool` for each modifier and update it on
  `KeyboardInput` for `ShiftLeft`/`ShiftRight`, `ControlLeft`/`ControlRight`, and so on.

## Windows and their properties

| winit 0.30                          | tpt-window                                          |
|-------------------------------------|-----------------------------------------------------|
| `WindowAttributes::default().with_title(..)` | `WindowBuilder::new().with_title(..)`      |
| `with_inner_size(LogicalSize)`      | `with_inner_size(LogicalSize<f64>)`                 |
| `with_min_inner_size` / `with_max_inner_size` | `with_min_size` / `with_max_size`         |
| `with_position`                     | `with_position(LogicalPosition<f64>)`               |
| `with_resizable`, `with_decorations`, `with_visible`, `with_transparent` | same names (see gaps for transparency) |
| `window.inner_size()`               | `window.size()` (cached, no system call)            |
| `window.scale_factor()`             | `window.scale_factor()`                             |
| `window.request_redraw()`           | `window.request_redraw()`                           |
| `window.set_title(..)`              | `window.set_title(..)` (queued, applied on next step) |
| `window.set_visible(..)`            | `window.set_visible(..)`                            |
| `window.set_outer_position(..)`     | `window.set_position(..)`                           |
| `window.set_min_inner_size` etc.    | Set at build time only.                             |
| `window.has_focus()`                | `window.is_focused()`                               |
| `window.set_ime_cursor_area(pos, size)` | `window.set_ime_cursor_area(pos, size)` (Win32 only, see gaps) |
| `window.window_handle()`            | `window.raw_window_handle()` (returns `Result`)      |
| `window.display_handle()`           | `window.raw_display_handle()` (returns `Result`)     |

Setters and getters are cheap. Getters read a cached snapshot and setters queue a command,
so neither makes a system call in the render loop. Setters take effect on the next `step`.

## Rendering

tpt-window does not render. It gives you the raw handles for `wgpu` or any other API:

```rust
let handle = window.raw_window_handle()?;
let display = window.raw_display_handle()?;
let surface = unsafe { instance.create_surface_unsafe(SurfaceTargetUnsafe::RawHandle {
    raw_display_handle: display, raw_window_handle: handle,
})? };
```

Reconfigure the surface on `Resized` and `ScaleFactorChanged`, and render from your loop
rather than from `RedrawRequested` if you want the loop to stay in control. The
`tpt-visual` crate does this (see its `simple_player` example).

## Threads

Event handling runs on the thread that created the event loop. A `Window` handle can be
used from other threads: setters queue commands for the next `step`. On macOS and Wayland,
keep the event loop on the main thread.

## Gaps (not yet provided)

Check these before you migrate. Each one is tracked in `todo.md`.

- **IME candidate window placement** (`set_ime_cursor_area`) works on Win32 only. Other
  platforms accept the call and ignore it, so candidate lists may open in the wrong place.
- **IME on Cocoa, Wayland and X11** is not started. Win32 is written but not yet verified
  with a Japanese or Chinese input method.
- **Cursor**: no visibility, grab, icon or custom cursor.
- **Fullscreen, icons, window levels and themes** are not provided.
- **Transparency** (`with_transparent`) needs a composition swapchain in the renderer. Setting
  the flag alone gives a white window on Windows.
- **Clipboard** is deferred to tpt-archon.
- **macOS**: the close button is vetoed by a window delegate, and cursor enter/leave comes
  from a tracking area. Both compile for macOS but have not been run on a Mac.
- **Wayland**: positions and sizes are chosen by the compositor, so `set_size`,
  `set_position` and `set_visible` have no effect. Fractional scale factors are not
  supported (integer scale only).
- **Gamepads** are in `tpt-window-gamepad`, not in the window event stream.
- **Ring buffer is caller-owned, drop-newest.** A full queue returns `BufferFull` and counts
  the drop. winit has no queue to fill; if you relied on it never losing events, size the
  queue for your frame rate and check `queue.dropped()`.
