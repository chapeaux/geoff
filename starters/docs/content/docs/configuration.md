+++
title = "Configuration"
template = "doc-page.html"
type = "Documentation"
description = "Configure your Geoff site."
+++

# Configuration

Geoff is configured through `geoff.toml` at the root of your site.

## Site Settings

```toml
base_url = "https://docs.example.com"
title = "My Documentation"
content_dir = "content"
output_dir = "dist"
template_dir = "templates"
```

## Content Types

Map your content types to schema.org terms in `ontology/mappings.toml`:

```toml
[types]
"Documentation" = "http://schema.org/TechArticle"
"Guide" = "http://schema.org/HowTo"
```

## Templates

Templates use the Tera template engine. Available variables:

- `title` — Page title
- `content` — Rendered HTML content
- `date` — Publication date
- `author` — Author name
- `tags` — List of tags
- `json_ld` — JSON-LD structured data
- `config.title` — Site title
