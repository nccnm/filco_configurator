//! A small file written (and flushed) before the first page is written and removed when the write finished
//! cleanly. If the app or the computer stops in between, the next connection finds it and tells the user which
//! layers hold the old pages, the new ones, or neither, and where the backup is.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use filco_core::LayerSet;
use serde::{Deserialize, Serialize};

pub const FILE_NAME: &str = "journal.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalLayer {
    pub layer: usize,
    /// SHA-256 of the page before the write (as in the backup).
    pub original: String,
    /// SHA-256 of the page being written.
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Journal {
    pub model_id: String,
    pub device_path: String,
    pub started: String,
    pub backup: PathBuf,
    pub layers: Vec<JournalLayer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerState {
    Original,
    Target,
    Unknown,
}

impl Journal {
    pub fn new(
        model_id: &str,
        device_path: &str,
        started: &str,
        backup: &Path,
        layers: &[usize],
        original: &LayerSet,
        target: &LayerSet,
    ) -> Self {
        let digest =
            |set: &LayerSet, l: usize| set.page(l).map(filco_core::Page::sha256_hex).unwrap_or_default();
        Journal {
            model_id: model_id.to_owned(),
            device_path: device_path.to_owned(),
            started: started.to_owned(),
            backup: backup.to_owned(),
            layers: layers
                .iter()
                .map(|&layer| JournalLayer {
                    layer,
                    original: digest(original, layer),
                    target: digest(target, layer),
                })
                .collect(),
        }
    }

    /// What each journal layer holds now, given pages just read from the keyboard.
    pub fn classify(&self, now: &LayerSet) -> Vec<(usize, LayerState)> {
        self.layers
            .iter()
            .map(|j| {
                let state = match now.page(j.layer).map(filco_core::Page::sha256_hex) {
                    Some(d) if d == j.target => LayerState::Target,
                    Some(d) if d == j.original => LayerState::Original,
                    _ => LayerState::Unknown,
                };
                (j.layer, state)
            })
            .collect()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    #[error("journal {path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("journal {path} is damaged: {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
}

/// The journal of one app installation, at `<dir>/journal.json`.
#[derive(Debug, Clone)]
pub struct JournalFile {
    path: PathBuf,
}

impl JournalFile {
    pub fn new(dir: &Path) -> Self {
        JournalFile {
            path: dir.join(FILE_NAME),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn io(&self) -> impl Fn(std::io::Error) -> JournalError + '_ {
        |source| JournalError::Io {
            path: self.path.clone(),
            source,
        }
    }

    /// Writes and flushes the journal. Fails if one is already there: an unfinished write must be dealt with first.
    pub fn begin(&self, journal: &Journal) -> Result<(), JournalError> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(self.io())?;
        }
        let mut text = serde_json::to_string_pretty(journal).unwrap_or_default();
        text.push('\n');
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.path)
            .map_err(self.io())?;
        f.write_all(text.as_bytes()).map_err(self.io())?;
        f.sync_all().map_err(self.io())
    }

    pub fn pending(&self) -> Result<Option<Journal>, JournalError> {
        match fs::read_to_string(&self.path) {
            Ok(text) => serde_json::from_str(&text)
                .map(Some)
                .map_err(|source| JournalError::Parse {
                    path: self.path.clone(),
                    source,
                }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(self.io()(e)),
        }
    }

    pub fn finish(&self) -> Result<(), JournalError> {
        match fs::remove_file(&self.path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(self.io()(e)),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use filco_core::{Slot, SlotAddressing};

    use super::*;

    #[test]
    fn begin_pending_classify_finish() {
        let tmp = tempfile::tempdir().unwrap();
        let jf = JournalFile::new(tmp.path());
        assert!(jf.pending().unwrap().is_none());
        let original = LayerSet::empty(4);
        let mut target = original.clone();
        let a = SlotAddressing::CostarInterleaved8;
        for l in [1, 2, 3] {
            target.page_mut(l).unwrap().set_slot(a, 27, Slot([2, 0, 0x13, 0]));
        }
        let j = Journal::new("m", "p", "t", Path::new("b"), &[1, 2, 3], &original, &target);
        jf.begin(&j).unwrap();
        assert!(
            jf.begin(&j).is_err(),
            "an unfinished journal is never overwritten"
        );
        let back = jf.pending().unwrap().unwrap();
        assert_eq!(back, j);

        let mut now = original.clone();
        *now.page_mut(1).unwrap() = target.page(1).unwrap().clone();
        now.page_mut(2).unwrap().set_slot(a, 0, Slot([2, 0, 4, 0]));
        assert_eq!(
            back.classify(&now),
            vec![
                (1, LayerState::Target),
                (2, LayerState::Unknown),
                (3, LayerState::Original)
            ]
        );
        jf.finish().unwrap();
        jf.finish().unwrap();
        assert!(jf.pending().unwrap().is_none());
    }

    #[test]
    fn damaged_journal_is_an_error_not_silence() {
        let tmp = tempfile::tempdir().unwrap();
        let jf = JournalFile::new(tmp.path());
        fs::write(jf.path(), "{").unwrap();
        assert!(matches!(jf.pending(), Err(JournalError::Parse { .. })));
    }
}
