# tpt-window-wgpu

wgpu helpers for [tpt-window](../tpt-window) windows: surface creation from a window, resize
reconfiguration, and the acquire-or-skip logic a render loop needs.

```rust,no_run
let (gpu, mut surface) = tpt_window_wgpu::create(&window)?;
// on Resized: surface.resize(&gpu.device, size);
if let Some(frame) = surface.acquire(&gpu.device)? {
    // draw into frame.texture, submit, then:
    frame.present(&gpu.queue);
}
```

The loop must not wait in `step`. With the default `Fifo` present mode, `present` waits for the
display, which paces the loop.

## Example

```sh
cargo run -p tpt-window-wgpu --example wgpu_triangle
```

The example was run for three seconds on a GPU (NVIDIA, Windows) without errors. Its picture was
not checked by eye.

## Changes

See [CHANGELOG.md](CHANGELOG.md).

## License

Licensed under either of Apache License, Version 2.0 or MIT license, at your option.
