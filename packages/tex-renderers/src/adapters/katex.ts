import katex from "katex";
import type { CompileOptions, CompileResult, TexCompiler } from "../types.js";

export function createKaTeXCompiler(): TexCompiler {
  return {
    name: "katex",
    async compile(tex, options) {
      const wrapped = options.mode === "text" ? `\\text{${tex}}` : tex;
      try {
        const html = katex.renderToString(wrapped, {
          throwOnError: true,
          strict: false,
          displayMode: options.display ?? false,
        });
        return { success: true, output: html };
      } catch (e: any) {
        return { success: false, error: e.message };
      }
    },
  };
}
