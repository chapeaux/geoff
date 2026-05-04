mod optimize;
mod publish;

use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use clap::{Parser, Subcommand};
use colored::Colorize;

use publish::{cmd_publish_download, cmd_publish_github, cmd_publish_openshift, cmd_publish_solid};

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
    /// Theme management commands
    Theme {
        #[command(subcommand)]
        action: ThemeCommands,
    },
    /// Publish the built site
    Publish {
        #[command(subcommand)]
        target: PublishTarget,
        /// Path to the site root
        #[arg(short, long, default_value = ".")]
        path: Utf8PathBuf,
    },
}

#[derive(Subcommand)]
enum PublishTarget {
    /// Download the built site as a ZIP archive
    Download {
        /// Output file path (defaults to site-name.zip)
        #[arg(short, long)]
        output: Option<Utf8PathBuf>,
    },
    /// Push the built site to GitHub Pages (gh-pages branch)
    Github,
    /// Deploy as a container on OpenShift
    Openshift {
        /// Application name
        #[arg(long)]
        name: Option<String>,
    },
    /// Publish to a Solid pod
    Solid {
        /// Solid pod URL (e.g. https://paa.pub/username/)
        #[arg(long)]
        pod: String,
        /// Bearer token for authentication (or set SOLID_TOKEN env var)
        #[arg(long)]
        token: Option<String>,
    },
}

#[derive(Subcommand)]
enum ThemeCommands {
    /// Preview the current theme with sample content
    Preview {
        /// Path to site root (defaults to current directory)
        #[arg(default_value = ".")]
        path: Utf8PathBuf,
        /// Port to listen on
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },
    /// Visual theme editor with live preview
    Edit {
        /// Path to site root (defaults to current directory)
        #[arg(default_value = ".")]
        path: Utf8PathBuf,
        /// Port to listen on
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },
    /// Generate a theme from design system tokens
    Generate {
        /// Theme name (creates themes/{name}/theme.json)
        name: String,
        /// Path to site root (defaults to current directory)
        #[arg(short, long, default_value = ".")]
        path: Utf8PathBuf,
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
        Commands::Theme { action } => match action {
            ThemeCommands::Preview { path, port } => cmd_theme_preview(&path, port, v).await,
            ThemeCommands::Edit { path, port } => cmd_theme_edit(&path, port, v).await,
            ThemeCommands::Generate { name, path } => cmd_theme_generate(&path, &name, v),
        },
        Commands::Publish { target, path } => match target {
            PublishTarget::Download { output } => {
                cmd_publish_download(&path, output.as_deref(), v).await
            }
            PublishTarget::Github => cmd_publish_github(&path, v).await,
            PublishTarget::Openshift { name } => {
                cmd_publish_openshift(&path, name.as_deref(), v).await
            }
            PublishTarget::Solid { pod, token } => {
                cmd_publish_solid(&path, &pod, token.as_deref(), v).await
            }
        },
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

    // Build renderer with layered template directories if a theme is configured
    let mut renderer = if let Some(ref theme_name) = config.theme.name {
        let mut dirs = vec![path.join("themes").join(theme_name).join("templates")];
        if let Some(ref base_name) = config.theme.base {
            dirs.push(path.join("themes").join(base_name).join("templates"));
        }
        dirs.push(template_dir.clone());
        let dir_refs: Vec<&camino::Utf8Path> = dirs.iter().map(|d| d.as_path()).collect();
        SiteRenderer::with_theme_dirs(&dir_refs)
            .map_err(|e| format!("Failed to load templates: {e}"))?
    } else {
        SiteRenderer::new(&template_dir)
            .map_err(|e| format!("Failed to load templates from {template_dir}: {e}"))?
    };
    let store_arc = Arc::new(store.clone());
    renderer.register_sparql_function(store_arc.clone());
    renderer.register_component_function(path.join("components"));
    renderer.register_devspaces_function(&config.devspaces);

    // Register RDFa template helpers if enabled
    if config.linked_data.rdfa {
        let mappings_path = path.join("ontology/mappings.toml");
        let mut rdfa_registry =
            geoff_ontology::mappings::MappingRegistry::load(&mappings_path).unwrap_or_default();
        if !config.linked_data.prefixes.is_empty() {
            rdfa_registry.add_prefixes(config.linked_data.prefixes.clone());
        }
        renderer.register_rdfa_functions(store_arc.clone(), Arc::new(rdfa_registry));
    }

    // Load and register theme tokens (before content build so theme_css() is available)
    let theme_result =
        geoff_render::pipeline::load_and_register_theme(path, &config, &mut renderer, &store)?;
    if theme_result.is_some() {
        v.detail("Loaded theme design tokens");
    }

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

    // Phase 1: Ingest content into the RDF graph
    let (mut to_render, stats, page_index) =
        geoff_render::pipeline::ingest_content(path, &config, &store, old_cache.as_ref())?;
    renderer.set_page_index(page_index);

    // Hook: on_graph_updated — all content triples are in the store,
    // plugins can read them and inject additional triples before rendering
    registry
        .dispatch_graph_updated(&config, &store)
        .await
        .map_err(ss)?;

    // Hook: on_page_render — plugins can inject extra template variables per page
    for page in &mut to_render {
        let mut page_data = geoff_plugin::context::PageData {
            path: page.output_path.clone(),
            title: Some(page.title.clone()),
            content_type: None,
            html: page.content_html.clone(),
            raw_body: String::new(),
            frontmatter: std::collections::HashMap::new(),
        };
        let mut extra_vars = page.extra_vars.clone();
        registry
            .dispatch_page_render(&config, &store, &mut page_data, &mut extra_vars)
            .await
            .map_err(ss)?;
        page.extra_vars = extra_vars;
    }

    // Phase 2: Render pages (SPARQL queries and extra_vars both available)
    let pages =
        geoff_render::pipeline::render_pages(&to_render, &config, &renderer, stats.skipped)?;

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

    // Copy static files to output directory
    let static_dir = path.join("static");
    if static_dir.exists() {
        copy_dir_recursive(static_dir.as_std_path(), output_dir.as_std_path())?;
        v.detail("Copied static files");
    }

    // Write deferred theme CSS to output
    if let Some(ref theme) = theme_result {
        let theme_output_dir = output_dir.join("theme");
        std::fs::create_dir_all(&theme_output_dir)?;
        let css_content = format!(":root {{\n{}}}\n", theme.css);
        std::fs::write(theme_output_dir.join("tokens.css"), &css_content)?;
        v.detail("Wrote theme/tokens.css");
    }

    // Write shared theme files when theme.share is enabled
    if config.theme.share
        && let Some(ref theme) = theme_result
    {
        let theme_name = config.theme.name.as_deref().unwrap_or("default");

        // Write DTCG JSON (merged tokens)
        let tokens_json_path = output_dir.join(format!("{theme_name}.tokens.json"));
        std::fs::write(
            &tokens_json_path,
            serde_json::to_string_pretty(&theme.merged_json)?,
        )?;
        v.detail(&format!("Wrote shared theme: {theme_name}.tokens.json"));

        // Write N-Triples (design token triples from the graph)
        let nt = store.export_search_ntriples()?;
        let nt_path = output_dir.join(format!("{theme_name}.nt"));
        std::fs::write(&nt_path, &nt)?;
        v.detail(&format!("Wrote shared theme: {theme_name}.nt"));
    }

    // Optimize assets (CSS/JS minification, cache-busting hashes, image conversion)
    let opt = &config.theme.optimize;
    if opt.minify_css || opt.minify_js || opt.hash_assets || opt.images.webp {
        let opt_stats = optimize::optimize_assets(output_dir.as_std_path(), opt)?;
        if opt_stats.css_minified > 0 {
            v.detail(&format!("Minified {} CSS file(s)", opt_stats.css_minified));
        }
        if opt_stats.js_minified > 0 {
            v.detail(&format!("Minified {} JS file(s)", opt_stats.js_minified));
        }
        if opt_stats.assets_hashed > 0 {
            v.detail(&format!(
                "Added content hashes to {} asset(s)",
                opt_stats.assets_hashed
            ));
        }
        if opt_stats.images_converted > 0 {
            v.detail(&format!(
                "Converted {} image(s) to WebP",
                opt_stats.images_converted
            ));
        }
    }

    // Generate search index if enabled
    if config.search.enabled {
        let nt = store.export_search_ntriples()?;
        let search_path = output_dir.join(&config.search.output);
        std::fs::write(&search_path, &nt)?;
        v.detail(&format!("Wrote search index: {}", config.search.output));
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
            && let Ok((frontmatter, _, _)) = parse_frontmatter(fm_str)
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

fn cmd_theme_generate(
    path: &Utf8Path,
    name: &str,
    v: Verbosity,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    use geoff_core::config::SiteConfig;
    use std::collections::{BTreeMap, HashSet};

    let config = SiteConfig::from_file(&path.join("geoff.toml"))?;

    if config.design.tokens.is_empty() {
        return Err("No [design] tokens configured in geoff.toml. Add:\n\n[design]\ntokens = [\"path/to/tokens.json\"]\n".into());
    }

    // Load and merge design system tokens
    let mut merged: Option<serde_json::Value> = None;
    for token_path in &config.design.tokens {
        let full_path = path.join(token_path);
        if !full_path.exists() {
            return Err(format!("Design system token file not found: {full_path}").into());
        }
        let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&full_path)?)?;
        v.detail(&format!("Loaded {token_path}"));
        merged = Some(match merged {
            Some(base) => geoff_theme::merge_tokens(&base, &json),
            None => json,
        });
    }

    let merged = merged.unwrap();
    let tokens = geoff_theme::DesignTokens::from_json(&merged.to_string())?;
    let mut flat = tokens.flatten();
    geoff_theme::resolve_references(&mut flat);

    v.detail(&format!("Loaded {} design system tokens", flat.len()));

    // Detect -on-light/-on-dark pairs (suffix and group conventions)
    let mut pairs: Vec<(String, String, String)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for path_key in flat.keys() {
        // Suffix: "foo-on-light" / "foo-on-dark"
        if let Some(base) = path_key.strip_suffix("-on-light") {
            let dark = format!("{base}-on-dark");
            if flat.contains_key(&dark) && seen.insert(base.to_string()) {
                pairs.push((base.to_string(), path_key.clone(), dark));
            }
        }
        // Group: "foo.on.light" / "foo.on.dark"
        if let Some(base) = path_key.strip_suffix(".on.light") {
            let dark = format!("{base}.on.dark");
            if flat.contains_key(&dark) && seen.insert(base.to_string()) {
                pairs.push((base.to_string(), path_key.clone(), dark));
            }
        }
    }

    v.detail(&format!("Detected {} light/dark pairs", pairs.len()));

    // Build theme.json as a DTCG token structure
    let mut theme_obj = serde_json::Map::new();

    // Group tokens by their top-level group for DTCG structure
    let mut groups: BTreeMap<String, serde_json::Map<String, serde_json::Value>> = BTreeMap::new();

    // Add light/dark pairs with aggregates
    for (base, light_path, dark_path) in &pairs {
        let light_ref = format!("{{{light_path}}}");
        let dark_ref = format!("{{{dark_path}}}");
        let aggregate = format!("light-dark({light_ref}, {dark_ref})");

        let (group, _) = first_group(light_path);
        let group_map = groups.entry(group.clone()).or_default();

        // Get the token type from the light token
        let token_type = flat.get(light_path).and_then(|t| t.token_type.clone());

        // Add -on-light reference
        let light_local = light_path
            .strip_prefix(&format!("{group}."))
            .unwrap_or(light_path);
        group_map.insert(
            light_local.to_string(),
            make_token_entry(&light_ref, token_type.as_deref()),
        );

        // Add -on-dark reference
        let dark_local = dark_path
            .strip_prefix(&format!("{group}."))
            .unwrap_or(dark_path);
        group_map.insert(
            dark_local.to_string(),
            make_token_entry(&dark_ref, token_type.as_deref()),
        );

        // Add aggregate
        let base_local = base.strip_prefix(&format!("{group}.")).unwrap_or(base);
        let mut agg_entry = serde_json::Map::new();
        agg_entry.insert("$value".to_string(), serde_json::json!(aggregate));
        agg_entry.insert(
            "$description".to_string(),
            serde_json::json!("Auto-generated light-dark aggregate"),
        );
        if let Some(t) = &token_type {
            agg_entry.insert("$type".to_string(), serde_json::json!(t));
        }
        group_map.insert(base_local.to_string(), serde_json::Value::Object(agg_entry));
    }

    // Add non-paired tokens as references
    for (path_key, token) in &flat {
        // Skip tokens that are part of a pair (light, dark, or base)
        if seen.contains(path_key) {
            continue;
        }
        let is_pair_member = path_key.ends_with("-on-light")
            || path_key.ends_with("-on-dark")
            || path_key.ends_with(".on.light")
            || path_key.ends_with(".on.dark");
        if is_pair_member {
            continue;
        }

        let reference = format!("{{{path_key}}}");
        let (group, _) = first_group(path_key);
        let group_map = groups.entry(group.clone()).or_default();
        let local = path_key
            .strip_prefix(&format!("{group}."))
            .unwrap_or(path_key);
        group_map.insert(
            local.to_string(),
            make_token_entry(&reference, token.token_type.as_deref()),
        );
    }

    // Build final JSON with group $type annotations
    for (group_name, entries) in &groups {
        let mut group_obj = serde_json::Map::new();

        // Infer group $type from first token's type
        if let Some(first_entry) = entries.values().next()
            && let Some(t) = first_entry.get("$type")
        {
            group_obj.insert("$type".to_string(), t.clone());
        }

        for (key, val) in entries {
            group_obj.insert(key.clone(), val.clone());
        }
        theme_obj.insert(group_name.clone(), serde_json::Value::Object(group_obj));
    }

    // Write to themes/{name}/theme.json
    let theme_dir = path.join("themes").join(name);
    std::fs::create_dir_all(&theme_dir)?;
    let theme_path = theme_dir.join("theme.json");
    let json_str = serde_json::to_string_pretty(&serde_json::Value::Object(theme_obj))?;
    std::fs::write(&theme_path, &json_str)?;

    v.success(&format!(
        "Generated {theme_path} ({} tokens, {} light/dark pairs)",
        flat.len(),
        pairs.len()
    ));
    Ok(())
}

fn first_group(path: &str) -> (String, String) {
    if let Some(pos) = path.find('.') {
        (path[..pos].to_string(), path[pos + 1..].to_string())
    } else {
        (path.to_string(), String::new())
    }
}

fn make_token_entry(value: &str, token_type: Option<&str>) -> serde_json::Value {
    let mut entry = serde_json::Map::new();
    entry.insert("$value".to_string(), serde_json::json!(value));
    if let Some(t) = token_type {
        entry.insert("$type".to_string(), serde_json::json!(t));
    }
    serde_json::Value::Object(entry)
}

async fn cmd_theme_preview(
    path: &Utf8Path,
    port: u16,
    v: Verbosity,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    use geoff_core::config::SiteConfig;
    use geoff_ontology::mappings::MappingRegistry;
    use geoff_theme::DesignTokens;

    let config_path = path.join("geoff.toml");
    let config = SiteConfig::from_file(&config_path)
        .map_err(|e| format!("Failed to load {config_path}: {e}"))?;

    let theme_name = config
        .theme
        .name
        .as_deref()
        .ok_or("No theme configured in geoff.toml (set [theme] name = \"...\")")?;

    v.detail(&format!("Previewing theme: {theme_name}"));

    // Load and flatten theme tokens
    let theme_dir = path.join("themes").join(theme_name);
    let tokens_path = theme_dir.join("tokens.json");
    if !tokens_path.exists() {
        return Err(format!("Theme token file not found: {tokens_path}").into());
    }

    let theme_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&tokens_path)?)?;

    let merged_json = if let Some(ref base_name) = config.theme.base {
        let base_path = path.join("themes").join(base_name).join("tokens.json");
        if base_path.exists() {
            let base_json: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&base_path)?)?;
            geoff_theme::merge_tokens(&base_json, &theme_json)
        } else {
            theme_json
        }
    } else {
        theme_json
    };

    let tokens = DesignTokens::from_json(&merged_json.to_string())?;
    let mut flat = tokens.flatten();
    geoff_theme::resolve_references(&mut flat);

    // Handle dark mode tokens
    let dark_flat = if let Some(ref dark_file) = config.theme.modes.dark {
        let dark_path = theme_dir.join(dark_file);
        if dark_path.exists() {
            let dark_tokens = DesignTokens::from_file(camino::Utf8Path::new(dark_path.as_str()))?;
            let mut df = dark_tokens.flatten();
            geoff_theme::resolve_references(&mut df);
            Some(df)
        } else {
            None
        }
    } else {
        None
    };

    let prefix = config.theme.prefix.as_deref();
    let css = geoff_theme::generate_css_with_prefix(&flat, dark_flat.as_ref(), false, prefix);
    let critical_css =
        geoff_theme::generate_css_with_prefix(&flat, dark_flat.as_ref(), true, prefix);
    let full_css = format!(":root {{\n{critical_css}{css}}}\n");

    // Load content type mappings for sample content
    let mappings_path = path.join("ontology/mappings.toml");
    let registry = MappingRegistry::load(&mappings_path)?;
    let content_types: Vec<&String> = registry.types.keys().collect();

    // Create a temp directory for the preview site
    let preview_dir = tempfile::tempdir()?;
    let preview_root = Utf8PathBuf::try_from(preview_dir.path().to_path_buf())
        .map_err(|e| format!("Non-UTF8 temp path: {e}"))?;

    // Create preview site structure
    let preview_content = preview_root.join("content");
    let preview_templates = preview_root.join("templates");
    let preview_static = preview_root.join("static");
    std::fs::create_dir_all(&preview_content)?;
    std::fs::create_dir_all(&preview_templates)?;
    std::fs::create_dir_all(&preview_static)?;

    // Write theme CSS as a static file
    std::fs::write(preview_static.join("tokens.css"), &full_css)?;

    // Generate sample content for each content type
    let today = chrono_today();
    for ct in &content_types {
        let slug = ct.to_lowercase().replace(' ', "-");
        let sample = format!(
            r#"+++
title = "Sample {ct}"
date = {today}
template = "page.html"
type = "{ct}"
author = "Preview Author"
description = "A sample {ct} for theme preview."
+++

# Sample {ct}

This is a **sample page** of type *{ct}*, generated for theme preview.

## Subheading

Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.

### Third level heading

> A blockquote to show how quotes look with this theme.

- List item one
- List item two
- List item three

1. Ordered item one
2. Ordered item two
3. Ordered item three

Here is some `inline code` and a code block:

```
fn main() {{
    println!("Hello, theme preview!");
}}
```

And a [link to somewhere](https://example.com) for good measure.
"#
        );
        std::fs::write(preview_content.join(format!("{slug}.md")), sample)?;
    }

    // Always generate at least one sample page if no types are mapped
    if content_types.is_empty() {
        let sample = format!(
            r#"+++
title = "Sample Page"
date = {today}
template = "page.html"
type = "Web Page"
author = "Preview Author"
description = "A sample page for theme preview."
+++

# Sample Page

This is a **sample page** generated for theme preview.

## Typography

Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.

### Third level heading

> A blockquote to show how quotes look with this theme.

- List item one
- List item two
- List item three

Here is some `inline code` and a code block:

```
fn main() {{
    println!("Hello, theme preview!");
}}
```

And a [link to somewhere](https://example.com) for good measure.
"#
        );
        std::fs::write(preview_content.join("sample-page.md"), sample)?;
    }

    // Generate the preview index page (static HTML, no template dependency)
    let index_html = generate_preview_index(theme_name, &flat, &content_types);
    std::fs::write(
        preview_content.join("index.md"),
        format!(
            r#"+++
title = "Theme Preview: {theme_name}"
date = {today}
template = "preview-index.html"
type = "Web Page"
+++

Theme preview index.
"#
        ),
    )?;

    // Create a simple page template
    let page_template = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{ title }} — Theme Preview</title>
    <link rel="stylesheet" href="/tokens.css">
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: var(--typography-body-font-family, system-ui, -apple-system, sans-serif);
            font-size: var(--typography-body-font-size, 16px);
            line-height: var(--typography-body-line-height, 1.6);
            color: var(--color-text, #1a1a1a);
            background: var(--color-bg, #ffffff);
            padding: var(--spacing-lg, 2rem);
            max-width: 48rem;
            margin: 0 auto;
        }
        nav { margin-bottom: var(--spacing-lg, 2rem); }
        nav a {
            color: var(--color-primary, #0066cc);
            text-decoration: none;
            margin-right: var(--spacing-sm, 0.5rem);
        }
        nav a:hover { text-decoration: underline; }
        article { margin-top: var(--spacing-md, 1rem); }
        h1 { font-size: var(--typography-heading-font-size, 2rem); margin-bottom: var(--spacing-md, 1rem); color: var(--color-heading, inherit); }
        h2 { font-size: 1.5rem; margin-top: var(--spacing-lg, 2rem); margin-bottom: var(--spacing-sm, 0.5rem); }
        h3 { font-size: 1.25rem; margin-top: var(--spacing-md, 1rem); margin-bottom: var(--spacing-sm, 0.5rem); }
        p { margin-bottom: var(--spacing-md, 1rem); }
        blockquote {
            border-left: 3px solid var(--color-primary, #0066cc);
            padding-left: var(--spacing-md, 1rem);
            margin: var(--spacing-md, 1rem) 0;
            color: var(--color-text-secondary, #555);
            font-style: italic;
        }
        code {
            font-family: var(--typography-code-font-family, monospace);
            background: var(--color-surface, #f5f5f5);
            padding: 0.1em 0.3em;
            border-radius: var(--border-radius-sm, 3px);
        }
        pre { background: var(--color-surface, #f5f5f5); padding: var(--spacing-md, 1rem); border-radius: var(--border-radius-md, 6px); overflow-x: auto; margin-bottom: var(--spacing-md, 1rem); }
        pre code { background: none; padding: 0; }
        ul, ol { margin-bottom: var(--spacing-md, 1rem); padding-left: var(--spacing-lg, 2rem); }
        li { margin-bottom: var(--spacing-xs, 0.25rem); }
        a { color: var(--color-primary, #0066cc); }
        time { color: var(--color-text-secondary, #666); font-size: 0.9em; }
    </style>
</head>
<body>
    <nav><a href="/">← Theme Preview Index</a></nav>
    <article>
        <h1>{{ title }}</h1>
        {% if date %}<time>{{ date }}</time>{% endif %}
        {{ content }}
    </article>
</body>
</html>
"#;
    std::fs::write(preview_templates.join("page.html"), page_template)?;

    // Create the preview index template
    std::fs::write(preview_templates.join("preview-index.html"), &index_html)?;

    // Write preview geoff.toml (no theme reference — we use static CSS)
    std::fs::write(
        preview_root.join("geoff.toml"),
        format!(
            r#"base_url = "http://localhost:{port}"
title = "Theme Preview: {theme_name}"
content_dir = "content"
output_dir = "dist"
template_dir = "templates"
"#
        ),
    )?;

    // Create ontology/mappings.toml (empty, not needed for preview)
    std::fs::create_dir_all(preview_root.join("ontology"))?;
    std::fs::write(
        preview_root.join("ontology/mappings.toml"),
        "[types]\n[properties]\n",
    )?;

    v.success(&format!(
        "Theme preview for '{theme_name}' starting on http://localhost:{port}"
    ));

    // Start the dev server on the preview site
    geoff_server::server::run(preview_root, port).await?;

    // Keep the temp dir alive as long as the server runs
    drop(preview_dir);
    Ok(())
}

/// Generate a preview index HTML page showing color swatches, typography specimens,
/// and spacing scale from the flattened design tokens.
fn generate_preview_index(
    theme_name: &str,
    tokens: &std::collections::BTreeMap<String, geoff_theme::FlatToken>,
    content_types: &[&String],
) -> String {
    use std::fmt::Write;

    // Categorize tokens by type
    let mut colors: Vec<(&str, &str)> = Vec::new();
    let mut typography: Vec<(&str, &geoff_theme::FlatToken)> = Vec::new();
    let mut dimensions: Vec<(&str, &str)> = Vec::new();

    for (path, token) in tokens {
        let token_type = token.token_type.as_deref().unwrap_or("");
        match token_type {
            "color" => {
                if let geoff_theme::TokenValue::String(ref val) = token.value {
                    colors.push((path.as_str(), val.as_str()));
                }
            }
            "typography" => {
                typography.push((path.as_str(), token));
            }
            "dimension" => {
                let val = match &token.value {
                    geoff_theme::TokenValue::String(s) => s.clone(),
                    geoff_theme::TokenValue::Number(n) => {
                        if n.fract() == 0.0 {
                            format!("{}px", *n as i64)
                        } else {
                            format!("{n}px")
                        }
                    }
                    geoff_theme::TokenValue::Object(obj) => {
                        let v = obj.get("value").and_then(|n| n.as_f64()).unwrap_or(0.0);
                        let unit = obj.get("unit").and_then(|u| u.as_str()).unwrap_or("px");
                        if v.fract() == 0.0 {
                            format!("{}{unit}", v as i64)
                        } else {
                            format!("{v}{unit}")
                        }
                    }
                    _ => String::new(),
                };
                if !val.is_empty() {
                    // Leak into a static str for the Vec -- this is fine for a short-lived preview
                    dimensions.push((path.as_str(), Box::leak(val.into_boxed_str())));
                }
            }
            _ => {}
        }
    }

    let mut html = String::new();

    // Build the full template (not a Tera template — raw HTML)
    let _ = write!(
        html,
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{{{ title }}}} — Theme Preview</title>
    <link rel="stylesheet" href="/tokens.css">
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{
            font-family: system-ui, -apple-system, sans-serif;
            line-height: 1.6;
            color: #1a1a1a;
            background: #ffffff;
            padding: 2rem;
            max-width: 64rem;
            margin: 0 auto;
        }}
        h1 {{ font-size: 2rem; margin-bottom: 1rem; }}
        h2 {{ font-size: 1.5rem; margin-top: 2rem; margin-bottom: 1rem; border-bottom: 1px solid #ddd; padding-bottom: 0.5rem; }}
        h3 {{ font-size: 1.1rem; margin-top: 1rem; margin-bottom: 0.5rem; }}
        .section {{ margin-bottom: 2rem; }}
        .swatch-grid {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); gap: 1rem; }}
        .swatch {{
            border: 1px solid #ddd;
            border-radius: 6px;
            overflow: hidden;
        }}
        .swatch-color {{
            height: 60px;
            border-bottom: 1px solid #ddd;
        }}
        .swatch-info {{
            padding: 0.5rem;
            font-size: 0.75rem;
        }}
        .swatch-name {{
            font-weight: 600;
            word-break: break-all;
        }}
        .swatch-value {{
            color: #666;
            font-family: monospace;
        }}
        .type-specimen {{
            padding: 1rem;
            border: 1px solid #ddd;
            border-radius: 6px;
            margin-bottom: 1rem;
        }}
        .type-label {{
            font-size: 0.75rem;
            color: #666;
            font-family: monospace;
            margin-bottom: 0.25rem;
        }}
        .spacing-bar {{
            display: flex;
            align-items: center;
            gap: 1rem;
            margin-bottom: 0.5rem;
        }}
        .spacing-visual {{
            background: #0066cc33;
            border: 1px solid #0066cc;
            height: 1.5rem;
            border-radius: 3px;
            min-width: 2px;
        }}
        .spacing-label {{
            font-size: 0.75rem;
            font-family: monospace;
            white-space: nowrap;
        }}
        .links {{ margin-bottom: 1rem; }}
        .links a {{
            display: inline-block;
            margin-right: 1rem;
            margin-bottom: 0.5rem;
            color: #0066cc;
            text-decoration: none;
        }}
        .links a:hover {{ text-decoration: underline; }}
    </style>
</head>
<body>
    <h1>Theme Preview: {theme_name}</h1>
"#
    );

    // Links to sample pages
    let _ = writeln!(html, "    <div class=\"section\">");
    let _ = writeln!(html, "        <h2>Template Variations</h2>");
    let _ = writeln!(html, "        <div class=\"links\">");
    if content_types.is_empty() {
        let _ = writeln!(
            html,
            "            <a href=\"/sample-page.html\">Sample Page (Web Page)</a>"
        );
    } else {
        for ct in content_types {
            let slug = ct.to_lowercase().replace(' ', "-");
            let _ = writeln!(html, "            <a href=\"/{slug}.html\">{ct}</a>");
        }
    }
    let _ = writeln!(html, "        </div>");
    let _ = writeln!(html, "    </div>");

    // Color swatches
    if !colors.is_empty() {
        let _ = writeln!(html, "    <div class=\"section\">");
        let _ = writeln!(html, "        <h2>Colors</h2>");
        let _ = writeln!(html, "        <div class=\"swatch-grid\">");
        for (path, value) in &colors {
            let var_name = path_to_css_var_name(path);
            let _ = writeln!(
                html,
                r#"            <div class="swatch">
                <div class="swatch-color" style="background: {value};"></div>
                <div class="swatch-info">
                    <div class="swatch-name">{var_name}</div>
                    <div class="swatch-value">{value}</div>
                </div>
            </div>"#
            );
        }
        let _ = writeln!(html, "        </div>");
        let _ = writeln!(html, "    </div>");
    }

    // Typography specimens
    if !typography.is_empty() {
        let _ = writeln!(html, "    <div class=\"section\">");
        let _ = writeln!(html, "        <h2>Typography</h2>");
        for (path, token) in &typography {
            let var_prefix = path_to_css_var_name(path);
            let sample_text = "The quick brown fox jumps over the lazy dog";

            // Try to extract font properties for inline style
            let mut inline_style = String::new();
            if let geoff_theme::TokenValue::Object(ref obj) = token.value {
                if let Some(family) = obj.get("fontFamily").and_then(|v| v.as_str()) {
                    let _ = write!(inline_style, "font-family: {family}; ");
                }
                if let Some(size) = obj.get("fontSize") {
                    if let Some(s) = size.as_str() {
                        let _ = write!(inline_style, "font-size: {s}; ");
                    } else if let Some(o) = size.as_object() {
                        let v = o.get("value").and_then(|n| n.as_f64()).unwrap_or(16.0);
                        let u = o.get("unit").and_then(|u| u.as_str()).unwrap_or("px");
                        let _ = write!(inline_style, "font-size: {v}{u}; ");
                    }
                }
                if let Some(weight) = obj.get("fontWeight").and_then(|v| v.as_f64()) {
                    let _ = write!(inline_style, "font-weight: {}; ", weight as i64);
                }
                if let Some(lh) = obj.get("lineHeight").and_then(|v| v.as_f64()) {
                    let _ = write!(inline_style, "line-height: {lh}; ");
                }
            }

            let _ = writeln!(
                html,
                r#"        <div class="type-specimen">
            <div class="type-label">{var_prefix}</div>
            <div style="{inline_style}">{sample_text}</div>
        </div>"#
            );
        }
        let _ = writeln!(html, "    </div>");
    }

    // Spacing scale
    if !dimensions.is_empty() {
        let _ = writeln!(html, "    <div class=\"section\">");
        let _ = writeln!(html, "        <h2>Spacing / Dimensions</h2>");
        for (path, value) in &dimensions {
            let var_name = path_to_css_var_name(path);
            let _ = writeln!(
                html,
                r#"        <div class="spacing-bar">
            <div class="spacing-label" style="min-width: 12rem;">{var_name}</div>
            <div class="spacing-visual" style="width: {value};"></div>
            <div class="spacing-label">{value}</div>
        </div>"#
            );
        }
        let _ = writeln!(html, "    </div>");
    }

    let _ = writeln!(html, "</body>");
    let _ = writeln!(html, "</html>");

    html
}

/// Convert a dot-separated token path to a CSS custom property name.
fn path_to_css_var_name(path: &str) -> String {
    let mut result = String::with_capacity(path.len() + 2);
    result.push_str("--");
    let chars: Vec<char> = path.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c == '.' {
            result.push('-');
        } else if c.is_uppercase() && i > 0 && chars[i - 1] != '.' {
            result.push('-');
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c.to_ascii_lowercase());
        }
    }
    result
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

async fn cmd_theme_edit(
    path: &Utf8Path,
    port: u16,
    v: Verbosity,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let config_path = path.join("geoff.toml");
    if !config_path.exists() {
        return Err(format!("No geoff.toml found at {path}").into());
    }

    let config = geoff_core::config::SiteConfig::from_file(&config_path)?;
    let theme_name = config
        .theme
        .name
        .as_deref()
        .ok_or("No [theme] name configured in geoff.toml")?;

    v.success(&format!(
        "Starting theme editor for '{theme_name}' at http://localhost:{port}/__geoff__/theme/"
    ));
    v.detail("Edit tokens visually — changes save to themes/ on disk");

    // Build the editor HTML that loads the web components
    let editor_html = format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Geoff Theme Editor — {theme_name}</title>
  <script type="module" src="/__geoff__/theme/geoff-token-field.js"></script>
  <script type="module" src="/__geoff__/theme/geoff-token-group.js"></script>
  <script type="module" src="/__geoff__/theme/geoff-token-editor.js"></script>
  <script type="module" src="/__geoff__/theme/geoff-theme-preview.js"></script>
  <script type="module" src="/__geoff__/theme/geoff-theme-editor-app.js"></script>
</head>
<body style="margin:0;height:100vh;overflow:hidden">
  <geoff-theme-editor-app></geoff-theme-editor-app>
</body>
</html>"##
    );

    // Copy editor components to a temp static dir so the server can serve them
    let editor_dir = tempfile::tempdir()?;
    let editor_path = editor_dir.path().join("__geoff__/theme");
    std::fs::create_dir_all(&editor_path)?;

    // Write the editor index
    std::fs::write(editor_path.join("index.html"), &editor_html)?;

    // Copy web components from the geoff components directory
    let components_dir = find_components_dir();
    if let Some(ref comp_dir) = components_dir {
        for name in [
            "geoff-token-field.js",
            "geoff-token-group.js",
            "geoff-token-editor.js",
            "geoff-theme-preview.js",
            "geoff-theme-editor-app.js",
        ] {
            let src = comp_dir.join(name);
            if src.exists() {
                std::fs::copy(&src, editor_path.join(name))?;
            }
        }
    }

    // Start the dev server (which serves the site + the editor at /__geoff__/theme/)
    v.detail(&format!(
        "Open http://localhost:{port}/__geoff__/theme/ in your browser"
    ));

    // Use geoff serve with the editor static files overlaid
    cmd_serve(path.to_owned(), port, true).await
}

fn find_components_dir() -> Option<Utf8PathBuf> {
    if let Ok(exe) = std::env::current_exe()
        && let Some(exe_dir) = exe.parent()
    {
        let candidates = [
            exe_dir.join("../share/geoff/components"),
            exe_dir.join("../../components"),
        ];
        for c in &candidates {
            if let Ok(utf8) = Utf8PathBuf::try_from(c.to_path_buf())
                && utf8.exists()
            {
                return Some(utf8);
            }
        }
    }
    let cwd = Utf8PathBuf::from("components");
    if cwd.exists() {
        return Some(cwd);
    }
    None
}
