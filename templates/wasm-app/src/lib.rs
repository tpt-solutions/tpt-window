//! A tpt-window page. The browser cannot block, so the loop is driven from
//! `requestAnimationFrame`: each frame polls the events, then schedules the next frame.

use std::cell::RefCell;
use std::rc::Rc;

use tpt_window::EventLoop;
use tpt_window::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

struct App {
    event_loop: EventLoop,
    queue: EventQueue,
    _window: Window,
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let event_loop = new_event_loop().map_err(|e| JsValue::from_str(&e.to_string()))?;
    let window = WindowBuilder::new()
        .with_title("{{project-name}}")
        .build(&event_loop)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let app = Rc::new(RefCell::new(App {
        event_loop,
        queue: EventQueue::with_capacity(DEFAULT_CAPACITY),
        _window: window,
    }));
    frame(app);
    Ok(())
}

/// Runs one frame, then schedules the next unless the window asked to close.
fn frame(app: Rc<RefCell<App>>) {
    let next = app.clone();
    let callback = Closure::once_into_js(move || {
        let keep_going = {
            let mut app = next.borrow_mut();
            let App {
                event_loop, queue, ..
            } = &mut *app;
            event_loop.poll_events(&mut EventSink::new(queue));
            let closing = queue
                .drain()
                .any(|ev| ev.event == WindowEvent::CloseRequested);
            // Rendering would happen here.
            !closing
        };
        if keep_going {
            frame(next);
        }
    });
    if let Some(window) = web_sys::window() {
        let _ = window.request_animation_frame(callback.unchecked_ref());
    }
}
