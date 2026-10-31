//! Read-only developer tool. Nothing here can write a keyboard: every device is opened with
//! `WritePolicy::ReadOnly`, which refuses any report other than the 0x10 / 0x02 commands of a page read.
//!
//! ```text
//! filco-cli list                                   HID interfaces of FILCO keyboards (no report sent)
//! filco-cli caps                                   System Control input ranges (descriptor only)
//! filco-cli read [--compare DIR] [--delay-ms N]    read all layers, print SHA-256, compare with a backup
//! filco-cli measure-delays [--compare DIR]         read with shorter pauses, at most 50 page reads in total
//! filco-cli decode DIR                             print the slots of a backup folder
//! filco-cli validate-model FILE...                 check model descriptor files
//! ```
#![forbid(unsafe_code)]

use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use filco_core::label::{Lang, slot_label};
use filco_core::model::builtin;
use filco_core::{ModelDescriptor, Page, backup, model};
use filco_hid_windows::{HidDevice, PolicyTransport, WritePolicy};
use filco_protocol::{Delays, ProtocolError, Session, SystemClock};
use filco_service::discovery::{InterfaceInfo, find_keyboards};

type Res<T> = Result<T, String>;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("list") => list(),
        Some("caps") => caps(),
        Some("read") => read(&args[1..]),
        Some("measure-delays") => measure_delays(&args[1..]),
        Some("decode") => args
            .get(1)
            .ok_or_else(|| "decode needs a folder".to_owned())
            .and_then(|d| decode(Path::new(d))),
        Some("validate-model") => validate(&args[1..]),
        _ => Err(include_str!("main.rs")
            .lines()
            .skip(4)
            .take(8)
            .map(|l| l.trim_start_matches("//! "))
            .collect::<Vec<_>>()
            .join("\n")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn option<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn models() -> Res<(Vec<ModelDescriptor>, model::KnownDevices)> {
    let m = builtin::models().map_err(|e| e.to_string())?;
    let k = builtin::known_devices().map_err(|e| e.to_string())?;
    Ok((m, k))
}

fn interfaces() -> Res<Vec<InterfaceInfo>> {
    let t = Instant::now();
    let all = filco_hid_windows::enumerate().map_err(|e| e.to_string())?;
    eprintln!(
        "enumerated {} HID interfaces in {:.1} ms",
        all.len(),
        t.elapsed().as_secs_f64() * 1000.0
    );
    Ok(all
        .into_iter()
        .map(|d| InterfaceInfo {
            path: d.path,
            vendor_id: d.vendor_id,
            product_id: d.product_id,
            version: d.version,
            usage_page: d.usage_page,
            input_len: d.input_len,
            output_len: d.output_len,
            feature_len: d.feature_len,
        })
        .collect())
}

fn list() -> Res<()> {
    let (models, known) = models()?;
    let all = interfaces()?;
    for i in all.iter().filter(|i| i.vendor_id == 0x2efd) {
        println!(
            "{:04x}:{:04x} rev {:04x} usage page {:#06x} in {} out {} feature {}  {}",
            i.vendor_id,
            i.product_id,
            i.version,
            i.usage_page,
            i.input_len,
            i.output_len,
            i.feature_len,
            i.path
        );
    }
    let found = find_keyboards(&all, &models, &known);
    println!("configuration interfaces: {}", found.len());
    for f in &found {
        let name = f
            .model
            .map(|m| models[m].name.clone())
            .or_else(|| f.known_name.clone())
            .unwrap_or_default();
        println!(
            "  {name} (model descriptor: {})",
            if f.model.is_some() { "yes" } else { "no" }
        );
    }
    Ok(())
}

fn caps() -> Res<()> {
    for (info, ranges) in filco_hid_windows::system_control_ranges(0x2efd).map_err(|e| e.to_string())? {
        println!("{:04x}:{:04x} {}", info.vendor_id, info.product_id, info.path);
        for r in ranges {
            println!(
                "  {} page {:#06x} usages {:#06x}..={:#06x} logical {}..={} flags {:#06x} ({}) count {}",
                if r.is_button { "button" } else { "value" },
                r.usage_page,
                r.usage_min,
                r.usage_max,
                r.logical_min,
                r.logical_max,
                r.bit_field,
                if r.bit_field & 2 == 0 { "array" } else { "variable" },
                r.report_count
            );
        }
    }
    Ok(())
}

fn filco_assist_running() -> Res<bool> {
    let procs = filco_hid_windows::running_processes().map_err(|e| e.to_string())?;
    Ok(procs.iter().any(|p| {
        let exe = p.exe.to_ascii_lowercase();
        exe == "filcoassistui.exe" || exe == "filcoassisttray.exe"
    }))
}

fn open_one(
    models: &[ModelDescriptor],
    known: &model::KnownDevices,
) -> Res<(PolicyTransport<HidDevice>, usize)> {
    let found = find_keyboards(&interfaces()?, models, known);
    let [kb] = found.as_slice() else {
        return Err(format!(
            "expected exactly one FILCO configuration interface, found {}",
            found.len()
        ));
    };
    let model = kb.model.ok_or("the keyboard has no model descriptor")?;
    if filco_assist_running()? {
        return Err("FILCO Assist is running; close it (and its tray icon) first.".into());
    }
    let dev = filco_hid_windows::open(&kb.info.path, WritePolicy::ReadOnly).map_err(|e| e.to_string())?;
    Ok((dev, model))
}

fn read_all(
    session: &mut Session<PolicyTransport<HidDevice>, SystemClock>,
    layers: usize,
) -> Result<Vec<Page>, ProtocolError> {
    (0..layers)
        .map(|l| session.read_page(u8::try_from(l).unwrap_or(0), &mut |_| {}))
        .collect()
}

fn compare(pages: &[Page], dir: Option<&str>) -> Res<bool> {
    let Some(dir) = dir else { return Ok(true) };
    let b = backup::read(Path::new(dir)).map_err(|e| e.to_string())?;
    let mut same = true;
    for (layer, page) in pages.iter().enumerate() {
        let want = b.pages.get(&layer).map(Page::sha256_hex).unwrap_or_default();
        let ok = want == page.sha256_hex();
        same &= ok;
        println!(
            "  layer {layer}: {} {}",
            page.sha256_hex(),
            if ok { "matches" } else { "DIFFERS" }
        );
    }
    Ok(same)
}

fn read(args: &[String]) -> Res<()> {
    let (models, known) = models()?;
    let (dev, m) = open_one(&models, &known)?;
    let model = &models[m];
    let mut delays = Delays::from_ms(model.protocol.delay_ms.read, model.protocol.delay_ms.write);
    if let Some(ms) = option(args, "--delay-ms") {
        let ms: u64 = ms.parse().map_err(|_| "--delay-ms needs a number")?;
        if !(10..=1000).contains(&ms) {
            return Err("--delay-ms must be 10..=1000".into());
        }
        delays.read = Duration::from_millis(ms);
    }
    println!(
        "{} rev {:#06x}, read pause {} ms",
        model.name,
        dev.inner().info().version,
        delays.read.as_millis()
    );
    let mut s = Session::new(dev, SystemClock, delays);
    let t = Instant::now();
    let pages = read_all(&mut s, model.layers.len()).map_err(|e| e.to_string())?;
    println!(
        "read {} layers in {:.3} s",
        pages.len(),
        t.elapsed().as_secs_f64()
    );
    for (l, p) in pages.iter().enumerate() {
        println!("  layer {l}: sha256 {}", p.sha256_hex());
    }
    if !compare(&pages, option(args, "--compare"))? {
        return Err("read pages differ from the backup".into());
    }
    if s.transport().refused() != 0 {
        return Err("internal error: the read-only policy refused a report".into());
    }
    Ok(())
}

/// Page reads allowed on the real keyboard in one run (review 1 of the plan).
const MAX_PAGE_READS: usize = 50;

fn measure_delays(args: &[String]) -> Res<()> {
    let (models, known) = models()?;
    let (dev, m) = open_one(&models, &known)?;
    let model = &models[m];
    let layers = model.layers.len();
    let mut s = Session::new(dev, SystemClock, Delays::FILCO_ASSIST);
    s.set_read_retries(0);
    let mut used = 0usize;
    let t = Instant::now();
    let reference = read_all(&mut s, layers).map_err(|e| format!("reference read at 100 ms failed: {e}"))?;
    used += layers;
    println!("100 ms: reference read in {:.3} s", t.elapsed().as_secs_f64());
    if !compare(&reference, option(args, "--compare"))? {
        return Err("reference read differs from the backup; not measuring".into());
    }
    let mut best = 100u64;
    'levels: for ms in [50u64, 25, 15, 10] {
        s.set_delays(Delays {
            read: Duration::from_millis(ms),
            write: Duration::from_millis(100),
        });
        for run in 0..2 {
            if used + layers > MAX_PAGE_READS {
                println!("page read budget ({MAX_PAGE_READS}) reached");
                break 'levels;
            }
            used += layers;
            let t = Instant::now();
            match read_all(&mut s, layers) {
                Ok(pages) if pages == reference => {
                    println!("{ms:>3} ms: run {run} ok in {:.3} s", t.elapsed().as_secs_f64());
                }
                Ok(_) => {
                    println!("{ms:>3} ms: run {run} returned different bytes; stopping");
                    break 'levels;
                }
                Err(e) => {
                    println!("{ms:>3} ms: run {run} failed: {e}; stopping");
                    break 'levels;
                }
            }
        }
        best = ms;
    }
    println!("smallest pause that passed every run: {best} ms; page reads used: {used}");
    Ok(())
}

fn decode(dir: &Path) -> Res<()> {
    let (models, _) = models()?;
    let b = backup::read(dir).map_err(|e| e.to_string())?;
    let pid = b.manifest.device.product_id().unwrap_or(0);
    let model = models
        .iter()
        .find(|m| m.matches(0x2efd, pid))
        .ok_or("no model for this backup")?;
    for (layer, page) in &b.pages {
        println!("layer {layer} ({})", page.sha256_hex());
        for index in 0..128u8 {
            let Some(slot) = page.slot(model.addressing(), index) else {
                continue;
            };
            if slot.is_empty() {
                continue;
            }
            let key = model.key_by_index(index).map_or("?", |k| k.id.as_str());
            println!(
                "  {index:>3} {key:<7} {slot}  {}",
                slot_label(slot.kind(), Lang::En).long
            );
        }
    }
    Ok(())
}

fn validate(files: &[String]) -> Res<()> {
    if files.is_empty() {
        return Err("validate-model needs one or more files".into());
    }
    let mut ok = true;
    for f in files {
        let text = std::fs::read_to_string(f).map_err(|e| format!("{f}: {e}"))?;
        match ModelDescriptor::from_json(&text) {
            Ok(m) => println!("{f}: ok ({}, {} keys)", m.name, m.layout.keys.len()),
            Err(e) => {
                ok = false;
                println!("{f}: {e}");
            }
        }
    }
    if ok {
        Ok(())
    } else {
        Err("invalid model files".into())
    }
}
