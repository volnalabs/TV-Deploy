//! One-time migration of user data from TV Deploy (identifier `io.github.volnalabs.tvdeploy`)
//! to EasyTizen (`io.github.volnalabs.easytizen`).
//!
//! The app data folder is derived from the bundle identifier, so renaming the product moves
//! it. Without this step existing users would lose their saved TVs, SDK location and
//! certificate profiles. The Tizen SDK itself and its keystore live outside the app data
//! folder and are untouched.

use std::path::Path;

pub const LEGACY_IDENTIFIER: &str = "io.github.volnalabs.tvdeploy";

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Copies the legacy folder next to `new_dir` into it, when `new_dir` has no settings yet.
/// The old folder is left in place (a downgrade keeps working). Returns whether data moved.
pub fn migrate_legacy_data(new_dir: &Path) -> bool {
    if new_dir.join("settings.json").exists() {
        return false;
    }
    let Some(legacy) = new_dir.parent().map(|p| p.join(LEGACY_IDENTIFIER)) else {
        return false;
    };
    if !legacy.join("settings.json").is_file() {
        return false;
    }
    for name in ["settings.json", "samsung-home"] {
        let from = legacy.join(name);
        let to = new_dir.join(name);
        let result = if from.is_dir() {
            copy_dir(&from, &to)
        } else if from.is_file() {
            std::fs::create_dir_all(new_dir).and_then(|_| std::fs::copy(&from, &to).map(|_| ()))
        } else {
            Ok(())
        };
        if result.is_err() {
            // Never leave a half-migrated settings file behind.
            let _ = std::fs::remove_file(new_dir.join("settings.json"));
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_settings_and_private_home_once() {
        let root = tempfile::tempdir().unwrap();
        let legacy = root.path().join(LEGACY_IDENTIFIER);
        std::fs::create_dir_all(legacy.join("samsung-home/nested")).unwrap();
        std::fs::write(legacy.join("settings.json"), r#"{"theme":"dark"}"#).unwrap();
        std::fs::write(legacy.join("samsung-home/.tizen.sdk.path.config"), "/sdk").unwrap();
        std::fs::write(legacy.join("samsung-home/nested/x"), "1").unwrap();
        let new_dir = root.path().join("io.github.volnalabs.easytizen");

        assert!(migrate_legacy_data(&new_dir));
        assert_eq!(
            std::fs::read_to_string(new_dir.join("settings.json")).unwrap(),
            r#"{"theme":"dark"}"#
        );
        assert_eq!(
            std::fs::read_to_string(new_dir.join("samsung-home/.tizen.sdk.path.config")).unwrap(),
            "/sdk"
        );
        assert!(new_dir.join("samsung-home/nested/x").is_file());
        assert!(
            legacy.join("settings.json").is_file(),
            "legacy data is kept"
        );

        // A second start must not overwrite what the user changed since.
        std::fs::write(new_dir.join("settings.json"), r#"{"theme":"light"}"#).unwrap();
        assert!(!migrate_legacy_data(&new_dir));
        assert_eq!(
            std::fs::read_to_string(new_dir.join("settings.json")).unwrap(),
            r#"{"theme":"light"}"#
        );
    }

    #[test]
    fn nothing_to_migrate() {
        let root = tempfile::tempdir().unwrap();
        assert!(!migrate_legacy_data(
            &root.path().join("io.github.volnalabs.easytizen")
        ));
    }
}
