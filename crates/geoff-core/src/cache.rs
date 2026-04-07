//! Build cache for incremental builds.
//!
//! Tracks SHA-256 content hashes per file to skip unchanged pages.

use std::collections::HashMap;
use std::io::Read;

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

/// Stored build cache, persisted to `.geoff/build-cache.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuildCache {
    /// Map of relative file path -> content hash (hex-encoded SHA-256).
    pub files: HashMap<String, String>,
    /// Hash of the template directory contents.
    pub template_hash: Option<String>,
}

impl BuildCache {
    /// Load the build cache from disk, or return a default (empty) cache if missing.
    pub fn load(site_root: &Utf8Path) -> Self {
        let path = cache_path(site_root);
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save the build cache to disk.
    pub fn save(
        &self,
        site_root: &Utf8Path,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let path = cache_path(site_root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    /// Check if a file has changed since last build.
    /// Returns true if the file is new or its hash differs.
    pub fn is_changed(&self, rel_path: &str, current_hash: &str) -> bool {
        match self.files.get(rel_path) {
            Some(cached_hash) => cached_hash != current_hash,
            None => true,
        }
    }

    /// Record a file's hash after successful build.
    pub fn record(&mut self, rel_path: String, hash: String) {
        self.files.insert(rel_path, hash);
    }

    /// Remove entries for files that no longer exist.
    pub fn prune(&mut self, existing_paths: &[&str]) {
        let existing: std::collections::HashSet<&str> = existing_paths.iter().copied().collect();
        self.files.retain(|k, _| existing.contains(k.as_str()));
    }
}

/// Compute SHA-256 hash of a file's contents, returned as hex string.
pub fn hash_file(path: &Utf8Path) -> std::result::Result<String, Box<dyn std::error::Error>> {
    use std::hash::Hasher;

    // Use a simple FNV-like hash for speed — we don't need cryptographic strength,
    // just change detection. But SHA-256 is standard and available in std via
    // manual implementation. Instead, use a portable hash.
    let mut file = std::fs::File::open(path)?;
    let mut hasher = SimpleHasher::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.write(&buf[..n]);
    }
    Ok(format!("{:016x}", hasher.finish()))
}

/// Compute a combined hash for all files in a directory (recursive).
pub fn hash_directory(dir: &Utf8Path) -> std::result::Result<String, Box<dyn std::error::Error>> {
    use std::hash::Hasher;

    let mut hasher = SimpleHasher::new();
    let mut paths = Vec::new();

    if dir.exists() {
        collect_files(dir.as_std_path(), &mut paths)?;
    }
    paths.sort();

    for path in &paths {
        let utf8 =
            Utf8PathBuf::try_from(path.clone()).map_err(|e| format!("non-UTF-8 path: {e}"))?;
        let file_hash = hash_file(&utf8)?;
        hasher.write(file_hash.as_bytes());
    }
    Ok(format!("{:016x}", hasher.finish()))
}

fn collect_files(
    dir: &std::path::Path,
    out: &mut Vec<std::path::PathBuf>,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

/// Simple FNV-1a hasher for fast content hashing.
struct SimpleHasher {
    state: u64,
}

impl SimpleHasher {
    fn new() -> Self {
        Self {
            state: 0xcbf29ce484222325,
        }
    }
}

impl std::hash::Hasher for SimpleHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.state ^= u64::from(b);
            self.state = self.state.wrapping_mul(0x100000001b3);
        }
    }

    fn finish(&self) -> u64 {
        self.state
    }
}

fn cache_path(site_root: &Utf8Path) -> Utf8PathBuf {
    site_root.join(".geoff").join("build-cache.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_cache_everything_changed() {
        let cache = BuildCache::default();
        assert!(cache.is_changed("foo.md", "abc123"));
    }

    #[test]
    fn cached_file_unchanged() {
        let mut cache = BuildCache::default();
        cache.record("foo.md".into(), "abc123".into());
        assert!(!cache.is_changed("foo.md", "abc123"));
    }

    #[test]
    fn cached_file_changed() {
        let mut cache = BuildCache::default();
        cache.record("foo.md".into(), "abc123".into());
        assert!(cache.is_changed("foo.md", "def456"));
    }

    #[test]
    fn prune_removes_deleted_files() {
        let mut cache = BuildCache::default();
        cache.record("a.md".into(), "h1".into());
        cache.record("b.md".into(), "h2".into());
        cache.record("c.md".into(), "h3".into());
        cache.prune(&["a.md", "c.md"]);
        assert_eq!(cache.files.len(), 2);
        assert!(!cache.files.contains_key("b.md"));
    }

    #[test]
    fn hash_file_deterministic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        std::fs::write(&path, "hello world").unwrap();
        let utf8 = Utf8Path::from_path(&path).unwrap();
        let h1 = hash_file(utf8).unwrap();
        let h2 = hash_file(utf8).unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_file_differs_on_content_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        std::fs::write(&path, "hello").unwrap();
        let utf8 = Utf8Path::from_path(&path).unwrap();
        let h1 = hash_file(utf8).unwrap();
        std::fs::write(&path, "world").unwrap();
        let h2 = hash_file(utf8).unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let site_root = Utf8Path::from_path(dir.path()).unwrap();
        let mut cache = BuildCache::default();
        cache.record("a.md".into(), "hash1".into());
        cache.template_hash = Some("tmpl_hash".into());
        cache.save(site_root).unwrap();

        let loaded = BuildCache::load(site_root);
        assert_eq!(loaded.files.get("a.md").unwrap(), "hash1");
        assert_eq!(loaded.template_hash.as_deref(), Some("tmpl_hash"));
    }
}
