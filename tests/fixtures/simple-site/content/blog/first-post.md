+++
title = "Getting Started with Geoff"
date = 2026-04-10
template = "blog-page.html"
type = "Blog Post"
author = "Jane Smith"
description = "A quick introduction to building sites with Geoff, the semantically rich static site generator."
tags = ["tutorial", "getting-started"]
language = "en"
+++

# Getting Started with Geoff

Geoff is a static site generator that makes linked data practical for everyday
web publishing. You write plain Markdown with simple frontmatter, and Geoff
handles the semantic web complexity behind the scenes.

## Your First Site

Create a new site with `geoff init`:

```
$ geoff init my-site
$ cd my-site
$ geoff build
```

That's it. Your `dist/` directory now contains HTML pages with embedded JSON-LD
structured data, ready for search engines and knowledge graphs.

## Content Types

When you set `type = "Blog Post"` in your frontmatter, Geoff maps that to the
correct schema.org type automatically. No IRIs, no prefixes, no RDF knowledge
required.
