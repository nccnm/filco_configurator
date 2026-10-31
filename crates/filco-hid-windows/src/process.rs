//! Which programs are running (to warn when FILCO Assist could write the keyboard at the same time).

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunningProcess {
    pub pid: u32,
    pub exe: String,
}

/// Every running process with its executable name.
pub fn running_processes() -> windows::core::Result<Vec<RunningProcess>> {
    // SAFETY: snapshot of all processes; the handle is closed below.
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }?;
    let mut out = Vec::new();
    let mut entry = PROCESSENTRY32W {
        dwSize: u32::try_from(size_of::<PROCESSENTRY32W>()).unwrap_or(0),
        ..Default::default()
    };
    // SAFETY: valid snapshot handle and an entry with dwSize set, as the API requires.
    let mut ok = unsafe { Process32FirstW(snap, &raw mut entry) }.is_ok();
    while ok {
        let len = entry
            .szExeFile
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(entry.szExeFile.len());
        out.push(RunningProcess {
            pid: entry.th32ProcessID,
            exe: String::from_utf16_lossy(&entry.szExeFile[..len]),
        });
        // SAFETY: as above.
        ok = unsafe { Process32NextW(snap, &raw mut entry) }.is_ok();
    }
    // SAFETY: the snapshot handle is closed exactly once.
    let _ = unsafe { CloseHandle(snap) };
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_this_test_process() {
        let me = std::process::id();
        let procs = running_processes().unwrap();
        assert!(
            procs
                .iter()
                .any(|p| p.pid == me && p.exe.to_ascii_lowercase().ends_with(".exe"))
        );
    }
}
