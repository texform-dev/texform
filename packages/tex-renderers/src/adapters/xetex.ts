import type { TexCompiler } from "../types.js";
import { compileSingleXeTeX } from "./xetex-shared.js";

export function createXeTeXCompiler(): TexCompiler {
  return {
    name: "xetex",
    async compile(tex, options) {
      return compileSingleXeTeX({ tex, options }, "xetex-");
    },
  };
}
