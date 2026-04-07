use std::sync::Arc;

use camino::Utf8Path;
use geoff_core::config::SiteConfig;
use geoff_graph::store::ContentStore;
use geoff_render::pipeline::build_site;
use geoff_render::renderer::SiteRenderer;

fn fixture_path() -> camino::Utf8PathBuf {
    let manifest_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("../../tests/fixtures/simple-site")
}

/// Run the build pipeline on the test fixtures and return (output_dir, store).
fn run_build() -> (tempfile::TempDir, ContentStore) {
    let site_path_buf = fixture_path();
    let site_path = site_path_buf.as_path();
    let config = SiteConfig::from_file(&site_path.join("geoff.toml")).unwrap();

    let template_dir = site_path.join(&config.template_dir);
    let output_tmpdir = tempfile::tempdir().unwrap();
    let output_dir = Utf8Path::from_path(output_tmpdir.path()).unwrap();

    let store = ContentStore::new().unwrap();
    let mut renderer = SiteRenderer::new(&template_dir).unwrap();
    renderer.register_sparql_function(Arc::new(store.clone()));

    let pages = build_site(site_path, &config, &store, &renderer).unwrap();

    for page in &pages {
        let out_path = output_dir.join(&page.output_path);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&out_path, &page.html).unwrap();
    }

    (output_tmpdir, store)
}

#[test]
fn output_files_exist() {
    let (tmpdir, _store) = run_build();
    let output = Utf8Path::from_path(tmpdir.path()).unwrap();

    assert!(output.join("index.html").exists());
    assert!(output.join("about.html").exists());
    assert!(output.join("blog/first-post.html").exists());
    assert!(output.join("blog/second-post.html").exists());
}

#[test]
fn html_contains_jsonld_script_block() {
    let (tmpdir, _store) = run_build();
    let output = Utf8Path::from_path(tmpdir.path()).unwrap();

    let first_post = std::fs::read_to_string(output.join("blog/first-post.html")).unwrap();
    assert!(
        first_post.contains("application/ld+json"),
        "Output HTML should contain a JSON-LD script block"
    );
}

#[test]
fn jsonld_has_correct_type() {
    let (tmpdir, _store) = run_build();
    let output = Utf8Path::from_path(tmpdir.path()).unwrap();

    let first_post = std::fs::read_to_string(output.join("blog/first-post.html")).unwrap();
    assert!(
        first_post.contains("\"BlogPosting\""),
        "Blog post JSON-LD should have @type BlogPosting"
    );

    let about = std::fs::read_to_string(output.join("about.html")).unwrap();
    assert!(
        about.contains("\"WebPage\""),
        "About page JSON-LD should have @type WebPage"
    );
}

#[test]
fn jsonld_has_correct_title() {
    let (tmpdir, _store) = run_build();
    let output = Utf8Path::from_path(tmpdir.path()).unwrap();

    let first_post = std::fs::read_to_string(output.join("blog/first-post.html")).unwrap();
    assert!(
        first_post.contains("Getting Started with Geoff"),
        "JSON-LD should contain the page title"
    );
}

#[test]
fn html_contains_rendered_markdown() {
    let (tmpdir, _store) = run_build();
    let output = Utf8Path::from_path(tmpdir.path()).unwrap();

    let first_post = std::fs::read_to_string(output.join("blog/first-post.html")).unwrap();
    assert!(
        first_post.contains("<h1>Getting Started with Geoff</h1>"),
        "Output should contain rendered Markdown heading"
    );
    assert!(
        first_post.contains("<h2>Your First Site</h2>"),
        "Output should contain rendered Markdown subheading"
    );
}

#[test]
fn sparql_returns_all_titles() {
    let (_tmpdir, store) = run_build();

    let json = store
        .query_to_json(
            "SELECT ?title WHERE { GRAPH ?g { ?s <http://schema.org/name> ?title } } ORDER BY ?title",
        )
        .unwrap();

    let rows = json.as_array().unwrap();
    assert_eq!(rows.len(), 4, "Should have 4 pages with titles");

    let titles: Vec<&str> = rows.iter().map(|r| r["title"].as_str().unwrap()).collect();
    assert!(titles.contains(&"About"));
    assert!(titles.contains(&"Getting Started with Geoff"));
    assert!(titles.contains(&"Minimal Frontmatter"));
    assert!(titles.contains(&"Welcome"));
}

#[test]
fn sparql_blog_posts_have_correct_type() {
    let (_tmpdir, store) = run_build();

    let json = store
        .query_to_json("SELECT ?s WHERE { GRAPH ?g { ?s a <http://schema.org/BlogPosting> } }")
        .unwrap();

    let rows = json.as_array().unwrap();
    assert_eq!(
        rows.len(),
        2,
        "Should have 2 blog posts typed as BlogPosting"
    );
}
