//! Windows access to FILCO keyboards, the way `fnmap.py` and FILCO Assist do it:
//!
//! 1. enumerate HID interfaces with SetupAPI (`GUID_DEVINTERFACE_HID`, present devices only);
//! 2. open each with `CreateFileW(path, 0, FILE_SHARE_READ | FILE_SHARE_WRITE, ..)`: zero access rights, because
//!    Windows keeps keyboard collections exclusive, but feature reports still work on such a handle;
//! 3. read `HidD_GetAttributes` (VID, PID, REV) and `HidP_GetCaps` (usage page, report lengths);
//! 4. exchange 65-byte feature reports with `HidD_SetFeature` / `HidD_GetFeature`.
//!
//! [`open`] always returns the device behind a [`PolicyTransport`], so the caller must choose a
//! [`WritePolicy`] explicitly.
#![cfg(windows)]

mod device;
mod enumerate;
mod process;
mod watch;

pub use device::{HidDevice, open};
pub use enumerate::{DeviceInfo, EnumerateError, ValueRange, enumerate, system_control_ranges};
pub use filco_protocol::{PolicyTransport, WritePolicy};
pub use process::{RunningProcess, running_processes};
pub use watch::{HidWatcher, WatchError};
