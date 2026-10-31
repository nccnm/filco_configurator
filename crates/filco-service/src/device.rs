//! An opened keyboard. Reads are always allowed; the transport is read-only except inside
//! [`Device::with_writes`], which puts it back to read-only whatever happens.

use filco_core::{LayerSet, ModelDescriptor};
use filco_protocol::{
    Delays, PolicyTransport, Progress, ProtocolError, Session, TransportError, WritePolicy,
};

use crate::backend::{Backend, BoxClock, BoxTransport};
use crate::discovery::InterfaceInfo;

pub type DeviceSession = Session<PolicyTransport<BoxTransport>, BoxClock>;

pub struct Device {
    info: InterfaceInfo,
    layers: usize,
    session: DeviceSession,
}

impl std::fmt::Debug for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Device")
            .field("info", &self.info)
            .field("policy", &self.policy())
            .finish_non_exhaustive()
    }
}

impl Device {
    /// Opens `info` with the delays of `model` (read delay measured per model; others use 100 ms).
    pub fn open(
        backend: &dyn Backend,
        info: &InterfaceInfo,
        model: &ModelDescriptor,
    ) -> Result<Self, TransportError> {
        let raw = backend.open(&info.path)?;
        let d = &model.protocol.delay_ms;
        Ok(Device {
            info: info.clone(),
            layers: model.layers.len(),
            session: Session::new(
                PolicyTransport::new(raw, WritePolicy::ReadOnly),
                backend.clock(),
                Delays::from_ms(d.read, d.write),
            ),
        })
    }

    pub fn info(&self) -> &InterfaceInfo {
        &self.info
    }

    pub fn policy(&self) -> WritePolicy {
        self.session.transport().policy()
    }

    /// Reads every layer page.
    pub fn read_all(&mut self, progress: &mut dyn FnMut(Progress)) -> Result<LayerSet, ProtocolError> {
        let mut pages = Vec::with_capacity(self.layers);
        for layer in 0..self.layers {
            let l = u8::try_from(layer).unwrap_or(u8::MAX);
            pages.push(self.session.read_page(l, progress)?);
        }
        Ok(LayerSet::new(pages))
    }

    /// Runs `f` with writes allowed, then makes the transport read-only again.
    pub(crate) fn with_writes<R>(&mut self, f: impl FnOnce(&mut DeviceSession) -> R) -> R {
        self.session.transport_mut().set_policy(WritePolicy::ReadWrite);
        let r = f(&mut self.session);
        self.session.transport_mut().set_policy(WritePolicy::ReadOnly);
        r
    }
}
