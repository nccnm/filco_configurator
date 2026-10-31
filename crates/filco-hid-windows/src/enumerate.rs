//! Listing HID interfaces and reading their attributes and capabilities without sending any report.

use windows::Win32::Devices::DeviceAndDriverInstallation::{
    DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
    SetupDiGetDeviceInterfaceDetailW,
};
use windows::Win32::Devices::HumanInterfaceDevice::{
    HIDD_ATTRIBUTES, HIDP_BUTTON_CAPS, HIDP_CAPS, HIDP_STATUS_SUCCESS, HIDP_VALUE_CAPS,
    HidD_FreePreparsedData, HidD_GetAttributes, HidD_GetHidGuid, HidD_GetPreparsedData, HidP_GetButtonCaps,
    HidP_GetCaps, HidP_GetValueCaps, HidP_Input, PHIDP_PREPARSED_DATA,
};
use windows::core::PCWSTR;

use crate::device::OwnedHandle;

/// What enumeration learned about one HID interface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    pub path: String,
    pub vendor_id: u16,
    pub product_id: u16,
    /// `VersionNumber` of the USB device; the firmware revision on FILCO keyboards (e.g. 0x0072).
    pub version: u16,
    pub usage_page: u16,
    pub usage: u16,
    pub input_len: u16,
    pub output_len: u16,
    pub feature_len: u16,
}

#[derive(Debug, thiserror::Error)]
pub enum EnumerateError {
    #[error("SetupDiGetClassDevsW failed: {0}")]
    ClassDevs(windows::core::Error),
}

/// Lists every present HID interface that can be opened with zero access rights. No report is sent.
pub fn enumerate() -> Result<Vec<DeviceInfo>, EnumerateError> {
    let mut out = Vec::new();
    for path in interface_paths()? {
        let Ok(handle) = OwnedHandle::open_zero_access(&path) else {
            continue;
        };
        if let Some(info) = describe(&handle, path) {
            out.push(info);
        }
    }
    Ok(out)
}

fn interface_paths() -> Result<Vec<String>, EnumerateError> {
    // SAFETY: HidD_GetHidGuid only writes the GUID it returns.
    let guid = unsafe { HidD_GetHidGuid() };
    // SAFETY: the GUID pointer is valid for the call; flags are documented constants; the returned list is
    // destroyed below in every path.
    let info = unsafe {
        SetupDiGetClassDevsW(
            Some(&raw const guid),
            PCWSTR::null(),
            None,
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        )
    }
    .map_err(EnumerateError::ClassDevs)?;
    let mut paths = Vec::new();
    for index in 0.. {
        let mut data = SP_DEVICE_INTERFACE_DATA {
            cbSize: u32::try_from(size_of::<SP_DEVICE_INTERFACE_DATA>()).unwrap_or(0),
            ..Default::default()
        };
        // SAFETY: `info` is a live device info list and `data.cbSize` is set as the API requires.
        if unsafe { SetupDiEnumDeviceInterfaces(info, None, &raw const guid, index, &raw mut data) }.is_err()
        {
            break;
        }
        let mut needed = 0u32;
        // SAFETY: first call with no buffer only reports the size needed; failure is expected.
        let _ = unsafe {
            SetupDiGetDeviceInterfaceDetailW(info, &raw const data, None, 0, Some(&raw mut needed), None)
        };
        if needed < 8 {
            continue;
        }
        // u32 storage keeps the buffer aligned for SP_DEVICE_INTERFACE_DETAIL_DATA_W.
        let mut buf = vec![0u32; (needed as usize).div_ceil(4)];
        let detail = buf.as_mut_ptr().cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
        // SAFETY: `buf` holds at least `needed` bytes and is 4-byte aligned; cbSize is the size of the fixed
        // part of the structure as the API requires (8 on 64-bit, 6 on 32-bit).
        let fixed = u32::try_from(size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>()).unwrap_or(0);
        // SAFETY: as above; writes the first field inside the buffer.
        unsafe { (*detail).cbSize = fixed };
        // SAFETY: the buffer is large enough (`needed`) and stays alive for the call.
        let ok = unsafe {
            SetupDiGetDeviceInterfaceDetailW(info, &raw const data, Some(detail), needed, None, None)
        };
        if ok.is_err() {
            continue;
        }
        // SAFETY: on success DevicePath holds a NUL-terminated UTF-16 string inside `buf`.
        let path = unsafe { PCWSTR((&raw const (*detail).DevicePath).cast()).to_string() };
        if let Ok(path) = path {
            paths.push(path);
        }
    }
    // SAFETY: `info` came from SetupDiGetClassDevsW and is destroyed exactly once.
    let _ = unsafe { SetupDiDestroyDeviceInfoList(info) };
    Ok(paths)
}

/// Preparsed data of an open handle, freed on drop.
struct Preparsed(PHIDP_PREPARSED_DATA);

impl Preparsed {
    fn get(handle: &OwnedHandle) -> Option<Self> {
        let mut pre = PHIDP_PREPARSED_DATA::default();
        // SAFETY: the handle is open; on success the preparsed data is owned by us and freed in Drop.
        unsafe { HidD_GetPreparsedData(handle.raw(), &raw mut pre) }.then_some(Preparsed(pre))
    }
}

impl Drop for Preparsed {
    fn drop(&mut self) {
        // SAFETY: the pointer came from HidD_GetPreparsedData and is freed exactly once.
        let _ = unsafe { HidD_FreePreparsedData(self.0) };
    }
}

pub(crate) fn describe(handle: &OwnedHandle, path: String) -> Option<DeviceInfo> {
    let mut attrs = HIDD_ATTRIBUTES {
        Size: u32::try_from(size_of::<HIDD_ATTRIBUTES>()).unwrap_or(0),
        ..Default::default()
    };
    // SAFETY: the handle is open and `attrs.Size` is set as the API requires.
    if !unsafe { HidD_GetAttributes(handle.raw(), &raw mut attrs) } {
        return None;
    }
    let pre = Preparsed::get(handle)?;
    let mut caps = HIDP_CAPS::default();
    // SAFETY: valid preparsed data and an output structure we own.
    if unsafe { HidP_GetCaps(pre.0, &raw mut caps) } != HIDP_STATUS_SUCCESS {
        return None;
    }
    Some(DeviceInfo {
        path,
        vendor_id: attrs.VendorID,
        product_id: attrs.ProductID,
        version: attrs.VersionNumber,
        usage_page: caps.UsagePage,
        usage: caps.Usage,
        input_len: caps.InputReportByteLength,
        output_len: caps.OutputReportByteLength,
        feature_len: caps.FeatureReportByteLength,
    })
}

/// Logical and usage ranges of one input field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValueRange {
    pub usage_page: u16,
    pub usage_min: u16,
    pub usage_max: u16,
    pub logical_min: i32,
    pub logical_max: i32,
    pub is_button: bool,
    /// HID main item flags of the field (bit 1 set = Variable, clear = Array).
    pub bit_field: u16,
    pub report_count: u16,
}

/// Input field ranges of the System Control collection (usage page 1, usage 0x80) of `vendor_id`'s devices.
/// Tells which byte values a type-4 slot can carry. Reads the report descriptor only; no report is sent.
pub fn system_control_ranges(vendor_id: u16) -> Result<Vec<(DeviceInfo, Vec<ValueRange>)>, EnumerateError> {
    let mut out = Vec::new();
    for path in interface_paths()? {
        let Ok(handle) = OwnedHandle::open_zero_access(&path) else {
            continue;
        };
        let Some(info) = describe(&handle, path) else {
            continue;
        };
        if info.vendor_id != vendor_id || info.usage_page != 1 || info.usage != 0x80 {
            continue;
        }
        let Some(pre) = Preparsed::get(&handle) else {
            continue;
        };
        out.push((info, input_ranges(&pre)));
    }
    Ok(out)
}

fn input_ranges(pre: &Preparsed) -> Vec<ValueRange> {
    let mut ranges = Vec::new();
    let mut caps = HIDP_CAPS::default();
    // SAFETY: valid preparsed data and an output structure we own.
    if unsafe { HidP_GetCaps(pre.0, &raw mut caps) } != HIDP_STATUS_SUCCESS {
        return ranges;
    }
    let mut n = caps.NumberInputValueCaps;
    let mut values = vec![HIDP_VALUE_CAPS::default(); usize::from(n)];
    let status = if n > 0 {
        // SAFETY: `values` has room for `n` entries; the API writes at most `n` and updates `n`.
        unsafe { HidP_GetValueCaps(HidP_Input, values.as_mut_ptr(), &raw mut n, pre.0) }
    } else {
        HIDP_STATUS_SUCCESS
    };
    if status == HIDP_STATUS_SUCCESS {
        for v in &values[..usize::from(n)] {
            // SAFETY: IsRange selects which union member the API filled.
            let (lo, hi) = unsafe {
                if v.IsRange {
                    (v.Anonymous.Range.UsageMin, v.Anonymous.Range.UsageMax)
                } else {
                    (v.Anonymous.NotRange.Usage, v.Anonymous.NotRange.Usage)
                }
            };
            ranges.push(ValueRange {
                usage_page: v.UsagePage,
                usage_min: lo,
                usage_max: hi,
                logical_min: v.LogicalMin,
                logical_max: v.LogicalMax,
                is_button: false,
                bit_field: v.BitField,
                report_count: v.ReportCount,
            });
        }
    }
    let mut n = caps.NumberInputButtonCaps;
    let mut buttons = vec![HIDP_BUTTON_CAPS::default(); usize::from(n)];
    let status = if n > 0 {
        // SAFETY: as above, for button capabilities.
        unsafe { HidP_GetButtonCaps(HidP_Input, buttons.as_mut_ptr(), &raw mut n, pre.0) }
    } else {
        HIDP_STATUS_SUCCESS
    };
    if status == HIDP_STATUS_SUCCESS {
        for b in &buttons[..usize::from(n)] {
            // SAFETY: IsRange selects which union member the API filled.
            let (lo, hi) = unsafe {
                if b.IsRange {
                    (b.Anonymous.Range.UsageMin, b.Anonymous.Range.UsageMax)
                } else {
                    (b.Anonymous.NotRange.Usage, b.Anonymous.NotRange.Usage)
                }
            };
            ranges.push(ValueRange {
                usage_page: b.UsagePage,
                usage_min: lo,
                usage_max: hi,
                logical_min: 0,
                logical_max: 1,
                is_button: true,
                bit_field: b.BitField,
                report_count: b.ReportCount,
            });
        }
    }
    ranges
}
