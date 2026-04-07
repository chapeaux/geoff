# Geoff

Geoff is a semantically rich static site generator built on W3C web standards (RDF/SPARQL/SHACL), with a Rust core and Deno plugin support.

## Getting Started

See `INITIAL_PLAN.md` for the full architecture plan, workspace structure, phased roadmap, and design decisions.

## Core Principle

**Users should never need to know RDF.** Geoff abstracts semantic web complexity behind human-readable interfaces. Users write plain frontmatter (`type = "Blog Post"`), and Geoff resolves it to ontology terms (`schema:BlogPosting`) via fuzzy matching and interactive prompts. Mappings are persisted in `ontology/mappings.toml`.

## Team

Each role has a `SKILL.md` defining responsibilities, handoff protocols, and standards. Always start with the `team-lead` to understand the orchestration model.

Roles: Team Lead, Ontologist, Architect, Rust Engineer, Deno Engineer, Frontend Engineer, Designer, QA Engineer, Legal, Compliance, DevOps.

## Part of Chapeaux

Geoff follows beret's conventions (edition 2024, `chapeaux-geoff` crate name, Oxigraph for RDF, release LTO). See `../beret/` for reference patterns.
