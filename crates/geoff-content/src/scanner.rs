use camino::{Utf8Path, Utf8PathBuf};
use ignore::WalkBuilder;

/// Scan a directory for Markdown content files, respecting .gitignore.
pub fn scan_content_dir(
    dir: &Utf8Path,
) -> std::result::Result<Vec<Utf8PathBuf>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();

    for entry in WalkBuilder::new(dir.as_std_path()).build() {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && let Some(ext) = path.extension()
            && (ext == "md" || ext == "markdown")
        {
            let utf8 = Utf8PathBuf::try_from(path.to_path_buf())
                .map_err(|e| format!("Non-UTF8 path: {e}"))?;
            files.push(utf8);
        }
    }

    files.sort();
    Ok(files)
}

/// Scan a directory for Turtle (.ttl) RDF data files, respecting .gitignore.
pub fn scan_data_dir(
    dir: &Utf8Path,
) -> std::result::Result<Vec<Utf8PathBuf>, Box<dyn std::error::Error>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in WalkBuilder::new(dir.as_std_path()).build() {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && let Some(ext) = path.extension()
            && ext == "ttl"
        {
            let utf8 = Utf8PathBuf::try_from(path.to_path_buf())
                .map_err(|e| format!("Non-UTF8 path: {e}"))?;
            files.push(utf8);
        }
    }
    files.sort();
    Ok(files)
}

/// Check if a sidecar .ttl file exists for the given Markdown file.
pub fn sidecar_ttl_path(md_path: &Utf8Path) -> Option<Utf8PathBuf> {
    let ttl_path = md_path.with_extension("ttl");
    if ttl_path.exists() {
        Some(ttl_path)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let path = Utf8Path::from_path(dir.path()).unwrap();
        let files = scan_content_dir(path).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn scan_finds_markdown_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("post.md"), "# Hello").unwrap();
        std::fs::write(dir.path().join("readme.txt"), "ignore").unwrap();
        let path = Utf8Path::from_path(dir.path()).unwrap();
        let files = scan_content_dir(path).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].as_str().ends_with("post.md"));
    }

    #[test]
    fn scan_data_dir_finds_ttl() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("people.ttl"), "@prefix : <urn:> .").unwrap();
        std::fs::write(dir.path().join("readme.md"), "# ignore").unwrap();
        let path = Utf8Path::from_path(dir.path()).unwrap();
        let files = scan_data_dir(path).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].as_str().ends_with("people.ttl"));
    }

    #[test]
    fn scan_data_dir_nonexistent_returns_empty() {
        let path = Utf8Path::new("/nonexistent/data");
        let files = scan_data_dir(path).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn sidecar_ttl_found() {
        let dir = tempfile::tempdir().unwrap();
        let md_path = dir.path().join("post.md");
        let ttl_path = dir.path().join("post.ttl");
        std::fs::write(&md_path, "# Hello").unwrap();
        std::fs::write(&ttl_path, "@prefix : <urn:> .").unwrap();
        let md_utf8 = Utf8Path::from_path(&md_path).unwrap();
        let result = sidecar_ttl_path(md_utf8);
        assert!(result.is_some());
        assert!(result.unwrap().as_str().ends_with("post.ttl"));
    }

    #[test]
    fn sidecar_ttl_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let md_path = dir.path().join("post.md");
        std::fs::write(&md_path, "# Hello").unwrap();
        let md_utf8 = Utf8Path::from_path(&md_path).unwrap();
        assert!(sidecar_ttl_path(md_utf8).is_none());
    }
}
