// SPDX-License-Identifier: MIT OR Apache-2.0
//! Real-time safety: the event hot path must perform zero heap allocations.
//!
//! A counting global allocator tallies allocations per thread. Everything is set up
//! (queue, backend script) *before* measuring; only `poll_events`, `step`, `push`,
//! `pop`/`drain` run inside the measured region.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;
use std::time::Duration;

use tpt_window_core::*;

struct Counting;

thread_local! {
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn bump() {
    // `try_with` so allocations during thread teardown don't panic.
    let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
}

// SAFETY: forwards every call to the system allocator unchanged.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        bump();
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        bump();
        unsafe { System.realloc(p, l, n) }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        bump();
        unsafe { System.alloc_zeroed(l) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn allocs() -> usize {
    ALLOCS.with(Cell::get)
}

/// Backend that replays a pre-built script; polling never allocates.
struct Scripted {
    script: Vec<Event>,
    next: usize,
}

impl EventLoopBackend for Scripted {
    fn poll(&mut self, sink: &mut EventSink<'_>) -> PollStatus {
        let mut any = false;
        while self.next < self.script.len() {
            match sink.push(self.script[self.next]) {
                EventStatus::Accepted => {
                    self.next += 1;
                    any = true;
                }
                _ => break,
            }
        }
        if any {
            PollStatus::Events
        } else {
            PollStatus::Idle
        }
    }
    fn wait(&mut self, sink: &mut EventSink<'_>, _timeout: Duration) -> PollStatus {
        self.poll(sink)
    }
    fn request_exit(&self) {}
    fn create_window(&self, _: &WindowAttributes) -> Result<Arc<dyn WindowBackend>, WindowError> {
        Err(WindowError::Unsupported("scripted backend"))
    }
}

fn sample_events(n: usize) -> Vec<Event> {
    (0..n)
        .map(|i| Event {
            window: WindowId(1),
            event: match i % 4 {
                0 => WindowEvent::CursorMoved {
                    position: PhysicalPosition::new(i as f64, 2.0),
                    pressure: 0.0,
                },
                1 => WindowEvent::KeyboardInput {
                    state: ElementState::Pressed,
                    key_code: KeyCode::A,
                    physical_key: PhysicalKey::Code(KeyCode::A),
                },
                2 => WindowEvent::Ime(ImeEvent::Commit {
                    text: ImeText::new("héllo wörld"),
                    continued: false,
                }),
                _ => WindowEvent::Resized(PhysicalSize::new(640, 480)),
            },
        })
        .collect()
}

#[test]
fn hot_path_allocates_nothing() {
    const FRAMES: usize = 1000;
    const PER_FRAME: usize = 8;

    // --- setup (allocation allowed) ---
    let mut queue = EventQueue::with_capacity(64);
    let mut el = EventLoop::with_backend(Box::new(Scripted {
        script: sample_events(FRAMES * PER_FRAME),
        next: 0,
    }));
    // Warm up any lazily initialised thread-locals / OnceLocks.
    {
        let mut sink = EventSink::new(&mut queue);
        el.poll_events(&mut sink);
        queue.clear();
    }

    // --- measured region ---
    let before = allocs();
    let mut handled = 0usize;
    for _ in 0..FRAMES {
        let mut sink = EventSink::new(&mut queue);
        el.poll_events(&mut sink);
        handled += queue.drain().count();
    }
    // Also exercise `step` and direct `push` on the sink.
    let mut sink = EventSink::new(&mut queue);
    el.step(&mut sink, Duration::ZERO);
    let _ = sink.push(Event {
        window: WindowId(1),
        event: WindowEvent::RedrawRequested,
    });
    queue.clear();
    let after = allocs();

    assert!(handled > 0, "test processed no events");
    assert_eq!(after - before, 0, "hot path performed heap allocations");
}

#[test]
fn counter_actually_counts() {
    // Guards against a vacuous pass: a Vec allocation must be seen.
    let before = allocs();
    let v = std::hint::black_box(vec![0u8; 128]);
    assert!(allocs() > before);
    drop(v);
}

#[test]
fn frame_ring_and_encoder_allocate_nothing_in_steady_state() {
    use tpt_window_core::channel::{FrameRing, RingMemory};
    use tpt_window_core::protocol::{Message, encode};

    /// Plain memory, so the ring itself is what is measured.
    struct Mem(Vec<u8>);
    impl RingMemory for Mem {
        fn len(&self) -> usize {
            self.0.len()
        }
        fn read(&self, offset: usize, out: &mut [u8]) {
            out.copy_from_slice(&self.0[offset..offset + out.len()]);
        }
        fn write(&mut self, offset: usize, data: &[u8]) {
            self.0[offset..offset + data.len()].copy_from_slice(data);
        }
        fn load_u32(&self, offset: usize) -> u32 {
            let mut b = [0u8; 4];
            b.copy_from_slice(&self.0[offset..offset + 4]);
            u32::from_le_bytes(b)
        }
        fn store_u32(&mut self, offset: usize, value: u32) {
            self.0[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
    }

    let msg = Message::Hello { version: 1 };
    // --- setup (allocation allowed) ---
    let mut ring = FrameRing::new(Mem(vec![0; 16 + 8192])).unwrap();
    ring.reset();
    let mut out = Vec::with_capacity(64);
    // Warm up: the first push/pop sizes the reusable buffers.
    ring.push(&msg).unwrap();
    assert!(ring.pop().unwrap().is_some());
    out.clear();
    encode(&msg, &mut out);

    // --- measured region ---
    let before = allocs();
    for _ in 0..1000 {
        ring.push(&msg).unwrap();
        assert!(ring.pop().unwrap().is_some());
        out.clear();
        encode(&msg, &mut out);
    }
    let after = allocs();
    assert_eq!(after - before, 0, "frame ring or encoder allocated");
}
