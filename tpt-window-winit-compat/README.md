# tpt-window-winit-compat

A winit 0.30 style `ApplicationHandler` over tpt-window. If your application is written against
`winit::application::ApplicationHandler` and `EventLoop::run_app`, this crate lets you keep that
shape: implement the trait, create windows in `resumed`, and call `run_app`.

```rust,no_run
use tpt_window_winit_compat::{ActiveLoop, ApplicationHandler, run_app};
use tpt_window::{Window, WindowBuilder, WindowEvent, WindowId};

#[derive(Default)]
struct App { window: Option<Window> }

impl ApplicationHandler for App {
    fn resumed(&mut self, ctx: &mut ActiveLoop<'_>) {
        self.window = Some(ctx.create_window(WindowBuilder::new().with_title("app")).unwrap());
    }
    fn window_event(&mut self, ctx: &mut ActiveLoop<'_>, _id: WindowId, ev: WindowEvent) {
        if ev == WindowEvent::CloseRequested { ctx.exit(); }
    }
    fn about_to_wait(&mut self, _ctx: &mut ActiveLoop<'_>) { /* render */ }
}

run_app(&mut App::default()).unwrap();
```

## Callback order

1. `resumed`, once, before the first event.
2. `window_event`, once per event, in arrival order.
3. `about_to_wait`, after each batch of events, and on idle ticks at most every `IDLE_WAIT`
   (16 ms).
4. `exiting`, once, when the loop stops.

## Differences from winit

- **Windows are created with `ctx.create_window`** and returned as `tpt_window::Window`. Dropping
  the handle closes the window, as in tpt-window everywhere.
- **Control flow.** `ControlFlow` (`Wait`, `Poll`, `WaitUntil`) is set with
  `ctx.set_control_flow`, as in winit. With `Wait`, the loop still wakes at least every
  `IDLE_WAIT` (16 ms) so `about_to_wait` keeps running.
- **No user events.** Use `ctx.proxy()` to wake the loop or stop it from another thread. The
  proxy wakes the loop at once on Win32 and at the next idle tick elsewhere.
- **Types are tpt-window's.** `WindowEvent`, `WindowId` and `WindowBuilder` come from
  `tpt-window`, so some variants and fields differ from winit's. See [MIGRATION.md](../MIGRATION.md)
  for the event mapping.
- **Yield points are skipped.** tpt-window's per-frame `YieldPoint` events are not passed to
  the handler.

## Changes

See [CHANGELOG.md](CHANGELOG.md).

## Testing

```sh
cargo test -p tpt-window-winit-compat
```

The run test needs a display. It runs on Windows only for now.

## License

Licensed under either of Apache License, Version 2.0 or MIT license, at your option.
