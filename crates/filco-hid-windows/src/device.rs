//! An open HID interface exchanging feature reports.

use filco_protocol::{PolicyTransport, REPORT_LEN, Report, Transport, TransportError, WritePolicy};
use windows::Win32::Devices::HumanInterfaceDevice::{HidD_GetFeature, HidD_SetFeature};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_BAD_COMMAND, ERROR_DEVICE_NOT_CONNECTED, ERROR_FILE_NOT_FOUND, ERROR_INVALID_HANDLE,
    ERROR_NOT_READY, ERROR_PATH_NOT_FOUND, GetLastError, HANDLE, WIN32_ERROR,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::core::HSTRING;

use crate::enumerate::{DeviceInfo, describe};

/// A handle closed on drop.
#[derive(Debug)]
pub(crate) struct OwnedHandle(HANDLE);

impl OwnedHandle {
    /// `CreateFileW(path, 0, FILE_SHARE_READ | FILE_SHARE_WRITE, NULL, OPEN_EXISTING, 0, NULL)`, as `fnmap.py` and
    /// FILCO Assist do. Zero access rights: enough for feature reports, and allowed on keyboard collections.
    pub(crate) fn open_zero_access(path: &str) -> windows::core::Result<Self> {
        let wide = HSTRING::from(path);
        // SAFETY: `wide` is a valid NUL-terminated path that outlives the call; no security attributes or template.
        let h = unsafe {
            CreateFileW(
                &wide,
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            )
        }?;
        Ok(OwnedHandle(h))
    }

    pub(crate) fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: the handle came from CreateFileW and is closed exactly once.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

// SAFETY: a file handle may be used from any thread; HidDevice serialises use through &mut self.
unsafe impl Send for OwnedHandle {}

/// One open HID interface.
#[derive(Debug)]
pub struct HidDevice {
    handle: OwnedHandle,
    info: DeviceInfo,
}

impl HidDevice {
    pub fn info(&self) -> &DeviceInfo {
        &self.info
    }
}

/// Opens the interface at `path` behind a write policy. Fails if the path is gone or the interface does not
/// use 65-byte feature reports.
pub fn open(path: &str, policy: WritePolicy) -> Result<PolicyTransport<HidDevice>, TransportError> {
    let handle = OwnedHandle::open_zero_access(path).map_err(|e| map_hresult(&e))?;
    let info = describe(&handle, path.to_owned()).ok_or(TransportError::Disconnected)?;
    if usize::from(info.feature_len) != REPORT_LEN {
        return Err(TransportError::Io {
            code: 0,
            message: format!(
                "feature report length is {}, expected {REPORT_LEN}",
                info.feature_len
            ),
        });
    }
    Ok(PolicyTransport::new(HidDevice { handle, info }, policy))
}

fn map_win32(code: WIN32_ERROR) -> TransportError {
    let gone = [
        ERROR_DEVICE_NOT_CONNECTED,
        ERROR_INVALID_HANDLE,
        ERROR_FILE_NOT_FOUND,
        ERROR_PATH_NOT_FOUND,
        ERROR_NOT_READY,
        ERROR_BAD_COMMAND,
    ];
    if gone.contains(&code) {
        TransportError::Disconnected
    } else {
        TransportError::Io {
            code: code.0,
            message: windows::core::Error::from(code.to_hresult()).message(),
        }
    }
}

fn map_hresult(e: &windows::core::Error) -> TransportError {
    let code = WIN32_ERROR::from_error(e).unwrap_or(WIN32_ERROR(0));
    if code.0 == 0 {
        TransportError::Io {
            code: 0,
            message: e.message(),
        }
    } else {
        map_win32(code)
    }
}

fn last_error() -> TransportError {
    // SAFETY: GetLastError has no preconditions.
    map_win32(unsafe { GetLastError() })
}

impl Transport for HidDevice {
    fn set_feature(&mut self, report: &Report) -> Result<(), TransportError> {
        let mut buf = *report;
        let len = u32::try_from(REPORT_LEN).unwrap_or(0);
        // SAFETY: `buf` is a writable 65-byte buffer that outlives the call; the handle is open.
        if unsafe { HidD_SetFeature(self.handle.raw(), buf.as_mut_ptr().cast(), len) } {
            Ok(())
        } else {
            Err(last_error())
        }
    }

    fn get_feature(&mut self) -> Result<Report, TransportError> {
        let mut buf = [0u8; REPORT_LEN];
        let len = u32::try_from(REPORT_LEN).unwrap_or(0);
        // SAFETY: as above; byte 0 is report id 0, which is what the keyboard uses.
        if unsafe { HidD_GetFeature(self.handle.raw(), buf.as_mut_ptr().cast(), len) } {
            Ok(buf)
        } else {
            Err(last_error())
        }
    }
}
