//! [![Crates.io](https://img.shields.io/crates/v/embree4-rs.svg)](https://crates.io/crates/embree4-rs)
//!
//! High-level wrapper for [Intel's Embree](https://www.embree.org/) 4 high-performance ray tracing
//! library.
//!
//! FFI Bindings from [embree4-sys](https://crates.io/crates/embree4-sys).
//!
//! A valid Embree installation is required. See
//! [Installation of Embree](https://github.com/embree/embree#installation-of-embree)
//! from the Embree docs.
//!
//! # Documentation
//!
//! Docs at [docs.rs](https://docs.rs/embree4-rs).
//!
//! See the [examples/](https://github.com/psytrx/embree4-rs/tree/main/examples) for a quick start
//! on how to use this crate.

pub mod device;
pub mod geometry;
pub mod scene;

use std::arch::asm;

use anyhow::{bail, Result};

pub mod prelude {
    pub use crate::device::Device;
    pub use crate::scene::{CommittedScene, Scene, SceneOptions};
}

pub mod sys {
    pub use embree4_sys::*;
}

fn device_error_raw(device: embree4_sys::RTCDevice) -> Option<embree4_sys::RTCError> {
    let err = unsafe { embree4_sys::rtcGetDeviceError(device) };
    if err != embree4_sys::RTCError::NONE {
        Some(err)
    } else {
        None
    }
}

fn device_error_or<T>(device: &device::Device, ok_value: T, message: &str) -> Result<T> {
    device_error_raw(device.handle)
        .map(|error| bail!("{}: {:?}", message, error))
        .unwrap_or(Ok(ok_value))
}

// Ensure that "Flush to Zero" and "Denormals are Zero" are enabled and restore old flags when
// needed
pub(crate) struct Mxcsr {
    old: u32,
}

impl Mxcsr {
    pub(crate) unsafe fn setup() -> Self {
        let mut this = Self { old: 0 };

        #[cfg(all(target_arch = "x86_64", target_feature = "sse"))]
        {
            asm!("stmxcsr [{}]", in(reg)(std::ptr::addr_of_mut!(this.old)));

            let new = this.old | 0x8040;
            asm!("ldmxcsr [{}]", in(reg) &new);
        }

        this
    }
}

#[cfg(all(target_arch = "x86_64", target_feature = "sse"))]
impl Drop for Mxcsr {
    fn drop(&mut self) {
        unsafe {
            asm!("ldmxcsr [{}]", in(reg) &self.old);
        }
    }
}
