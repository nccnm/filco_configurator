//! Where keyboards come from: the Windows HID stack in the app, a simulated keyboard in tests and in the
//! `e2e-sim` build. The backend hands out raw transports; [`crate::device::Device`] always wraps them in a
//! read-only [`filco_protocol::PolicyTransport`].

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use filco_protocol::{Clock, Report, Transport, TransportError};
use filco_sim::SimKeyboard;

use crate::discovery::InterfaceInfo;
use crate::guard::ProcessProbe;

pub type BoxTransport = Box<dyn Transport + Send>;
pub type BoxClock = Box<dyn Clock + Send>;

pub trait Backend: ProcessProbe + Send + Sync {
    /// All HID interfaces, configuration or not; [`crate::discovery::find_keyboards`] picks.
    fn interfaces(&self) -> Result<Vec<InterfaceInfo>, String>;
    fn open(&self, path: &str) -> Result<BoxTransport, TransportError>;
    fn clock(&self) -> BoxClock;
}

/// A simulated M10SP US behind a shared lock, so tests (and the E2E build) can unplug it, inject faults and
/// look at its flash while the app holds it.
#[derive(Clone, Debug)]
pub struct SimBackend {
    pub keyboard: Arc<Mutex<SimKeyboard>>,
    pub info: InterfaceInfo,
    pub processes: Arc<Mutex<Vec<String>>>,
    /// Real time per simulated pause: zero for unit tests, a little for E2E so progress is visible.
    pub pace: Option<Duration>,
}

impl SimBackend {
    pub fn new(keyboard: SimKeyboard) -> Self {
        SimBackend {
            keyboard: Arc::new(Mutex::new(keyboard)),
            info: InterfaceInfo {
                path: r"sim:\m10sp-us".into(),
                vendor_id: 0x2efd,
                product_id: 0x1972,
                version: 0x0072,
                usage_page: 1,
                input_len: 9,
                output_len: 2,
                feature_len: 65,
            },
            processes: Arc::new(Mutex::new(vec!["explorer.exe".into()])),
            pace: None,
        }
    }

    /// Runs `f` on the simulated keyboard.
    pub fn with<R>(&self, f: impl FnOnce(&mut SimKeyboard) -> R) -> R {
        f(&mut self.keyboard.lock().unwrap_or_else(PoisonError::into_inner))
    }

    pub fn set_processes(&self, names: &[&str]) {
        *self.processes.lock().unwrap_or_else(PoisonError::into_inner) =
            names.iter().map(|s| (*s).to_owned()).collect();
    }
}

impl ProcessProbe for SimBackend {
    fn running(&self) -> Result<Vec<String>, String> {
        Ok(self
            .processes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone())
    }
}

struct SharedSim(Arc<Mutex<SimKeyboard>>);

impl Transport for SharedSim {
    fn set_feature(&mut self, report: &Report) -> Result<(), TransportError> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .set_feature(report)
    }

    fn get_feature(&mut self) -> Result<Report, TransportError> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_feature()
    }
}

struct PacedClock(Option<Duration>);

impl Clock for PacedClock {
    fn sleep(&mut self, _: Duration) {
        if let Some(d) = self.0 {
            std::thread::sleep(d);
        }
    }
}

impl Backend for SimBackend {
    fn interfaces(&self) -> Result<Vec<InterfaceInfo>, String> {
        Ok(if self.with(|k| k.is_connected()) {
            vec![self.info.clone()]
        } else {
            Vec::new()
        })
    }

    fn open(&self, path: &str) -> Result<BoxTransport, TransportError> {
        if path != self.info.path || !self.with(|k| k.is_connected()) {
            return Err(TransportError::Disconnected);
        }
        Ok(Box::new(SharedSim(Arc::clone(&self.keyboard))))
    }

    fn clock(&self) -> BoxClock {
        Box::new(PacedClock(self.pace))
    }
}
