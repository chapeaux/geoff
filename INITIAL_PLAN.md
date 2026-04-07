# Geoff — Architecture Plan

## Context

Geoff is a new open source static site generator named after the "Jeff Cap" (newsboy cap), part of the Chapeaux project ecosystem. The problem it addresses: existing static site generators treat content as flat files with metadata bolted on as an afterthought. Geoff makes **linked data the foundation** — every piece of content is an RDF resource from the start, queryable via SPARQL, validatable via SHACL, and published with embedded structured data. The intended outcome is a tool that makes W3C semantic web standards (RDF 1.2, SPARQL 1.2, SHACL 1.2) accessible and practical for everyday web publishing, while providing a modern component-based authoring experience.

**Core UX principle: Users should never need to know RDF.** Geoff abstracts semantic web complexity behind human-readable interfaces. Users never type IRIs or prefixes. Instead, Geoff presents plain-language choices — "Is this a Blog Post, Video, or Location?" — and maps those to the correct ontology terms behind the scenes. The RDF layer is always there for power users who want it, but the default experience is approachable vocabulary selection, not IRI memorization.

**No existing Rust SSG integrates semantic web technologies** — this is greenfield. The Rust ecosystem has mature libraries for all core needs (Oxigraph, rudof, Sophia).

---

## Workspace Structure

```
geoff/
├── Cargo.toml                    # Workspace root
├── geoff.toml.example            # Example site config
├── CLAUDE.md
├── LICENSE (MIT)
│
├── crates/
│   ├── geoff-core/               # Config, error types, shared traits
│   ├── geoff-graph/              # Oxigraph RDF store, SPARQL, SHACL (via rudof)
│   ├── geoff-content/            # Markdown (pulldown-cmark), TOML frontmatter, [rdf] extraction
│   ├── geoff-ontology/           # Vocabulary loading, fuzzy matching, mapping resolution, shape generation
│   ├── geoff-render/             # Tera templates, JSON-LD/RDFa output, sparql() template fn
│   ├── geoff-plugin/             # Plugin trait, lifecycle hooks, registry
│   ├── geoff-deno/               # Deno subprocess bridge (JSON-RPC over stdin/stdout)
│   ├── geoff-server/             # Axum dev server, notify file watcher, WebSocket hot reload
│   └── geoff-cli/                # Binary: init, build, serve, validate, suggest, shapes
│
├── components/                   # Built-in web components (vanilla JS) for authoring UI
│   ├── geoff-editor/             # Markdown + RDF-aware frontmatter editor
│   ├── geoff-graph-view/         # Graph visualization
│   ├── geoff-vocab-picker/       # Vocabulary term browser
│   └── geoff-shacl-panel/        # Validation dashboard
│
├── ontologies/                   # Bundled vocabulary fragments (Turtle, with rdfs:label for human matching)
│   ├── schema-org.ttl            # schema.org subset for web content
│   ├── dublin-core.ttl
│   ├── foaf.ttl
│   ├── sioc.ttl                  # Blog/forum content
│   └── geoff.ttl                 # Geoff's own site-structure ontology
│
└── npm/                          # npm distribution (@chapeaux/geoff)
    ├── package.json
    ├── install.js
    └── run.js
```

Follows beret's conventions: edition 2024, `chapeaux-geoff` crate name, `geoff` binary name, release profile with `lto = true` + `codegen-units = 1`.

---

## Core Data Flow (Build Pipeline)

```
geoff.toml → [LOAD CONFIG] → [LOAD ONTOLOGY (site + public)]
                                       ↓
          [SCAN CONTENT (md + ttl)]  ←→  [LOAD PLUGINS (Rust + Deno)]
                    ↓
          [PARSE & INGEST]  →  onContentParsed hook
          Markdown → HTML, frontmatter → triples (via mappings)
                    ↓
          [BUILD GRAPH]  →  onGraphUpdated hook
          All content → unified Oxigraph store (named graphs per page)
                    ↓
          [VALIDATE (SHACL)]  →  onValidationComplete hook
                    ↓
          [RENDER PAGES]  →  onPageRender hook
          Templates + sparql() queries → HTML + JSON-LD
                    ↓
          [EMIT OUTPUT]  →  onBuildComplete hook
          Static files → dist/
```

**Named graph strategy:** Each page's triples go into `<urn:geoff:content:{path}>`, ontology into `<urn:geoff:ontology>`, site-level into `<urn:geoff:site>`. Mapped to real URLs at render time via `base_url` from `geoff.toml`.

---

## Content Model

### Primary: Human-readable TOML frontmatter with semantic mapping

Users write plain, intuitive frontmatter. **No IRIs, no prefixes, no RDF syntax.** Geoff maps it to the correct ontology terms.

```markdown
+++
title = "Getting Started with Geoff"
date = 2026-04-10
template = "blog-page.html"
type = "Blog Post"
author = "ldary"
about = ["Web Application"]
language = "en"
tags = ["tutorial", "getting-started"]
+++

# Getting Started with Geoff
Content here...
```

**How Geoff resolves this behind the scenes:**
- `type = "Blog Post"` → matched to `schema:BlogPosting` via the vocabulary index (label lookup)
- `author = "ldary"` → matched to a `schema:Person` entity defined in site data
- `about = ["Web Application"]` → matched to `schema:WebApplication`
- `language = "en"` → matched to `dc:language`

**When a term is ambiguous**, Geoff prompts during `geoff build` or in the authoring UI:
```
? "author" could map to:
  1. Author (schema.org) — The author of this creative work
  2. Creator (Dublin Core) — An entity responsible for making the resource
  3. Maker (FOAF) — An agent that made this thing
  Select [1]:
```

Once resolved, the mapping is saved to `ontology/mappings.toml` so the user is never asked twice:
```toml
[mappings]
author = "schema:author"
language = "dc:language"
type = "rdf:type"
about = "schema:about"
```

**Power user escape hatch:** Users who *want* to use IRIs directly can use an `[rdf]` table:
```toml
[rdf.custom]
"schema:wordCount" = 1500
"myontology:specialField" = "value"
```

`geoff-content` transforms all frontmatter into triples with the page URI as subject, using the mappings registry for resolution.

### Secondary: Sidecar `.ttl` files

For complex graphs, users place a companion `.ttl` alongside the `.md`. Both are merged into the page's named graph. A `content/data/` directory can hold pure RDF data (no Markdown).

---

## Plugin Lifecycle

```rust
#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    async fn on_init(&self, ctx: &mut InitContext) -> Result<()> { Ok(()) }
    async fn on_build_start(&self, ctx: &mut BuildContext) -> Result<()> { Ok(()) }
    async fn on_content_parsed(&self, ctx: &mut ContentContext) -> Result<()> { Ok(()) }
    async fn on_graph_updated(&self, ctx: &mut GraphContext) -> Result<()> { Ok(()) }
    async fn on_validation_complete(&self, ctx: &mut ValidationContext) -> Result<()> { Ok(()) }
    async fn on_page_render(&self, ctx: &mut RenderContext) -> Result<()> { Ok(()) }
    async fn on_build_complete(&self, ctx: &mut OutputContext) -> Result<()> { Ok(()) }
    async fn on_file_changed(&self, ctx: &mut WatchContext) -> Result<()> { Ok(()) }
}
```

- **Rust plugins:** `cdylib` loaded via `libloading`, exporting `create_plugin() -> Box<dyn Plugin>`
- **Deno plugins:** Subprocess + JSON-RPC over stdin/stdout (matching beret's MCP stdio pattern). Simple, debuggable, avoids embedding `deno_core`. `.ts` files export handler functions.

```toml
# geoff.toml
[[plugins]]
name = "reading-time"
runtime = "rust"
path = "plugins/geoff-reading-time"

[[plugins]]
name = "social-cards"
runtime = "deno"
path = "plugins/social-cards.ts"
```

---

## Ontology Assistance (the "Semantic Copilot")

The central design goal: **users think in human terms, Geoff handles the RDF.** The ontology system is the bridge.

### Vocabulary Resolution Pipeline

When Geoff encounters a frontmatter field it hasn't seen before:

1. **Check `ontology/mappings.toml`** — has the user already resolved this field?
2. **Fuzzy match against loaded vocabularies** — search term labels, comments, and aliases across all loaded ontologies using string similarity (`strsim`)
3. **If a single high-confidence match exists** — auto-map it and log the decision (user can override later)
4. **If multiple candidates or low confidence** — prompt the user with plain-language descriptions (in CLI during build, or in the authoring UI during `serve`)
5. **Save the resolution** to `ontology/mappings.toml` so it's never asked again

### Content Type Assistance

When creating new content (`geoff new`), Geoff offers content types from the loaded ontology in plain language:

```
$ geoff new content/blog/my-post.md

? What type of content is this?
  1. Blog Post — A blog post or article (schema.org)
  2. How-To Guide — Step-by-step instructions (schema.org)
  3. FAQ Page — A page of frequently asked questions (schema.org)
  4. Event — An event happening at a certain time and location (schema.org)
  5. Custom type...
  Select [1]:
```

Geoff then scaffolds the frontmatter with the appropriate fields for that type, pre-filled with human-readable field names.

### Components

1. **Bundled vocabulary fragments** — curated subsets of schema.org, Dublin Core, FOAF, SIOC with labels, descriptions, and aliases for human-readable matching
2. **`geoff.ttl` ontology** — Geoff's own vocabulary for site structure (`geoff:Site`, `geoff:Page`, `geoff:Section`, etc.)
3. **`ontology/mappings.toml`** — persistent field→IRI mappings, auto-generated from user choices, editable by hand
4. **`geoff shapes --generate`** — introspects existing content, detects frontmatter patterns, emits starter SHACL `NodeShape` definitions for the user to refine

---

## Dev Server (geoff-server)

- **axum** HTTP server with in-memory output cache (no disk writes during dev)
- **notify** file watcher on `content/`, `templates/`, `ontology/`, `components/`, `geoff.toml`
- **WebSocket** hot reload — injects `<script>` in dev mode, sends `reload` or `full-reload` messages
- **`/api/sparql`** endpoint — dev-only, powers authoring UI components
- **`/__geoff__/`** — authoring SPA built from web components (editor, graph view, vocab picker, validation dashboard)

---

## Key Dependencies

| Crate | Purpose | Used By |
|-------|---------|---------|
| `oxigraph` 0.5 | RDF store + SPARQL engine | `geoff-graph` |
| `rudof` | SHACL validation | `geoff-graph` |
| `sophia_turtle` | Turtle parsing (if needed beyond Oxigraph) | `geoff-ontology` |
| `pulldown-cmark` | Markdown → HTML | `geoff-content` |
| `toml` | Frontmatter parsing | `geoff-content` |
| `tera` | Jinja2-style templates (Zola-compatible) | `geoff-render` |
| `json-ld` | JSON-LD serialization | `geoff-render` |
| `axum` | HTTP server | `geoff-server` |
| `notify` | File watcher | `geoff-server` |
| `tokio` | Async runtime | throughout |
| `clap` | CLI args | `geoff-cli` |
| `libloading` | Dynamic library loading | `geoff-plugin` |
| `strsim` | String similarity for vocab suggestions | `geoff-ontology` |
| `ignore` | Gitignore-aware file walking | `geoff-content` |
| `serde` + `serde_json` | Serialization | throughout |

---

## Phased Roadmap

### Phase 1: Foundation
Minimal SSG — Markdown in, HTML + JSON-LD out, with RDF graph from frontmatter.
- `geoff-core`, `geoff-content`, `geoff-graph`, `geoff-render`, `geoff-cli`
- Commands: `geoff init`, `geoff build`
- Deliverable: `geoff build` produces static HTML with embedded JSON-LD

### Phase 2: Dev Experience
- `geoff-server` with hot reload
- `sparql()` Tera template function for querying site graph
- Dev SPARQL endpoint
- Command: `geoff serve`

### Phase 3: Ontology & Validation (the Semantic Copilot)
- `geoff-ontology` with bundled vocabularies, fuzzy matching, and interactive mapping resolution
- `ontology/mappings.toml` — persistent field→IRI resolution, auto-generated from user choices
- SHACL integration via `rudof`
- Sidecar `.ttl` support
- Content type scaffolding in `geoff new` with plain-language type selection
- Commands: `geoff validate`, `geoff new`, `geoff shapes --generate`

### Phase 4: Plugin System
- `geoff-plugin` trait + registry
- `libloading` for Rust plugins
- `geoff-deno` subprocess bridge
- Example plugins: `reading-time`, `sitemap`, `feed`

### Phase 5: Authoring UI
- Web components: editor, graph view, vocab picker, SHACL panel
- `/__geoff__/` dev UI shell

### Phase 6: Polish & Distribution
- Incremental builds, parallel rendering
- `cargo install chapeaux-geoff`, `@chapeaux/geoff` npm package
- Documentation site built with Geoff (dogfooding)
- Starter templates: `blog`, `docs`, `portfolio`

---

## Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Template engine | Tera | Zola-compatible, familiar to Chapeaux ecosystem |
| RDF store | Oxigraph | SPARQL engine is critical; matches beret's pattern |
| SHACL library | rudof | Most comprehensive (ShEx + SHACL + DCTAP) |
| Deno integration | Subprocess + JSON-RPC | Simple, debuggable, avoids embedding deno_core |
| Output format | JSON-LD (default), RDFa (opt-in) | JSON-LD is easier to generate, preferred by search engines |
| Internal URIs | `urn:geoff:content:{path}` | Stable across environments, mapped to real URLs at render |
| Spec version | Build on stable 1.1, opt-in `sparql-12` flag | 1.2 specs are still Working Drafts (as of April 2026) |
| Web components | Vanilla JS, no framework | Matches Chapeaux philosophy, no build step needed |
| User-facing semantics | Plain language, never IRIs | Users pick "Blog Post" not `schema:BlogPosting`; mappings saved in `ontology/mappings.toml` |
| Frontmatter format | TOML only | Consistent with Chapeaux ecosystem, Zola conventions |

---

## Verification Plan

1. **Phase 1:** Create a test site with 3-5 Markdown files with frontmatter. Run `geoff build`. Verify output HTML contains correct JSON-LD `<script>` blocks. Verify SPARQL queries against the built graph return expected results.
2. **Phase 2:** Run `geoff serve`, edit a Markdown file, verify browser auto-reloads with updated content. Test `sparql()` template function renders dynamic content from the graph.
3. **Phase 3:** Add SHACL shapes to `ontology/site.ttl`. Introduce a content file that violates a shape. Run `geoff validate` and verify violations are reported. Test vocabulary resolution prompts appear for unmapped fields.
4. **Phase 4:** Create a Rust plugin and a Deno plugin. Verify both receive lifecycle events and can modify the build pipeline.
5. **Phase 5:** Open `/__geoff__/` in browser. Verify editor saves content, graph view renders, validation panel shows status.
