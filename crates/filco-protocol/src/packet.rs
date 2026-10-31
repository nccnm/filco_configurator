//! Building and checking individual reports. Byte positions follow `fnmap.py` `Keyboard._command`.

/// Feature report length including the report id byte.
pub const REPORT_LEN: usize = 65;
/// Data bytes carried by one report while reading or writing a page.
pub const CHUNK_LEN: usize = 64;
pub const CHUNKS_PER_PAGE: usize = 8;

pub const CMD_FINISH: u8 = 0x02;
pub const CMD_READ_PAGE: u8 = 0x10;
pub const CMD_WRITE_PAGE: u8 = 0x11;

/// Parameter length byte `[9]` of each command.
pub const LEN_PAGE: u8 = 8;
pub const LEN_FINISH: u8 = 0;

const MARKER: u8 = 0x04;
const POS_MARKER: usize = 1;
const POS_CMD: usize = 2;
const POS_ACK: usize = 4;
const POS_SUM: usize = 5;
const POS_LEN: usize = 9;
const POS_LAYER: usize = 0xb;

/// One feature report, report id included.
pub type Report = [u8; REPORT_LEN];

/// A command packet: `[1] = 0x04, [2] = cmd, [9] = len`, and `[0xb] = layer` for page commands.
pub fn command_packet(cmd: u8, len: u8, layer: Option<u8>) -> Report {
    let mut p = [0u8; REPORT_LEN];
    p[POS_MARKER] = MARKER;
    p[POS_CMD] = cmd;
    p[POS_LEN] = len;
    if let Some(layer) = layer {
        p[POS_LAYER] = layer;
    }
    p
}

/// A data packet of a page write: report id 0 then 64 data bytes.
pub fn data_packet(chunk: &[u8; CHUNK_LEN]) -> Report {
    let mut p = [0u8; REPORT_LEN];
    p[1..].copy_from_slice(chunk);
    p
}

/// Sum of all 65 bytes of a report, as the device and `fnmap.py` add them into the session checksum.
pub fn report_sum(report: &Report) -> u32 {
    report.iter().map(|b| u32::from(*b)).sum()
}

/// A device acknowledgement that passed [`check_ack`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ack {
    /// `[5..9]` little-endian; the device checksum in the reply to 0x02.
    pub value: u32,
}

/// Checks a reply the way FILCO Assist's `CheckDeviceAckResult` does: `[1] == 4, [2] == cmd, [4] == 1,
/// [9] == len`.
pub fn check_ack(reply: &Report, cmd: u8, len: u8) -> Option<Ack> {
    let ok =
        reply[POS_MARKER] == MARKER && reply[POS_CMD] == cmd && reply[POS_ACK] == 1 && reply[POS_LEN] == len;
    ok.then(|| Ack {
        value: u32::from_le_bytes([
            reply[POS_SUM],
            reply[POS_SUM + 1],
            reply[POS_SUM + 2],
            reply[POS_SUM + 3],
        ]),
    })
}

/// Recognises a command packet built by [`command_packet`]: returns `(cmd, len, layer)`. Every byte that
/// [`command_packet`] leaves zero must be zero.
pub fn parse_command(p: &Report) -> Option<(u8, u8, u8)> {
    if p[0] != 0 || p[POS_MARKER] != MARKER {
        return None;
    }
    let zero_elsewhere = p
        .iter()
        .enumerate()
        .all(|(i, b)| matches!(i, POS_MARKER | POS_CMD | POS_LEN | POS_LAYER) || *b == 0);
    zero_elsewhere.then_some((p[POS_CMD], p[POS_LEN], p[POS_LAYER]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_layout_matches_fnmap() {
        // fnmap.py: pkt[1], pkt[2], pkt[9] = 4, cmd, length; pkt[0xB] = layer.
        let p = command_packet(CMD_READ_PAGE, LEN_PAGE, Some(3));
        let mut want = [0u8; REPORT_LEN];
        want[1] = 4;
        want[2] = 0x10;
        want[9] = 8;
        want[0xb] = 3;
        assert_eq!(p, want);
        assert_eq!(parse_command(&p), Some((0x10, 8, 3)));
        assert_eq!(
            parse_command(&command_packet(CMD_FINISH, LEN_FINISH, None)),
            Some((2, 0, 0))
        );
    }

    #[test]
    fn ack_check() {
        let mut r = [0u8; REPORT_LEN];
        r[1] = 4;
        r[2] = 2;
        r[4] = 1;
        r[5..9].copy_from_slice(&0x0001_2345u32.to_le_bytes());
        assert_eq!(check_ack(&r, 2, 0), Some(Ack { value: 0x12345 }));
        for (i, v) in [(1, 5), (2, 3), (4, 0), (9, 1)] {
            let mut bad = r;
            bad[i] = v;
            assert_eq!(check_ack(&bad, 2, 0), None, "byte {i}");
        }
    }

    #[test]
    fn data_packet_and_sum() {
        let chunk = [0xffu8; CHUNK_LEN];
        let p = data_packet(&chunk);
        assert_eq!(p[0], 0);
        assert_eq!(report_sum(&p), 64 * 255);
        assert_eq!(parse_command(&p), None);
    }

    #[test]
    fn stray_bytes_make_a_packet_not_a_command() {
        let mut p = command_packet(CMD_READ_PAGE, LEN_PAGE, Some(0));
        p[30] = 1;
        assert_eq!(parse_command(&p), None);
    }
}
