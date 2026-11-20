// SPDX-License-Identifier: MIT OR Apache-2.0
//! A coloured triangle on a wgpu surface, with resize and vsync-paced presentation.
//!
//! `cargo run -p tpt-window-wgpu --example wgpu_triangle`
//!
//! Set `TPT_EXAMPLE_SECONDS=5` to auto-exit.

use std::time::{Duration, Instant};

use tpt_window::prelude::*;
use tpt_window_wgpu::create;

const SHADER: &str = r#"
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VsOut {
    var corners = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 0.6),
        vec2<f32>(-0.6, -0.5),
        vec2<f32>(0.6, -0.5),
    );
    var colors = array<vec3<f32>, 3>(
        vec3<f32>(1.0, 0.2, 0.2),
        vec3<f32>(0.2, 1.0, 0.2),
        vec3<f32>(0.2, 0.4, 1.0),
    );
    var out: VsOut;
    out.position = vec4<f32>(corners[index], 0.0, 1.0);
    out.color = colors[index];
    return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color, 1.0);
}
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let limit = std::env::var("TPT_EXAMPLE_SECONDS")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .map(Duration::from_secs_f64);
    let started = Instant::now();

    let mut event_loop = new_event_loop()?;
    let window = WindowBuilder::new()
        .with_title("tpt-window: wgpu triangle")
        .with_inner_size(LogicalSize::new(640.0, 480.0))
        .build(&event_loop)?;

    let (gpu, mut surface) = create(&window)?;
    let shader = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("triangle"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("triangle layout"),
            ..Default::default()
        });
    let pipeline = gpu
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("triangle"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface.format(),
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

    let mut queue = EventQueue::with_capacity(DEFAULT_CAPACITY);
    'frames: loop {
        // Never block here: the present below waits for the display, which paces the loop.
        event_loop.poll_events(&mut EventSink::new(&mut queue));
        for ev in queue.drain() {
            match ev.event {
                WindowEvent::CloseRequested => break 'frames,
                WindowEvent::Resized(size) => surface.resize(&gpu.device, size),
                _ => {}
            }
        }
        if limit.is_some_and(|l| started.elapsed() >= l) {
            break;
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
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("triangle pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.05,
                            b: 0.08,
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
            pass.set_pipeline(&pipeline);
            pass.draw(0..3, 0..1);
        }
        gpu.queue.submit(Some(encoder.finish()));
        frame.present(&gpu.queue);
    }
    Ok(())
}
