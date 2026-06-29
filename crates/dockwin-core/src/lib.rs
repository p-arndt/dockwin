//! dockwin-core — the shared engine brain.
//!
//! WSL2 engine-distro lifecycle ([`wsl`]) and provisioning operations ([`ops`]),
//! ported from the original `scripts/*.ps1`. Both the `dockwin` CLI and the
//! Tauri GUI depend on this crate so install / uninstall / start / stop / status
//! exist in exactly one implementation — no PowerShell, no duplication.

pub mod ops;
pub mod wsl;
