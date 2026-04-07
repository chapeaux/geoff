/**
 * Geoff Plugin: Sitemap
 *
 * Generates a sitemap.xml from all rendered pages.
 *
 * Usage in geoff.toml:
 *   [[plugins]]
 *   name = "sitemap"
 *   runtime = "deno"
 *   path = "plugins/sitemap.ts"
 */

import { definePlugin } from "./sdk/mod.ts";
import type { OutputContext, BuildCompleteResult, PageInfo } from "./sdk/mod.ts";

function pageUrl(baseUrl: string, page: PageInfo): string {
  // Convert source path to URL path:
  //   "blog/my-post.md" → "blog/my-post/"
  //   "index.md"        → ""
  const slug = page.path
    .replace(/\.md$/, "")
    .replace(/\/index$/, "")
    .replace(/^index$/, "");
  const base = baseUrl.replace(/\/$/, "");
  return slug === "" ? `${base}/` : `${base}/${slug}/`;
}

function escapeXml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&apos;");
}

function buildSitemap(baseUrl: string, pages: PageInfo[]): string {
  const urls = pages.map((page) => {
    const loc = escapeXml(pageUrl(baseUrl, page));
    const date = page.frontmatter.date;
    const lastmod = date ? `\n    <lastmod>${date}</lastmod>` : "";
    return `  <url>\n    <loc>${loc}</loc>${lastmod}\n  </url>`;
  });

  return [
    `<?xml version="1.0" encoding="UTF-8"?>`,
    `<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">`,
    ...urls,
    `</urlset>`,
  ].join("\n");
}

definePlugin({
  name: "sitemap",

  on_build_complete(ctx: OutputContext): BuildCompleteResult {
    const xml = buildSitemap(ctx.config.base_url, ctx.pages);
    return {
      addFiles: [{ path: "sitemap.xml", content: xml }],
    };
  },
});
