use std::{ffi::c_void, marker::PhantomData, ptr::null_mut};

use anyhow::{bail, Result};
use embree4_sys::RTCBounds;

use crate::{device::Device, device_error_or, device_error_raw, geometry::Geometry};

pub struct Scene<'a> {
    device: &'a Device,
    handle: embree4_sys::RTCScene,
}

impl<'a> Scene<'a> {
    /// Constructs a new `Scene` instance from the given options.
    ///
    /// # Arguments
    /// * `device` - A reference to the `Device` instance.
    /// * `options` - The options for creating the scene.
    ///
    /// # Returns
    /// A `Result` containing the `Scene` instance if successful, or an error if an error occurred.
    ///
    /// # Example
    /// ```
    /// use embree4_rs::*;
    /// use embree4_sys::*;
    ///
    /// let device = Device::try_new(None).unwrap();
    /// let options = SceneOptions {
    ///     build_quality: RTCBuildQuality::HIGH,
    ///     flags: RTCSceneFlags::COMPACT | RTCSceneFlags::ROBUST,
    /// };
    /// let scene = Scene::try_new(&device, options).unwrap();
    /// ```
    pub fn try_new(device: &'a Device, options: SceneOptions) -> Result<Self> {
        let handle = unsafe { embree4_sys::rtcNewScene(device.handle) };

        if handle.is_null() {
            let error = device_error_raw(device.handle);
            bail!("Could not create scene: {:?}", error);
        }

        let scene = Scene { device, handle };

        if options.build_quality != Default::default() {
            scene.set_build_quality(options.build_quality)?;
        }

        if options.flags != Default::default() {
            scene.set_flags(options.flags)?;
        }

        Ok(scene)
    }

    /// Sets the build quality of the scene.
    ///
    /// # Arguments
    /// * `quality` - The build quality to set.
    ///
    /// # Returns
    /// A `Result` indicating success or failure.
    pub fn set_build_quality(&self, quality: embree4_sys::RTCBuildQuality) -> Result<()> {
        unsafe {
            embree4_sys::rtcSetSceneBuildQuality(self.handle, quality);
        }
        device_error_or(self.device, (), "Could not set scene build quality")
    }

    /// Sets the flags of the scene.
    ///
    /// # Arguments
    /// * `flags` - The flags to set.
    ///
    /// # Returns
    /// A `Result` indicating success or failure.
    pub fn set_flags(&self, flags: embree4_sys::RTCSceneFlags) -> Result<()> {
        unsafe {
            embree4_sys::rtcSetSceneFlags(self.handle, flags);
        }
        device_error_or(self.device, (), "Could not set scene flags")
    }

    /// Attaches the given geometry to the scene.
    ///
    /// # Arguments
    /// * `geometry` - A reference to the `Geometry` instance to attach.
    ///
    /// # Returns
    /// * A `Result` containing the geometry ID if successful, or an error if an error occurred.
    pub fn attach_geometry(&self, geometry: &impl Geometry) -> Result<u32> {
        let geom_id = unsafe { embree4_sys::rtcAttachGeometry(self.handle, geometry.geometry()) };
        device_error_or(self.device, geom_id, "Could not attach geometry")
    }

    /// Commits the scene.
    ///
    /// # Returns
    /// A `Result` containing the `CommittedScene` instance if successful, or an error if an error occurred.
    ///
    /// # Example
    /// ```
    /// use embree4_rs::*;
    /// use embree4_sys::*;
    ///
    /// let device = Device::try_new(None).unwrap();
    /// let options = Default::default();
    /// let scene = Scene::try_new(&device, options).unwrap();
    /// let scene = scene.commit().unwrap();
    /// ```
    pub fn commit(&self) -> Result<CommittedScene<'a>> {
        unsafe {
            embree4_sys::rtcCommitScene(self.handle);
        }
        device_error_or(
            self.device,
            CommittedScene {
                device: self.device,
                handle: self.handle,
            },
            "Could not commit scene",
        )
    }

    /// Setup a callback that is called on progress and returns a structure that will remove is on drop.
    ///
    /// For semantic see the reference for [rtcSetSceneProgressMonitorFunction](https://github.com/RenderKit/embree/blob/master/doc/src/api/rtcSetSceneProgressMonitorFunction.md).
    ///
    /// To setup a permanent callback, use [std::mem::forget] on the returned [SceneProgressCallbackScope] but this will force the callback to have a `'static` lifetime.
    pub fn register_scene_progress_monitor_callback<'scope, F: FnMut(f64) -> bool + 'scope>(
        &mut self,
        mut f: F,
    ) -> SceneProgressCallbackScope<'scope> {
        unsafe extern "C" fn trampoline<'scope, F: FnMut(f64) -> bool + 'scope>(
            user_ptr: *mut ::std::os::raw::c_void,
            progress: f64,
        ) -> bool {
            let f = &mut *(user_ptr as *mut F);
            f(progress)
        }
        unsafe {
            embree4_sys::rtcSetSceneProgressMonitorFunction(
                self.handle,
                Some(trampoline::<F>),
                &mut f as &mut _ as *mut _ as *mut c_void,
            )
        }

        SceneProgressCallbackScope {
            handle: self.handle,
            lifetime: Default::default(),
        }
    }

    /// Remove a previously setup progress monitor callback.
    ///
    /// This function should not be needed as the SceneProgressCallbackScope struct should do it automatically.
    pub fn remove_scene_progress_monitor_callback(&mut self) {
        unsafe { embree4_sys::rtcSetSceneProgressMonitorFunction(self.handle, None, null_mut()) }
    }
}

impl<'a> Drop for Scene<'a> {
    fn drop(&mut self) {
        unsafe {
            embree4_sys::rtcReleaseScene(self.handle);
        }
    }
}

pub struct SceneProgressCallbackScope<'a> {
    handle: embree4_sys::RTCScene,
    lifetime: PhantomData<&'a ()>,
}

impl Drop for SceneProgressCallbackScope<'_> {
    fn drop(&mut self) {
        unsafe { embree4_sys::rtcSetSceneProgressMonitorFunction(self.handle, None, null_mut()) }
    }
}

#[derive(Default)]
pub struct SceneOptions {
    pub build_quality: embree4_sys::RTCBuildQuality,
    pub flags: embree4_sys::RTCSceneFlags,
}

pub struct CommittedScene<'a> {
    device: &'a Device,
    handle: embree4_sys::RTCScene,
}

unsafe impl<'a> Sync for CommittedScene<'a> {}

impl<'a> CommittedScene<'a> {
    pub fn intersect_1(&self, ray: embree4_sys::RTCRay) -> Result<Option<embree4_sys::RTCRayHit>> {
        let mut ray_hit = embree4_sys::RTCRayHit {
            ray,
            hit: Default::default(),
        };

        unsafe {
            embree4_sys::rtcIntersect1(self.handle, &mut ray_hit, std::ptr::null_mut());
        }
        device_error_or(self.device, (), "Could not intersect ray")?;

        Ok(
            if ray_hit.hit.geomID != embree4_sys::RTC_INVALID_GEOMETRY_ID {
                Some(ray_hit)
            } else {
                None
            },
        )
    }

    /// Returns the axis-aligned bounding box og the scene
    pub fn bounds(&self) -> Result<embree4_sys::RTCBounds> {
        let mut bounds = embree4_sys::RTCBounds::default();
        unsafe { embree4_sys::rtcGetSceneBounds(self.handle, &mut bounds as *mut RTCBounds) };
        device_error_or(self.device, bounds, "Could not get bounds")
    }
}
