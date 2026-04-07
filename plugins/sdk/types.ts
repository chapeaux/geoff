/**
 * Geoff Plugin SDK — Type Definitions
 *
 * Types for the data structures exchanged between Geoff's Rust core
 * and Deno plugins over the JSON-RPC bridge.
 */

// ---------------------------------------------------------------------------
// Core data types
// ---------------------------------------------------------------------------

/** An RDF triple in serialized form. */
export interface Triple {
  subject: string;
  predicate: string;
  object: string;
  /** If present, the value is a literal with this datatype IRI. */
  datatype?: string;
  /** If present, the value is a literal with this language tag. */
  language?: string;
}

/** Metadata about a content page. */
export interface PageInfo {
  /** Internal URI, e.g. "urn:geoff:content:blog/my-post" */
  uri: string;
  /** Page title from frontmatter. */
  title: string;
  /** Source path relative to content_dir, e.g. "blog/my-post.md" */
  path: string;
  /** Rendered HTML content (body only, no template wrapper). */
  content: string;
  /** Raw TOML frontmatter parsed to a JS object. */
  frontmatter: Record<string, unknown>;
}

/** A file to be written to the output directory. */
export interface OutputFile {
  /** Path relative to output_dir, e.g. "sitemap.xml" */
  path: string;
  /** File content as a UTF-8 string. */
  content: string;
}

/** A SHACL validation result. */
export interface ValidationResult {
  /** Severity: "violation", "warning", or "info". */
  severity: "violation" | "warning" | "info";
  /** The URI of the node that failed validation. */
  focusNode: string;
  /** Human-readable message describing the issue. */
  message: string;
  /** The property path that was validated. */
  path?: string;
}

/** Site configuration from geoff.toml. */
export interface SiteConfig {
  title: string;
  base_url: string;
  content_dir: string;
  template_dir: string;
  output_dir: string;
  [key: string]: unknown;
}

// ---------------------------------------------------------------------------
// Lifecycle context types
//
// Each context is passed to the corresponding plugin hook. Fields are
// populated by Geoff's Rust core and serialized as JSON over the bridge.
// ---------------------------------------------------------------------------

/** Passed to on_init — plugin startup. */
export interface InitContext {
  config: SiteConfig;
}

/** Passed to on_build_start — build is about to begin. */
export interface BuildContext {
  config: SiteConfig;
}

/** Passed to on_content_parsed — a single content file was parsed. */
export interface ContentContext {
  config: SiteConfig;
  /** The page that was just parsed. */
  page: PageInfo;
  /** Triples extracted from frontmatter for this page. */
  triples: Triple[];
}

/** Passed to on_graph_updated — all content has been ingested into the graph. */
export interface GraphContext {
  config: SiteConfig;
  /** All pages in the site. */
  pages: PageInfo[];
  /** Execute a SPARQL SELECT query against the site graph. */
  // Note: in the JSON-RPC bridge, SPARQL is handled via a request/response
  // round-trip. The SDK wraps this for the plugin author.
}

/** Passed to on_validation_complete — SHACL validation finished. */
export interface ValidationContext {
  config: SiteConfig;
  /** Validation results from SHACL. */
  results: ValidationResult[];
  /** Whether validation passed (no violations). */
  conforms: boolean;
}

/** Passed to on_page_render — a page is about to be rendered. */
export interface RenderContext {
  config: SiteConfig;
  /** The page being rendered. */
  page: PageInfo;
  /** Template variables available to the template engine. */
  templateVars: Record<string, unknown>;
}

/** Passed to on_build_complete — all pages rendered, output ready. */
export interface OutputContext {
  config: SiteConfig;
  /** All pages that were rendered. */
  pages: PageInfo[];
  /** Files already written to the output directory. */
  outputFiles: OutputFile[];
}

/** Passed to on_file_changed — a source file changed during `geoff serve`. */
export interface WatchContext {
  config: SiteConfig;
  /** Path of the changed file, relative to the site root. */
  changedPath: string;
  /** Type of change. */
  changeKind: "create" | "modify" | "delete";
}

// ---------------------------------------------------------------------------
// Plugin definition
// ---------------------------------------------------------------------------

/**
 * A Geoff plugin definition.
 *
 * Implement any subset of the lifecycle hooks. Hooks that are not provided
 * are simply skipped during the build.
 */
export interface GeoffPlugin {
  /** Unique plugin name. */
  name: string;

  on_init?(ctx: InitContext): Promise<void> | void;
  on_build_start?(ctx: BuildContext): Promise<void> | void;
  on_content_parsed?(ctx: ContentContext): Promise<ContentHookResult | void> | ContentHookResult | void;
  on_graph_updated?(ctx: GraphContext): Promise<GraphHookResult | void> | GraphHookResult | void;
  on_validation_complete?(ctx: ValidationContext): Promise<void> | void;
  on_page_render?(ctx: RenderContext): Promise<RenderHookResult | void> | RenderHookResult | void;
  on_build_complete?(ctx: OutputContext): Promise<BuildCompleteResult | void> | BuildCompleteResult | void;
  on_file_changed?(ctx: WatchContext): Promise<void> | void;
}

// ---------------------------------------------------------------------------
// Hook result types — what plugins can return to modify the build
// ---------------------------------------------------------------------------

/** Returned from on_content_parsed to inject or modify triples. */
export interface ContentHookResult {
  /** Additional triples to add to the page's named graph. */
  addTriples?: Triple[];
}

/** Returned from on_graph_updated to inject triples into the store. */
export interface GraphHookResult {
  /** Additional triples to add (graph URI determined by subject). */
  addTriples?: Triple[];
}

/** Returned from on_page_render to modify template variables. */
export interface RenderHookResult {
  /** Additional or overridden template variables. */
  templateVars?: Record<string, unknown>;
}

/** Returned from on_build_complete to emit additional output files. */
export interface BuildCompleteResult {
  /** Extra files to write to the output directory. */
  addFiles?: OutputFile[];
}
