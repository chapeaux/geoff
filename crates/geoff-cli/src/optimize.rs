//! Post-build asset optimization: CSS/JS minification, cache-busting hashes,
//! and image conversion to WebP.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use geoff_core::config::OptimizeConfig;
use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

/// Run all configured optimizations on the output directory.
///
/// The order matters: minify first, then hash (so hashes reflect minified content),
/// then images (independent of CSS/JS pipeline).
pub fn optimize_assets(
    output_dir: &Path,
    config: &OptimizeConfig,
) -> Result<OptimizeStats, Box<dyn std::error::Error>> {
    let mut stats = OptimizeStats::default();

    if config.minify_css {
        stats.css_minified = minify_css_files(output_dir)?;
    }
    if config.minify_js {
        stats.js_minified = minify_js_files(output_dir)?;
    }
    if config.hash_assets {
        stats.assets_hashed = hash_assets(output_dir)?;
    }
    if config.images.webp {
        stats.images_converted =
            convert_images_to_webp(output_dir, config.images.quality, config.images.max_width)?;
    }

    Ok(stats)
}

/// Statistics from the optimization pass.
#[derive(Debug, Default)]
pub struct OptimizeStats {
    pub css_minified: usize,
    pub js_minified: usize,
    pub assets_hashed: usize,
    pub images_converted: usize,
}

// ---------------------------------------------------------------------------
// CSS minification
// ---------------------------------------------------------------------------

fn collect_files_by_extension(dir: &Path, ext: &str) -> Vec<PathBuf> {
    WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case(ext))
        })
        .map(|e| e.into_path())
        .collect()
}

fn minify_css_file(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(path)?;
    let mut stylesheet = StyleSheet::parse(&source, ParserOptions::default())
        .map_err(|e| format!("CSS parse error in {}: {e}", path.display()))?;
    stylesheet.minify(MinifyOptions::default())?;
    let result = stylesheet.to_css(PrinterOptions {
        minify: true,
        ..Default::default()
    })?;
    std::fs::write(path, result.code)?;
    Ok(())
}

fn minify_css_files(output_dir: &Path) -> Result<usize, Box<dyn std::error::Error>> {
    let files = collect_files_by_extension(output_dir, "css");
    for file in &files {
        minify_css_file(file)?;
    }
    Ok(files.len())
}

// ---------------------------------------------------------------------------
// JS minification (SWC)
// ---------------------------------------------------------------------------

fn minify_js_file(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use swc_common::{FileName, GLOBALS, Globals, SourceMap, sync::Lrc};
    use swc_ecma_codegen::{Emitter, text_writer::JsWriter};
    use swc_ecma_parser::{EsSyntax, Syntax};

    let source = std::fs::read_to_string(path)?;
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(Lrc::new(FileName::Real(path.to_path_buf())), source);

    let globals = Globals::new();
    let minified_code = GLOBALS.set(
        &globals,
        || -> Result<Vec<u8>, Box<dyn std::error::Error>> {
            let mut errors = vec![];
            let module = swc_ecma_parser::parse_file_as_module(
                &fm,
                Syntax::Es(EsSyntax::default()),
                swc_ecma_ast::EsVersion::EsNext,
                None,
                &mut errors,
            )
            .map_err(|e| format!("parse error in {}: {e:?}", path.display()))?;

            let mut buf = vec![];
            {
                let wr = JsWriter::new(cm.clone(), "\n", &mut buf, None);
                let mut emitter = Emitter {
                    cfg: swc_ecma_codegen::Config::default().with_minify(true),
                    cm,
                    comments: None,
                    wr: Box::new(wr),
                };
                emitter.emit_module(&module)?;
            }
            Ok(buf)
        },
    )?;

    std::fs::write(path, minified_code)?;
    Ok(())
}

fn minify_js_files(output_dir: &Path) -> Result<usize, Box<dyn std::error::Error>> {
    let files = collect_files_by_extension(output_dir, "js");
    for file in &files {
        if let Err(e) = minify_js_file(file) {
            eprintln!("warning: JS minify failed for {}: {e}", file.display());
        }
    }
    Ok(files.len())
}

// ---------------------------------------------------------------------------
// Cache-busting hashes
// ---------------------------------------------------------------------------

fn content_hash(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    // First 8 hex characters
    format!("{:x}", digest)[..8].to_string()
}

fn hash_assets(output_dir: &Path) -> Result<usize, Box<dyn std::error::Error>> {
    // Collect CSS and JS files
    let mut asset_files: Vec<PathBuf> = Vec::new();
    asset_files.extend(collect_files_by_extension(output_dir, "css"));
    asset_files.extend(collect_files_by_extension(output_dir, "js"));

    if asset_files.is_empty() {
        return Ok(0);
    }

    // Map from original filename (relative to output_dir) to hashed filename
    let mut renames: HashMap<String, String> = HashMap::new();

    for file in &asset_files {
        let content = std::fs::read(file)?;
        let hash = content_hash(&content);

        let stem = file
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| format!("Invalid filename: {}", file.display()))?;
        let ext = file
            .extension()
            .and_then(|s| s.to_str())
            .ok_or_else(|| format!("No extension: {}", file.display()))?;

        let hashed_name = format!("{stem}.{hash}.{ext}");
        let hashed_path = file.with_file_name(&hashed_name);

        // Compute original relative path for HTML replacement
        let rel_original = file
            .strip_prefix(output_dir)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        let rel_hashed = hashed_path
            .strip_prefix(output_dir)
            .unwrap_or(&hashed_path)
            .to_string_lossy()
            .replace('\\', "/");

        std::fs::rename(file, &hashed_path)?;
        renames.insert(rel_original, rel_hashed);
    }

    // Update HTML files to reference hashed filenames
    let html_files = collect_files_by_extension(output_dir, "html");
    for html_file in &html_files {
        let mut html = std::fs::read_to_string(html_file)?;
        let mut changed = false;
        for (original, hashed) in &renames {
            // Replace both bare references and prefixed with /
            if html.contains(original.as_str()) {
                html = html.replace(original.as_str(), hashed.as_str());
                changed = true;
            }
        }
        if changed {
            std::fs::write(html_file, html)?;
        }
    }

    Ok(renames.len())
}

// ---------------------------------------------------------------------------
// Image optimization (WebP conversion)
// ---------------------------------------------------------------------------

fn convert_images_to_webp(
    output_dir: &Path,
    quality: u8,
    max_width: Option<u32>,
) -> Result<usize, Box<dyn std::error::Error>> {
    let mut image_files: Vec<PathBuf> = Vec::new();
    image_files.extend(collect_files_by_extension(output_dir, "png"));
    image_files.extend(collect_files_by_extension(output_dir, "jpg"));
    image_files.extend(collect_files_by_extension(output_dir, "jpeg"));

    let mut converted = 0;
    for file in &image_files {
        match convert_single_image(file, quality, max_width) {
            Ok(()) => converted += 1,
            Err(e) => {
                eprintln!("warning: failed to convert {} to WebP: {e}", file.display());
            }
        }
    }

    Ok(converted)
}

fn convert_single_image(
    path: &Path,
    _quality: u8,
    max_width: Option<u32>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut img = image::open(path)?;

    // Resize if wider than max_width, preserving aspect ratio
    if let Some(mw) = max_width
        && img.width() > mw
    {
        img = img.resize(mw, u32::MAX, image::imageops::FilterType::Lanczos3);
    }

    let webp_path = path.with_extension("webp");

    // The image crate 0.25 supports lossless WebP encoding.
    // The quality parameter is accepted in config for forward-compatibility
    // but the current encoder uses lossless compression, which typically
    // produces smaller files than PNG while preserving exact pixel data.
    img.save(&webp_path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_deterministic() {
        let h1 = content_hash(b"hello world");
        let h2 = content_hash(b"hello world");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 8);
    }

    #[test]
    fn content_hash_differs_for_different_input() {
        let h1 = content_hash(b"hello");
        let h2 = content_hash(b"world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_assets_renames_and_updates_html() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path();

        // Create a CSS file
        std::fs::write(dir_path.join("style.css"), "body { color: red; }").unwrap();
        // Create an HTML file referencing it
        std::fs::write(
            dir_path.join("index.html"),
            r#"<link rel="stylesheet" href="style.css">"#,
        )
        .unwrap();

        let count = hash_assets(dir_path).unwrap();
        assert_eq!(count, 1);

        // The original style.css should be gone
        assert!(!dir_path.join("style.css").exists());

        // There should be a style.<hash>.css file
        let css_files = collect_files_by_extension(dir_path, "css");
        assert_eq!(css_files.len(), 1);
        let css_name = css_files[0].file_name().unwrap().to_str().unwrap();
        assert!(css_name.starts_with("style."));
        assert!(css_name.ends_with(".css"));
        assert!(css_name.len() > "style..css".len()); // has a hash

        // The HTML should reference the hashed filename
        let html = std::fs::read_to_string(dir_path.join("index.html")).unwrap();
        assert!(html.contains(css_name));
        assert!(!html.contains("style.css\""));
    }

    #[test]
    fn css_minification_reduces_size() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path();

        let css = "body {\n    color: red;\n    margin: 0;\n}\n";
        std::fs::write(dir_path.join("test.css"), css).unwrap();

        let count = minify_css_files(dir_path).unwrap();
        assert_eq!(count, 1);

        let minified = std::fs::read_to_string(dir_path.join("test.css")).unwrap();
        assert!(
            minified.len() < css.len(),
            "Minified CSS ({} bytes) should be smaller than original ({} bytes)",
            minified.len(),
            css.len()
        );
        assert!(minified.contains("color"));
    }

    #[test]
    fn optimize_noop_when_all_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let config = OptimizeConfig::default();
        let stats = optimize_assets(dir.path(), &config).unwrap();
        assert_eq!(stats.css_minified, 0);
        assert_eq!(stats.js_minified, 0);
        assert_eq!(stats.assets_hashed, 0);
        assert_eq!(stats.images_converted, 0);
    }
}
