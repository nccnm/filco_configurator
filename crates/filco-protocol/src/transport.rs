//! What the protocol needs from a device and from time.

use std::time::Duration;

use crate::packet::Report;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransportError {
    /// The device is gone (unplugged, or the handle became invalid).
    #[error("the keyboard was disconnected")]
    Disconnected,
    /// The operating system refused the request.
    #[error("HID request failed: {message} (code {code})")]
    Io { code: u32, message: String },
    /// A [`crate::WritePolicy`] refused to send the report; nothing reached the device.
    #[error("refused by write policy: {0}")]
    Refused(String),
}

/// A HID device that exchanges 65-byte feature reports.
pub trait Transport {
    fn set_feature(&mut self, report: &Report) -> Result<(), TransportError>;
    fn get_feature(&mut self) -> Result<Report, TransportError>;
}

impl<T: Transport + ?Sized> Transport for &mut T {
    fn set_feature(&mut self, report: &Report) -> Result<(), TransportError> {
        (**self).set_feature(report)
    }

    fn get_feature(&mut self) -> Result<Report, TransportError> {
        (**self).get_feature()
    }
}

impl<T: Transport + ?Sized> Transport for Box<T> {
    fn set_feature(&mut self, report: &Report) -> Result<(), TransportError> {
        (**self).set_feature(report)
    }

    fn get_feature(&mut self) -> Result<Report, TransportError> {
        (**self).get_feature()
    }
}

/// Pauses between reports.
pub trait Clock {
    fn sleep(&mut self, d: Duration);
}

/// Real time.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn sleep(&mut self, d: Duration) {
        if !d.is_zero() {
            std::thread::sleep(d);
        }
    }
}

/// Test time: sleeping only adds to a counter, so tests are fast and never depend on the machine's speed.
#[derive(Debug, Default, Clone, Copy)]
pub struct VirtualClock {
    pub elapsed: Duration,
    pub sleeps: u32,
}

impl Clock for VirtualClock {
    fn sleep(&mut self, d: Duration) {
        self.elapsed += d;
        self.sleeps += 1;
    }
}

impl<C: Clock + ?Sized> Clock for &mut C {
    fn sleep(&mut self, d: Duration) {
        (**self).sleep(d);
    }
}
