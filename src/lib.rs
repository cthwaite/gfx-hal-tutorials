#[cfg(windows)]
extern crate gfx_backend_dx12 as gfx_backend;
#[cfg(target_os = "macos")]
pub extern crate gfx_backend_metal as gfx_backend;
#[cfg(all(unix, not(target_os = "macos")))]
extern crate gfx_backend_vulkan as gfx_backend;

extern crate gfx_hal;
extern crate winit;

pub mod prelude;
pub use gfx_backend as backend;
