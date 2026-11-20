// SPDX-License-Identifier: MIT OR Apache-2.0
//! wgpu helpers for tpt-window windows: surface creation, resize reconfiguration, and the
//! acquire-or-skip logic every render loop needs.
//!
//! ```ignore
//! let (gpu, mut surface) = tpt_window_wgpu::create(&window)?;
//! loop {
//!     event_loop.poll_events(&mut sink); // never blocks: the present below paces the loop
//!     // on each `Resized`: surface.resize(&gpu.device, size);
//!     if let Some(frame) = surface.acquire(&gpu.device)? {
//!         // record and submit with `frame.texture.create_view(...)`, then:
//!         frame.present();
//!     }
//! }
//! ```
//!
//! The loop must not wait in `step`: with the default `Fifo` present mode, `present` blocks until
//! the display is ready, which is the pacing a game loop wants. Waiting in `step` as well would
//! add a timeout's worth of delay per frame.

use tpt_window::{PhysicalSize, SurfaceTarget, Window};

/// What went wrong creating or using the GPU.
#[derive(Debug)]
pub enum GpuError {
    /// The window's handles could not become a surface.
    Surface(String),
    /// No adapter matches the window's surface.
    Adapter(String),
    /// The adapter could not give a device.
    Device(String),
    /// The surface has no default configuration for this adapter.
    NoConfiguration,
    /// The GPU reported a validation error while acquiring a frame.
    Validation,
}

impl core::fmt::Display for GpuError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            GpuError::Surface(e) => write!(f, "cannot create a surface: {e}"),
            GpuError::Adapter(e) => write!(f, "no suitable GPU adapter: {e}"),
            GpuError::Device(e) => write!(f, "cannot create a device: {e}"),
            GpuError::NoConfiguration => f.write_str("the surface has no default configuration"),
            GpuError::Validation => f.write_str("validation error while acquiring a frame"),
        }
    }
}

impl std::error::Error for GpuError {}

/// The GPU objects a renderer needs. Keep them for the life of the application.
pub struct Gpu {
    /// The instance the adapter came from.
    pub instance: wgpu::Instance,
    /// The adapter chosen for the window's surface.
    pub adapter: wgpu::Adapter,
    /// The logical device.
    pub device: wgpu::Device,
    /// The device's queue.
    pub queue: wgpu::Queue,
}

/// A window's surface, with the configuration that matches its size.
pub struct GpuSurface<'w> {
    surface: wgpu::Surface<'w>,
    config: wgpu::SurfaceConfiguration,
    /// Set after an acquire reported the surface as outdated, to reconfigure on the next one.
    needs_configure: bool,
}

/// A frame acquired for drawing. Draw into `texture`, then call [`Frame::present`].
pub struct Frame {
    /// The texture to draw into.
    pub texture: wgpu::SurfaceTexture,
}

impl Frame {
    /// Shows the frame on the window. Call it after the commands that draw into the frame have
    /// been submitted.
    pub fn present(self, queue: &wgpu::Queue) {
        queue.present(self.texture);
    }
}

/// Creates the GPU objects and a surface for `window`, sized to the window's current size.
pub fn create(window: &Window) -> Result<(Gpu, GpuSurface<'_>), GpuError> {
    let instance = wgpu::Instance::default();
    let surface = instance
        .create_surface(SurfaceTarget::new(window))
        .map_err(|e| GpuError::Surface(e.to_string()))?;
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        compatible_surface: Some(&surface),
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        apply_limit_buckets: false,
    }))
    .map_err(|e| GpuError::Adapter(e.to_string()))?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .map_err(|e| GpuError::Device(e.to_string()))?;
    let size = window.size();
    let config = surface
        .get_default_config(&adapter, size.width.max(1), size.height.max(1))
        .ok_or(GpuError::NoConfiguration)?;
    surface.configure(&device, &config);
    Ok((
        Gpu {
            instance,
            adapter,
            device,
            queue,
        },
        GpuSurface {
            surface,
            config,
            needs_configure: false,
        },
    ))
}

impl GpuSurface<'_> {
    /// The format frames are drawn in. Pipelines must target it.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Follows a window resize. A zero size (a minimized window) is ignored, because a surface
    /// cannot be zero-sized.
    pub fn resize(&mut self, device: &wgpu::Device, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        if self.config.width == size.width && self.config.height == size.height {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(device, &self.config);
    }

    /// Acquires the next frame. `Ok(None)` means skip this frame: the GPU is busy, the window is
    /// hidden, or the surface was reconfigured. Call it once per loop turn.
    pub fn acquire(&mut self, device: &wgpu::Device) -> Result<Option<Frame>, GpuError> {
        if self.needs_configure {
            self.surface.configure(device, &self.config);
            self.needs_configure = false;
        }
        match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) => Ok(Some(Frame { texture })),
            // The frame is usable; the surface is reconfigured before the next one.
            wgpu::CurrentSurfaceTexture::Suboptimal(texture) => {
                self.needs_configure = true;
                Ok(Some(Frame { texture }))
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(device, &self.config);
                Ok(None)
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                Ok(None)
            }
            wgpu::CurrentSurfaceTexture::Validation => Err(GpuError::Validation),
        }
    }
}
