//! Publish subcommands: download (ZIP), GitHub Pages, and OpenShift deployment.

use std::io::Write;
use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};
use walkdir::WalkDir;

use crate::{Verbosity, cmd_build};

/// Create a ZIP archive of the built site.
pub async fn cmd_publish_download(
    path: &Utf8Path,
    output: Option<&Utf8Path>,
    v: Verbosity,
) -> Result<(), Box<dyn std::error::Error>> {
    cmd_build(path, false, v).await?;

    let config_path = path.join("geoff.toml");
    let config = geoff_core::config::SiteConfig::from_file(&config_path)
        .map_err(|e| format!("Failed to load {config_path}: {e}"))?;

    let dist_dir = path.join(&config.output_dir);
    if !dist_dir.exists() {
        return Err(format!("Output directory not found: {dist_dir}").into());
    }

    let slug = config
        .title
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric(), "-");
    let slug = slug.trim_matches('-');
    let default_name = format!("{slug}.zip");
    let output_path = output
        .map(Utf8PathBuf::from)
        .unwrap_or_else(|| path.join(&default_name));

    let file = std::fs::File::create(&output_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for entry in WalkDir::new(dist_dir.as_std_path()) {
        let entry = entry?;
        let entry_path = entry.path();
        let rel = entry_path
            .strip_prefix(dist_dir.as_std_path())
            .unwrap_or(entry_path);

        if entry.file_type().is_dir() {
            if rel.as_os_str().is_empty() {
                continue;
            }
            let dir_name = format!("{}/", rel.to_string_lossy().replace('\\', "/"));
            zip.add_directory(&dir_name, options)?;
        } else {
            let file_name = rel.to_string_lossy().replace('\\', "/");
            zip.start_file(&file_name, options)?;
            let data = std::fs::read(entry_path)?;
            zip.write_all(&data)?;
        }
    }

    zip.finish()?;

    let metadata = std::fs::metadata(&output_path)?;
    let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);

    v.success(&format!("Created {output_path} ({size_mb:.1} MB)"));
    Ok(())
}

/// Push the built site to the gh-pages branch.
pub async fn cmd_publish_github(
    path: &Utf8Path,
    v: Verbosity,
) -> Result<(), Box<dyn std::error::Error>> {
    cmd_build(path, false, v).await?;

    let config_path = path.join("geoff.toml");
    let config = geoff_core::config::SiteConfig::from_file(&config_path)
        .map_err(|e| format!("Failed to load {config_path}: {e}"))?;

    let dist_dir = path.join(&config.output_dir);
    if !dist_dir.exists() {
        return Err(format!("Output directory not found: {dist_dir}").into());
    }

    // Check we are in a git repo
    let status = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(path.as_std_path())
        .output()?;
    if !status.status.success() {
        return Err("Not inside a git repository. Initialize one with `git init`.".into());
    }

    // Stash current work
    let _ = Command::new("git")
        .args(["stash"])
        .current_dir(path.as_std_path())
        .output()?;

    // Try to checkout existing gh-pages, or create orphan branch
    let checkout = Command::new("git")
        .args(["checkout", "gh-pages"])
        .current_dir(path.as_std_path())
        .output()?;
    if !checkout.status.success() {
        let orphan = Command::new("git")
            .args(["checkout", "--orphan", "gh-pages"])
            .current_dir(path.as_std_path())
            .output()?;
        if !orphan.status.success() {
            let _ = restore_branch(path);
            return Err("Failed to create gh-pages branch.".into());
        }
    }

    // Remove all tracked files from the working tree
    let _ = Command::new("git")
        .args(["rm", "-rf", "."])
        .current_dir(path.as_std_path())
        .output()?;

    // Copy dist/* to the repo root
    copy_dir_contents(dist_dir.as_std_path(), path.as_std_path())?;

    // Stage everything and commit
    let add = Command::new("git")
        .args(["add", "-A"])
        .current_dir(path.as_std_path())
        .output()?;
    if !add.status.success() {
        let _ = restore_branch(path);
        return Err("Failed to stage files for gh-pages.".into());
    }

    let commit = Command::new("git")
        .args(["commit", "-m", "Deploy site"])
        .current_dir(path.as_std_path())
        .output()?;
    if !commit.status.success() {
        let stderr = String::from_utf8_lossy(&commit.stderr);
        // "nothing to commit" is not an error
        if !stderr.contains("nothing to commit") {
            let _ = restore_branch(path);
            return Err(format!("Failed to commit to gh-pages: {stderr}").into());
        }
    }

    // Push to remote
    let push = Command::new("git")
        .args(["push", "origin", "gh-pages", "--force"])
        .current_dir(path.as_std_path())
        .output()?;
    if !push.status.success() {
        let stderr = String::from_utf8_lossy(&push.stderr);
        v.warn(&format!(
            "Push failed (you may need to push manually): {stderr}"
        ));
    }

    // Return to previous branch and restore stash
    restore_branch(path)?;

    v.success("Pushed to gh-pages branch");
    Ok(())
}

/// Deploy the built site to OpenShift using `oc`.
pub async fn cmd_publish_openshift(
    path: &Utf8Path,
    name: Option<&str>,
    v: Verbosity,
) -> Result<(), Box<dyn std::error::Error>> {
    cmd_build(path, false, v).await?;

    let config_path = path.join("geoff.toml");
    let config = geoff_core::config::SiteConfig::from_file(&config_path)
        .map_err(|e| format!("Failed to load {config_path}: {e}"))?;

    let dist_dir = path.join(&config.output_dir);
    if !dist_dir.exists() {
        return Err(format!("Output directory not found: {dist_dir}").into());
    }

    // Check that oc is available
    let oc_check = Command::new("oc").arg("version").output();
    if oc_check.is_err() || !oc_check.unwrap().status.success() {
        return Err(
            "OpenShift CLI (`oc`) not found or not logged in. Install it and run `oc login` first."
                .into(),
        );
    }

    let app_name = name.unwrap_or_else(|| {
        config
            .title
            .split_whitespace()
            .next()
            .unwrap_or("geoff-site")
    });
    let app_name = app_name
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != '-', "-")
        .trim_matches('-')
        .to_string();

    // Create a temp directory for the build context
    let tmp = tempfile::tempdir()?;
    let tmp_path = tmp.path();

    // Write Dockerfile
    std::fs::write(
        tmp_path.join("Dockerfile"),
        "FROM registry.access.redhat.com/ubi9/httpd-24:latest\nCOPY dist/ /var/www/html/\n",
    )?;

    // Copy dist/ into the temp context
    let tmp_dist = tmp_path.join("dist");
    std::fs::create_dir_all(&tmp_dist)?;
    copy_dir_contents(dist_dir.as_std_path(), &tmp_dist)?;

    // Create build config if it does not exist
    let get_bc = Command::new("oc").args(["get", "bc", &app_name]).output()?;
    if !get_bc.status.success() {
        v.detail(&format!("Creating new build config: {app_name}"));
        let nb = Command::new("oc")
            .args([
                "new-build",
                "--binary",
                &format!("--name={app_name}"),
                "--strategy=docker",
            ])
            .current_dir(tmp_path)
            .output()?;
        if !nb.status.success() {
            let stderr = String::from_utf8_lossy(&nb.stderr);
            return Err(format!("Failed to create build config: {stderr}").into());
        }
    }

    // Start build
    v.detail("Starting build on OpenShift...");
    let sb = Command::new("oc")
        .args(["start-build", &app_name, "--from-dir=.", "--follow"])
        .current_dir(tmp_path)
        .status()?;
    if !sb.success() {
        return Err("OpenShift build failed.".into());
    }

    // Expose service if not already exposed
    let get_route = Command::new("oc")
        .args(["get", "route", &app_name])
        .output()?;
    if !get_route.status.success() {
        v.detail("Exposing service...");
        let expose = Command::new("oc")
            .args(["expose", &format!("svc/{app_name}")])
            .output()?;
        if !expose.status.success() {
            let stderr = String::from_utf8_lossy(&expose.stderr);
            v.warn(&format!("Could not expose service: {stderr}"));
        }
    }

    v.success(&format!("Deployed to OpenShift as {app_name}"));
    Ok(())
}

/// Switch back to the previous branch and pop the stash.
fn restore_branch(path: &Utf8Path) -> Result<(), Box<dyn std::error::Error>> {
    let _ = Command::new("git")
        .args(["checkout", "-"])
        .current_dir(path.as_std_path())
        .output()?;
    let _ = Command::new("git")
        .args(["stash", "pop"])
        .current_dir(path.as_std_path())
        .output()?;
    Ok(())
}

/// Publish a Geoff site to a Solid pod.
///
/// 1. Build the site.
/// 2. Resolve the bearer token from `token` arg or `SOLID_TOKEN` env var.
/// 3. PUT each file in `dist/` to `{pod_url}/geoff/site/{relative_path}`.
///
/// CLI interface: `geoff publish solid --pod https://paa.pub/ldary/ [--token TOKEN]`
pub async fn cmd_publish_solid(
    path: &Utf8Path,
    pod_url: &str,
    token: Option<&str>,
    v: Verbosity,
) -> Result<(), Box<dyn std::error::Error>> {
    // Resolve bearer token
    let bearer = match token {
        Some(t) => t.to_string(),
        None => std::env::var("SOLID_TOKEN").map_err(|_| {
            "No Solid token provided.\n\
             Pass --token TOKEN or set the SOLID_TOKEN environment variable.\n\
             You can generate a token from your pod's settings page."
        })?,
    };

    // Build the site
    cmd_build(path, false, v).await?;

    let config_path = path.join("geoff.toml");
    let config = geoff_core::config::SiteConfig::from_file(&config_path)
        .map_err(|e| format!("Failed to load {config_path}: {e}"))?;

    let output_dir = path.join(&config.output_dir);
    if !output_dir.exists() {
        return Err(format!("Output directory not found: {output_dir}").into());
    }

    // Normalize pod URL
    let base = normalize_pod_url(pod_url);
    let site_base = format!("{base}geoff/site/");

    v.success(&format!("Publishing to {site_base}"));

    // Walk dist/ and upload each file
    let client = reqwest::Client::new();
    let mut uploaded = 0u32;
    let mut errors = 0u32;

    for entry in WalkDir::new(output_dir.as_std_path()) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }

        let abs_path = entry.path();
        let rel = abs_path
            .strip_prefix(output_dir.as_std_path())
            .map_err(|e| format!("Path error: {e}"))?;
        let rel_str = rel.to_string_lossy();

        let url = format!("{site_base}{rel_str}");
        let content_type =
            mime_for_extension(abs_path.extension().and_then(|e| e.to_str()).unwrap_or(""));

        let body =
            std::fs::read(abs_path).map_err(|e| format!("Failed to read {}: {e}", rel_str))?;

        v.detail(&format!(
            "PUT {rel_str} ({content_type}, {} bytes)",
            body.len()
        ));

        let res = client
            .put(&url)
            .header("Authorization", format!("Bearer {bearer}"))
            .header("Content-Type", content_type)
            .body(body)
            .send()
            .await
            .map_err(|e| format!("Request failed for {rel_str}: {e}"))?;

        if res.status().is_success() || res.status().as_u16() == 201 {
            uploaded += 1;
        } else {
            eprintln!(
                "  warning: failed to upload {rel_str}: HTTP {}",
                res.status()
            );
            errors += 1;
        }
    }

    if errors > 0 {
        v.success(&format!(
            "Published {uploaded} file(s) with {errors} error(s) to {site_base}"
        ));
    } else {
        v.success(&format!("Published {uploaded} file(s) to {site_base}"));
    }

    Ok(())
}

/// Normalize a pod URL to end with `/`.
fn normalize_pod_url(url: &str) -> String {
    let mut u = url.to_string();
    if !u.starts_with("http") {
        u = format!("https://{u}");
    }
    if !u.ends_with('/') {
        u.push('/');
    }
    u
}

/// Map file extension to MIME content type.
fn mime_for_extension(ext: &str) -> &'static str {
    match ext {
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "mjs" => "application/javascript",
        "json" | "jsonld" => "application/json",
        "xml" => "application/xml",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "txt" => "text/plain",
        "nt" => "application/n-triples",
        "ttl" => "text/turtle",
        _ => "application/octet-stream",
    }
}

/// Copy the contents of `src` directory into `dst` (files and subdirectories).
fn copy_dir_contents(
    src: &std::path::Path,
    dst: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            std::fs::create_dir_all(&dst_path)?;
            copy_dir_contents(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
