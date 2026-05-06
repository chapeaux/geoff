# Geoff

Geoff is a semantically rich static site generator built on W3C web standards (RDF/SPARQL/SHACL), with a Rust core and Deno plugin support.

## Getting Started

See `INITIAL_PLAN.md` for the full architecture plan, workspace structure, phased roadmap, and design decisions.

## Core Principle

**Users should never need to know RDF.** Geoff abstracts semantic web complexity behind human-readable interfaces. Users write plain frontmatter (`type = "Blog Post"`), and Geoff resolves it to ontology terms (`schema:BlogPosting`) via fuzzy matching and interactive prompts. Mappings are persisted in `ontology/mappings.toml`.

## Key Architecture Notes

- **Schema.org namespace**: All IRIs use `https://schema.org/` (with TLS). `KNOWN_PREFIXES` in `mappings.rs` is the canonical prefix list.
- **MappingRegistry**: Owns both built-in and user-declared prefixes (`extra_prefixes` field). `expand_iri()` and `compact_iri()` are instance methods that check both. User prefixes come from `config.linked_data.prefixes`.
- **Pipeline phases**: `ingest_content()` → plugin hooks → `render_pages()`. The page index (for `pages()`/`tree()`) is built during ingestion for ALL pages (including incremental-skipped ones). Auto-generated `urn:geoff:meta:depth` and `urn:geoff:meta:parent` triples per page.
- **`pages()` depth**: `pages(section="foundations", depth=1)` returns only direct children. Depth counts path segments relative to the section prefix.
- **Template variables**: `page_url`, `page_uri`, `rdfa_attrs`, `critical_css`, `frontmatter` are built-in on every page. `frontmatter` contains ALL TOML fields as JSON.
- **Frontmatter → triples**: Top-level frontmatter fields with a mapping in `ontology/mappings.toml` (or seeded defaults) are automatically stored as RDF triples via `insert_mapped_frontmatter_triples()`. No `[rdf.custom]` or `[data]` needed for mapped fields.
- **Frontmatter sections**: `[rdf.custom]` for explicit IRIs (also checks mapping registry), `[data]` for friendly-name linked data resolved via the mapping registry.
- **RDFa helpers**: `rdfa_prefix()`, `rdfa_prop()`, `rdfa_meta()` functions and `rdfa` filter in renderer.rs. Registered via `register_rdfa_functions()` which needs `Arc<ContentStore>` and `Arc<MappingRegistry>`.
- **Markdown RDFa**: `[text](rdfa:property)` → `<span property="...">text</span>` via `rewrite_rdfa_links()` in markdown.rs.
- **JSON-LD**: `build_jsonld_from_graph()` in jsonld.rs serializes all page triples. `build_jsonld()` is the legacy fallback.
- **`[linked_data]` config**: Controls RDFa, JSON-LD richness, Markdown link rewriting, default vocab, and custom prefixes.
- **Critical CSS**: `static/critical.css` (global) and `static/critical-{template}.css` (per-template) are scanned during ingestion and inlined via the `critical_css` template variable. Populated per page based on template name.
- **Design system tokens**: `[design] tokens = [...]` config loads external DTCG files. `geoff theme generate` creates `theme.json` with light-dark() aggregates. Token references resolve across file boundaries via `resolve_references_with_base()`. Inline `{ref}` resolution works in any string context (light-dark, color-mix, calc, etc.).

## Team

Each role has a `SKILL.md` defining responsibilities, handoff protocols, and standards. Always start with the `team-lead` to understand the orchestration model.

Roles: Team Lead, Ontologist, Architect, Rust Engineer, Deno Engineer, Frontend Engineer, Designer, QA Engineer, Legal, Compliance, DevOps.

## Part of Chapeaux

Geoff follows beret's conventions (edition 2024, `chapeaux-geoff` crate name, Oxigraph for RDF, release LTO). See `../beret/` for reference patterns.
