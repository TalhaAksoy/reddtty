//! Built-in configuration presets.
//!
//! The TOML files under `configs/` are embedded in the binary so users who
//! install ReddTTY without cloning the repository can still choose a working
//! starter config from the first-run/settings UI.

use std::fs;
use std::path::{Path, PathBuf};

pub struct ConfigPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub contents: &'static str,
}

pub const PRESETS: &[ConfigPreset] = &[
    ConfigPreset {
        id: "demo",
        name: "Demo (offline)",
        description: "Synthetic posts and comments. No network or Reddit account.",
        contents: include_str!("../../configs/demo.toml"),
    },
    ConfigPreset {
        id: "redlib-hbubli",
        name: "Redlib · hbubli.cc",
        description: "Third-party Redlib instance; manually smoke-tested.",
        contents: include_str!("../../configs/redlib-hbubli.toml"),
    },
    ConfigPreset {
        id: "redlib-zanity",
        name: "Redlib · zanity.net",
        description: "Third-party Redlib instance; manually smoke-tested.",
        contents: include_str!("../../configs/redlib-zanity.toml"),
    },
    ConfigPreset {
        id: "safereddit",
        name: "Redlib · safereddit.com",
        description: "Third-party Redlib instance; some listings may be blocked.",
        contents: include_str!("../../configs/safereddit.toml"),
    },
];

pub fn find(id: &str) -> Option<&'static ConfigPreset> {
    PRESETS.iter().find(|preset| preset.id == id)
}

/// Back up an existing config before replacing it. Returns `None` if the file
/// does not exist.
pub fn backup_existing(destination: &Path) -> Result<Option<PathBuf>, String> {
    if !destination.is_file() {
        return Ok(None);
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let mut backup = destination.with_extension(format!("toml.bckp_{stamp}"));
    let mut suffix = 1;
    while backup.exists() {
        backup = destination.with_extension(format!("toml.bckp_{stamp}_{suffix}"));
        suffix += 1;
    }
    fs::copy(destination, &backup)
        .map_err(|error| format!("could not back up {}: {error}", destination.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&backup, fs::Permissions::from_mode(0o600));
    }
    Ok(Some(backup))
}

pub struct InstallResult {
    pub destination: PathBuf,
    pub backup: Option<PathBuf>,
}

/// Install a preset without ever overwriting an existing config silently.
/// The old file is copied to `config.toml.bckp_<timestamp>` first.
pub fn install(id: &str, destination: &Path) -> Result<InstallResult, String> {
    let preset = find(id).ok_or_else(|| format!("unknown config preset `{id}`"))?;
    let parent = destination
        .parent()
        .ok_or_else(|| "config path has no parent directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;

    let backup = backup_existing(destination)?;

    let tmp = parent.join(format!(".config.toml.installing-{}", std::process::id()));
    fs::write(&tmp, preset.contents)
        .map_err(|error| format!("could not write {}: {error}", tmp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }
    if let Err(error) = fs::rename(&tmp, destination) {
        let _ = fs::remove_file(&tmp);
        return Err(format!(
            "could not install {}: {error}",
            destination.display()
        ));
    }
    Ok(InstallResult {
        destination: destination.to_path_buf(),
        backup,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;

    fn temp_path(label: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "reddtty-preset-{label}-{}-{stamp}/config.toml",
            std::process::id()
        ))
    }

    #[test]
    fn every_preset_parses_as_settings() {
        for preset in PRESETS {
            let settings: Settings = toml::from_str(preset.contents)
                .unwrap_or_else(|error| panic!("{}: {error}", preset.id));
            assert!(!settings.general.default_feed.is_empty());
        }
    }

    #[test]
    fn install_backs_up_an_existing_config() {
        let path = temp_path("backup");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "favorites = []\n").unwrap();
        let result = install("demo", &path).unwrap();
        assert!(
            result
                .backup
                .as_ref()
                .is_some_and(|backup| backup.is_file())
        );
        assert!(path.is_file());
        let (loaded, warnings) = Settings::load(&path);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(loaded.provider.kind, crate::config::ProviderKind::Demo);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
