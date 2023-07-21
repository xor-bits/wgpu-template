use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

use anyhow::Result;
use wgpu::{
    Adapter, CompositeAlphaMode, Device, Instance, PresentMode, SurfaceCapabilities,
    SurfaceConfiguration, SurfaceError, SurfaceTexture, TextureFormat, TextureUsages,
};
use winit::{dpi::PhysicalSize, window::Window};

use crate::settings::GraphicsSettings;

//

pub struct SurfaceBuilder {
    instance: Arc<Instance>,

    // in Rust, this one is dropped ...
    pub surface: wgpu::Surface,
    // always before this one
    pub window: Arc<Window>,
}

pub struct Surface {
    device: Arc<Device>,

    inner: SurfaceBuilder,
    vsync: bool,
    format: TextureFormat,

    alpha_modes: Vec<CompositeAlphaMode>,
}

//

impl SurfaceBuilder {
    pub fn new(instance: Arc<Instance>, window: Arc<Window>) -> Result<Self> {
        // SAFETY: safe as `window` is freed only after the surface is
        // and the Arc makes sure the data the pointer points to is never moved
        //
        // look at the struct definition
        let surface = unsafe { instance.create_surface(window.as_ref()) }?;

        Ok(SurfaceBuilder {
            instance,
            surface,
            window,
        })
    }

    pub fn build(self, settings: &GraphicsSettings, gpu: &Adapter, device: Arc<Device>) -> Surface {
        let SurfaceCapabilities {
            formats,
            alpha_modes,
            ..
            // present_modes,
        } = self.surface.get_capabilities(gpu);

        let format = *formats.first().expect("Surface is incompatible somehow");

        let mut surface = Surface {
            device,

            inner: self,
            vsync: settings.vsync,
            format,

            alpha_modes,
        };

        surface.configure(None);
        surface
    }
}

impl Surface {
    pub fn format(&self) -> TextureFormat {
        self.format
    }

    pub fn configure(&mut self, size: Option<(u32, u32)>) {
        let present_mode = if self.vsync {
            PresentMode::AutoVsync
        } else {
            PresentMode::AutoNoVsync
        };

        /* let view_formats = if format.is_srgb() {
            vec![format]
        } else {
            vec![format, format.add_srgb_suffix()]
        }; */
        let view_formats = vec![self.format];

        let (width, height) = size.unwrap_or_else(|| {
            let PhysicalSize { width, height } = self.inner.window.inner_size();
            (width, height)
        });

        let mut alpha_mode = CompositeAlphaMode::Auto;
        if self
            .alpha_modes
            .contains(&CompositeAlphaMode::PostMultiplied)
        {
            alpha_mode = CompositeAlphaMode::PostMultiplied
        } else if self
            .alpha_modes
            .contains(&CompositeAlphaMode::PreMultiplied)
        {
            alpha_mode = CompositeAlphaMode::PreMultiplied
        };

        // tracing::debug!("surface configured to {width}x{height}");

        self.inner.surface.configure(
            &self.device,
            &SurfaceConfiguration {
                usage: TextureUsages::RENDER_ATTACHMENT,
                format: self.format,
                width,
                height,
                present_mode,
                alpha_mode,
                view_formats,
            },
        );
    }

    pub fn recreate(&mut self) -> Result<()> {
        self.inner = SurfaceBuilder::new(self.instance.clone(), self.window.clone())?;
        self.configure(None);

        Ok(())
    }

    pub fn acquire(&mut self) -> Result<SurfaceTexture> {
        loop {
            if let Some(texture) = self.try_acquire()? {
                return Ok(texture);
            }
        }
    }

    pub fn try_acquire(&mut self) -> Result<Option<SurfaceTexture>> {
        match self.inner.surface.get_current_texture() {
            Ok(texture) => {
                if texture.suboptimal {
                    // tracing::debug!("Surface suboptimal");
                    drop(texture);
                    self.configure(None);
                    return Ok(None);
                }

                Ok(Some(texture))
            }

            // TODO: autosave before
            // closing
            Err(SurfaceError::OutOfMemory) => panic!("Out of VRAM"),

            Err(SurfaceError::Timeout) => {
                tracing::trace!("Surface timeout");
                Ok(None)
            }

            Err(SurfaceError::Lost) => {
                tracing::debug!("Surface lost");
                self.recreate()?;
                Ok(None)
            }

            Err(SurfaceError::Outdated) => {
                tracing::debug!("Surface outdated");
                self.configure(None);
                Ok(None)
            }
        }
    }
}

impl Deref for Surface {
    type Target = SurfaceBuilder;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Surface {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
