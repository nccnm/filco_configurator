//! The feature-report protocol FILCO Assist 2.0.97 uses to read and write layer pages (`CostarKBHID`), as
//! implemented by `fnmap.py` of the analysis repository and verified on a real M10SP.
//!
//! Every report is 65 bytes, byte 0 being the report id (0). A command packet carries `[1] = 0x04`,
//! `[2] = command`, `[9] = parameter length` and `[0xb] = layer`; the device acknowledges it in the next
//! GetFeature with `[1] = 4, [2] = command, [4] = 1, [9] = length`.
//!
//! - Read page (0x10, len 8): ACK, then 8 GetFeature of 64 data bytes (`[1..65]`).
//! - Write page (0x11, len 8): ACK, then 8 SetFeature `[0] + 64 data bytes`.
//! - Finish (0x02, len 0): ACK carrying the device's 32-bit checksum at `[5..9]` (little-endian), which must equal
//!   the sum of all 65 bytes of the 8 data reports of the session.
//!
//! The protocol is generic over a [`Transport`] (real HID device or simulator) and a [`Clock`] (real sleeps or a
//! virtual clock in tests). [`PolicyTransport`] refuses, before anything reaches the device, every report a
//! [`WritePolicy`] does not allow.
#![forbid(unsafe_code)]

mod packet;
mod policy;
mod session;
mod transport;

pub use packet::{
    Ack, CHUNK_LEN, CHUNKS_PER_PAGE, CMD_FINISH, CMD_READ_PAGE, CMD_WRITE_PAGE, LEN_FINISH, LEN_PAGE,
    REPORT_LEN, Report, check_ack, command_packet, data_packet, parse_command, report_sum,
};
pub use policy::{PolicyTransport, WritePolicy};
pub use session::{Delays, Phase, Progress, ProtocolError, Session};
pub use transport::{Clock, SystemClock, Transport, TransportError, VirtualClock};
