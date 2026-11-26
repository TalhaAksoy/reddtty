//! Shared, defensive file persistence for config and local library data.
//!
//! Writes use a unique temporary file in the destination directory, flush it,
//! and replace the destination only after the new bytes are durable. Corrupt
//! files are copied to a unique recovery backup before callers may replace them.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistenceError(pub String);

impl fmt::Display for PersistenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PersistenceError {}

fn unique_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{nanos}-{counter}", std::process::id())
}

fn temporary_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "data".into());
    parent.join(format!(".{name}.{}.tmp", unique_suffix()))
}

fn recovery_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "data".into());
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%S");
    parent.join(format!("{name}.corrupt-{stamp}-{}", unique_suffix()))
}

/// An advisory inter-process lock. The lock file itself is never deleted, so a
/// crashed process cannot leave an unbreakable stale lock; the operating system
/// releases the lock when the handle closes.
#[derive(Debug)]
pub struct FileLock {
    _file: File,
}

impl FileLock {
    pub fn acquire(path: &Path, timeout: Duration) -> Result<Self, PersistenceError> {
        let lock_path = path.with_extension("lock");
        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                PersistenceError(format!(
                    "Could not create lock directory {}: {error}",
                    parent.display()
                ))
            })?;
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|error| {
                PersistenceError(format!(
                    "Could not open lock file {}: {error}",
                    lock_path.display()
                ))
            })?;
        let deadline = Instant::now() + timeout;
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Self { _file: file }),
                Err(std::fs::TryLockError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(std::fs::TryLockError::WouldBlock) => {
                    return Err(PersistenceError(format!(
                        "Timed out waiting for another ReddTTY process to finish writing {}",
                        path.display()
                    )));
                }
                Err(std::fs::TryLockError::Error(error)) => {
                    return Err(PersistenceError(format!(
                        "Could not lock {}: {error}",
                        path.display()
                    )));
                }
            }
        }
    }
}

/// Write bytes through a unique sibling temporary file and replace the
/// destination with the platform's rename operation. Rust's Windows rename
/// uses `MoveFileExW(..., MOVEFILE_REPLACE_EXISTING)`, so the destination is
/// not deleted first. This gives atomic visibility on Unix and Windows; it
/// does not by itself claim power-loss durability on every filesystem.
pub fn atomic_write(path: &Path, bytes: &[u8], sensitive: bool) -> Result<(), PersistenceError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PersistenceError(format!("Could not create {}: {error}", parent.display()))
        })?;
    }
    let temp = temporary_path(path);
    let result = (|| -> Result<(), PersistenceError> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        if sensitive {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp).map_err(|error| {
            PersistenceError(format!("Could not create {}: {error}", temp.display()))
        })?;
        file.write_all(bytes).map_err(|error| {
            PersistenceError(format!("Could not write {}: {error}", temp.display()))
        })?;
        file.flush().map_err(|error| {
            PersistenceError(format!("Could not flush {}: {error}", temp.display()))
        })?;
        file.sync_all().map_err(|error| {
            PersistenceError(format!("Could not sync {}: {error}", temp.display()))
        })?;
        #[cfg(unix)]
        if sensitive {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&temp, fs::Permissions::from_mode(0o600));
        }

        fs::rename(&temp, path).map_err(|error| {
            PersistenceError(format!("Could not replace {}: {error}", path.display()))
        })?;
        #[cfg(unix)]
        if let Some(parent) = path.parent()
            && let Ok(directory) = File::open(parent)
        {
            // Best-effort directory durability. Some filesystems do not
            // support fsync on directories; the file itself was already synced.
            let _ = directory.sync_all();
        }
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// Copy a corrupt file to a unique recovery path without overwriting any
/// existing backup. The original file is left untouched.
pub fn backup_corrupt(path: &Path) -> Result<PathBuf, PersistenceError> {
    let bytes = fs::read(path)
        .map_err(|error| PersistenceError(format!("Could not read {}: {error}", path.display())))?;
    for _ in 0..8 {
        let backup = recovery_path(path);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&backup) {
            Ok(mut file) => {
                file.write_all(&bytes).map_err(|error| {
                    PersistenceError(format!(
                        "Could not write recovery backup {}: {error}",
                        backup.display()
                    ))
                })?;
                file.flush().map_err(|error| {
                    PersistenceError(format!(
                        "Could not flush recovery backup {}: {error}",
                        backup.display()
                    ))
                })?;
                file.sync_all().map_err(|error| {
                    PersistenceError(format!(
                        "Could not sync recovery backup {}: {error}",
                        backup.display()
                    ))
                })?;
                return Ok(backup);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(PersistenceError(format!(
                    "Could not create recovery backup for {}: {error}",
                    path.display()
                )));
            }
        }
    }
    Err(PersistenceError(format!(
        "Could not allocate a unique recovery backup for {}",
        path.display()
    )))
}

pub fn write_private(path: &Path, bytes: &[u8]) -> Result<(), PersistenceError> {
    atomic_write(path, bytes, true)
}

/// Warn about unknown top-level keys so a typo is not silently replaced by a
/// default value on the next save.
pub fn unknown_top_level_keys(text: &str, known: &[&str]) -> Vec<String> {
    let Ok(toml::Value::Table(table)) = text.parse::<toml::Value>() else {
        return Vec::new();
    };
    table
        .keys()
        .filter(|key| !known.contains(&key.as_str()))
        .cloned()
        .collect()
}

pub fn read_text(path: &Path) -> Result<Option<String>, PersistenceError> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(PersistenceError(format!(
            "Could not read {}: {error}",
            path.display()
        ))),
    }
}

/// Open a file for tests/utilities that need an empty private file.
pub fn create_private(path: &Path) -> Result<File, PersistenceError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .map_err(|error| PersistenceError(format!("Could not create {}: {error}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("reddtty-persist-{tag}-{}", unique_suffix()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn atomic_write_replaces_and_preserves_bytes() {
        let dir = temp_dir("replace");
        let path = dir.join("data.toml");
        atomic_write(&path, b"old", true).unwrap();
        atomic_write(&path, b"new", true).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new");
        let leftovers = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp"))
            .count();
        assert_eq!(leftovers, 0);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn corrupt_backup_never_overwrites_and_keeps_original() {
        let dir = temp_dir("backup");
        let path = dir.join("library.toml");
        fs::write(&path, b"broken [").unwrap();
        let first = backup_corrupt(&path).unwrap();
        let second = backup_corrupt(&path).unwrap();
        assert_ne!(first, second);
        assert_eq!(fs::read(&path).unwrap(), b"broken [");
        assert_eq!(fs::read(&first).unwrap(), b"broken [");
        assert_eq!(fs::read(&second).unwrap(), b"broken [");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn write_private_uses_restrictive_permissions_on_unix() {
        let dir = temp_dir("mode");
        let path = dir.join("secret.toml");
        write_private(&path, b"secret").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let _ = fs::remove_dir_all(dir);
    }
}

#[cfg(test)]
mod lock_tests {
    use super::*;

    #[test]
    fn a_second_writer_times_out_instead_of_blocking_forever() {
        let dir = std::env::temp_dir().join(format!("reddtty-lock-{}", unique_suffix()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("library.toml");
        let _held = FileLock::acquire(&path, Duration::from_millis(50)).unwrap();
        let error = FileLock::acquire(&path, Duration::from_millis(30)).unwrap_err();
        assert!(error.to_string().contains("Timed out"), "{error}");
        drop(_held);
        assert!(FileLock::acquire(&path, Duration::from_millis(50)).is_ok());
        let _ = fs::remove_dir_all(dir);
    }
}
