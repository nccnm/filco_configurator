//! FILCO Assist talks to the same interface. Two programs writing pages at once could leave a layer half
//! written, so every write (and the first read) checks that it is not running.

/// Process names of FILCO Assist 2.0.97 (UI and tray helper).
pub const FILCO_ASSIST_PROCESSES: [&str; 2] = ["FilcoAssistUI.exe", "FilcoAssistTray.exe"];

/// Lists running process image names (e.g. `"explorer.exe"`).
pub trait ProcessProbe {
    fn running(&self) -> Result<Vec<String>, String>;
}

impl<F: Fn() -> Result<Vec<String>, String>> ProcessProbe for F {
    fn running(&self) -> Result<Vec<String>, String> {
        self()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GuardError {
    #[error("FILCO Assist is running ({}); close it from the tray first", .0.join(", "))]
    AssistRunning(Vec<String>),
    /// When the process list cannot be read, the check fails closed.
    #[error("could not check whether FILCO Assist is running: {0}")]
    Probe(String),
}

/// Fails if a FILCO Assist process is running, or if that cannot be determined.
pub fn ensure_assist_closed(probe: &dyn ProcessProbe) -> Result<(), GuardError> {
    let names = probe.running().map_err(GuardError::Probe)?;
    let found: Vec<String> = FILCO_ASSIST_PROCESSES
        .iter()
        .filter(|p| names.iter().any(|n| n.eq_ignore_ascii_case(p)))
        .map(|p| (*p).to_owned())
        .collect();
    if found.is_empty() {
        Ok(())
    } else {
        Err(GuardError::AssistRunning(found))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_either_process_case_insensitively() {
        let none = || Ok(vec!["explorer.exe".to_owned()]);
        assert_eq!(ensure_assist_closed(&none), Ok(()));
        let tray = || Ok(vec!["filcoassisttray.EXE".to_owned(), "explorer.exe".to_owned()]);
        assert_eq!(
            ensure_assist_closed(&tray),
            Err(GuardError::AssistRunning(vec!["FilcoAssistTray.exe".into()]))
        );
        let broken = || Err("access denied".to_owned());
        assert!(matches!(ensure_assist_closed(&broken), Err(GuardError::Probe(_))));
    }
}
