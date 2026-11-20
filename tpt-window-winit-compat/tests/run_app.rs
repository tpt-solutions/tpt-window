// SPDX-License-Identifier: MIT OR Apache-2.0
//! `run_app` calls the handler in the documented order and stops when asked.
#![cfg(windows)]

use tpt_window::Window;
use tpt_window_winit_compat::{
    ActiveLoop, ApplicationHandler, WindowBuilder, WindowEvent, WindowId, run_app,
};

#[derive(Default)]
struct App {
    resumed: u32,
    exiting: u32,
    about_to_wait: u32,
    window: Option<Window>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, ctx: &mut ActiveLoop<'_>) {
        self.resumed += 1;
        self.window = Some(
            ctx.create_window(WindowBuilder::new().with_title("winit-compat test"))
                .expect("window"),
        );
    }

    fn window_event(&mut self, _ctx: &mut ActiveLoop<'_>, _id: WindowId, _ev: WindowEvent) {}

    fn about_to_wait(&mut self, ctx: &mut ActiveLoop<'_>) {
        self.about_to_wait += 1;
        if self.about_to_wait >= 3 {
            ctx.exit();
        }
    }

    fn exiting(&mut self) {
        self.exiting += 1;
    }
}

#[test]
fn callbacks_run_in_order_and_exit_stops_the_loop() {
    let mut app = App::default();
    run_app(&mut app).expect("run_app");
    assert_eq!(app.resumed, 1, "resumed runs once");
    assert!(
        app.about_to_wait >= 3,
        "about_to_wait ran {} times",
        app.about_to_wait
    );
    assert_eq!(app.exiting, 1, "exiting runs once");
    assert!(app.window.is_some());
}
