//! Notification when any HID interface arrives or leaves (`CM_Register_Notification`).

use std::ffi::c_void;

use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_NOTIFY_ACTION, CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL, CM_NOTIFY_ACTION_DEVICEINTERFACEREMOVAL,
    CM_NOTIFY_EVENT_DATA, CM_NOTIFY_FILTER, CM_NOTIFY_FILTER_0, CM_NOTIFY_FILTER_0_0,
    CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE, CM_Register_Notification, CM_Unregister_Notification, CR_SUCCESS,
    HCMNOTIFICATION,
};
use windows::Win32::Devices::HumanInterfaceDevice::HidD_GetHidGuid;

type Callback = Box<dyn Fn() + Send + Sync + 'static>;

/// Calls a function whenever a HID interface is added or removed, until dropped. The callback runs on a
/// system thread pool thread: keep it short (send on a channel).
pub struct HidWatcher {
    handle: HCMNOTIFICATION,
    context: *mut Callback,
}

impl std::fmt::Debug for HidWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HidWatcher")
    }
}

// SAFETY: the notification handle may be unregistered from any thread; the context is only read by the
// callback and freed after unregistering, which waits for running callbacks to finish.
unsafe impl Send for HidWatcher {}

#[derive(Debug, thiserror::Error)]
#[error("CM_Register_Notification failed with CONFIGRET {0}")]
pub struct WatchError(pub u32);

unsafe extern "system" fn on_change(
    _notify: HCMNOTIFICATION,
    context: *const c_void,
    action: CM_NOTIFY_ACTION,
    _data: *const CM_NOTIFY_EVENT_DATA,
    _size: u32,
) -> u32 {
    if action == CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL || action == CM_NOTIFY_ACTION_DEVICEINTERFACEREMOVAL
    {
        // SAFETY: `context` is the `*mut Callback` registered by `HidWatcher::new`, alive until the watcher is
        // dropped, and Drop unregisters (waiting for callbacks) before freeing it.
        let cb = unsafe { &*context.cast::<Callback>() };
        cb();
    }
    0
}

impl HidWatcher {
    pub fn new(callback: impl Fn() + Send + Sync + 'static) -> Result<Self, WatchError> {
        let context: *mut Callback = Box::into_raw(Box::new(Box::new(callback)));
        // SAFETY: HidD_GetHidGuid only returns the GUID.
        let hid_guid = unsafe { HidD_GetHidGuid() };
        let filter = CM_NOTIFY_FILTER {
            cbSize: u32::try_from(size_of::<CM_NOTIFY_FILTER>()).unwrap_or(0),
            FilterType: CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE,
            u: CM_NOTIFY_FILTER_0 {
                DeviceInterface: CM_NOTIFY_FILTER_0_0 { ClassGuid: hid_guid },
            },
            ..Default::default()
        };
        let mut handle = HCMNOTIFICATION::default();
        // SAFETY: the filter is fully initialised, the context stays valid until Drop, and the callback has the
        // signature the API requires.
        let r = unsafe {
            CM_Register_Notification(
                &raw const filter,
                Some(context.cast_const().cast()),
                Some(on_change),
                &raw mut handle,
            )
        };
        if r != CR_SUCCESS {
            // SAFETY: registration failed, so nothing else references the context.
            drop(unsafe { Box::from_raw(context) });
            return Err(WatchError(r.0));
        }
        Ok(HidWatcher { handle, context })
    }
}

impl Drop for HidWatcher {
    fn drop(&mut self) {
        // SAFETY: the handle came from CM_Register_Notification. Unregistering waits for callbacks in progress,
        // after which the context can be freed exactly once.
        unsafe {
            let _ = CM_Unregister_Notification(self.handle);
            drop(Box::from_raw(self.context));
        }
    }
}
