use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use clap::{Parser, Subcommand};
use colored::Colorize;

#[derive(Parser)]
#[command(
    name = "geoff",
    about = "Semantically rich static site generator",
    version
)]
struct Cli {
    /// Increase output verbosity
    #[arg(short, long, global = true)]
    verbose: bool,
    /// Suppress non-error output
    #[arg(short, long, global = true)]
    quiet: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new Geoff site
    Init {
        /// Directory to create the site in (defaults to current directory)
        #[arg(default_value = ".")]
        path: Utf8PathBuf,
        /// Starter template to use
        #[arg(short, long, default_value = "blog")]
        template: String,
    },
    /// Build the site
    Build {
        /// Path to the site root (defaults to current directory)
        #[arg(default_value = ".")]
        path: Utf8PathBuf,
        /// Force a full rebuild, ignoring the build cache
        #[arg(long)]
        full: bool,
    },
    /// Start the dev server with hot reload
    Serve {
        /// Path to the site root (defaults to current directory)
        #[arg(default_value = ".")]
        path: Utf8PathBuf,
        /// Port to listen on
        #[arg(short, long, default_value = "3000")]
        port: u16,
        /// Open the site in the default browser
        #[arg(short, long)]
        open: bool,
    },
    /// Validate content against SHACL shapes
    Validate {
        /// Path to the site root (defaults to current directory)
        #[arg(default_value = ".")]
        path: Utf8PathBuf,
        /// Path to a custom SHACL shapes file (defaults to shapes/ directory)
        #[arg(short, long)]
        shapes: Option<Utf8PathBuf>,
    },
    /// Generate starter SHACL shapes from content
    Shapes {
        /// Path to the site root (defaults to current directory)
        #[arg(default_value = ".")]
        path: Utf8PathBuf,
    },
    /// Create a new content file with frontmatter
    New {
        /// Path for the new content file (relative to content dir, e.g. "blog/my-post.md")
        file: Utf8PathBuf,
        /// Content type (e.g. "Blog Post", "Article", "Web Page")
        #[arg(short = 't', long = "type", default_value = "Blog Post")]
        content_type: String,
        /// Title for the new page
        #[arg(long)]
        title: Option<String>,
        /// Path to site root (defaults to current directory)
        #[arg(short, long, default_value = ".")]
        path: Utf8PathBuf,
        /// List available content types and exit
        #[arg(long)]
        list_types: bool,
    },
}

/// Verbosity level derived from CLI flags.
#[derive(Clone, Copy)]
struct Verbosity {
    verbose: bool,
    quiet: bool,
}

impl Verbosity {
    fn success(&self, msg: &str) {
        if !self.quiet {
            eprintln!("{} {}", "✓".green().bold(), msg.green());
        }
    }

    fn warn(&self, msg: &str) {
        eprintln!("{} {}", "warning:".yellow().bold(), msg);
    }

    fn detail(&self, msg: &str) {
        if self.verbose {
            eprintln!("  {}", msg.dimmed());
        }
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let v = Verbosity {
        verbose: cli.verbose,
        quiet: cli.quiet,
    };

    let result = match cli.command {
        Commands::Init { path, template } => cmd_init(&path, &template, v),
        Commands::Build { path, full } => cmd_build(&path, full, v).await,
        Commands::Serve { path, port, open } => cmd_serve(path, port, open).await,
        Commands::Validate { path, shapes } => cmd_validate(&path, shapes.as_deref(), v),
        Commands::Shapes { path } => cmd_shapes(&path, v),
        Commands::New {
            file,
            content_type,
            title,
            path,
            list_types,
        } => cmd_new(&path, &file, &content_type, title.as_deref(), list_types, v),
    };

    if let Err(e) = result {
        eprintln!("{} {e}", "error:".red().bold());
        std::process::exit(1);
    }
}

fn cmd_init(
    path: &Utf8Path,
    template: &str,
    v: Verbosity,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let starters_dir = find_starters_dir();

    if let Some(ref starters) = starters_dir {
        let template_dir = starters.join(template);
        if template_dir.exists() {
            copy_starter(&template_dir, path, v)?;
            v.success(&format!(
                "Initialized new Geoff site at {path} (template: {template})"
            ));
            return Ok(());
        }
        v.warn(&format!(
            "Template '{template}' not found, using default scaffold"
        ));
    }

    // Fallback: inline scaffold
    scaffold_default(path)?;
    v.success(&format!("Initialized new Geoff site at {path}"));
    Ok(())
}

fn find_starters_dir() -> Option<Utf8PathBuf> {
    // Check relative to the binary location
    if let Ok(exe) = std::env::current_exe()
        && let Some(exe_dir) = exe.parent()
    {
        let candidates = [
            exe_dir.join("../share/geoff/starters"),
            exe_dir.join("../../starters"),
        ];
        for c in &candidates {
            if let Ok(utf8) = Utf8PathBuf::try_from(c.to_path_buf())
                && utf8.exists()
            {
                return Some(utf8);
            }
        }
    }
    // Check in cwd (development mode)
    let cwd = Utf8PathBuf::from("starters");
    if cwd.exists() {
        return Some(cwd);
    }
    None
}

fn copy_starter(
    src: &Utf8Path,
    dst: &Utf8Path,
    v: Verbosity,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    copy_dir_recursive(src.as_std_path(), dst.as_std_path())?;
    v.detail(&format!("Copied from {src}"));
    Ok(())
}

fn copy_dir_recursive(
    src: &std::path::Path,
    dst: &std::path::Path,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

fn scaffold_default(path: &Utf8Path) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let content_dir = path.join("content");
    let templates_dir = path.join("templates");
    let dist_dir = path.join("dist");

    std::fs::create_dir_all(&content_dir)?;
    std::fs::create_dir_all(&templates_dir)?;
    std::fs::create_dir_all(&dist_dir)?;

    let config_path = path.join("geoff.toml");
    if !config_path.exists() {
        std::fs::write(
            &config_path,
            r#"base_url = "http://localhost:8080"
title = "My Geoff Site"
content_dir = "content"
output_dir = "dist"
template_dir = "templates"
"#,
        )?;
    }

    let default_template = templates_dir.join("page.html");
    if !default_template.exists() {
        std::fs::write(
            &default_template,
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{ title }}</title>
</head>
<body>
    <article>
        <h1>{{ title }}</h1>
        {% if date %}<time>{{ date }}</time>{% endif %}
        {{ content }}
    </article>
    {% if json_ld %}
    <script type="application/ld+json">
    {{ json_ld }}
    </script>
    {% endif %}
</body>
</html>
"#,
        )?;
    }

    let sample_post = content_dir.join("hello-world.md");
    if !sample_post.exists() {
        std::fs::write(
            &sample_post,
            r#"+++
title = "Hello World"
date = 2026-04-10
template = "page.html"
type = "Blog Post"
author = "Anonymous"
+++

# Hello World

Welcome to your new Geoff site! This is a sample blog post.
"#,
        )?;
    }

    Ok(())
}

/// Convert a Send+Sync error box to a plain error box.
fn ss(e: Box<dyn std::error::Error + Send + Sync>) -> Box<dyn std::error::Error> {
    e
}

async fn cmd_build(
    path: &Utf8Path,
    full: bool,
    v: Verbosity,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    use std::collections::HashMap;

    use geoff_core::cache::{BuildCache, hash_directory, hash_file};
    use geoff_core::config::{PluginRuntime, SiteConfig};
    use geoff_graph::store::ContentStore;
    use geoff_plugin::registry::PluginRegistry;
    use geoff_render::pipeline::build_site_incremental;
    use geoff_render::renderer::SiteRenderer;

    let start = std::time::Instant::now();
    let config_path = path.join("geoff.toml");
    let config = SiteConfig::from_file(&config_path)
        .map_err(|e| format!("Failed to load {config_path}: {e}"))?;

    let content_dir = path.join(&config.content_dir);
    let output_dir = path.join(&config.output_dir);
    let template_dir = path.join(&config.template_dir);

    std::fs::create_dir_all(&output_dir)?;

    let store = ContentStore::new()?;
    let mut renderer = SiteRenderer::new(&template_dir)
        .map_err(|e| format!("Failed to load templates from {template_dir}: {e}"))?;
    renderer.register_sparql_function(Arc::new(store.clone()));

    // Load plugins from config
    let mut registry = PluginRegistry::new();
    for plugin_cfg in &config.plugins {
        v.detail(&format!(
            "Loading plugin: {} ({})",
            plugin_cfg.name,
            match plugin_cfg.runtime {
                PluginRuntime::Rust => "rust",
                PluginRuntime::Deno => "deno",
            }
        ));
        match plugin_cfg.runtime {
            PluginRuntime::Rust => {
                let lib_path = path.join(&plugin_cfg.path);
                let mut loader = geoff_plugin::loader::RustPluginLoader::new();
                // SAFETY: user-configured plugin path, trusted by site author
                unsafe {
                    loader.load(lib_path.as_std_path()).map_err(|e| {
                        format!(
                            "Failed to load plugin '{}' from {lib_path}: {e}",
                            plugin_cfg.name
                        )
                    })?;
                }
                registry.register_all(loader.into_plugins());
            }
            PluginRuntime::Deno => {
                let script_path = path.join(&plugin_cfg.path);
                let deno_plugin =
                    geoff_deno::plugin::DenoPlugin::new(&plugin_cfg.name, script_path.as_str())
                        .await
                        .map_err(ss)?;
                registry.register(Box::new(deno_plugin));
            }
        }
    }

    // Dispatch on_init
    let plugin_options: HashMap<String, HashMap<String, toml::Value>> = config
        .plugins
        .iter()
        .map(|p| (p.name.clone(), p.options.clone()))
        .collect();
    registry
        .dispatch_init(&config, &plugin_options)
        .await
        .map_err(ss)?;

    // Dispatch on_build_start
    registry
        .dispatch_build_start(&config, &store)
        .await
        .map_err(ss)?;

    // Load or skip cache based on --full flag
    let old_cache = if full {
        v.detail("Full rebuild requested, ignoring cache");
        None
    } else {
        Some(BuildCache::load(path))
    };

    let (pages, stats) =
        build_site_incremental(path, &config, &store, &renderer, old_cache.as_ref())?;

    // Dispatch on_graph_updated (all content is now in the store)
    registry
        .dispatch_graph_updated(&config, &store)
        .await
        .map_err(ss)?;

    if pages.is_empty() && stats.skipped == 0 {
        v.warn(&format!("No content files found in {content_dir}"));
        return Ok(());
    }

    // Write output files
    let mut outputs = HashMap::new();
    for page in &pages {
        let out_path = output_dir.join(&page.output_path);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&out_path, &page.html)?;
        v.detail(&format!("Wrote {}", page.output_path));
        outputs.insert(page.output_path.clone(), page.html.clone());
    }

    // Dispatch on_build_complete
    let output_dir_utf8 = camino::Utf8Path::new(output_dir.as_str());
    registry
        .dispatch_build_complete(&config, &store, &outputs, output_dir_utf8)
        .await
        .map_err(ss)?;

    // Update build cache
    let mut new_cache = old_cache.unwrap_or_default();
    let content_files = geoff_content::scanner::scan_content_dir(&content_dir)?;
    let rel_paths: Vec<String> = content_files
        .iter()
        .filter_map(|f| f.strip_prefix(&content_dir).ok())
        .map(|r| r.as_str().to_string())
        .collect();
    let rel_refs: Vec<&str> = rel_paths.iter().map(|s| s.as_str()).collect();
    new_cache.prune(&rel_refs);
    for file_path in &content_files {
        if let Ok(rel) = file_path.strip_prefix(&content_dir)
            && let Ok(h) = hash_file(file_path)
        {
            new_cache.record(rel.as_str().to_string(), h);
        }
    }
    new_cache.template_hash = Some(hash_directory(&template_dir)?);
    new_cache.save(path)?;

    let elapsed = start.elapsed();
    if stats.skipped > 0 {
        v.success(&format!(
            "Built {} page(s) in {:.1}s ({} unchanged, skipped) → {}",
            stats.built,
            elapsed.as_secs_f64(),
            stats.skipped,
            output_dir,
        ));
    } else {
        v.success(&format!(
            "Built {} page(s) in {:.1}s → {}",
            stats.built,
            elapsed.as_secs_f64(),
            output_dir,
        ));
    }
    Ok(())
}

async fn cmd_serve(
    path: Utf8PathBuf,
    port: u16,
    open: bool,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    if open {
        let url = format!("http://localhost:{port}");
        // Best-effort: try to open browser, don't fail if it doesn't work
        let _ = std::process::Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .or_else(|_| std::process::Command::new("open").arg(&url).spawn());
    }

    geoff_server::server::run(path, port).await
}

fn cmd_validate(
    path: &Utf8Path,
    shapes_override: Option<&Utf8Path>,
    v: Verbosity,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    use geoff_core::config::SiteConfig;
    use geoff_graph::store::ContentStore;
    use geoff_ontology::validation::validate_shacl;
    use geoff_render::pipeline::build_site;
    use geoff_render::renderer::SiteRenderer;

    let start = std::time::Instant::now();
    let config_path = path.join("geoff.toml");
    let config = SiteConfig::from_file(&config_path)?;
    let template_dir = path.join(&config.template_dir);

    let store = ContentStore::new()?;
    let mut renderer = SiteRenderer::new(&template_dir)?;
    renderer.register_sparql_function(Arc::new(store.clone()));

    v.detail("Building site graph for validation...");
    let _pages = build_site(path, &config, &store, &renderer)?;

    let data_ttl = store.export_turtle()?;

    let shapes_ttl = if let Some(shapes_path) = shapes_override {
        std::fs::read_to_string(shapes_path)
            .map_err(|e| format!("Failed to read shapes file {shapes_path}: {e}"))?
    } else {
        let shapes_dir = path.join("shapes");
        if !shapes_dir.exists() {
            return Err("No shapes/ directory found. Use `geoff shapes` to generate starter shapes, or pass --shapes <file>.".into());
        }
        let mut combined = String::new();
        for entry in std::fs::read_dir(&shapes_dir)? {
            let entry = entry?;
            let p = entry.path();
            if p.extension().is_some_and(|e| e == "ttl") {
                combined.push_str(&std::fs::read_to_string(&p)?);
                combined.push('\n');
            }
        }
        if combined.is_empty() {
            return Err("No .ttl shapes files found in shapes/ directory.".into());
        }
        combined
    };

    let outcome = validate_shacl(&data_ttl, &shapes_ttl)?;
    let elapsed = start.elapsed();

    if outcome.conforms {
        v.success(&format!(
            "Validation passed in {:.1}s — all content conforms to shapes",
            elapsed.as_secs_f64()
        ));
    } else {
        eprintln!(
            "{} {} violation(s), {} warning(s)",
            "Validation failed:".red().bold(),
            outcome.violations,
            outcome.warnings
        );
        eprintln!("{}", outcome.report_text);
        std::process::exit(1);
    }

    Ok(())
}

fn cmd_shapes(
    path: &Utf8Path,
    v: Verbosity,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    use geoff_content::frontmatter::{parse_frontmatter, split_frontmatter};
    use geoff_content::scanner::scan_content_dir;
    use geoff_core::config::SiteConfig;
    use geoff_ontology::validation::generate_shapes;

    let config_path = path.join("geoff.toml");
    let config = SiteConfig::from_file(&config_path)?;
    let content_dir = path.join(&config.content_dir);

    let files = scan_content_dir(&content_dir)?;
    let mut types = std::collections::HashSet::new();

    for file_path in &files {
        let raw = std::fs::read_to_string(file_path)?;
        if let Ok((fm_str, _body)) = split_frontmatter(&raw)
            && let Ok((frontmatter, _)) = parse_frontmatter(fm_str)
            && let Some(ct) = frontmatter.get("type").and_then(|v| v.as_str())
        {
            types.insert(ct.to_string());
        }
    }

    let type_refs: Vec<&str> = types.iter().map(|s| s.as_str()).collect();
    let shapes = generate_shapes(&type_refs);

    let shapes_dir = path.join("shapes");
    std::fs::create_dir_all(&shapes_dir)?;
    let output = shapes_dir.join("content.shacl.ttl");
    std::fs::write(&output, &shapes)?;

    v.success(&format!(
        "Generated shapes for {} content type(s) → {output}",
        type_refs.len()
    ));
    Ok(())
}

fn cmd_new(
    site_root: &Utf8Path,
    file: &Utf8Path,
    content_type: &str,
    title: Option<&str>,
    list_types: bool,
    v: Verbosity,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    use geoff_core::config::SiteConfig;
    use geoff_ontology::vocabulary::VocabularyIndex;

    if list_types {
        let ontologies_dir = site_root.join("ontologies");
        let mut index = VocabularyIndex::new();
        index.load_directory(&ontologies_dir)?;

        if index.is_empty() {
            v.warn("No vocabularies loaded. Add .ttl files to ontologies/ directory.");
            return Ok(());
        }

        eprintln!("{}", "Available content types:".bold());
        let mut classes: Vec<_> = index.classes().collect();
        classes.sort_by(|a, b| a.label.cmp(&b.label));
        for term in classes {
            eprintln!(
                "  {} {}",
                term.label.bold(),
                format!("({})", term.source).dimmed()
            );
        }
        return Ok(());
    }

    let config_path = site_root.join("geoff.toml");
    let config = SiteConfig::from_file(&config_path)?;
    let content_dir = site_root.join(&config.content_dir);

    let file_path = if file.extension().is_none() {
        content_dir.join(file.with_extension("md"))
    } else {
        content_dir.join(file)
    };

    let derived_title = title.map(|t| t.to_string()).unwrap_or_else(|| {
        file.file_stem()
            .unwrap_or("Untitled")
            .replace(['-', '_'], " ")
    });

    let today = chrono_today();
    let frontmatter = format!(
        r#"+++
title = "{derived_title}"
date = {today}
template = "page.html"
type = "{content_type}"
+++"#
    );

    let content = format!("{frontmatter}\n\n# {derived_title}\n\nWrite your content here.\n");

    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if file_path.exists() {
        return Err(format!("File already exists: {file_path}").into());
    }

    std::fs::write(&file_path, content)?;
    v.success(&format!("Created {file_path}"));
    Ok(())
}

fn chrono_today() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let days = now.as_secs() / 86400;
    let mut y = 1970i32;
    let mut remaining = days as i32;
    loop {
        let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            366
        } else {
            365
        };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let days_in_months = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 1u32;
    for &dim in &days_in_months {
        if remaining < dim {
            break;
        }
        remaining -= dim;
        m += 1;
    }
    let d = remaining + 1;
    format!("{y}-{m:02}-{d:02}")
}
