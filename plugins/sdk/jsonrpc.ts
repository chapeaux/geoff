/**
 * Geoff Plugin SDK — JSON-RPC Protocol Handler
 *
 * Reads newline-delimited JSON-RPC 2.0 requests from stdin, dispatches
 * them to the appropriate plugin handler, and writes responses to stdout.
 */

import type { GeoffPlugin } from "./types.ts";

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 types
// ---------------------------------------------------------------------------

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
  error?: JsonRpcError;
}

interface JsonRpcError {
  code: number;
  message: string;
  data?: unknown;
}

// Standard JSON-RPC error codes
const METHOD_NOT_FOUND = -32601;
const INTERNAL_ERROR = -32603;
const PARSE_ERROR = -32700;

// ---------------------------------------------------------------------------
// Dispatcher
// ---------------------------------------------------------------------------

/** Maps JSON-RPC method names to GeoffPlugin handler names. */
const METHOD_MAP: Record<string, keyof GeoffPlugin> = {
  on_init: "on_init",
  on_build_start: "on_build_start",
  on_content_parsed: "on_content_parsed",
  on_graph_updated: "on_graph_updated",
  on_validation_complete: "on_validation_complete",
  on_page_render: "on_page_render",
  on_build_complete: "on_build_complete",
  on_file_changed: "on_file_changed",
};

function makeResponse(id: number | string, result: unknown): JsonRpcResponse {
  return { jsonrpc: "2.0", id, result: result ?? null };
}

function makeError(id: number | string, code: number, message: string, data?: unknown): JsonRpcResponse {
  return { jsonrpc: "2.0", id, error: { code, message, data } };
}

async function dispatch(plugin: GeoffPlugin, request: JsonRpcRequest): Promise<JsonRpcResponse> {
  const handlerKey = METHOD_MAP[request.method];

  if (!handlerKey) {
    return makeError(request.id, METHOD_NOT_FOUND, `Unknown method: ${request.method}`);
  }

  const handler = plugin[handlerKey];
  if (typeof handler !== "function") {
    // Plugin doesn't implement this hook — return null (no-op)
    return makeResponse(request.id, null);
  }

  try {
    // deno-lint-ignore no-explicit-any
    const result = await (handler as any).call(plugin, request.params);
    return makeResponse(request.id, result);
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : String(err);
    const stack = err instanceof Error ? err.stack : undefined;
    return makeError(request.id, INTERNAL_ERROR, message, stack);
  }
}

// ---------------------------------------------------------------------------
// I/O loop
// ---------------------------------------------------------------------------

const encoder = new TextEncoder();
const decoder = new TextDecoder();

function writeLine(line: string): void {
  Deno.stdout.writeSync(encoder.encode(line + "\n"));
}

/**
 * Starts the JSON-RPC listener. Reads from stdin line-by-line,
 * dispatches each request to the plugin, and writes the response to stdout.
 *
 * This function runs until stdin is closed (i.e. the Geoff process exits).
 */
export async function serve(plugin: GeoffPlugin): Promise<void> {
  const buf = new Uint8Array(65536);
  let leftover = "";

  while (true) {
    const n = await Deno.stdin.read(buf);
    if (n === null) break; // EOF — Geoff closed the pipe

    const chunk = leftover + decoder.decode(buf.subarray(0, n));
    const lines = chunk.split("\n");

    // Last element may be an incomplete line — save it for next iteration
    leftover = lines.pop() ?? "";

    for (const line of lines) {
      const trimmed = line.trim();
      if (trimmed === "") continue;

      let request: JsonRpcRequest;
      try {
        request = JSON.parse(trimmed) as JsonRpcRequest;
      } catch {
        writeLine(JSON.stringify(makeError(0, PARSE_ERROR, "Invalid JSON")));
        continue;
      }

      const response = await dispatch(plugin, request);
      writeLine(JSON.stringify(response));
    }
  }
}
