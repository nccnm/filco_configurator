//! Everything between the UI and the device that is not a Tauri concern: discovering keyboards, the editing
//! session with undo, and the safe write flow (fresh read, compare, backup, journal, write, read back, restore).
//! Testable without Tauri and without a keyboard.
#![forbid(unsafe_code)]

pub mod discovery;
