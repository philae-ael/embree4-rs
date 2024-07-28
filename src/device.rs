use std::{
    ffi::{c_void, CStr},
    marker::PhantomData,
    ptr::null_mut,
};

use anyhow::{bail, Result};
use embree4_sys::RTCError;

use crate::device_error_raw;

pub struct Device {
    pub(crate) handle: embree4_sys::RTCDevice,
}

unsafe impl Send for Device {}
unsafe impl Sync for Device {}

impl Device {
    /// Constructs a new `Device` using the provided configuration string.
    ///
    /// # Arguments
    /// * `config` - A string representing the configuration for the device. Can be an empty string.
    ///              See [rtcNewDevice](https://github.com/embree/embree/blob/master/doc/src/api/rtcNewDevice.md) for valid configuration values.
    ///
    /// # Returns
    /// A `Result` containing the created `Device` if successful, or an error if the device creation fails.
    ///
    /// # Examples
    /// ```
    /// use embree4_rs::Device;
    ///
    /// let device = Device::try_new(Some("verbose=1")).unwrap();
    /// // Use the device...
    /// ```
    pub fn try_new(config: Option<&str>) -> Result<Self> {
        let handle = match config {
            None => unsafe { embree4_sys::rtcNewDevice(null_mut()) },
            Some(config) => unsafe {
                embree4_sys::rtcNewDevice(config.as_bytes() as *const _ as _)
            },
        };

        if handle.is_null() {
            let error = device_error_raw(null_mut());
            bail!("Failed to create device: {:?}", error);
        }

        Ok(Device { handle })
    }

    /// Returns the error code associated with the device, if any.
    ///
    /// # Returns
    /// `Some(error_code)` if there is an error associated with the device, otherwise `None`.
    pub fn error(&self) -> Option<embree4_sys::RTCError> {
        device_error_raw(self.handle)
    }

    /// Returns the device as a raw handle.
    ///
    /// # Safety
    ///
    /// The device must not be released.
    pub unsafe fn as_raw_handle(&self) -> embree4_sys::RTCDevice {
        self.handle
    }

    /// Setup a callback that is called on error and returns a structure that will remove is on drop.
    ///
    /// For semantic see the reference for [RtcSetDeviceErrorFunction](https://github.com/RenderKit/embree/blob/master/doc/src/api/rtcSetDeviceErrorFunction.md).
    ///
    /// To setup a permanent callback, use [std::mem::forget] on the returned [ErrorCallBackScope] but this will force the callback to have a `'static` lifetime.
    ///
    /// # Example
    ///```
    /// use embree4_rs::*;
    ///
    /// let device = Device::try_new(None).unwrap();
    ///
    /// // setup a permanent callback
    /// std::mem::forget(device.register_error_callback(|code, err|{
    ///     print!("Embree error ({code:?}): {err}");
    /// }));
    /// ```
    pub fn register_error_callback<'scope, F: FnMut(RTCError, &str) + 'scope>(
        &self,
        mut callback: F,
    ) -> ErrorCallBackScope<'scope> {
        // adapted from https://adventures.michaelfbryan.com/posts/rust-closures-in-ffi/
        unsafe extern "C" fn trampoline<'scope, F: FnMut(RTCError, &str) + 'scope>(
            user_ptr: *mut ::std::os::raw::c_void,
            code: RTCError,
            str: *const ::std::os::raw::c_char,
        ) {
            let f = &mut *(user_ptr as *mut F);
            let s = CStr::from_ptr(str);
            (f)(code, &s.to_string_lossy());
        }

        unsafe {
            embree4_sys::rtcSetDeviceErrorFunction(
                self.handle,
                Some(trampoline::<F>),
                &mut callback as *mut _ as *mut c_void,
            )
        };

        ErrorCallBackScope {
            device: self.handle,
            lifetime: Default::default(),
        }
    }

    /// Setup a callback that is called on memory allocation and deallocations and returns a structure that will remove is on drop.
    ///
    /// For semantic see the reference for [RTCSetDeviceMemoryMonitorFunction](https://github.com/RenderKit/embree/blob/master/doc/src/api/rtcSetSceneProgressMonitorFunction.md).
    ///
    /// To setup a permanent callback, use [std::mem::forget] on the returned [MemoryMonitorCallBackScope] but this will force the callback to have a `'static` lifetime.
    ///
    ///```
    /// use embree4_rs::*;
    ///
    /// let device = Device::try_new(None).unwrap();
    /// let _memory_monitor_callback = device.register_device_memory_monitor_callback(|size, _| {
    ///     if size >= 0 {
    ///         print!("Embree allocation of {}bytes", size);
    ///     } else {
    ///         print!("Embree deallocation of {}bytes", -size);
    ///     }
    ///      true
    /// });
    /// ```
    pub fn register_device_memory_monitor_callback<
        'scope,
        F: FnMut(isize, bool) -> bool + 'scope,
    >(
        &self,
        mut callback: F,
    ) -> MemoryMonitorCallBackScope<'scope> {
        // adapted from https://adventures.michaelfbryan.com/posts/rust-closures-in-ffi/
        unsafe extern "C" fn trampoline<'scope, F: FnMut(isize, bool) -> bool + 'scope>(
            user_ptr: *mut ::std::os::raw::c_void,
            size: isize,
            post: bool,
        ) -> bool {
            let f = &mut *(user_ptr as *mut F);
            f(size, post)
        }

        unsafe {
            embree4_sys::rtcSetDeviceMemoryMonitorFunction(
                self.handle,
                Some(trampoline::<F>),
                &mut callback as *mut _ as *mut c_void,
            )
        };
        MemoryMonitorCallBackScope {
            device: self.handle,
            lifetime: Default::default(),
        }
    }

    /// Remove a previously setup error callback.
    ///
    /// This function should not be needed as the [ErrorCallBackScope] struct should do it automatically.
    pub fn remove_error_callback(&mut self) {
        unsafe {
            embree4_sys::rtcSetDeviceMemoryMonitorFunction(self.handle, None, null_mut());
        }
    }

    /// Remove a previously setup memory monitor callback.
    ///
    /// This function should not be needed as the [MemoryMonitorCallBackScope] struct should do it
    /// automatically.
    pub fn remove_memory_monitor_callback(&mut self) {
        unsafe {
            embree4_sys::rtcSetDeviceMemoryMonitorFunction(self.handle, None, null_mut());
        }
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            embree4_sys::rtcReleaseDevice(self.handle);
        }
    }
}

/// A type that will remove the device error callback on drop
///
/// # Note:
/// The previous callback is not restored on drop
pub struct ErrorCallBackScope<'scope> {
    device: embree4_sys::RTCDevice,
    lifetime: PhantomData<&'scope ()>,
}

impl Drop for ErrorCallBackScope<'_> {
    fn drop(&mut self) {
        unsafe {
            embree4_sys::rtcSetDeviceErrorFunction(self.device, None, null_mut());
        }
    }
}

/// A type that will remove the memory monitor callback on drop
///
/// # Note:
/// The previous callback is not restored on drop
pub struct MemoryMonitorCallBackScope<'scope> {
    device: embree4_sys::RTCDevice,
    lifetime: PhantomData<&'scope ()>,
}

impl Drop for MemoryMonitorCallBackScope<'_> {
    fn drop(&mut self) {
        unsafe {
            embree4_sys::rtcSetDeviceMemoryMonitorFunction(self.device, None, null_mut());
        }
    }
}

#[test]
fn try_new_valid_config() {
    let ok_device = Device::try_new(Some("verbose=0"));
    assert!(ok_device.is_ok());
}

#[test]
fn try_new_invalid_config() {
    let err_device = Device::try_new(Some("verbose=bruh"));
    assert!(err_device.is_err());
}

#[test]
fn try_new_no_config() {
    let ok_device = Device::try_new(None);
    assert!(ok_device.is_ok());
}
