//! FILCO Assist's own settings, read only: a layer linked to a FILCO Assist profile is rebuilt from that profile
//! whenever FILCO Assist saves it, which would silently undo edits made here. The editor warns about such layers.
//!
//! File: `%APPDATA%\FILCO\filco_assist_config.json`. Each keyboard entry (e.g. `"Costar F72 Keyboard-..."`) has
//! `Layers: [{LayerNo, ProfileUuid, ...}]` and `Profiles: [{ProfileUuid, Name, ...}]`; an empty `ProfileUuid` means
//! the layer is not linked.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinkedLayer {
    pub keyboard: String,
    /// 0-based layer number.
    pub layer: usize,
    pub profile: String,
}

pub fn default_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("FILCO").join("filco_assist_config.json"))
}

/// Layers linked to a profile. A missing file means FILCO Assist was never set up: no warning. A file that
/// cannot be understood gives `Err`, and the UI says it could not check.
pub fn linked_layers(path: &Path) -> Result<Vec<LinkedLayer>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    parse(&text)
}

pub fn parse(text: &str) -> Result<Vec<LinkedLayer>, String> {
    let root: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let obj = root.as_object().ok_or("not a JSON object")?;
    let mut out = Vec::new();
    for (keyboard, entry) in obj {
        let Some(layers) = entry.get("Layers").and_then(Value::as_array) else {
            continue;
        };
        let profiles = entry
            .get("Profiles")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for l in layers {
            let uuid = l.get("ProfileUuid").and_then(Value::as_str).unwrap_or("");
            if uuid.is_empty() {
                continue;
            }
            let Some(layer) = l
                .get("LayerNo")
                .and_then(Value::as_u64)
                .and_then(|n| usize::try_from(n).ok())
            else {
                continue;
            };
            let profile = profiles
                .iter()
                .find(|p| p.get("ProfileUuid").and_then(Value::as_str) == Some(uuid))
                .and_then(|p| p.get("Name").and_then(Value::as_str))
                .unwrap_or(uuid)
                .to_owned();
            out.push(LinkedLayer {
                keyboard: keyboard.clone(),
                layer,
                profile,
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_linked_layers_and_profile_names() {
        let text = r#"{
          "ConfigFileVersion": 5,
          "Costar F72 Keyboard-1": {
            "Layers": [
              {"LayerNo": 0, "ProfileUuid": ""},
              {"LayerNo": 1, "ProfileUuid": "u-1"},
              {"LayerNo": 2, "ProfileUuid": "u-gone"}
            ],
            "Profiles": [{"ProfileUuid": "u-1", "Name": "s1"}]
          },
          "GlobalSettings": {"AutoCheckUpdate": true}
        }"#;
        let got = parse(text).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!((got[0].layer, got[0].profile.as_str()), (1, "s1"));
        assert_eq!((got[1].layer, got[1].profile.as_str()), (2, "u-gone"));
        assert!(parse("[").is_err());
        assert_eq!(parse("{}").unwrap(), Vec::new());
    }

    #[test]
    fn missing_file_is_not_a_warning() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(linked_layers(&tmp.path().join("nope.json")).unwrap(), Vec::new());
    }
}
