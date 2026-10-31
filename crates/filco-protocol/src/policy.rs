//! A guard in front of a transport that refuses every report the current policy does not allow.

use crate::packet::{CMD_FINISH, CMD_READ_PAGE, LEN_FINISH, LEN_PAGE, Report, parse_command};
use crate::transport::{Transport, TransportError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WritePolicy {
    /// Only the command packets of a page read session may be sent: 0x10 (read page, len 8, layer) and 0x02
    /// (finish, len 0). Data packets and every other command are refused before reaching the device.
    ReadOnly,
    /// Everything the protocol sends is allowed. Used only while a confirmed write is in progress.
    ReadWrite,
}

/// Wraps a transport and enforces a [`WritePolicy`] on `set_feature`.
#[derive(Debug)]
pub struct PolicyTransport<T> {
    inner: T,
    policy: WritePolicy,
    refused: u32,
}

impl<T: Transport> PolicyTransport<T> {
    pub fn new(inner: T, policy: WritePolicy) -> Self {
        PolicyTransport {
            inner,
            policy,
            refused: 0,
        }
    }

    pub fn policy(&self) -> WritePolicy {
        self.policy
    }

    pub fn set_policy(&mut self, policy: WritePolicy) {
        self.policy = policy;
    }

    /// How many reports were refused so far.
    pub fn refused(&self) -> u32 {
        self.refused
    }

    pub fn inner(&self) -> &T {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    pub fn into_inner(self) -> T {
        self.inner
    }

    fn allowed(&self, report: &Report) -> Result<(), String> {
        match self.policy {
            WritePolicy::ReadWrite => Ok(()),
            WritePolicy::ReadOnly => match parse_command(report) {
                Some((CMD_READ_PAGE, LEN_PAGE, _) | (CMD_FINISH, LEN_FINISH, 0)) => Ok(()),
                Some((cmd, len, layer)) => Err(format!(
                    "command {cmd:#04x} (len {len}, layer {layer}) is not allowed in read-only mode"
                )),
                None => Err("data reports are not allowed in read-only mode".into()),
            },
        }
    }
}

impl<T: Transport> Transport for PolicyTransport<T> {
    fn set_feature(&mut self, report: &Report) -> Result<(), TransportError> {
        if let Err(why) = self.allowed(report) {
            self.refused += 1;
            return Err(TransportError::Refused(why));
        }
        self.inner.set_feature(report)
    }

    fn get_feature(&mut self) -> Result<Report, TransportError> {
        self.inner.get_feature()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::{CMD_WRITE_PAGE, REPORT_LEN, command_packet, data_packet};

    #[derive(Default)]
    struct Recorder(Vec<Report>);

    impl Transport for Recorder {
        fn set_feature(&mut self, report: &Report) -> Result<(), TransportError> {
            self.0.push(*report);
            Ok(())
        }

        fn get_feature(&mut self) -> Result<Report, TransportError> {
            Ok([0; REPORT_LEN])
        }
    }

    #[test]
    fn read_only_allows_exactly_the_read_session_commands() {
        let mut t = PolicyTransport::new(Recorder::default(), WritePolicy::ReadOnly);
        for layer in 0..4 {
            t.set_feature(&command_packet(CMD_READ_PAGE, LEN_PAGE, Some(layer)))
                .unwrap();
        }
        t.set_feature(&command_packet(CMD_FINISH, LEN_FINISH, None))
            .unwrap();
        assert_eq!(t.inner().0.len(), 5);

        let refused = [
            command_packet(CMD_WRITE_PAGE, LEN_PAGE, Some(0)),
            command_packet(0x15, 5, None),
            command_packet(0x16, 2, None),
            command_packet(0x17, 2, None),
            command_packet(0x06, 1, None),
            command_packet(0x05, 1, None),
            command_packet(CMD_READ_PAGE, 7, Some(0)),
            command_packet(CMD_FINISH, LEN_FINISH, Some(1)),
            data_packet(&[0u8; 64]),
            data_packet(&[0xa5; 64]),
        ];
        for r in &refused {
            assert!(matches!(t.set_feature(r), Err(TransportError::Refused(_))));
        }
        assert_eq!(t.inner().0.len(), 5, "nothing refused reached the device");
        assert_eq!(t.refused(), u32::try_from(refused.len()).unwrap());

        t.set_policy(WritePolicy::ReadWrite);
        t.set_feature(&command_packet(CMD_WRITE_PAGE, LEN_PAGE, Some(0)))
            .unwrap();
        assert_eq!(t.inner().0.len(), 6);
    }

    #[test]
    fn every_command_byte_other_than_read_and_finish_is_refused() {
        let mut t = PolicyTransport::new(Recorder::default(), WritePolicy::ReadOnly);
        for cmd in 0..=255u8 {
            for len in [0u8, 1, 2, 4, 5, 8] {
                let ok = t.set_feature(&command_packet(cmd, len, Some(0))).is_ok();
                assert_eq!(
                    ok,
                    (cmd, len) == (CMD_READ_PAGE, LEN_PAGE) || (cmd, len) == (CMD_FINISH, LEN_FINISH)
                );
            }
        }
    }
}
