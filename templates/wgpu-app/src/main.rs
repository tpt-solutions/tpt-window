use tpt_window::prelude::*;
use tpt_window_wgpu::create;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop = new_event_loop()?;
    let window = WindowBuilder::new()
        .with_title("{{project-name}}")
        .build(&event_loop)?;
    let (gpu, mut surface) = create(&window)?;

    let mut queue = EventQueue::with_capacity(DEFAULT_CAPACITY);
    'frames: loop {
        // The present waits for the display, so the loop does not wait here.
        event_loop.poll_events(&mut EventSink::new(&mut queue));
        for ev in queue.drain() {
            match ev.event {
                WindowEvent::CloseRequested => break 'frames,
                WindowEvent::Resized(size) => surface.resize(&gpu.device, size),
                _ => {}
            }
        }
        let Some(frame) = surface.acquire(&gpu.device)? else {
            continue;
        };
        let view = frame
            .texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            // A render pass that only clears: draw your scene here.
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        gpu.queue.submit(Some(encoder.finish()));
        frame.present(&gpu.queue);
    }
    Ok(())
}
