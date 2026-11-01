//! Page read and write sessions, report for report like `fnmap.py` `Keyboard.read_page` / `write_page`.

use std::time::Duration;

use filco_core::{PAGE_LEN, Page};

use crate::packet::{
    CHUNK_LEN, CHUNKS_PER_PAGE, CMD_FINISH, CMD_INFO, CMD_READ_PAGE, CMD_WRITE_PAGE, LEN_FINISH, LEN_PAGE,
    Report, check_ack, command_packet, data_packet, report_sum,
};
use crate::transport::{Clock, Transport, TransportError};

/// Pause after every SetFeature and GetFeature (FILCO Assist and `fnmap.py`: 100 ms).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Delays {
    pub read: Duration,
    pub write: Duration,
}

impl Delays {
    pub const FILCO_ASSIST: Delays = Delays {
        read: Duration::from_millis(100),
        write: Duration::from_millis(100),
    };

    pub fn from_ms(read: u32, write: u32) -> Self {
        Delays {
            read: Duration::from_millis(u64::from(read)),
            write: Duration::from_millis(u64::from(write)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Read,
    Write,
    /// Reading back a page just written.
    Verify,
    /// Reading the firmware info block (command 0x30).
    Info,
}

/// Progress of the page or block currently being transferred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    /// Layer being read or written; 0 for the info block, which has no layer.
    pub layer: u8,
    pub phase: Phase,
    /// Reports exchanged so far in this session, out of `total`.
    pub done: u8,
    pub total: u8,
}

/// Reports in one page session: command + ACK, 8 data, finish + ACK.
const REPORTS_PER_SESSION: u8 = 12;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProtocolError {
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error("the keyboard did not acknowledge command {cmd:#04x} (reply starts {got})")]
    NoAck { cmd: u8, got: String },
    #[error("checksum mismatch: keyboard {device:#x}, computer {host:#x}")]
    Checksum { device: u32, host: u32 },
    #[error(
        "layer {layer}: read-back differs from what was written (first difference at byte {first_diff:#05x})"
    )]
    Readback { layer: u8, first_diff: usize },
}

impl ProtocolError {
    /// Errors after which a read may be tried again.
    fn retryable_read(&self) -> bool {
        matches!(self, ProtocolError::NoAck { .. } | ProtocolError::Checksum { .. })
    }
}

/// Retries of a whole page read after a NoAck or checksum error (never for writes).
pub const READ_RETRIES: u32 = 2;

/// Drives the protocol over one transport.
#[derive(Debug)]
pub struct Session<T, C> {
    transport: T,
    clock: C,
    delays: Delays,
    read_retries: u32,
}

impl<T: Transport, C: Clock> Session<T, C> {
    pub fn new(transport: T, clock: C, delays: Delays) -> Self {
        Session {
            transport,
            clock,
            delays,
            read_retries: READ_RETRIES,
        }
    }

    /// Sets how often a failed page read is retried (default [`READ_RETRIES`]); 0 for measurements.
    pub fn set_read_retries(&mut self, retries: u32) {
        self.read_retries = retries;
    }

    /// Changes the pauses, e.g. after measuring the read delay a model tolerates.
    pub fn set_delays(&mut self, delays: Delays) {
        self.delays = delays;
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    pub fn clock(&self) -> &C {
        &self.clock
    }

    pub fn delays(&self) -> Delays {
        self.delays
    }

    pub fn into_parts(self) -> (T, C) {
        (self.transport, self.clock)
    }

    fn set(&mut self, report: &Report, delay: Duration) -> Result<(), ProtocolError> {
        self.transport.set_feature(report)?;
        self.clock.sleep(delay);
        Ok(())
    }

    fn get(&mut self, delay: Duration) -> Result<Report, ProtocolError> {
        let r = self.transport.get_feature()?;
        self.clock.sleep(delay);
        Ok(r)
    }

    /// Command packet then its acknowledgement; returns the ACK's value field.
    fn command(
        &mut self,
        cmd: u8,
        len: u8,
        layer: Option<u8>,
        delay: Duration,
    ) -> Result<u32, ProtocolError> {
        self.set(&command_packet(cmd, len, layer), delay)?;
        let reply = self.get(delay)?;
        check_ack(&reply, cmd, len)
            .map(|a| a.value)
            .ok_or_else(|| ProtocolError::NoAck {
                cmd,
                got: filco_core::hex::encode_spaced(&reply[..12]),
            })
    }

    fn finish(&mut self, host_sum: u32, delay: Duration) -> Result<(), ProtocolError> {
        let device = self.command(CMD_FINISH, LEN_FINISH, None, delay)?;
        if device == host_sum {
            Ok(())
        } else {
            Err(ProtocolError::Checksum {
                device,
                host: host_sum,
            })
        }
    }

    /// One read session (ACK, 8 data reports, finish with checksum) for any read command. `layer` is `Some`
    /// for a page read (0x10) and `None` for the info block (0x30). Returns the 512 raw bytes.
    fn read_session_once(
        &mut self,
        cmd: u8,
        layer: Option<u8>,
        phase: Phase,
        delay: Duration,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<[u8; PAGE_LEN], ProtocolError> {
        let report_layer = layer.unwrap_or(0);
        let mut tick = |done: u8| {
            progress(Progress {
                layer: report_layer,
                phase,
                done,
                total: REPORTS_PER_SESSION,
            });
        };
        let mut sum = 0u32;
        self.command(cmd, LEN_PAGE, layer, delay)?;
        tick(2);
        let mut bytes = [0u8; PAGE_LEN];
        for (i, chunk) in bytes.as_chunks_mut::<CHUNK_LEN>().0.iter_mut().enumerate() {
            let r = self.get(delay)?;
            sum = sum.wrapping_add(report_sum(&r));
            chunk.copy_from_slice(&r[1..]);
            tick(3 + u8::try_from(i).unwrap_or(0));
        }
        self.finish(sum, delay)?;
        tick(REPORTS_PER_SESSION);
        Ok(bytes)
    }

    fn read_page_once(
        &mut self,
        layer: u8,
        phase: Phase,
        delay: Duration,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<Page, ProtocolError> {
        let bytes = self.read_session_once(CMD_READ_PAGE, Some(layer), phase, delay, progress)?;
        Ok(Page::from_bytes(&bytes).unwrap_or_else(|_| Page::empty()))
    }

    /// Reads the firmware info block (command 0x30): the same read session as a page, with no layer. Returns the
    /// 512 raw bytes, or `None` when the firmware does not know the command (it NACKs 0x30) - that is how a host
    /// tells a stock or older-patched image, which has no info block, apart from one that has one. Not retried.
    pub fn read_info(
        &mut self,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<Option<[u8; PAGE_LEN]>, ProtocolError> {
        let delay = self.delays.read;
        match self.read_session_once(CMD_INFO, None, Phase::Info, delay, progress) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(ProtocolError::NoAck { cmd, .. }) if cmd == CMD_INFO => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Reads one layer page: 0x10, 8 data reports, 0x02 with checksum. Retries [`READ_RETRIES`] times after a
    /// missing acknowledgement or a checksum mismatch.
    pub fn read_page(
        &mut self,
        layer: u8,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<Page, ProtocolError> {
        let delay = self.delays.read;
        let mut attempt = 0;
        loop {
            match self.read_page_once(layer, Phase::Read, delay, progress) {
                Err(e) if e.retryable_read() && attempt < self.read_retries => attempt += 1,
                other => return other,
            }
        }
    }

    /// Writes one layer page (0x11, 8 data reports, 0x02 with checksum), then reads it back and compares all
    /// 512 bytes. Never retried: the caller decides what to do after a failure.
    pub fn write_page(
        &mut self,
        layer: u8,
        page: &Page,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<(), ProtocolError> {
        let delay = self.delays.write;
        let mut sum = 0u32;
        self.command(CMD_WRITE_PAGE, LEN_PAGE, Some(layer), delay)?;
        progress(Progress {
            layer,
            phase: Phase::Write,
            done: 2,
            total: REPORTS_PER_SESSION,
        });
        for (i, chunk) in page.as_bytes().as_chunks::<CHUNK_LEN>().0.iter().enumerate() {
            let p = data_packet(chunk);
            self.set(&p, delay)?;
            sum = sum.wrapping_add(report_sum(&p));
            let done = 3 + u8::try_from(i).unwrap_or(0);
            progress(Progress {
                layer,
                phase: Phase::Write,
                done,
                total: REPORTS_PER_SESSION,
            });
        }
        self.finish(sum, delay)?;
        progress(Progress {
            layer,
            phase: Phase::Write,
            done: REPORTS_PER_SESSION,
            total: REPORTS_PER_SESSION,
        });
        // fnmap.py sleeps once more before reading back.
        self.clock.sleep(delay);
        let back = self.read_page_once(layer, Phase::Verify, delay, progress)?;
        match first_difference(page, &back) {
            None => Ok(()),
            Some(first_diff) => Err(ProtocolError::Readback { layer, first_diff }),
        }
    }
}

fn first_difference(a: &Page, b: &Page) -> Option<usize> {
    a.as_bytes().iter().zip(b.as_bytes()).position(|(x, y)| x != y)
}

const _: () = assert!(CHUNK_LEN * CHUNKS_PER_PAGE == PAGE_LEN);
