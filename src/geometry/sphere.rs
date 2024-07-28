use std::{mem::size_of, slice};

use anyhow::{bail, Result};

use crate::{device::Device, device_error_or};

use super::Geometry;

pub struct SphereGeometry {
    handle: embree4_sys::RTCGeometry,
}

impl SphereGeometry {
    /// Constructs a new `SphereGeometry` instance from a given point and radius
    ///
    /// # Example
    /// ```
    /// use embree4_rs::{*, geometry::*};
    /// use embree4_sys::*;
    ///
    /// let device = Device::try_new(None).unwrap();
    /// let geometry = SphereGeometry::try_new(&device, (0.0, 0.1, 0.2),  5.0).unwrap();
    /// let scene = Scene::try_new(device, SceneOptions::default()).unwrap();
    /// scene.attach_geometry(&geometry);
    /// ```
    pub fn try_new(device: &Device, origin: (f32, f32, f32), radius: f32) -> Result<Self> {
        let geometry = unsafe {
            embree4_sys::rtcNewGeometry(device.handle, embree4_sys::RTCGeometryType::SPHERE_POINT)
        };
        if geometry.is_null() {
            bail!("Failed to create geometry: {:?}", device.error());
        }

        let vertex_buf_ptr = unsafe {
            embree4_sys::rtcSetNewGeometryBuffer(
                geometry,
                embree4_sys::RTCBufferType::VERTEX,
                0,
                embree4_sys::RTCFormat::FLOAT4,
                4 * size_of::<f32>(),
                1,
            )
        };
        if vertex_buf_ptr.is_null() {
            bail!(
                "Failed to create sphere vertex buffer: {:?}",
                device.error()
            );
        }
        device_error_or(device, (), "Failed not create sphere vertex buffer")?;

        let vertex_buf = unsafe { slice::from_raw_parts_mut(vertex_buf_ptr as *mut f32, 4) };
        vertex_buf.copy_from_slice(&[origin.0, origin.1, origin.2, radius]);

        unsafe {
            embree4_sys::rtcCommitGeometry(geometry);
        }
        device_error_or(device, (), "Failed to commit sphere geometry")?;

        Ok(Self { handle: geometry })
    }
}

impl Drop for SphereGeometry {
    fn drop(&mut self) {
        unsafe {
            embree4_sys::rtcReleaseGeometry(self.handle);
        }
    }
}

impl Geometry for SphereGeometry {
    fn geometry(&self) -> embree4_sys::RTCGeometry {
        self.handle
    }
}
