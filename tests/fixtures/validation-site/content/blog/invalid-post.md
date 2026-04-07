+++
template = "blog-page.html"
type = "Blog Post"
tags = ["oops"]
+++

# Untitled Post

This blog post is intentionally missing required fields to trigger SHACL
violations:

- No `title` — violates BlogPostShape (schema:name minCount 1)
- No `date` — violates BlogPostShape (schema:datePublished minCount 1)

The validator should report these as violations with human-readable messages.
