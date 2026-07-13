use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

/// Small, deterministic on-disk cache for final text-only completions.
pub struct ResponseCache {
    max_entries: usize,
}

impl ResponseCache {
    pub fn new(max_entries: usize) -> Self {
        Self { max_entries }
    }

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        if self.max_entries == 0 {
            return Ok(None);
        }
        let path = self.path_for(key)?;
        match fs::read_to_string(&path) {
            Ok(value) => Ok(Some(value)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => {
                Err(error).with_context(|| format!("reading cache entry {}", path.display()))
            }
        }
    }

    pub fn put(&self, key: &str, value: &str) -> Result<()> {
        if self.max_entries == 0 {
            return Ok(());
        }
        let dir = Self::storage_dir()?;
        crate::fsutil::create_private_dir(&dir)
            .with_context(|| format!("creating cache directory {}", dir.display()))?;
        crate::fsutil::write_private(&self.path_for(key)?, value)
            .context("writing response cache entry")?;
        self.prune(&dir)
    }

    fn prune(&self, dir: &std::path::Path) -> Result<()> {
        let mut entries: Vec<_> = fs::read_dir(dir)?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
            .collect();
        entries.sort_by_key(|(modified, _)| *modified);
        let excess = entries.len().saturating_sub(self.max_entries);
        for (_, path) in entries.into_iter().take(excess) {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    fn path_for(&self, key: &str) -> Result<PathBuf> {
        Ok(Self::storage_dir()?.join(format!("{:016x}", hash(key))))
    }
    fn storage_dir() -> Result<PathBuf> {
        let base = dirs::config_dir().context("could not determine config directory")?;
        Ok(base.join("rgpt").join("response_cache"))
    }
}

/// FNV-1a is deliberately stable across process restarts. This is a cache
/// address key, not a security boundary.
fn hash(value: &str) -> u64 {
    value
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        })
}

#[cfg(test)]
mod tests {
    use super::hash;
    #[test]
    fn cache_key_is_repeatable_within_process() {
        assert_eq!(hash("request"), hash("request"));
    }
}
