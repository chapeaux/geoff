/**
 * Geoff Plugin SDK for Deno
 *
 * Write Geoff plugins in TypeScript. Import this module, define your plugin,
 * and call `definePlugin()` to register it.
 *
 * ```ts
 * import { definePlugin } from "./sdk/mod.ts";
 * import type { OutputContext, BuildCompleteResult } from "./sdk/mod.ts";
 *
 * definePlugin({
 *   name: "my-plugin",
 *   async on_build_complete(ctx: OutputContext): Promise<BuildCompleteResult> {
 *     return { addFiles: [{ path: "hello.txt", content: "Hello from my plugin!" }] };
 *   },
 * });
 * ```
 */

// Re-export all types
export type {
  Triple,
  PageInfo,
  OutputFile,
  ValidationResult,
  SiteConfig,
  InitContext,
  BuildContext,
  ContentContext,
  GraphContext,
  ValidationContext,
  RenderContext,
  OutputContext,
  WatchContext,
  GeoffPlugin,
  ContentHookResult,
  GraphHookResult,
  RenderHookResult,
  BuildCompleteResult,
} from "./types.ts";

import type { GeoffPlugin } from "./types.ts";
import { serve } from "./jsonrpc.ts";

/**
 * Registers a Geoff plugin and starts the JSON-RPC listener.
 *
 * Call this once at the top level of your plugin file. It starts reading
 * from stdin and never returns (until Geoff closes the pipe).
 */
export function definePlugin(plugin: GeoffPlugin): void {
  serve(plugin);
}
