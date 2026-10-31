//! Keyboard model descriptors: everything model-specific is data in `models/*.json`, checked against
//! `models/schema/model.schema.json` (generated from these types) and by [`ModelDescriptor::validate`].

use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::addressing::SlotAddressing;
use crate::hex;
use crate::page::PAGE_LEN;
use crate::slot::type_code;

/// Version of the descriptor format this build understands.
pub const SCHEMA_VERSION: u32 = 1;

/// Slot type codes the editor knows how to edit.
pub const EDITABLE_TYPES: [u8; 5] = [
    type_code::EMPTY,
    type_code::MOUSE,
    type_code::KEY,
    type_code::CONSUMER,
    type_code::SYSTEM,
];

/// A 16-bit number written as `"0x2EFD"` in model files.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HexU16(pub u16);

impl fmt::Debug for HexU16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#06x}", self.0)
    }
}

impl fmt::Display for HexU16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:04X}", self.0)
    }
}

impl Serialize for HexU16 {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for HexU16 {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let t = String::deserialize(d)?;
        if !t.starts_with("0x") {
            return Err(serde::de::Error::custom(format!("expected \"0x...\", got {t:?}")));
        }
        hex::parse_u16(&t).map(HexU16).map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for HexU16 {
    fn schema_name() -> Cow<'static, str> {
        "HexU16".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "pattern": "^0x[0-9A-Fa-f]{1,4}$",
            "description": "16-bit number in hex, e.g. \"0x2EFD\""
        })
    }
}

/// How far a model has been verified on a real keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Support {
    /// Read and write verified on a real keyboard.
    Verified,
    /// Reading only: the editor refuses to write.
    ReadOnly,
    /// Writing allowed with an extra warning; not verified on a real keyboard.
    Experimental,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HidCaps {
    pub usage_page: u16,
    pub input_len: u16,
    pub output_len: u16,
    pub feature_len: u16,
}

/// How to recognise the configuration interface of this model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeviceMatch {
    pub vendor_id: HexU16,
    pub product_ids: Vec<HexU16>,
    pub hid_caps: HidCaps,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ProtocolId {
    /// Feature-report protocol of FILCO Assist 2.0.97 (`CostarKBHID`): commands 0x10 / 0x11 / 0x02.
    #[serde(rename = "costar-v1")]
    CostarV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Delays {
    /// Pause after every feature report while reading, in ms. FILCO Assist uses 100.
    pub read: u32,
    /// Pause after every feature report while writing, in ms. FILCO Assist uses 100; at least 100.
    pub write: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Protocol {
    pub id: ProtocolId,
    /// Feature report length including the report id byte.
    pub report_len: u16,
    pub page_len: u16,
    /// Data bytes per report while reading or writing a page.
    pub chunk_len: u16,
    pub slot_addressing: SlotAddressing,
    pub delay_ms: Delays,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayerInfo {
    /// Name shown in the layer tabs, e.g. "1".
    pub label: String,
    /// Colour of the keyboard's layer LED, `#rrggbb`.
    pub led: String,
    /// Words for the LED colour, Vietnamese then English, e.g. `["trắng", "white"]`.
    pub led_name: [String; 2],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SlotTypes {
    /// Type codes the editor may write on this model.
    pub editable: Vec<u8>,
    /// Type codes shown read-only.
    pub view_only: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum KeyTag {
    /// A programmable M key (drawn with the soft fill).
    MKey,
    /// The key's matrix index is not verified on a real keyboard.
    Unverified,
}

fn one() -> f32 {
    1.0
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_one(v: &f32) -> bool {
    (*v - 1.0).abs() < f32::EPSILON
}

/// One physical key.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KeyDef {
    /// Stable id, e.g. `Esc`, `SpaceL`, `M10`.
    pub id: String,
    /// Matrix index (slot number).
    pub index: u8,
    /// Position and size in key units (1 unit = one alphanumeric key).
    pub x: f32,
    pub y: f32,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub w: f32,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub h: f32,
    /// Legend printed on the keycap.
    pub legend: String,
    /// HID keyboard usage the key sends when its slot is empty (US layout, DIP switches off), if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_usage: Option<HexU16>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<KeyTag>,
    /// Free text shown in the inspector.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    pub keys: Vec<KeyDef>,
}

/// Everything the editor needs to know about one keyboard model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModelDescriptor {
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub schema_version: u32,
    /// Stable id, lower-case words joined by `-`.
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub support: Support,
    #[serde(rename = "match")]
    pub device: DeviceMatch,
    pub protocol: Protocol,
    pub layers: Vec<LayerInfo>,
    pub slot_types: SlotTypes,
    pub layout: Layout,
    /// Hints shown in the UI, one sentence each.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// One problem found in a model file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelIssue {
    /// Where, e.g. `layout.keys[3]`.
    pub path: String,
    pub message: String,
}

impl fmt::Display for ModelIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("model file is not valid JSON for the schema: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("model {id} is invalid:\n{}", issues.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n"))]
    Invalid { id: String, issues: Vec<ModelIssue> },
}

fn is_id(s: &str) -> bool {
    !s.is_empty()
        && s.split('-')
            .all(|w| !w.is_empty() && w.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()))
}

fn is_key_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 24 && s.bytes().all(|c| c.is_ascii_graphic())
}

fn is_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|c| c.is_ascii_hexdigit())
}

impl ModelDescriptor {
    /// Parses and validates a model file.
    pub fn from_json(text: &str) -> Result<Self, ModelError> {
        let m: ModelDescriptor = serde_json::from_str(text)?;
        let issues = m.validate();
        if issues.is_empty() {
            Ok(m)
        } else {
            Err(ModelError::Invalid { id: m.id, issues })
        }
    }

    pub fn addressing(&self) -> SlotAddressing {
        self.protocol.slot_addressing
    }

    pub fn key(&self, id: &str) -> Option<&KeyDef> {
        self.layout.keys.iter().find(|k| k.id == id)
    }

    pub fn key_by_index(&self, index: u8) -> Option<&KeyDef> {
        self.layout.keys.iter().find(|k| k.index == index)
    }

    pub fn matches(&self, vendor_id: u16, product_id: u16) -> bool {
        self.device.vendor_id.0 == vendor_id && self.device.product_ids.iter().any(|p| p.0 == product_id)
    }

    pub fn can_write(&self) -> bool {
        self.support != Support::ReadOnly
    }

    pub fn is_editable_type(&self, code: u8) -> bool {
        self.slot_types.editable.contains(&code)
    }

    /// Semantic checks the JSON schema cannot express. Empty means valid.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Vec<ModelIssue> {
        let mut issues = Vec::new();
        let mut bad = |path: &str, message: String| {
            issues.push(ModelIssue {
                path: path.to_owned(),
                message,
            });
        };

        if self.schema_version != SCHEMA_VERSION {
            bad(
                "schema_version",
                format!("must be {SCHEMA_VERSION}, got {}", self.schema_version),
            );
        }
        if !is_id(&self.id) {
            bad(
                "id",
                format!("{:?} is not lower-case words joined by '-'", self.id),
            );
        }
        if self.name.trim().is_empty() {
            bad("name", "must not be empty".into());
        }
        if self.vendor.trim().is_empty() {
            bad("vendor", "must not be empty".into());
        }

        // Device match.
        if self.device.product_ids.is_empty() {
            bad("match.product_ids", "must list at least one product id".into());
        }
        let pids: HashSet<_> = self.device.product_ids.iter().collect();
        if pids.len() != self.device.product_ids.len() {
            bad("match.product_ids", "contains duplicates".into());
        }

        // Protocol.
        let p = &self.protocol;
        match p.id {
            ProtocolId::CostarV1 => {
                let expect = [
                    ("report_len", p.report_len, 65),
                    ("page_len", p.page_len, 512),
                    ("chunk_len", p.chunk_len, 64),
                ];
                for (name, got, want) in expect {
                    if got != want {
                        bad(
                            &format!("protocol.{name}"),
                            format!("costar-v1 needs {want}, got {got}"),
                        );
                    }
                }
                if self.device.hid_caps.feature_len != p.report_len {
                    bad(
                        "match.hid_caps.feature_len",
                        "must equal protocol.report_len".into(),
                    );
                }
                if usize::from(p.page_len) != PAGE_LEN {
                    bad(
                        "protocol.page_len",
                        format!("this build handles {PAGE_LEN}-byte pages only"),
                    );
                }
            }
        }
        if !(1..=1000).contains(&p.delay_ms.read) {
            bad(
                "protocol.delay_ms.read",
                format!("must be 1..=1000 ms, got {}", p.delay_ms.read),
            );
        }
        if !(100..=1000).contains(&p.delay_ms.write) {
            bad(
                "protocol.delay_ms.write",
                format!("must be 100..=1000 ms, got {}", p.delay_ms.write),
            );
        }
        let slot_count = p.slot_addressing.slot_count();
        if slot_count * 4 > usize::from(p.page_len) {
            bad(
                "protocol.slot_addressing",
                "addresses more bytes than a page has".into(),
            );
        }

        // Layers.
        if !(1..=16).contains(&self.layers.len()) {
            bad(
                "layers",
                format!("must have 1..=16 layers, got {}", self.layers.len()),
            );
        }
        for (i, l) in self.layers.iter().enumerate() {
            if l.label.trim().is_empty() {
                bad(&format!("layers[{i}].label"), "must not be empty".into());
            }
            if !is_color(&l.led) {
                bad(&format!("layers[{i}].led"), format!("{:?} is not #rrggbb", l.led));
            }
            if l.led_name.iter().any(|n| n.trim().is_empty()) {
                bad(
                    &format!("layers[{i}].led_name"),
                    "must name the colour in both languages".into(),
                );
            }
        }

        // Slot types.
        let t = &self.slot_types;
        for (name, list) in [("editable", &t.editable), ("view_only", &t.view_only)] {
            let set: BTreeSet<_> = list.iter().collect();
            if set.len() != list.len() {
                bad(&format!("slot_types.{name}"), "contains duplicates".into());
            }
            if let Some(c) = list.iter().find(|c| **c > 15) {
                bad(
                    &format!("slot_types.{name}"),
                    format!("type code {c} is not 0..=15"),
                );
            }
        }
        if let Some(c) = t.editable.iter().find(|c| !EDITABLE_TYPES.contains(c)) {
            bad("slot_types.editable", format!("this editor cannot edit type {c}"));
        }
        if !t.editable.contains(&type_code::EMPTY) {
            bad("slot_types.editable", "must include 0 (clearing a slot)".into());
        }
        if let Some(c) = t.editable.iter().find(|c| t.view_only.contains(c)) {
            bad("slot_types", format!("type {c} is both editable and view-only"));
        }

        // Layout.
        let keys = &self.layout.keys;
        if keys.is_empty() {
            bad("layout.keys", "must not be empty".into());
        }
        let mut ids = HashMap::new();
        let mut indexes = HashMap::new();
        for (i, k) in keys.iter().enumerate() {
            let path = format!("layout.keys[{i}]");
            if !is_key_id(&k.id) {
                bad(
                    &path,
                    format!("id {:?} must be 1-24 visible ASCII characters", k.id),
                );
            }
            if let Some(j) = ids.insert(k.id.as_str(), i) {
                bad(&path, format!("id {:?} also used by layout.keys[{j}]", k.id));
            }
            if let Some(j) = indexes.insert(k.index, i) {
                bad(
                    &path,
                    format!("matrix index {} also used by layout.keys[{j}]", k.index),
                );
            }
            if usize::from(k.index) >= slot_count {
                bad(
                    &path,
                    format!("matrix index {} is outside the {slot_count} slots", k.index),
                );
            }
            let finite = [k.x, k.y, k.w, k.h].iter().all(|v| v.is_finite());
            if !finite || k.x < 0.0 || k.y < 0.0 || k.w <= 0.0 || k.h <= 0.0 || k.w > 10.0 || k.h > 4.0 {
                bad(
                    &path,
                    "position must be >= 0 and size within 0 < w <= 10, 0 < h <= 4".into(),
                );
            }
            if k.legend.trim().is_empty() {
                bad(&path, "legend must not be empty".into());
            }
            if let Some(u) = k.default_usage
                && u.0 > 0xff
            {
                bad(&path, format!("default_usage {u} is not a keyboard-page usage"));
            }
        }
        for (i, a) in keys.iter().enumerate() {
            for (j, b) in keys.iter().enumerate().skip(i + 1) {
                let ox = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
                let oy = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
                if ox > 1e-3 && oy > 1e-3 {
                    bad(
                        &format!("layout.keys[{j}]"),
                        format!("key {:?} overlaps key {:?} (layout.keys[{i}])", b.id, a.id),
                    );
                }
            }
        }
        issues
    }
}

/// Checks a set of models together: ids and (vendor, product) pairs must be unique.
pub fn validate_set(models: &[ModelDescriptor]) -> Vec<ModelIssue> {
    let mut issues = Vec::new();
    let mut ids = HashSet::new();
    let mut devices = HashMap::new();
    for m in models {
        if !ids.insert(m.id.as_str()) {
            issues.push(ModelIssue {
                path: m.id.clone(),
                message: "duplicate model id".into(),
            });
        }
        for p in &m.device.product_ids {
            if let Some(other) = devices.insert((m.device.vendor_id, *p), m.id.as_str()) {
                issues.push(ModelIssue {
                    path: m.id.clone(),
                    message: format!("{}:{p} is also claimed by model {other}", m.device.vendor_id),
                });
            }
        }
    }
    issues
}

/// A keyboard the editor can name even without a layout (from FILCO Assist's VID/PID table).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KnownDevice {
    pub vendor_id: HexU16,
    pub product_id: HexU16,
    pub name: String,
    /// FILCO Assist's internal code, e.g. "F72".
    pub internal: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KnownDevices {
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub devices: Vec<KnownDevice>,
}

/// Models and the known-device list compiled into the application.
pub mod builtin {
    use super::{KnownDevices, ModelDescriptor, ModelError};

    pub const M10SP_US_JSON: &str = include_str!("../../../models/m10sp-us.json");
    pub const KNOWN_DEVICES_JSON: &str = include_str!("../../../models/known-devices.json");

    /// The built-in models. They are validated by the test suite, so failure here is a build bug.
    pub fn models() -> Result<Vec<ModelDescriptor>, ModelError> {
        Ok(vec![ModelDescriptor::from_json(M10SP_US_JSON)?])
    }

    pub fn known_devices() -> Result<KnownDevices, serde_json::Error> {
        serde_json::from_str(KNOWN_DEVICES_JSON)
    }
}

/// JSON schema of [`ModelDescriptor`], as committed in `models/schema/model.schema.json`.
pub fn model_schema_json() -> String {
    let schema = schemars::schema_for!(ModelDescriptor);
    let mut s = serde_json::to_string_pretty(&schema).unwrap_or_default();
    s.push('\n');
    s
}

/// JSON schema of [`KnownDevices`], as committed in `models/schema/known-devices.schema.json`.
pub fn known_devices_schema_json() -> String {
    let schema = schemars::schema_for!(KnownDevices);
    let mut s = serde_json::to_string_pretty(&schema).unwrap_or_default();
    s.push('\n');
    s
}
