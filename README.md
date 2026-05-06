# Geoff

A semantically rich static site generator that makes linked data the foundation of web publishing.

Geoff is named after the "Jeff Cap" (newsboy cap), part of the [Chapeaux](https://github.com/chapeaux) project ecosystem. It turns Markdown files with plain TOML frontmatter into static HTML with embedded [JSON-LD](https://json-ld.org/), backed by a queryable [RDF](https://www.w3.org/RDF/) graph.

**You never need to know RDF to use Geoff.** Write `type = "Blog Post"` in your frontmatter, and Geoff maps it to `schema:BlogPosting` behind the scenes.

## Features

- **Markdown + TOML frontmatter** — Write content in Markdown with plain-language metadata
- **RDF graph** — Every page is an RDF resource, queryable via SPARQL
- **JSON-LD output** — Embedded structured data for search engines
- **SHACL validation** — Enforce content quality with shape constraints
- **SPARQL in templates** — Build dynamic listings (blog indexes, related posts) by querying the graph
- **RDFa support** — Template helpers and Markdown syntax for semantic HTML output
- **Navigation helpers** — `pages()` and `tree()` template functions for filtering, sorting, and hierarchical navigation
- **`[data]` frontmatter** — Add linked data with friendly names, resolved via the mapping registry
- **Dev server** — Hot reload with WebSocket, SPARQL endpoint, authoring UI
- **Plugin system** — Extend with Rust (cdylib) or Deno (TypeScript) plugins
- **Vocabulary assistance** — Fuzzy matching resolves plain-language terms to ontology IRIs
- **Design token theming** — W3C DTCG tokens → CSS custom properties with inheritance, `light-dark()`, and critical/deferred split
- **Visual theme editor** — `geoff theme edit` serves a web component UI for live token editing with SHACL validation
- **Client-side SPARQL search** — Oxigraph WASM in the browser, querying the same graph that built the site
- **Asset optimization** — CSS/JS minification, image WebP conversion, cache-busting hashes
- **Incremental builds** — Only rebuild changed pages
- **Parallel rendering** — Pages render concurrently via Rayon

## Quick Start

### Install

```sh
# From crates.io
cargo install chapeaux-geoff

# Or via npm
npm install -g @chapeaux/geoff
```

### Create a Site

```sh
geoff init my-site --template blog
cd my-site
```

This scaffolds a ready-to-go blog with templates, sample content, and ontology mappings. Three starter templates are available: `blog`, `docs`, and `portfolio`.

### Write Content

Create Markdown files with TOML frontmatter:

```markdown
+++
title = "Getting Started with Geoff"
date = 2026-04-10
template = "blog-page.html"
type = "Blog Post"
author = "Jane Smith"
tags = ["tutorial", "getting-started"]
+++

# Getting Started with Geoff

Your content here...
```

No IRIs, no prefixes, no RDF syntax. Geoff resolves `type = "Blog Post"` to `schema:BlogPosting` and `author` to `schema:author` via the mapping registry. Any frontmatter field with a mapping in `ontology/mappings.toml` is automatically stored as an RDF triple — queryable via SPARQL and included in JSON-LD.

### Build

```sh
geoff build
```

Produces static HTML in `dist/` with embedded JSON-LD:

```html
<script type="application/ld+json">
{
  "@context": "https://schema.org/",
  "@type": "BlogPosting",
  "name": "Getting Started with Geoff",
  "datePublished": "2026-04-10",
  "author": { "@type": "Person", "name": "Jane Smith" }
}
</script>
```

### Develop

```sh
geoff serve
```

Starts a dev server on `http://localhost:3000` with:

- Hot reload on file changes
- SPARQL endpoint at `/api/sparql`
- Authoring UI at `/__geoff__/` (editor, graph view, vocabulary browser, validation dashboard)

## Commands

| Command | Description |
|---------|-------------|
| `geoff init [path]` | Scaffold a new site (`--template blog\|docs\|portfolio`) |
| `geoff build [path]` | Build the site to `dist/` (`--full` to skip cache) |
| `geoff serve [path]` | Start dev server with hot reload (`--port`, `--open`) |
| `geoff new <file>` | Create a new content file (`--type "Article"`, `--list-types`) |
| `geoff validate [path]` | Validate content against SHACL shapes (`--shapes <file>`) |
| `geoff shapes [path]` | Generate starter SHACL shapes from existing content |
| `geoff theme preview [path]` | Preview the current theme with sample content |
| `geoff theme edit [path]` | Visual theme editor with live preview |
| `geoff theme generate <name>` | Generate a theme from design system tokens |

Global flags: `--verbose`, `--quiet`, `--version`

## SPARQL in Templates

Query the site graph directly from Tera templates:

```html
{% set posts = sparql(query="
  SELECT ?title ?date ?path
  WHERE {
    GRAPH ?g {
      ?s a <https://schema.org/BlogPosting> ;
         <https://schema.org/name> ?title ;
         <https://schema.org/datePublished> ?date .
    }
  }
  ORDER BY DESC(?date)
") %}

{% for post in posts %}
  <article>
    <h2>{{ post.title }}</h2>
    <time>{{ post.date }}</time>
  </article>
{% endfor %}
```

## Navigation Helpers

Two template functions provide page listing and navigation without SPARQL:

### `pages()` — Filter and sort pages

```html
{% set nav = pages(section="about", sort="order") %}
{% set posts = pages(sort="date", reverse=true) %}
{% set docs = pages(navSection="docs", sort="order") %}

{% for p in nav %}
  <a href="{{ p.url }}" {% if p.url == page_url %}class="current"{% endif %}>
    {{ p.title }}
  </a>
{% endfor %}
```

Parameters: `section` (URL prefix filter), `sort` (field name), `reverse` (boolean), plus any frontmatter field as a filter (e.g. `navSection="about"`).

### `tree()` — Build navigation trees

```html
{% set nav = tree(sort="order") %}
{% for section in nav %}
  <h3>{{ section.title }}</h3>
  <ul>
    {% for child in section.children %}
      <li><a href="{{ child.url }}">{{ child.title }}</a></li>
    {% endfor %}
  </ul>
{% endfor %}
```

Parameters: `root` (subtree root path), `sort` (field name), `depth` (max nesting). Synthetic nodes are created for directories without an index page.

### Built-in Template Variables

Every page template has access to:

| Variable | Description |
|----------|-------------|
| `title` | Page title |
| `content` | Rendered HTML |
| `page_url` | Final URL path (e.g. `/about/` or `/about.html`) |
| `page_uri` | RDF graph URI (e.g. `urn:geoff:content:about.md`) |
| `date`, `author`, `description`, `tags` | Standard frontmatter fields |
| `frontmatter` | Full frontmatter as a JSON object — access any field via `{{ frontmatter.myField }}` |
| `rdfa_attrs` | Pre-built RDFa attributes for the page container (see Linked Data section) |
| `critical_css` | Inlined CSS from `static/critical*.css` files (see Critical CSS section) |
| `json_ld` | JSON-LD `<script>` block |
| `config.title` | Site title |

## Vocabulary Mapping

Geoff resolves plain frontmatter fields to ontology terms. Mappings are stored in `ontology/mappings.toml`:

```toml
[types]
"Blog Post" = "https://schema.org/BlogPosting"

[properties]
# Standard fields are mapped by default — override here if needed
title = "https://schema.org/name"
date = "https://schema.org/datePublished"
author = "https://schema.org/author"
tags = "https://schema.org/keywords"

# Custom fields — add mappings so they become RDF triples
order = "https://schema.org/position"
navSection = "urn:mysite:navSection"
status = "https://schema.org/creativeWorkStatus"
```

Any frontmatter field with a mapping becomes an RDF triple in the page's graph — queryable via SPARQL and included in JSON-LD output. Fields without mappings are still available via `{{ frontmatter.fieldName }}` and `pages()`, but won't appear in SPARQL or JSON-LD.

When Geoff encounters an unmapped field during `geoff new`, it fuzzy-matches against loaded vocabularies and prompts you to choose. The resolution is saved so you're never asked twice.

Power users can use the `[rdf]` table for direct IRI access (keys are also checked against the mapping registry):

```toml
[rdf.custom]
"schema:wordCount" = 1500
```

## Linked Data & RDFa

Geoff makes linked data a first-class feature with RDFa output, simplified frontmatter, and inline Markdown annotations — no plugins required.

### `[data]` Frontmatter

Add structured data with friendly names — no IRIs needed. Names are resolved through the mapping registry:

```toml
+++
title = "Getting Started"
type = "Blog Post"

[data]
wordCount = 1500
language = "en"
about = "Static site generators"
+++
```

Resolution chain: `ontology/mappings.toml` property lookup → prefixed IRI expansion → `urn:geoff:meta:{key}` fallback. Add mappings for any vocabulary:

```toml
# ontology/mappings.toml
[properties]
wordCount = "https://schema.org/wordCount"
broader = "http://www.w3.org/2004/02/skos/core#broader"
```

### RDFa in Templates

Template helpers generate RDFa Lite 1.1 attributes from the graph data:

```html
<html lang="en" {{ rdfa_prefix() | safe }}>
<body>
  <article {{ rdfa_attrs | safe }}>
    <h1 {{ rdfa_prop(name="title") | safe }}>{{ title }}</h1>
    <time {{ rdfa_prop(name="date") | safe }}>{{ date }}</time>
    <p>By {{ author | rdfa(prop="author") | safe }}</p>
    {{ content | safe }}
    {{ rdfa_meta(page_uri=page_uri) | safe }}
  </article>
</body>
```

Output:

```html
<html lang="en" prefix="schema: https://schema.org/ dc: http://purl.org/dc/terms/">
<body>
  <article vocab="https://schema.org/" typeof="BlogPosting" resource="/blog/welcome/">
    <h1 property="schema:name">Getting Started</h1>
    ...
  </article>
</body>
```

### Inline RDFa in Markdown

Annotate content with semantic properties using link syntax:

```markdown
Written by [John Doe](rdfa:author) at [Acme Corp](rdfa:publisher).
```

Renders as:

```html
Written by <span property="schema:author">John Doe</span> at
<span property="schema:publisher">Acme Corp</span>.
```

### Custom Vocabulary Prefixes

Declare additional vocabulary namespaces in `geoff.toml`:

```toml
[linked_data.prefixes]
skos = "http://www.w3.org/2004/02/skos/core#"
org = "http://www.w3.org/ns/org#"
```

These merge with the built-in prefixes (schema, dc, foaf, rdf, rdfs) and flow to IRI expansion, RDFa output, and JSON-LD contexts.

## Plugins

### Rust Plugin

```toml
# geoff.toml
[[plugins]]
name = "reading-time"
runtime = "rust"
path = "plugins/geoff-reading-time"
```

### Deno Plugin

```toml
[[plugins]]
name = "sitemap"
runtime = "deno"
path = "plugins/sitemap.ts"
```

Plugins hook into 8 lifecycle events: `on_init`, `on_build_start`, `on_content_parsed`, `on_graph_updated`, `on_validation_complete`, `on_page_render`, `on_build_complete`, `on_file_changed`.

A TypeScript SDK is available at `plugins/sdk/mod.ts` for writing Deno plugins.

## Project Structure

```
my-site/
├── geoff.toml              # Site configuration
├── content/                # Markdown content
│   └── blog/
│       └── my-post.md
├── templates/              # Tera templates
│   ├── base.html
│   └── blog-page.html
├── themes/                 # Design token themes
│   ├── default/
│   │   ├── tokens.json     # Base theme (DTCG format)
│   │   └── tokens.dark.json
│   └── my-brand/
│       └── theme.json      # Generated from [design] system tokens
├── ontology/
│   ├── mappings.toml       # Field → IRI mappings
│   └── shapes/             # SHACL validation shapes
├── ontologies/             # Vocabulary fragments (.ttl)
├── plugins/                # Rust/Deno plugins
├── static/                 # Static assets (CSS, images)
└── dist/                   # Build output
```

## Configuration

`geoff.toml`:

```toml
base_url = "https://example.com"
title = "My Site"

[theme]
name = "my-brand"
base = "default"
share = true

[theme.optimize]
minify_css = true

[search]
enabled = true

[[plugins]]
name = "sitemap"
runtime = "deno"
path = "plugins/sitemap.ts"

[build]
url_style = "directory"   # /about/ instead of /about.html

[linked_data]
rdfa = true
default_vocab = "https://schema.org/"

[linked_data.prefixes]
skos = "http://www.w3.org/2004/02/skos/core#"

[design]
tokens = ["./node_modules/@rhds/tokens/json/rhds.tokens.json"]
```

## Theming

Geoff's theming system is built on the [W3C Design Tokens](https://www.designtokens.org/) format (DTCG 2025.10). Themes are JSON token files that generate CSS custom properties, with inheritance for derivative themes.

### Token File

Create `themes/my-theme/tokens.json`:

```json
{
  "color-critical": {
    "$type": "color",
    "primary": { "$value": "#0066cc" },
    "background": { "$value": "#ffffff" },
    "text": { "$value": "#1a1a1a" }
  },
  "spacing": {
    "$type": "dimension",
    "md": { "$value": { "value": 16, "unit": "px" } }
  }
}
```

### Config

```toml
[theme]
name = "my-theme"
base = "default"        # Inherit from a parent theme
share = true            # Publish tokens for other sites

[theme.modes]
dark = "tokens.dark.json"

[theme.optimize]
minify_css = true
minify_js = true
hash_assets = true

[theme.optimize.images]
webp = true
quality = 80
max_width = 1920
```

### In Templates

```html
<style>
  :root {
    color-scheme: light dark;
    {{ theme_css(critical=true) | safe }}
  }
</style>
```

Token groups with `-critical` in the name are inlined in `<head>`. Everything else loads as a deferred external stylesheet via `<link rel="preload">`. Light/dark mode uses the CSS `light-dark()` function with `-on-light`/`-on-dark` primitives.

### Critical CSS from Files

CSS files in `static/` following the critical naming convention are automatically inlined into pages via the `{{ critical_css }}` template variable:

```
static/
├── critical.css                # Inlined on ALL pages
├── critical-blog-page.css      # Inlined only on pages using blog-page.html
├── critical-doc-sidebar.css    # Inlined only on pages using doc-sidebar.html
└── styles.css                  # Regular CSS (not inlined)
```

Combine with token-generated critical CSS in a single `<style>` block:

```html
<style>
  :root { {{ theme_css(critical=true) | safe }} }
  {{ critical_css | safe }}
</style>
```

### Theme Inheritance

A derivative theme overrides only what changes:

```json
{
  "color-critical": {
    "primary": { "$value": "#ee0000" }
  }
}
```

Everything else inherits from the base. Multi-level chains work (grandparent → parent → child).

### Visual Editor

```sh
geoff theme edit
```

Serves a web component UI at `/__geoff__/theme/` for live token editing — color pickers, dimension inputs, typography controls — with SHACL validation and instant preview via CSS variable injection.

### Theme Preview

```sh
geoff theme preview
```

Generates a preview site with color swatches, typography specimens, spacing scales, and every template variation rendered with sample content.

### Design System Tokens

Themes can be generated from external design system token files — separate from and independent of any theme. Configure design system token sources in `geoff.toml`:

```toml
[design]
tokens = [
  "./node_modules/@rhds/tokens/json/rhds.tokens.json",
  "./vendor/custom-spacing.json"
]
```

Multiple files merge in order (later overrides earlier). Then generate a theme:

```sh
geoff theme generate my-brand
# ✓ Generated themes/my-brand/theme.json (342 tokens, 48 light/dark pairs)
```

The generated `theme.json` references all design system tokens and auto-detects `-on-light`/`-on-dark` pairs (both suffix convention `color.brand-on-light` and group convention `color.brand.on.light`), producing `light-dark()` aggregates with fallback values:

```css
--color-brand-on-light: #e00;
--color-brand-on-dark: #f66;
--color-brand: light-dark(var(--color-brand-on-light, #e00), var(--color-brand-on-dark, #f66));
```

The design system exists independently — multiple themes can reference the same system. The generated `theme.json` is a starting point that can be customized: rename tokens, add semantic aliases, override values.

## Client-Side Search

Enable SPARQL-powered search in the browser:

```toml
[search]
enabled = true
```

```html
<script type="module" src="/geoff-search.js"></script>
<geoff-search></geoff-search>
```

At build time, Geoff exports the RDF graph as N-Triples. The `<geoff-search>` web component lazy-loads Oxigraph WASM and runs real SPARQL queries against it — the same engine and same data model that built the site. The search index is also available during `geoff serve`.

Search supports structured query syntax: `foo bar` (implicit AND), `"exact phrase"` (quoted), `foo OR bar`, and explicit `AND`. The component follows the ARIA combobox pattern with keyboard navigation (Arrow keys, Enter, Escape) and positions results using CSS anchor positioning with a fallback for older browsers.

## Architecture

Geoff is built as a Rust workspace with 10 crates:

| Crate | Purpose |
|-------|---------|
| `geoff-core` | Config, error types, shared newtypes |
| `geoff-graph` | Oxigraph RDF store, SPARQL queries |
| `geoff-content` | Markdown parsing, TOML frontmatter, content scanning |
| `geoff-ontology` | Vocabulary loading, fuzzy matching, SHACL validation |
| `geoff-render` | Tera templates, JSON-LD generation, SPARQL template function |
| `geoff-theme` | Design token parsing, CSS generation, theme inheritance |
| `geoff-plugin` | Plugin trait, lifecycle hooks, cdylib loader |
| `geoff-deno` | Deno subprocess bridge (JSON-RPC over stdin/stdout) |
| `geoff-server` | Axum dev server, file watcher, WebSocket hot reload |
| `geoff-cli` | CLI binary with all commands |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines. Contributions are accepted under the MIT license.

## License

MIT. See [LICENSE](LICENSE).

Bundled vocabulary fragments (`ontologies/`) are under their original licenses (CC BY-SA 3.0, CC BY 4.0, CC BY 1.0). See [NOTICE](NOTICE) for details.
