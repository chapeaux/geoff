/**
 * Geoff SSR Worker — Server-side rendering for web components via JSON-RPC 2.0
 * on stdin/stdout (same protocol as geoff's plugin bridge).
 *
 * Run: echo '{"jsonrpc":"2.0","method":"render_component","params":{...},"id":1}' \
 *        | deno run --allow-read --allow-net components/ssr-worker.ts
 */

// Shared browser globals (set up before linkedom, used by both renderers)
// deno-lint-ignore no-explicit-any
globalThis.window = globalThis as any;
// deno-lint-ignore no-explicit-any
globalThis.localStorage = {
  getItem: () => null, setItem: () => {}, removeItem: () => {},
  clear: () => {}, get length() { return 0; }, key: () => null,
} as any;
globalThis.requestAnimationFrame = (fn: FrameRequestCallback) =>
  setTimeout(fn, 0) as unknown as number;
globalThis.cancelAnimationFrame = (id: number) => clearTimeout(id);
// Stub fetch — SSR should not make real network requests
globalThis.fetch = (() =>
  Promise.reject(new Error("[ssr-worker] fetch is not available during SSR"))
) as typeof fetch;

// Lazy-loaded linkedom DOM environment (only initialized when linkedom renderer is first used)
let linkedomReady = false;

async function ensureLinkedom(): Promise<void> {
  if (linkedomReady) return;
  const { parseHTML } = await import("npm:linkedom");
  const { document, HTMLElement, customElements, CustomEvent, Event, MutationObserver } =
    parseHTML("<!DOCTYPE html><html><body></body></html>");

  globalThis.document = document;
  globalThis.HTMLElement = HTMLElement;
  globalThis.customElements = customElements;
  globalThis.CustomEvent = CustomEvent;
  globalThis.Event = Event;
  globalThis.MutationObserver = MutationObserver;
  linkedomReady = true;
}

// Module cache — components are imported once and persist across requests
const importedModules = new Set<string>();

interface RenderParams {
  scriptPath: string;
  tagName: string;
  attributes: Record<string, string>;
  children?: string;
  renderer?: "linkedom" | "lit";
}

async function renderComponent(params: RenderParams): Promise<{ html: string; hasShadowRoot: boolean }> {
  if (params.renderer === "lit") {
    return renderWithLit(params);
  }

  // Linkedom renderer: lazy-init DOM environment on first use
  await ensureLinkedom();

  if (!importedModules.has(params.scriptPath)) {
    const url = params.scriptPath.startsWith("file://")
      ? params.scriptPath
      : `file://${params.scriptPath}`;
    try {
      await import(url);
      importedModules.add(params.scriptPath);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      throw new Error(`Failed to import ${params.scriptPath}: ${msg}`);
    }
  }

  const el = document.createElement(params.tagName);

  for (const [k, v] of Object.entries(params.attributes || {})) {
    el.setAttribute(k, v);
  }
  if (params.children) {
    el.innerHTML = params.children;
  }

  // Append triggers connectedCallback
  document.body.appendChild(el);

  // Let microtasks and setTimeout(0) callbacks settle
  await new Promise((r) => setTimeout(r, 10));

  const shadowRoot = el.shadowRoot;
  const html = shadowRoot ? shadowRoot.innerHTML : el.innerHTML;
  const hasShadowRoot = !!shadowRoot;

  document.body.removeChild(el);
  return { html, hasShadowRoot };
}

/** Strip Lit SSR hydration markers from rendered HTML. */
function stripLitMarkers(html: string): string {
  return html
    .replace(/<\?>/g, "")
    .replace(/<!--lit-part[^>]*-->/g, "")
    .replace(/<!--\/lit-part-->/g, "")
    .replace(/<!--lit-node [^>]*-->/g, "")
    .replace(/<!--\/?-->/g, "");
}

// Lit SSR rendering path
let litSsrModule: any = null;

async function renderWithLit(params: RenderParams): Promise<{ html: string; hasShadowRoot: boolean }> {
  // Lazy-load @lit-labs/ssr
  if (!litSsrModule) {
    try {
      litSsrModule = await import("npm:@lit-labs/ssr");
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      throw new Error(
        `Lit SSR requires @lit-labs/ssr. Install with: deno add npm:@lit-labs/ssr. Error: ${msg}`
      );
    }
  }

  // Import the component module
  if (!importedModules.has(params.scriptPath)) {
    const url = params.scriptPath.startsWith("file://")
      ? params.scriptPath
      : `file://${params.scriptPath}`;
    try {
      await import(url);
      importedModules.add(params.scriptPath);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      throw new Error(`Failed to import ${params.scriptPath}: ${msg}`);
    }
  }

  const { render } = litSsrModule;
  const { html: litHtml } = await import("npm:lit");

  // Build attribute string for the template
  const attrStr = Object.entries(params.attributes || {})
    .map(([k, v]) => `${k}="${v.replace(/"/g, "&quot;")}"`)
    .join(" ");

  const children = params.children || "";
  const templateStr = `<${params.tagName}${attrStr ? " " + attrStr : ""}>${children}</${params.tagName}>`;

  // Use Lit's html tagged template to render
  // We need to use unsafeHTML since we're constructing from strings
  const { unsafeHTML } = await import("npm:lit/directives/unsafe-html.js");
  const template = litHtml`${unsafeHTML(templateStr)}`;

  // Render to string
  const result = render(template);
  let output = "";
  for (const chunk of result) {
    output += typeof chunk === "string" ? chunk : String(chunk);
  }

  // Strip Lit SSR hydration markers — only needed for Lit client-side hydration,
  // not for DSD initial paint
  output = stripLitMarkers(output);

  const hasShadowRoot = output.includes("shadowrootmode");
  return { html: output, hasShadowRoot };
}

// JSON-RPC 2.0 constants and types
const METHOD_NOT_FOUND = -32601;
const INTERNAL_ERROR = -32000;
const PARSE_ERROR = -32700;

interface JsonRpcRequest {
  jsonrpc: "2.0";
  id: number | string;
  method: string;
  params?: Record<string, unknown>;
}

interface JsonRpcResponse {
  jsonrpc: "2.0";
  id: number | string;
  result?: unknown;
  error?: { code: number; message: string };
}

function respond(id: number | string, result: unknown): JsonRpcResponse {
  return { jsonrpc: "2.0", id, result };
}

function respondError(id: number | string, code: number, message: string): JsonRpcResponse {
  return { jsonrpc: "2.0", id, error: { code, message } };
}

// I/O helpers (matching plugins/sdk/jsonrpc.ts style)
const encoder = new TextEncoder();
const decoder = new TextDecoder();

function writeLine(line: string): void {
  Deno.stdout.writeSync(encoder.encode(line + "\n"));
}

// Main I/O loop — reads newline-delimited JSON-RPC from stdin
async function main(): Promise<void> {
  const buf = new Uint8Array(65536);
  let leftover = "";

  while (true) {
    const n = await Deno.stdin.read(buf);
    if (n === null) break; // EOF

    const chunk = leftover + decoder.decode(buf.subarray(0, n));
    const lines = chunk.split("\n");
    leftover = lines.pop() ?? "";

    for (const line of lines) {
      const trimmed = line.trim();
      if (trimmed === "") continue;

      let request: JsonRpcRequest;
      try {
        request = JSON.parse(trimmed) as JsonRpcRequest;
      } catch {
        writeLine(JSON.stringify(respondError(0, PARSE_ERROR, "Invalid JSON")));
        continue;
      }

      let response: JsonRpcResponse;
      try {
        if (request.method === "shutdown") {
          writeLine(JSON.stringify(respond(request.id, null)));
          Deno.exit(0);
        } else if (request.method === "render_component") {
          const result = await renderComponent(request.params as unknown as RenderParams);
          response = respond(request.id, result);
        } else {
          response = respondError(request.id, METHOD_NOT_FOUND, `Unknown method: ${request.method}`);
        }
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        response = respondError(request.id, INTERNAL_ERROR, msg);
      }

      writeLine(JSON.stringify(response));
    }
  }
}

main();
