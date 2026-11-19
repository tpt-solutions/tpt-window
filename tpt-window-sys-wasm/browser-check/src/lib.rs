//! Browser check harness for the tpt-window web backend. Not part of the repo.
use std::cell::RefCell;
use std::collections::BTreeMap;

use tpt_window::*;
use wasm_bindgen::prelude::*;

struct App {
    el: EventLoop,
    queue: EventQueue,
    _window: Window,
    counts: BTreeMap<String, u32>,
    last_size: Option<PhysicalSize<u32>>,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

#[wasm_bindgen]
pub fn start() -> Result<(), JsValue> {
    let el = new_event_loop().map_err(|e| JsValue::from_str(&e.to_string()))?;
    let window = WindowBuilder::new()
        .with_title("wasm check")
        .with_inner_size(LogicalSize::new(400.0, 300.0))
        .build(&el)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let queue = EventQueue::with_capacity(DEFAULT_CAPACITY);
    APP.with(|a| {
        *a.borrow_mut() = Some(App {
            el,
            queue,
            _window: window,
            counts: BTreeMap::new(),
            last_size: None,
        })
    });
    Ok(())
}

/// Drains the loop once and returns cumulative counts as JSON.
#[wasm_bindgen]
pub fn tick() -> String {
    APP.with(|a| {
        let mut a = a.borrow_mut();
        let Some(app) = a.as_mut() else {
            return "{}".to_string();
        };
        app.el.poll_events(&mut EventSink::new(&mut app.queue));
        let events: Vec<Event> = app.queue.drain().collect();
        for ev in events {
            let name = format!("{:?}", ev.event);
            let name = name
                .split(['(', ' ', '{'])
                .next()
                .unwrap_or("?")
                .to_string();
            if let WindowEvent::Resized(s) = ev.event {
                app.last_size = Some(s);
            }
            *app.counts.entry(name).or_default() += 1;
        }
        let mut parts: Vec<String> = app
            .counts
            .iter()
            .map(|(k, v)| format!("\"{k}\":{v}"))
            .collect();
        if let Some(s) = app.last_size {
            parts.push(format!("\"size\":[{},{}]", s.width, s.height));
        }
        format!("{{{}}}", parts.join(","))
    })
}
