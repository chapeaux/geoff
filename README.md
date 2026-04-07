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
- **Dev server** — Hot reload with WebSocket, SPARQL endpoint, authoring UI
- **Plugin system** — Extend with Rust (cdylib) or Deno (TypeScript) plugins
- **Vocabulary assistance** — Fuzzy matching resolves plain-language terms to ontology IRIs
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

No IRIs, no prefixes, no RDF syntax. Geoff resolves `type = "Blog Post"` to `schema:BlogPosting` and `author` to `schema:author` via the mapping registry.

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
  "author": "Jane Smith"
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

Global flags: `--verbose`, `--quiet`, `--version`

## SPARQL in Templates

Query the site graph directly from Tera templates:

```html
{% set posts = sparql(query="
  SELECT ?title ?date ?path
  WHERE {
    GRAPH ?g {
      ?s a <http://schema.org/BlogPosting> ;
         <http://schema.org/name> ?title ;
         <http://schema.org/datePublished> ?date .
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

## Vocabulary Mapping

Geoff resolves plain frontmatter fields to ontology terms. Mappings are stored in `ontology/mappings.toml`:

```toml
[mappings]
title = "schema:name"
date = "schema:datePublished"
author = "schema:author"
type = "rdf:type"
tags = "schema:keywords"
description = "schema:description"
```

When Geoff encounters an unmapped field, it fuzzy-matches against loaded vocabularies and prompts you to choose. The resolution is saved so you're never asked twice.

Power users can use the `[rdf]` table for direct IRI access:

```toml
[rdf.custom]
"schema:wordCount" = 1500
```

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
├── ontology/
│   ├── mappings.toml       # Field → IRI mappings
│   └── shapes/             # SHACL validation shapes
├── ontologies/             # Vocabulary fragments (.ttl)
├── plugins/                # Rust/Deno plugins
└── dist/                   # Build output
```

## Configuration

`geoff.toml`:

```toml
base_url = "https://example.com"
title = "My Site"
content_dir = "content"
output_dir = "dist"
template_dir = "templates"

[[plugins]]
name = "sitemap"
runtime = "deno"
path = "plugins/sitemap.ts"
```

## Architecture

Geoff is built as a Rust workspace with 9 crates:

| Crate | Purpose |
|-------|---------|
| `geoff-core` | Config, error types, shared newtypes |
| `geoff-graph` | Oxigraph RDF store, SPARQL queries |
| `geoff-content` | Markdown parsing, TOML frontmatter, content scanning |
| `geoff-ontology` | Vocabulary loading, fuzzy matching, SHACL validation |
| `geoff-render` | Tera templates, JSON-LD generation, SPARQL template function |
| `geoff-plugin` | Plugin trait, lifecycle hooks, cdylib loader |
| `geoff-deno` | Deno subprocess bridge (JSON-RPC over stdin/stdout) |
| `geoff-server` | Axum dev server, file watcher, WebSocket hot reload |
| `geoff-cli` | CLI binary with all commands |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines. Contributions are accepted under the MIT license.

## License

MIT. See [LICENSE](LICENSE).

Bundled vocabulary fragments (`ontologies/`) are under their original licenses (CC BY-SA 3.0, CC BY 4.0, CC BY 1.0). See [NOTICE](NOTICE) for details.
