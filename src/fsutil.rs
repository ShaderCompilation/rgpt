use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result, bail};

/// Creates a directory (and any missing parents) that only the owner can
/// access: mode 0700 on Unix, so other local users can't list saved chats,
/// roles, cached responses, or the config that holds the API key. A no-op for
/// permissions on non-Unix targets, which don't have this permission model.
pub fn create_private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("creating directory {}", path.display()))?;
    set_private_dir_permissions(path)
}

/// Writes `contents` to `path` with owner-only permissions (mode 0600 on Unix).
/// The file is created 0600 from the start — not created world-readable and
/// then narrowed — so a secret (the API key) is never briefly exposed. An
/// existing file is also re-narrowed, migrating configs written by older
/// versions that predate this hardening.
pub fn write_private(path: &Path, contents: &str) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .with_context(|| format!("opening file {}", path.display()))?;
    file.write_all(contents.as_bytes())
        .with_context(|| format!("writing file {}", path.display()))?;
    // `mode()` only applies when the file is newly created; enforce 0600 on a
    // pre-existing (possibly world-readable) file too.
    set_private_file_permissions(path)
}

/// Creates (truncating) a file for writing with owner-only permissions (mode
/// 0600 on Unix) and returns the open handle, for callers that append to it
/// over time rather than writing once. Like [`write_private`], the file is
/// created 0600 from the start and any pre-existing file is re-narrowed.
pub fn create_private_file(path: &Path) -> Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(path)
        .with_context(|| format!("creating file {}", path.display()))?;
    set_private_file_permissions(path)?;
    Ok(file)
}

/// Rejects an identifier that is used verbatim as a filename under a storage
/// directory. Chat ids and role names arrive from argv and are joined directly
/// onto a base directory, so a value like `../../etc/passwd` or an absolute
/// path would otherwise let a read/write/delete escape that directory.
pub fn validate_identifier(kind: &str, name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("{kind} name must not be empty");
    }
    if name == "." || name == ".." {
        bail!("{kind} name {name:?} is not allowed");
    }
    if name.contains('/') || name.contains('\\') || name.contains('\0') {
        bail!("{kind} name must not contain path separators or null bytes: {name:?}");
    }
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("setting permissions on {}", path.display()))
}

#[cfg(unix)]
fn set_private_dir_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("setting permissions on {}", path.display()))
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(not(unix))]
fn set_private_dir_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_traversal_and_separators() {
        for bad in ["", ".", "..", "../etc", "a/b", "a\\b", "x\0y", "/abs"] {
            assert!(validate_identifier("chat", bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn accepts_ordinary_names() {
        for ok in ["temp", "my-chat", "ShellGPT", "Shell Command Generator", "a..b"] {
            assert!(validate_identifier("chat", ok).is_ok(), "{ok:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn written_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!("rgpt-fsutil-{}", std::process::id()));
        write_private(&path, "secret").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        std::fs::remove_file(&path).ok();
    }
}
