//! The host side of the configuration protocol on a booted image (port of `emu.HidHost`).
//!
//! Models the USB code of the firmware: SET_REPORT copies the report into the buffer and sets bit 0 of the HID
//! flags; GET_REPORT sends the buffer as it is, then sets bit 1. The main loop runs the HID task 0xa822 until both
//! flags are clear, so it prepares the next reply after each GET. Checked in the analysis repository against
//! the device-proven layer read of `fnmap.py` (ACK, 8 chunks, checksum).

use m0emu::Stop;

use crate::machine::V72;

/// Feature report byte i (report id 0 at i = 0) is RAM `HID_BUF + i`, both directions.
const HID_BUF: u32 = 0x2000_017F;
/// Bit 0: SET_REPORT data received, bit 1: GET_REPORT served.
const HID_FLAGS: u32 = 0x2000_0068;
const HID_TASK: u32 = 0xA822;

/// A host talking to one emulated keyboard.
#[derive(Debug)]
pub struct HidHost {
    pub m: V72,
}

impl HidHost {
    pub fn new(m: V72) -> Self {
        Self { m }
    }

    fn flags(&self) -> u8 {
        self.m.ram_u8(HID_FLAGS)
    }

    fn settle(&mut self) -> Result<(), Stop> {
        for _ in 0..10 {
            if self.flags().trailing_zeros() >= 2 {
                return Ok(());
            }
            self.m.call(HID_TASK)?;
        }
        Err(Stop {
            pc: 0,
            reason: "HID flags stay set".into(),
        })
    }

    pub fn set_feature(&mut self, report: &[u8; 65]) -> Result<(), Stop> {
        self.m.emu.write(HID_BUF + 1, &report[1..65]);
        let f = self.flags() | 1;
        self.m.emu.write(HID_FLAGS, &[f]);
        self.settle()
    }

    pub fn get_feature(&mut self) -> Result<[u8; 65], Stop> {
        let mut report = [0u8; 65];
        if let Some(b) = self.m.emu.mem.read(HID_BUF + 1, 64) {
            report[1..].copy_from_slice(b);
        }
        let f = self.flags() | 2;
        self.m.emu.write(HID_FLAGS, &[f]);
        self.settle()?;
        Ok(report)
    }

    pub fn command(cmd: u8, length: u8, layer: u8) -> [u8; 65] {
        let mut r = [0u8; 65];
        (r[1], r[2], r[9], r[0xB]) = (4, cmd, length, layer);
        r
    }

    /// Sends a command; returns the reply and whether it is an ACK (as `fnmap.py` checks it).
    pub fn ack(&mut self, cmd: u8, length: u8, layer: u8) -> Result<([u8; 65], bool), Stop> {
        self.set_feature(&Self::command(cmd, length, layer))?;
        let r = self.get_feature()?;
        let ok = r[1] == 4 && r[2] == cmd && r[4] == 1 && r[9] == length;
        Ok((r, ok))
    }

    /// A read session: command, chunks, 0x02 with checksum. `None` when the command or the checksum is refused.
    pub fn read(&mut self, cmd: u8, layer: u8, chunks: u8) -> Result<Option<Vec<u8>>, Stop> {
        let (_, ok) = self.ack(cmd, chunks, layer)?;
        if !ok {
            return Ok(None);
        }
        let mut data = Vec::new();
        let mut total = 0u32;
        for _ in 0..chunks {
            let r = self.get_feature()?;
            data.extend_from_slice(&r[1..]);
            total += r.iter().map(|&b| u32::from(b)).sum::<u32>();
        }
        let (fin, ok) = self.ack(0x02, 0, 0)?;
        if !ok || u32::from_le_bytes([fin[5], fin[6], fin[7], fin[8]]) != total {
            return Ok(None);
        }
        Ok(Some(data))
    }
}
