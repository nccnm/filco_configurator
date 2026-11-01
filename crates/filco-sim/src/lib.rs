//! A simulated keyboard behind the [`Transport`] trait.
//!
//! Behaviour is modelled on `fnmap.py`, `FN_ANALYSIS.md` section 2 and FAQ question 6 of the analysis repository:
//! - flash holds one 512-byte page per layer;
//! - a command packet is acknowledged in the next GetFeature (`[1] = 4, [2] = cmd, [4] = 1, [9] = len`);
//! - 0x10 streams the page in 8 GetFeature reports, 0x11 takes 8 SetFeature data reports into a RAM buffer;
//! - 0x02 replies with the sum of the session's 8 data reports (all 65 bytes each) and, for a complete write
//!   session, then stores the buffer in flash.
//!
//! [`Fault`]s break one chosen exchange: unplugging, a wrong acknowledgement, a wrong checksum, a corrupted read
//! report, or flash that stores something else than it received. Every report is recorded in a transcript.
//!
//! This is a model derived from analysis, not the firmware itself.
#![forbid(unsafe_code)]

use filco_core::{PAGE_LEN, Page};
use filco_protocol::{
    CHUNK_LEN, CHUNKS_PER_PAGE, CMD_FINISH, CMD_INFO, CMD_READ_PAGE, CMD_WRITE_PAGE, LEN_PAGE, REPORT_LEN,
    Report, Transport, TransportError, parse_command, report_sum,
};
use serde::{Deserialize, Serialize};

/// Which exchange of which session a fault hits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum Stage {
    /// The command packet (SetFeature) or its acknowledgement (GetFeature).
    Command,
    /// Data report `n` (0..8): GetFeature while reading, SetFeature while writing.
    Data { n: u8 },
    /// The 0x02 finish packet or its acknowledgement.
    Finish,
    /// Storing a written page in flash.
    Commit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaultKind {
    /// The keyboard disappears: this exchange and every later one fail with `Disconnected` until
    /// [`SimKeyboard::reconnect`].
    Disconnect,
    /// The OS reports an I/O error for this one exchange.
    IoError,
    /// The acknowledgement carries a wrong ACK byte.
    WrongAck,
    /// The finish acknowledgement carries a wrong checksum.
    BadChecksum,
    /// One byte of a read data report is flipped.
    CorruptData,
    /// Flash stores the written page with one byte flipped (seen on read-back).
    CorruptCommit,
}

/// One injected fault, used once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fault {
    /// Session command: 0x10 read, 0x11 write.
    pub cmd: u8,
    /// Which session of that command, counted from 0 since the simulator was created.
    pub nth: usize,
    #[serde(flatten)]
    pub stage: Stage,
    pub kind: FaultKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Set,
    Get,
}

#[derive(Clone, Debug)]
struct Current {
    cmd: u8,
    nth: usize,
    layer: usize,
    chunk: usize,
    sum: u32,
    buf: [u8; PAGE_LEN],
}

/// The simulated keyboard.
#[derive(Clone, Debug)]
pub struct SimKeyboard {
    flash: Vec<Page>,
    /// The firmware info block returned by command 0x30. `None` models stock firmware, which NACKs 0x30.
    info: Option<[u8; PAGE_LEN]>,
    current: Option<Current>,
    pending: Option<Report>,
    faults: Vec<Fault>,
    sessions: [usize; 2],
    connected: bool,
    transcript: Vec<(Direction, Report)>,
    record: bool,
    flash_writes: usize,
}

fn ack(cmd: u8, len: u8, value: u32) -> Report {
    let mut r = [0u8; REPORT_LEN];
    r[1] = 4;
    r[2] = cmd;
    r[4] = 1;
    r[5..9].copy_from_slice(&value.to_le_bytes());
    r[9] = len;
    r
}

impl SimKeyboard {
    pub fn new(pages: Vec<Page>) -> Self {
        SimKeyboard {
            flash: pages,
            info: None,
            current: None,
            pending: None,
            faults: Vec::new(),
            sessions: [0; 2],
            connected: true,
            transcript: Vec::new(),
            record: true,
            flash_writes: 0,
        }
    }

    /// A keyboard with `layers` empty pages.
    pub fn blank(layers: usize) -> Self {
        Self::new(vec![Page::empty(); layers])
    }

    /// Give this keyboard a firmware info block, so command 0x30 returns it (a patched image). Without one,
    /// 0x30 is an unknown command and goes unacknowledged, like stock firmware.
    #[must_use]
    pub fn with_info_block(mut self, block: [u8; PAGE_LEN]) -> Self {
        self.info = Some(block);
        self
    }

    /// Whether this keyboard answers command 0x30 with an info block.
    pub fn has_info_block(&self) -> bool {
        self.info.is_some()
    }

    #[must_use]
    pub fn with_fault(mut self, fault: Fault) -> Self {
        self.faults.push(fault);
        self
    }

    pub fn add_fault(&mut self, fault: Fault) {
        self.faults.push(fault);
    }

    pub fn pending_faults(&self) -> &[Fault] {
        &self.faults
    }

    pub fn flash(&self) -> &[Page] {
        &self.flash
    }

    /// Changes flash behind the host's back (another program writing the keyboard).
    pub fn flash_mut(&mut self) -> &mut Vec<Page> {
        &mut self.flash
    }

    /// Sessions of `cmd` (0x10 read, 0x11 write) started so far, i.e. the `nth` the next one will have.
    pub fn sessions(&self, cmd: u8) -> usize {
        Self::session_index(cmd).map_or(0, |i| self.sessions[i])
    }

    /// Number of pages stored in flash so far.
    pub fn flash_writes(&self) -> usize {
        self.flash_writes
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Plugs the keyboard back in. Any session in progress is lost, like on a real power cycle.
    pub fn reconnect(&mut self) {
        self.connected = true;
        self.current = None;
        self.pending = None;
    }

    pub fn unplug(&mut self) {
        self.connected = false;
    }

    pub fn transcript(&self) -> &[(Direction, Report)] {
        &self.transcript
    }

    pub fn clear_transcript(&mut self) {
        self.transcript.clear();
    }

    pub fn set_recording(&mut self, on: bool) {
        self.record = on;
    }

    fn session_index(cmd: u8) -> Option<usize> {
        match cmd {
            CMD_READ_PAGE => Some(0),
            CMD_WRITE_PAGE => Some(1),
            _ => None,
        }
    }

    /// Takes the fault for `stage` of the current session if one is armed and its kind is in `kinds`.
    fn take_fault(&mut self, stage: Stage, kinds: &[FaultKind]) -> Option<FaultKind> {
        let cur = self.current.as_ref()?;
        let pos = self.faults.iter().position(|f| {
            f.cmd == cur.cmd && f.nth == cur.nth && f.stage == stage && kinds.contains(&f.kind)
        })?;
        Some(self.faults.remove(pos).kind)
    }

    /// Faults that make the exchange itself fail.
    fn exchange_fault(&mut self, stage: Stage) -> Result<(), TransportError> {
        match self.take_fault(stage, &[FaultKind::Disconnect, FaultKind::IoError]) {
            Some(FaultKind::Disconnect) => {
                self.connected = false;
                Err(TransportError::Disconnected)
            }
            Some(_) => Err(TransportError::Io {
                code: 31,
                message: "simulated I/O error".into(),
            }),
            None => Ok(()),
        }
    }

    fn stage_of_pending(&self) -> Stage {
        match &self.current {
            Some(c) if c.chunk == usize::MAX => Stage::Finish,
            _ => Stage::Command,
        }
    }

    fn on_command(&mut self, cmd: u8, len: u8, layer: u8) -> Result<(), TransportError> {
        if cmd == CMD_FINISH {
            if self.current.is_some() {
                self.exchange_fault(Stage::Finish)?;
            }
            let Some(mut cur) = self.current.take() else {
                self.pending = Some(ack(CMD_FINISH, len, 0));
                return Ok(());
            };
            let mut value = cur.sum;
            let complete_write = cur.cmd == CMD_WRITE_PAGE && cur.chunk == CHUNKS_PER_PAGE;
            cur.chunk = usize::MAX; // marks "finishing" for faults on the acknowledgement
            self.current = Some(cur);
            let mut reply = ack(CMD_FINISH, len, 0);
            if self
                .take_fault(Stage::Finish, &[FaultKind::BadChecksum])
                .is_some()
            {
                value = value.wrapping_add(1);
            }
            reply[5..9].copy_from_slice(&value.to_le_bytes());
            if complete_write {
                self.commit()?;
            }
            self.pending = Some(reply);
            return Ok(());
        }
        if cmd == CMD_INFO {
            // 0x30 reads the info block: a read session with no layer. Without a block (stock firmware) the
            // command is unknown and goes unacknowledged.
            if len != LEN_PAGE || self.info.is_none() {
                self.current = None;
                self.pending = Some([0u8; REPORT_LEN]);
                return Ok(());
            }
            self.current = Some(Current {
                cmd,
                nth: 0,
                layer: 0,
                chunk: 0,
                sum: 0,
                buf: [0; PAGE_LEN],
            });
            self.exchange_fault(Stage::Command)?;
            self.pending = Some(ack(cmd, len, 0));
            return Ok(());
        }
        let Some(si) = Self::session_index(cmd).filter(|_| len == LEN_PAGE) else {
            // Unknown command: no acknowledgement.
            self.current = None;
            self.pending = Some([0u8; REPORT_LEN]);
            return Ok(());
        };
        let nth = self.sessions[si];
        self.sessions[si] += 1;
        self.current = Some(Current {
            cmd,
            nth,
            layer: usize::from(layer),
            chunk: 0,
            sum: 0,
            buf: [0; PAGE_LEN],
        });
        self.exchange_fault(Stage::Command)?;
        if usize::from(layer) >= self.flash.len() {
            self.current = None;
            self.pending = Some([0u8; REPORT_LEN]);
            return Ok(());
        }
        self.pending = Some(ack(cmd, len, 0));
        Ok(())
    }

    fn commit(&mut self) -> Result<(), TransportError> {
        self.exchange_fault(Stage::Commit)?;
        let corrupt = self
            .take_fault(Stage::Commit, &[FaultKind::CorruptCommit])
            .is_some();
        let Some(cur) = &self.current else { return Ok(()) };
        let mut buf = cur.buf;
        if corrupt {
            buf[PAGE_LEN / 2] ^= 0x01;
        }
        let layer = cur.layer;
        self.flash[layer] = Page::from_bytes(&buf).unwrap_or_else(|_| Page::empty());
        self.flash_writes += 1;
        Ok(())
    }

    fn rec(&mut self, d: Direction, r: &Report) {
        if self.record {
            self.transcript.push((d, *r));
        }
    }
}

impl Transport for SimKeyboard {
    fn set_feature(&mut self, report: &Report) -> Result<(), TransportError> {
        if !self.connected {
            return Err(TransportError::Disconnected);
        }
        self.rec(Direction::Set, report);
        if let Some((cmd, len, layer)) = parse_command(report) {
            return self.on_command(cmd, len, layer);
        }
        // Data report of a write session.
        let Some(cur) = self.current.as_ref() else {
            return Ok(());
        };
        if cur.cmd != CMD_WRITE_PAGE || cur.chunk >= CHUNKS_PER_PAGE {
            return Ok(());
        }
        let n = u8::try_from(cur.chunk).unwrap_or(u8::MAX);
        self.exchange_fault(Stage::Data { n })?;
        if let Some(cur) = self.current.as_mut() {
            let off = cur.chunk * CHUNK_LEN;
            cur.buf[off..off + CHUNK_LEN].copy_from_slice(&report[1..]);
            cur.sum = cur.sum.wrapping_add(report_sum(report));
            cur.chunk += 1;
        }
        Ok(())
    }

    fn get_feature(&mut self) -> Result<Report, TransportError> {
        if !self.connected {
            return Err(TransportError::Disconnected);
        }
        if let Some(mut reply) = self.pending.take() {
            let stage = self.stage_of_pending();
            self.exchange_fault(stage)?;
            if self.take_fault(stage, &[FaultKind::WrongAck]).is_some() {
                reply[4] = 0;
            }
            if stage == Stage::Finish {
                self.current = None;
            }
            self.rec(Direction::Get, &reply);
            return Ok(reply);
        }
        let streaming = matches!(&self.current,
            Some(c) if (c.cmd == CMD_READ_PAGE || c.cmd == CMD_INFO) && c.chunk < CHUNKS_PER_PAGE);
        if !streaming {
            let r = [0u8; REPORT_LEN];
            self.rec(Direction::Get, &r);
            return Ok(r);
        }
        let n = self
            .current
            .as_ref()
            .map_or(0, |c| u8::try_from(c.chunk).unwrap_or(0));
        self.exchange_fault(Stage::Data { n })?;
        let corrupt = self
            .take_fault(Stage::Data { n }, &[FaultKind::CorruptData])
            .is_some();
        let mut r = [0u8; REPORT_LEN];
        if let Some(cur) = self.current.as_ref() {
            let off = cur.chunk * CHUNK_LEN;
            let src = match cur.cmd {
                CMD_INFO => &self.info.as_ref().expect("info session needs a block")[off..off + CHUNK_LEN],
                _ => &self.flash[cur.layer].as_bytes()[off..off + CHUNK_LEN],
            };
            r[1..].copy_from_slice(src);
        }
        if let Some(cur) = self.current.as_mut() {
            // The device sums what it sends; a corrupted report changes on the way, after the sum.
            cur.sum = cur.sum.wrapping_add(report_sum(&r));
            cur.chunk += 1;
        }
        if corrupt {
            r[1] ^= 0x80;
        }
        self.rec(Direction::Get, &r);
        Ok(r)
    }
}
