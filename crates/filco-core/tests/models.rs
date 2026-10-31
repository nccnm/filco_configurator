//! Model descriptors: built-in models are valid, the committed schema matches the Rust types, and broken
//! descriptors are rejected with a useful message.

use std::path::PathBuf;

use filco_core::model::{
    self, KeyTag, ModelDescriptor, ModelError, Support, builtin, known_devices_schema_json, model_schema_json,
};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn m10sp() -> ModelDescriptor {
    ModelDescriptor::from_json(builtin::M10SP_US_JSON).expect("built-in model must be valid")
}

/// Regenerate with `UPDATE_SCHEMA=1 cargo test -p filco-core --test models`.
fn check_generated(rel: &str, expected: &str) {
    let path = repo().join(rel);
    if std::env::var_os("UPDATE_SCHEMA").is_some() {
        std::fs::write(&path, expected).unwrap();
    }
    let on_disk = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        on_disk == expected,
        "{rel} is out of date; run UPDATE_SCHEMA=1 cargo test -p filco-core --test models"
    );
}

#[test]
fn committed_schemas_match_the_types() {
    check_generated("models/schema/model.schema.json", &model_schema_json());
    check_generated(
        "models/schema/known-devices.schema.json",
        &known_devices_schema_json(),
    );
}

#[test]
fn every_model_file_passes_schema_and_semantic_checks() {
    let schema: serde_json::Value = serde_json::from_str(&model_schema_json()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let mut models = Vec::new();
    for entry in std::fs::read_dir(repo().join("models")).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if path.extension().is_none_or(|e| e != "json") || name == "known-devices.json" {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        let errors: Vec<String> = validator.iter_errors(&value).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{name} fails the schema: {errors:?}");
        models.push(ModelDescriptor::from_json(&text).unwrap_or_else(|e| panic!("{name}: {e}")));
    }
    assert_ne!(models.len(), 0);
    assert_eq!(model::validate_set(&models), vec![]);

    let known = builtin::known_devices().unwrap();
    let schema: serde_json::Value = serde_json::from_str(&known_devices_schema_json()).unwrap();
    let value: serde_json::Value = serde_json::from_str(builtin::KNOWN_DEVICES_JSON).unwrap();
    assert!(jsonschema::validator_for(&schema).unwrap().is_valid(&value));
    for m in &models {
        for pid in &m.device.product_ids {
            assert!(
                known
                    .devices
                    .iter()
                    .any(|d| d.vendor_id == m.device.vendor_id && d.product_id == *pid),
                "{} {pid} missing from known-devices.json",
                m.id
            );
        }
    }
}

#[test]
fn m10sp_layout_facts() {
    let m = m10sp();
    assert_eq!(m.layout.keys.len(), 72);
    assert_eq!(m.layers.len(), 4);
    assert!(m.matches(0x2efd, 0x1972));
    assert!(!m.matches(0x2efd, 0x1976));
    // Matrix positions verified on the real keyboard (analysis repository README sections 4, 11, 13, 15).
    for (id, index) in [
        ("P", 27),
        ("M10", 70),
        ("Fn", 73),
        ("SpaceR", 76),
        ("SpaceL", 77),
        ("LAlt", 83),
    ] {
        assert_eq!(m.key(id).unwrap().index, index, "{id}");
    }
    assert!(m.key("RAlt").unwrap().tags.contains(&KeyTag::Unverified));
    assert_eq!(
        m.layout
            .keys
            .iter()
            .filter(|k| k.tags.contains(&KeyTag::MKey))
            .count(),
        10
    );
    // Every row is 17 key units wide plus the gap between the halves.
    for y in 0..5u8 {
        let right = m
            .layout
            .keys
            .iter()
            .filter(|k| (k.y - f32::from(y)).abs() < f32::EPSILON)
            .map(|k| k.x + k.w)
            .fold(0.0_f32, f32::max);
        assert!((right - 17.9).abs() < 1e-3, "row {y} ends at {right}");
    }
    assert_eq!(m.support, Support::Experimental);
}

fn broken(edit: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut v: serde_json::Value = serde_json::from_str(builtin::M10SP_US_JSON).unwrap();
    edit(&mut v);
    match ModelDescriptor::from_json(&v.to_string()) {
        Ok(_) => panic!("broken model accepted"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn broken_models_are_rejected_with_a_clear_message() {
    type Edit = Box<dyn FnOnce(&mut serde_json::Value)>;
    let cases: Vec<(&str, Edit)> = vec![
        (
            "also used by layout.keys[0]",
            Box::new(|v| v["layout"]["keys"][1]["index"] = 0.into()),
        ),
        (
            "id \"Esc\" also used",
            Box::new(|v| v["layout"]["keys"][1]["id"] = "Esc".into()),
        ),
        (
            "overlaps key",
            Box::new(|v| v["layout"]["keys"][1]["x"] = 0.5.into()),
        ),
        (
            "outside the 128 slots",
            Box::new(|v| v["layout"]["keys"][0]["index"] = 128.into()),
        ),
        (
            "costar-v1 needs 512",
            Box::new(|v| v["protocol"]["page_len"] = 256.into()),
        ),
        (
            "must be 100..=1000 ms",
            Box::new(|v| v["protocol"]["delay_ms"]["write"] = 10.into()),
        ),
        (
            "cannot edit type 6",
            Box::new(|v| v["slot_types"]["editable"] = serde_json::json!([0, 2, 6])),
        ),
        (
            "must include 0",
            Box::new(|v| v["slot_types"]["editable"] = serde_json::json!([2])),
        ),
        (
            "is not #rrggbb",
            Box::new(|v| v["layers"][0]["led"] = "white".into()),
        ),
        ("lower-case words", Box::new(|v| v["id"] = "Filco M10".into())),
        ("must be 1", Box::new(|v| v["schema_version"] = 2.into())),
        (
            "at least one product id",
            Box::new(|v| v["match"]["product_ids"] = serde_json::json!([])),
        ),
        (
            "unknown field",
            Box::new(|v| v["layout"]["keys"][0]["colour"] = "red".into()),
        ),
        (
            "expected \\\"0x...\\\"",
            Box::new(|v| v["match"]["vendor_id"] = "2EFD".into()),
        ),
    ];
    for (needle, edit) in cases {
        let msg = broken(edit);
        assert!(
            msg.contains(needle.replace("\\\"", "\"").as_str()),
            "expected {needle:?} in:\n{msg}"
        );
    }
}

#[test]
fn duplicate_devices_across_models_are_rejected() {
    let a = m10sp();
    let mut b = m10sp();
    b.id = "other".into();
    let issues = model::validate_set(&[a, b]);
    assert_eq!(issues.len(), 1);
    assert!(issues[0].message.contains("also claimed by model filco-m10sp-us"));
}

#[test]
fn parse_errors_are_reported() {
    assert!(matches!(
        ModelDescriptor::from_json("{"),
        Err(ModelError::Parse(_))
    ));
}
