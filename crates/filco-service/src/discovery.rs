//! Deciding which HID interfaces are FILCO configuration interfaces and which model they are.

use filco_core::ModelDescriptor;
use filco_core::model::KnownDevices;
use serde::Serialize;

/// The facts discovery needs about one HID interface (from the OS, or from a simulator).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InterfaceInfo {
    pub path: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub version: u16,
    pub usage_page: u16,
    pub input_len: u16,
    pub output_len: u16,
    pub feature_len: u16,
}

/// A configuration interface found on this computer.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FoundKeyboard {
    pub info: InterfaceInfo,
    /// Index into the model list, if a model descriptor matches VID, PID and HID caps.
    pub model: Option<usize>,
    /// Name from the known-device list, for keyboards without a model descriptor.
    pub known_name: Option<String>,
}

fn caps_match(m: &ModelDescriptor, i: &InterfaceInfo) -> bool {
    let c = &m.device.hid_caps;
    i.usage_page == c.usage_page
        && i.input_len == c.input_len
        && i.output_len == c.output_len
        && i.feature_len == c.feature_len
}

/// Picks the configuration interfaces among `interfaces`.
///
/// An interface qualifies when a model's HID caps filter matches (FILCO Assist's `HidFilter_ConfigInterface`:
/// usage page 1, input 9, output 2, feature 65) and its vendor id is one the models or the known-device list name.
/// The model is chosen by VID and PID; a FILCO keyboard with matching caps but no model is still listed, so the UI
/// can name it and say it is not supported yet.
pub fn find_keyboards(
    interfaces: &[InterfaceInfo],
    models: &[ModelDescriptor],
    known: &KnownDevices,
) -> Vec<FoundKeyboard> {
    let vendors: Vec<u16> = models
        .iter()
        .map(|m| m.device.vendor_id.0)
        .chain(known.devices.iter().map(|d| d.vendor_id.0))
        .collect();
    interfaces
        .iter()
        .filter(|i| vendors.contains(&i.vendor_id) && models.iter().any(|m| caps_match(m, i)))
        .map(|i| FoundKeyboard {
            info: i.clone(),
            model: models
                .iter()
                .position(|m| m.matches(i.vendor_id, i.product_id) && caps_match(m, i)),
            known_name: known
                .devices
                .iter()
                .find(|d| d.vendor_id.0 == i.vendor_id && d.product_id.0 == i.product_id)
                .map(|d| d.name.clone()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use filco_core::model::builtin;

    use super::*;

    fn iface(path: &str, pid: u16, usage_page: u16, input: u16, output: u16, feature: u16) -> InterfaceInfo {
        InterfaceInfo {
            path: path.into(),
            vendor_id: 0x2efd,
            product_id: pid,
            version: 0x72,
            usage_page,
            input_len: input,
            output_len: output,
            feature_len: feature,
        }
    }

    #[test]
    fn picks_the_mi00_keyboard_collection_only() {
        let models = builtin::models().unwrap();
        let known = builtin::known_devices().unwrap();
        let all = vec![
            iface("mi_00 kbd", 0x1972, 1, 9, 2, 65),
            iface("mi_01 vendor", 0x1972, 0xffff, 4, 0, 0),
            iface("mi_01 consumer", 0x1972, 0x0c, 3, 0, 0),
            InterfaceInfo {
                vendor_id: 0x046d,
                ..iface("other kbd", 0xc31c, 1, 9, 2, 65)
            },
            iface("m10sp jp", 0x1976, 1, 9, 2, 65),
        ];
        let found = find_keyboards(&all, &models, &known);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].info.path, "mi_00 kbd");
        assert_eq!(found[0].model, Some(0));
        assert_eq!(found[1].model, None);
        assert_eq!(found[1].known_name.as_deref(), Some("Majestouch Xacro M10SP JP"));
    }
}
