//! Build a customised V72 image for the FILCO Majestouch Xacro M10SP US from the stock image.
//!
//! Port of the analysis repository's `flash_tool` (`hexfile.py`, `v72.py`, `taphold.py`, `fwinfo.py` and the
//! patch part of `build.py`), proven against its golden builds byte for byte: same image, same HEX, same
//! checksum tag, same patch log. Steps:
//!
//! 1. Parse the stock HEX and check the image's SHA-256 ([`v72::Firmware::new`]).
//! 2. Apply the [`config::Config`]: Fn-layer and keymap table edits, then the tap-hold code, then the info
//!    block and its HID command.
//! 3. Write the HEX like the stock one ([`hex::HexFile::write_like`]) under the name that carries the tag.
//!
//! Testing the result in an emulator is `filco-fwtest`; making the updater is `filco-updater`.
#![forbid(unsafe_code)]

pub mod config;
pub mod hex;
pub mod info;
pub mod layertap;
pub mod taphold;
pub mod v72;

use thiserror::Error;

pub use config::Config;
pub use hex::{HexFile, Image, word_sum};
pub use v72::{Change, Firmware};

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Config(String),
    #[error("input is not the stock V72 image (sha256 mismatch); always build from stock")]
    NotStock,
    #[error("stock HEX file sha256 mismatch")]
    NotStockHex,
    #[error("{0}")]
    Structure(String),
    #[error("assembler: {0}")]
    Asm(String),
    #[error(transparent)]
    Hex(#[from] hex::HexError),
}

/// Refuses anything a Cortex-M0 cannot run: every instruction must decode as ARMv6-M, 16-bit or `bl`.
pub(crate) fn check_m0(text: &[u8], base: u32) -> Result<(), Error> {
    let mut pos = 0;
    while pos < text.len() {
        let insn = thumb::decode::decode_at(text, pos);
        match insn {
            Some(i) if i.size == 2 || matches!(i.op, thumb::Op::Bl { .. }) => pos += usize::from(i.size),
            _ => {
                return Err(Error::Asm(format!(
                    "not Cortex-M0 code at {:#x}",
                    base as usize + pos
                )));
            }
        }
    }
    Ok(())
}

/// Report lines as the text files of the analysis repository hold them: CRLF after every line (Python text mode
/// on Windows), so the reports compare byte for byte with the golden ones.
pub fn crlf_text(lines: &[String]) -> String {
    let mut s = lines.join("\r\n");
    s.push_str("\r\n");
    s
}

/// The stock HEX and updater shipped in `firmware/` of this repository.
pub const STOCK_HEX_NAME: &str = "CT_F72US[0xF448]_20240125_0v72_PID1972.hex";
pub const STOCK_UPDATER_NAME: &str = "M10SP_72US_V72_stock.exe";

/// A finished image build (not yet tested).
#[derive(Debug, Clone)]
pub struct Build {
    pub config: Config,
    pub stock: Image,
    pub image: Image,
    pub log: Vec<Change>,
    /// Key index of the Fn key in the new image.
    pub fn_position: u8,
    /// Key names of the stock firmware.
    pub names: std::collections::BTreeMap<u8, String>,
    /// The info block written, when the config asks for one.
    pub info: Option<Vec<u8>>,
    pub hex_name: String,
    pub hex_text: String,
}

impl Build {
    /// Checksum tag the updater shows ("CheckSum : 0x....") and the HEX name carries.
    pub fn tag(&self) -> u16 {
        word_sum(&self.image).wrapping_add(v72::TAG_OFFSET)
    }

    /// The patch log as `build.py` writes it to PATCH_DIFF.txt, given the paths to print in the first two lines.
    pub fn patch_diff_lines(&self, config_path: &str, stock_path: &str) -> Vec<String> {
        let mut out = vec![
            format!("Config: {config_path}"),
            format!(
                "Stock:  {stock_path} (word sum {:04X}, tag F448)",
                word_sum(&self.stock)
            ),
            format!(
                "Build:  {}, {} bytes changed, word sum {:04X}, tag {:04X}, sha256 {}",
                self.config.name,
                self.log.len(),
                word_sum(&self.image),
                self.tag(),
                v72::sha256_hex(&self.image)
            ),
            String::new(),
        ];
        out.extend(
            self.log
                .iter()
                .map(|c| format!("  {:#06x}: {:02x} -> {:02x}  {}", c.addr, c.old, c.new, c.why)),
        );
        out
    }
}

/// Builds the image described by `config_toml` from the stock HEX text.
pub fn build(stock_hex: &str, config_toml: &str) -> Result<Build, Error> {
    if v72::sha256_hex(stock_hex.as_bytes()) != v72::STOCK_HEX_SHA256 {
        return Err(Error::NotStockHex);
    }
    let (template, stock) = HexFile::parse(stock_hex)?;
    let mut fw = Firmware::new(&stock)?;
    let config = Config::parse(config_toml, &fw)?;
    if !config.fn_layer.is_empty() || !config.keymap.is_empty() {
        fw.apply(&config.fn_layer, &config.keymap)?;
    }
    if config.has_tap_hold() {
        let full = |k: u8| -> Result<u8, Error> {
            let n = fw.names.get(&k).cloned().unwrap_or_default();
            if fw.capable(k) != Some(false) {
                return Err(Error::Config(format!(
                    "[[tap_hold]] {n} has a Fn function; not supported"
                )));
            }
            let stock_code = fw.b[v72::NAME_KEYMAP + 2 * usize::from(k)];
            if k == fw.fn_position() || stock_code == v72::FN_CODE {
                return Err(Error::Config("[[tap_hold]] cannot be the Fn key".into()));
            }
            Ok(stock_code)
        };
        let caps = match config.tap_key {
            Some((k, tap, hold, term)) => Some((k, full(k)?, tap, hold, term)),
            None => None,
        };
        let layers = config
            .tap_layers
            .iter()
            .map(|&(k, tap, layer, term)| Ok((k, full(k)?, tap, layer, term)))
            .collect::<Result<Vec<_>, Error>>()?;
        taphold::apply(&mut fw, caps, &layers)?;
    }
    if config.layer_tap_hold {
        layertap::apply(&mut fw)?;
    }
    let info = if config.info_block {
        let block = info::block(&info::BlockContent {
            name: config.name.clone(),
            tap_key: config.tap_key,
            tap_layers: config.tap_layers.clone(),
            fn_layer: config.fn_layer.clone(),
            keymap: config.keymap.clone(),
            layer_tap_hold: config.layer_tap_hold,
        })?;
        info::apply(&mut fw, &block)?;
        Some(block)
    } else {
        None
    };
    let hex_text = template.write_like(&stock, &fw.b)?;
    let fn_position = fw.fn_position();
    let tag = word_sum(&fw.b).wrapping_add(v72::TAG_OFFSET);
    Ok(Build {
        hex_name: format!("CT_F72US[0x{tag:04X}]_20240125_0v72_{}_PID1972.hex", config.name),
        config,
        stock,
        image: fw.b,
        log: fw.log,
        fn_position,
        names: fw.names,
        info,
        hex_text,
    })
}
