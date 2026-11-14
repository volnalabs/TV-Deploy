//! Locating the bundled Node.js runtime, the vendored Samsung CLI and the Tizen SDK.

use easytizen_core::samsung::runner::{sdb_path, Toolchain};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeSource {
    Bundled,
    /// Development builds only: a Node.js found on PATH.
    System,
    Missing,
}

pub struct Located {
    pub node: Option<PathBuf>,
    pub node_source: NodeSource,
    pub cli_dir: Option<PathBuf>,
}

fn which_node() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "node.exe" } else { "node" };
    std::env::var_os("PATH").and_then(|p| {
        std::env::split_paths(&p)
            .map(|d| d.join(exe))
            .find(|c| c.is_file())
    })
}

pub fn locate(app: &AppHandle) -> Located {
    let res = app.path().resource_dir().ok();
    let exe = if cfg!(windows) { "node.exe" } else { "node" };
    let bundled = res
        .as_ref()
        .map(|r| r.join("node").join(exe))
        .filter(|p| p.is_file());
    let (node, node_source) = match bundled {
        Some(p) => (Some(p), NodeSource::Bundled),
        // Release builds never fall back to a user-installed Node: we ship our own runtime.
        None if cfg!(debug_assertions) => match which_node() {
            Some(p) => (Some(p), NodeSource::System),
            None => (None, NodeSource::Missing),
        },
        None => (None, NodeSource::Missing),
    };
    let mut cli_dir = res
        .as_ref()
        .map(|r| r.join("samsung-cli/lib/cli"))
        .filter(|p| p.is_dir());
    if cli_dir.is_none() && cfg!(debug_assertions) {
        cli_dir =
            Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../resources/samsung-cli/lib/cli"))
                .filter(|p| p.is_dir());
    }
    Located {
        node,
        node_source,
        cli_dir,
    }
}

pub fn build(
    app: &AppHandle,
    data_dir: &Path,
    sdk: Option<&str>,
) -> Result<Toolchain, &'static str> {
    let l = locate(app);
    let node = l.node.ok_or("node_missing")?;
    let cli_dir = l.cli_dir.ok_or("cli_missing")?;
    let sdk_path = PathBuf::from(sdk.ok_or("sdk_not_set")?);
    let home_dir = data_dir.join("samsung-home");
    std::fs::create_dir_all(&home_dir).map_err(|_| "io_error")?;
    Ok(Toolchain {
        node,
        cli_dir,
        sdk_path,
        home_dir,
    })
}

pub fn sdk_is_usable(p: &Path) -> bool {
    sdb_path(p).is_file()
        && p.join("sdk.info").is_file()
        && p.join("tools")
            .join("tizen-core")
            .join(if cfg!(windows) { "tz.exe" } else { "tz" })
            .is_file()
}

/// Whether the Samsung TV Extension has been installed by the official installer.
pub fn tv_sdk_is_usable(p: &Path) -> bool {
    p.join(".tv-sdk-installed").is_file()
        || std::fs::read_to_string(p.join(".platform-installed"))
            .map(|s| s.contains("TV-SAMSUNG-Public"))
            .unwrap_or(false)
}

/// Common places an existing Tizen SDK / Tizen Studio may live. We only *look*; we never touch
/// or modify these installations.
pub fn candidate_sdk_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    let home =
        std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    if let Some(h) = &home {
        for n in ["tizen-studio", "tizen-sdk", "Tizen/tizen-studio"] {
            v.push(h.join(n));
        }
    }
    if cfg!(windows) {
        v.push(PathBuf::from(r"C:\tizen-studio"));
        v.push(PathBuf::from(r"C:\tizen-sdk"));
    }
    v
}
