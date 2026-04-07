+++
title = "Getting Started"
template = "doc-page.html"
type = "Documentation"
description = "Learn how to set up and use this project."
+++

# Getting Started

Welcome to the documentation. This guide will help you get up and running.

## Installation

```bash
cargo install chapeaux-geoff
```

## Quick Start

1. Initialize a new site: `geoff init --template docs my-docs`
2. Start the dev server: `geoff serve`
3. Edit files in `content/docs/`
4. Build for production: `geoff build`

## Project Structure

```
my-docs/
  geoff.toml          # Site configuration
  content/docs/       # Documentation pages
  templates/          # HTML templates
  ontology/           # Semantic mappings
  dist/               # Built output
```
