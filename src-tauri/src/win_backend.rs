//! The real keyboard, through the Windows HID stack.

use filco_hid_windows::WritePolicy;
use filco_protocol::{SystemClock, TransportError};
use filco_service::backend::{Backend, BoxClock, BoxTransport};
use filco_service::discovery::InterfaceInfo;
use filco_service::guard::ProcessProbe;

#[derive(Debug, Default)]
pub struct WindowsBackend;

impl ProcessProbe for WindowsBackend {
    fn running(&self) -> Result<Vec<String>, String> {
        filco_hid_windows::running_processes()
            .map(|v| v.into_iter().map(|p| p.exe).collect())
            .map_err(|e| e.to_string())
    }
}

impl Backend for WindowsBackend {
    fn interfaces(&self) -> Result<Vec<InterfaceInfo>, String> {
        Ok(filco_hid_windows::enumerate()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|d| InterfaceInfo {
                path: d.path,
                vendor_id: d.vendor_id,
                product_id: d.product_id,
                version: d.version,
                usage_page: d.usage_page,
                input_len: d.input_len,
                output_len: d.output_len,
                feature_len: d.feature_len,
            })
            .collect())
    }

    /// The raw device; [`filco_service::device::Device`] puts its own read-only policy in front of it.
    fn open(&self, path: &str) -> Result<BoxTransport, TransportError> {
        Ok(Box::new(
            filco_hid_windows::open(path, WritePolicy::ReadOnly)?.into_inner(),
        ))
    }

    fn clock(&self) -> BoxClock {
        Box::new(SystemClock)
    }
}
