//! The `.filco-layers.json` export file: all layer pages of one keyboard as text, easy to share and review.
//!
//! Only `layers[].hex` is used when importing; the decoded `slots` list is for people reading the file.

use serde::{Deserialize, Serialize};

use crate::hex;
use crate::label::{Lang, slot_label};
use crate::layers::LayerSet;
use crate::model::ModelDescriptor;
use crate::page::Page;

pub const FORMAT: &str = "filco-layers";
pub const FORMAT_VERSION: u32 = 1;
pub const EXTENSION: &str = "filco-layers.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportFile {
    pub format: String,
    pub format_version: u32,
    pub app_version: String,
    pub created: String,
    pub model_id: String,
    pub model_name: String,
    /// `"0x1972"`.
    pub product_id: String,
    pub layers: Vec<ExportLayer>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportLayer {
    pub layer: usize,
    /// The 512 bytes as lower-case hex (1024 digits).
    pub hex: String,
    pub sha256: String,
    /// Non-empty slots, decoded for readers. Ignored on import.
    #[serde(default)]
    pub slots: Vec<ExportSlot>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportSlot {
    pub index: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub bytes: String,
    pub meaning: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ImportError {
    #[error("not a {FORMAT} file (format {0:?})")]
    Format(String),
    #[error("file format version {0} is newer than this application understands ({FORMAT_VERSION})")]
    Version(u32),
    #[error("file is for model {file:?}, the connected keyboard is {model:?}")]
    Model { file: String, model: String },
    #[error("file has {got} layers, the model has {want}")]
    LayerCount { got: usize, want: usize },
    #[error("layer {0}: {1}")]
    Layer(usize, String),
    #[error("file is not valid JSON: {0}")]
    Json(String),
}

/// Builds the export file for `layers` of `model`.
pub fn build(model: &ModelDescriptor, layers: &LayerSet, app_version: &str, created: &str) -> ExportFile {
    let addressing = model.addressing();
    let slot_count = u8::try_from(addressing.slot_count()).unwrap_or(u8::MAX);
    let layers = layers
        .pages()
        .iter()
        .enumerate()
        .map(|(layer, page)| ExportLayer {
            layer,
            hex: hex::encode(page.as_bytes()),
            sha256: page.sha256_hex(),
            slots: (0..slot_count)
                .filter_map(|index| {
                    let slot = page.slot(addressing, index)?;
                    (!slot.is_empty()).then(|| ExportSlot {
                        index,
                        key: model.key_by_index(index).map(|k| k.id.clone()),
                        bytes: slot.to_string(),
                        meaning: slot_label(slot.kind(), Lang::En).long,
                    })
                })
                .collect(),
        })
        .collect();
    ExportFile {
        format: FORMAT.into(),
        format_version: FORMAT_VERSION,
        app_version: app_version.into(),
        created: created.into(),
        model_id: model.id.clone(),
        model_name: model.name.clone(),
        product_id: format!("{:#06x}", model.device.product_ids.first().map_or(0, |p| p.0)),
        layers,
    }
}

pub fn to_json(file: &ExportFile) -> String {
    let mut s = serde_json::to_string_pretty(file).unwrap_or_default();
    s.push('\n');
    s
}

/// Parses an export file and returns its layer pages, checked against `model`.
pub fn parse(text: &str, model: &ModelDescriptor) -> Result<LayerSet, ImportError> {
    let file: ExportFile = serde_json::from_str(text).map_err(|e| ImportError::Json(e.to_string()))?;
    if file.format != FORMAT {
        return Err(ImportError::Format(file.format));
    }
    if file.format_version > FORMAT_VERSION {
        return Err(ImportError::Version(file.format_version));
    }
    if file.model_id != model.id {
        return Err(ImportError::Model {
            file: file.model_id,
            model: model.id.clone(),
        });
    }
    if file.layers.len() != model.layers.len() {
        return Err(ImportError::LayerCount {
            got: file.layers.len(),
            want: model.layers.len(),
        });
    }
    let mut pages = Vec::with_capacity(file.layers.len());
    for (i, l) in file.layers.iter().enumerate() {
        if l.layer != i {
            return Err(ImportError::Layer(
                i,
                format!("layers must be in order, found layer {}", l.layer),
            ));
        }
        let bytes = hex::decode(&l.hex).map_err(|e| ImportError::Layer(i, e.to_string()))?;
        let page = Page::from_bytes(&bytes).map_err(|e| ImportError::Layer(i, e.to_string()))?;
        if !page.sha256_hex().eq_ignore_ascii_case(&l.sha256) {
            return Err(ImportError::Layer(
                i,
                "SHA-256 does not match the bytes (file damaged or edited)".into(),
            ));
        }
        pages.push(page);
    }
    Ok(LayerSet::new(pages))
}
