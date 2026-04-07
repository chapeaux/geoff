# Geoff

A semantically rich static site generator built on W3C web standards (RDF/SPARQL/SHACL).

Geoff transforms your content into a queryable knowledge graph while generating beautiful static websites. Write in plain Markdown with human-readable frontmatter — Geoff handles the semantic web complexity behind the scenes.

## Features

- **Semantic-first**: Every page is an RDF resource, queryable via SPARQL
- **Zero RDF knowledge required**: Plain-language content types, fuzzy vocabulary matching
- **SHACL validation**: Ensure content consistency with shape constraints
- **Modern dev experience**: Hot reload, live preview, graph visualization
- **Extensible**: Rust and Deno plugins for custom build logic
- **Standards-based**: RDF 1.2, SPARQL 1.2, SHACL, JSON-LD, schema.org

## Installation

### NPM

```bash
# Global installation
npm install -g @chapeaux/geoff

# Or use npx (no installation needed)
npx @chapeaux/geoff init my-site
```

### Cargo

```bash
cargo install chapeaux-geoff
```

Pre-built binaries are also available from [GitHub Releases](https://github.com/chapeaux/geoff/releases).

## Quick Start

```bash
# Create a new site
geoff init my-site
cd my-site

# Start the dev server
geoff serve

# Build for production
geoff build
```

## Usage

### Initialize a new site

```bash
geoff init my-site --template blog
# Templates: blog, docs, portfolio
```

### Create content

```bash
geoff new content/blog/my-post.md
```

Edit the generated Markdown file with human-readable frontmatter:

```markdown
+++
title = "Getting Started with Geoff"
date = 2026-04-10
type = "Blog Post"
author = "Your Name"
tags = ["tutorial", "getting-started"]
+++

# Getting Started

Your content here...
```

Geoff maps `type = "Blog Post"` to `schema:BlogPosting` automatically. No IRIs needed!

### Development server

```bash
geoff serve
# Opens http://localhost:3000
# Includes authoring UI at http://localhost:3000/__geoff__/
```

### Build static site

```bash
geoff build
# Output in dist/
```

### Validate content

```bash
geoff validate
# Checks all pages against SHACL shapes
```

## Templates

Geoff uses [Tera](https://tera.netlify.app/) templates (Jinja2-style). Access your site's knowledge graph directly in templates:

```jinja
{% set recent_posts = sparql('
  SELECT ?post ?title ?date
  WHERE {
    ?post a schema:BlogPosting ;
          schema:name ?title ;
          schema:datePublished ?date .
  }
  ORDER BY DESC(?date)
  LIMIT 5
') %}

<ul>
{% for post in recent_posts %}
  <li><a href="{{ post.url }}">{{ post.title }}</a></li>
{% endfor %}
</ul>
```

## Documentation

Full documentation at: https://github.com/chapeaux/geoff

## Philosophy

Geoff is part of the [Chapeaux](https://github.com/chapeaux) project ecosystem — tools for building and working with knowledge graphs.

**Core principle**: Users should never need to know RDF. Geoff abstracts semantic web complexity behind human-readable interfaces. The RDF layer is always there for power users, but the default experience is approachable vocabulary selection, not IRI memorization.

## License

MIT License - see [LICENSE](https://github.com/chapeaux/geoff/blob/main/LICENSE)

## Contributing

Contributions welcome! See [CONTRIBUTING.md](https://github.com/chapeaux/geoff/blob/main/CONTRIBUTING.md)

Report bugs at: https://github.com/chapeaux/geoff/issues
