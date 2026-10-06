export * from "./index.js";

/** Initialize the browser/Worker bindings. Concurrent calls share one attempt; failures allow retry. */
export declare function init(options?: {
  wasm?: WebAssembly.Module | BufferSource | Response | Promise<Response> | URL | string;
}): Promise<void>;
