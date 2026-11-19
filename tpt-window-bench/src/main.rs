// SPDX-License-Identifier: MIT OR Apache-2.0
//! tpt-window against winit 0.30 on the same work: a caller-owned loop that posts
//! `WM_MOUSEMOVE` to its own window and counts the cursor-move events it gets back.
//!
//! Measures two things:
//! - throughput: events per second when posting in batches of 400,
//! - latency: time from posting one message until the loop sees its event (p50, p99).
//!
//! Run one library per process, since both want the thread's message queue:
//!
//! ```text
//! cargo run --release -p tpt-window-bench -- tpt
//! cargo run --release -p tpt-window-bench -- winit
//! ```
//!
//! Windows only. Windows are created hidden. Both sides use a zero-timeout pump, so the
//! loop spins instead of blocking.

#[cfg(not(windows))]
fn main() {
    eprintln!("tpt-window-bench runs on Windows only (it posts Win32 messages).");
}

#[cfg(windows)]
fn main() {
    let which = std::env::args().nth(1).unwrap_or_default();
    match which.as_str() {
        "tpt" => run(&mut win::Tpt::new()),
        "winit" => run(&mut win::Winit::new()),
        _ => eprintln!("usage: tpt-window-bench <tpt|winit>"),
    }
}

#[cfg(windows)]
use std::time::{Duration, Instant};

/// One library's window and loop, driven by this benchmark.
#[cfg(windows)]
trait Harness {
    fn name(&self) -> &'static str;
    /// Posts a `WM_MOUSEMOVE` to the window at `(x, y)`.
    fn post_move(&mut self, x: i32, y: i32);
    /// Pumps once without blocking and returns how many cursor-move events arrived.
    fn pump(&mut self) -> usize;
}

/// Pumps until `n` events have arrived. Panics if none arrive for 5 seconds.
#[cfg(windows)]
fn collect(h: &mut dyn Harness, n: usize) {
    let mut got = 0;
    let mut stalled_since = Instant::now();
    while got < n {
        let k = h.pump();
        if k > 0 {
            got += k;
            stalled_since = Instant::now();
        } else if stalled_since.elapsed() > Duration::from_secs(5) {
            panic!("{}: {} of {} events lost", h.name(), n - got, n);
        }
    }
}

#[cfg(windows)]
fn run(h: &mut dyn Harness) {
    // Warm-up: first events pay for lazy setup on either side.
    for i in 0..2_000 {
        h.post_move(i % 500, 1);
    }
    collect(h, 2_000);

    // Throughput: 100 000 events in batches of 400, so the message queue never overflows.
    const BATCH: usize = 400;
    const BATCHES: usize = 250;
    let start = Instant::now();
    for b in 0..BATCHES {
        for i in 0..BATCH {
            h.post_move((i % 800) as i32, (b % 600) as i32);
        }
        collect(h, BATCH);
    }
    let elapsed = start.elapsed();
    let total = (BATCH * BATCHES) as f64;
    let per_sec = total / elapsed.as_secs_f64();

    // Latency: one message at a time.
    const SAMPLES: usize = 20_000;
    let mut samples = Vec::with_capacity(SAMPLES);
    for i in 0..SAMPLES {
        let t = Instant::now();
        h.post_move((i % 800) as i32, 3);
        collect(h, 1);
        samples.push(t.elapsed());
    }
    samples.sort_unstable();
    let p50 = samples[SAMPLES / 2];
    let p99 = samples[SAMPLES * 99 / 100];

    println!(
        "{}: throughput {:.0} events/s ({:.1} ns/event); latency p50 {:.1} us, p99 {:.1} us ({} samples)",
        h.name(),
        per_sec,
        elapsed.as_nanos() as f64 / total,
        p50.as_secs_f64() * 1e6,
        p99.as_secs_f64() * 1e6,
        SAMPLES,
    );
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;
    use std::time::Duration;

    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use tpt_window::{
        DEFAULT_CAPACITY, EventLoop, EventQueue, EventSink, Window, WindowBuilder, WindowEvent,
    };
    use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_MOUSEMOVE};

    use super::Harness;

    fn post(hwnd: HWND, x: i32, y: i32) {
        let lparam = ((y as u32 & 0xFFFF) << 16 | (x as u32 & 0xFFFF)) as isize;
        // SAFETY: `hwnd` is a live window owned by the harness; posting is thread-safe.
        unsafe { PostMessageW(hwnd, WM_MOUSEMOVE, 0 as WPARAM, lparam as LPARAM) };
    }

    fn hwnd_of(window: &impl HasWindowHandle) -> HWND {
        match window.window_handle().unwrap().as_raw() {
            RawWindowHandle::Win32(h) => (h.hwnd.get() as usize) as *mut c_void as HWND,
            other => panic!("expected a Win32 handle, got {other:?}"),
        }
    }

    pub struct Tpt {
        el: EventLoop,
        queue: EventQueue,
        _window: Window,
        hwnd: HWND,
    }

    impl Tpt {
        pub fn new() -> Self {
            let el = tpt_window::new_event_loop().expect("event loop");
            let window = WindowBuilder::new()
                .with_title("bench tpt")
                .with_visible(false)
                .build(&el)
                .expect("window");
            let hwnd = {
                let raw = window.raw_window_handle().expect("handle");
                match raw {
                    RawWindowHandle::Win32(h) => (h.hwnd.get() as usize) as *mut c_void as HWND,
                    other => panic!("expected Win32, got {other:?}"),
                }
            };
            let queue = EventQueue::with_capacity(DEFAULT_CAPACITY);
            Self {
                el,
                queue,
                _window: window,
                hwnd,
            }
        }
    }

    impl Harness for Tpt {
        fn name(&self) -> &'static str {
            "tpt-window"
        }

        fn post_move(&mut self, x: i32, y: i32) {
            post(self.hwnd, x, y);
        }

        fn pump(&mut self) -> usize {
            self.el.poll_events(&mut EventSink::new(&mut self.queue));
            self.queue
                .drain()
                .filter(|e| matches!(e.event, WindowEvent::CursorMoved { .. }))
                .count()
        }
    }

    // ---- winit 0.30 -------------------------------------------------------

    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent as WinitEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoop as WinitLoop};
    use winit::platform::pump_events::EventLoopExtPumpEvents;
    use winit::window::{Window as WinitWindow, WindowAttributes, WindowId};

    #[derive(Default)]
    struct App {
        window: Option<WinitWindow>,
        moves: usize,
    }

    impl ApplicationHandler for App {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.window.is_none() {
                let attrs = WindowAttributes::default()
                    .with_title("bench winit")
                    .with_visible(false);
                self.window = Some(event_loop.create_window(attrs).expect("window"));
            }
        }

        fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, event: WinitEvent) {
            if let WinitEvent::CursorMoved { .. } = event {
                self.moves += 1;
            }
        }
    }

    pub struct Winit {
        el: WinitLoop<()>,
        app: App,
        hwnd: HWND,
    }

    impl Winit {
        pub fn new() -> Self {
            let mut el = WinitLoop::new().expect("event loop");
            let mut app = App::default();
            // The first pump starts the loop and calls `resumed`, which creates the window.
            el.pump_app_events(Some(Duration::ZERO), &mut app);
            let hwnd = hwnd_of(app.window.as_ref().expect("window created in resumed"));
            Self { el, app, hwnd }
        }
    }

    impl Harness for Winit {
        fn name(&self) -> &'static str {
            "winit 0.30"
        }

        fn post_move(&mut self, x: i32, y: i32) {
            post(self.hwnd, x, y);
        }

        fn pump(&mut self) -> usize {
            self.el.pump_app_events(Some(Duration::ZERO), &mut self.app);
            std::mem::take(&mut self.app.moves)
        }
    }
}
