/**
 * Geoff Plugin: Atom Feed
 *
 * Generates an Atom feed (feed.xml) from blog posts.
 *
 * Usage in geoff.toml:
 *   [[plugins]]
 *   name = "feed"
 *   runtime = "deno"
 *   path = "plugins/feed.ts"
 */

import { definePlugin } from "./sdk/mod.ts";
import type { OutputContext, BuildCompleteResult, PageInfo } from "./sdk/mod.ts";

function escapeXml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&apos;");
}

function pageUrl(baseUrl: string, page: PageInfo): string {
  const slug = page.path
    .replace(/\.md$/, "")
    .replace(/\/index$/, "")
    .replace(/^index$/, "");
  const base = baseUrl.replace(/\/$/, "");
  return slug === "" ? `${base}/` : `${base}/${slug}/`;
}

function isBlogPost(page: PageInfo): boolean {
  const type = page.frontmatter.type;
  if (typeof type !== "string") return false;
  const normalized = type.toLowerCase();
  return normalized === "blog post" || normalized === "blog posting" || normalized === "post";
}

function buildAtomFeed(baseUrl: string, title: string, pages: PageInfo[]): string {
  const base = baseUrl.replace(/\/$/, "");
  const feedUrl = `${base}/feed.xml`;

  // Filter to blog posts and sort by date descending
  const posts = pages
    .filter(isBlogPost)
    .filter((p) => p.frontmatter.date)
    .sort((a, b) => {
      const da = String(a.frontmatter.date);
      const db = String(b.frontmatter.date);
      return db.localeCompare(da);
    });

  // Limit to 20 most recent
  const recent = posts.slice(0, 20);

  const updated = recent.length > 0
    ? `${recent[0].frontmatter.date}T00:00:00Z`
    : new Date().toISOString();

  const entries = recent.map((page) => {
    const url = escapeXml(pageUrl(baseUrl, page));
    const pageTitle = escapeXml(page.title);
    const date = `${page.frontmatter.date}T00:00:00Z`;
    const author = page.frontmatter.author
      ? `\n    <author><name>${escapeXml(String(page.frontmatter.author))}</name></author>`
      : "";
    const summary = page.frontmatter.description
      ? `\n    <summary>${escapeXml(String(page.frontmatter.description))}</summary>`
      : "";

    return [
      `  <entry>`,
      `    <title>${pageTitle}</title>`,
      `    <link href="${url}" />`,
      `    <id>${url}</id>`,
      `    <updated>${date}</updated>${author}${summary}`,
      `  </entry>`,
    ].join("\n");
  });

  return [
    `<?xml version="1.0" encoding="UTF-8"?>`,
    `<feed xmlns="http://www.w3.org/2005/Atom">`,
    `  <title>${escapeXml(title)}</title>`,
    `  <link href="${escapeXml(base)}/" />`,
    `  <link href="${escapeXml(feedUrl)}" rel="self" />`,
    `  <id>${escapeXml(base)}/</id>`,
    `  <updated>${updated}</updated>`,
    ...entries,
    `</feed>`,
  ].join("\n");
}

definePlugin({
  name: "feed",

  on_build_complete(ctx: OutputContext): BuildCompleteResult {
    const xml = buildAtomFeed(ctx.config.base_url, ctx.config.title, ctx.pages);
    return {
      addFiles: [{ path: "feed.xml", content: xml }],
    };
  },
});
