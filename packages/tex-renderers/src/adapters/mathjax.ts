import MathJax from "mathjax";
import { PACKAGE_MAP } from "../package-map.js";
import type { CompileOptions, CompileResult, TexCompiler } from "../types.js";

let mjInstance: any = null;

async function ensureInit(): Promise<any> {
  if (mjInstance) return mjInstance;

  const allPkgs = Object.values(PACKAGE_MAP).flatMap((m) => m.mathjax);
  const extensions = allPkgs
    .filter((p) => p !== "base")
    .map((p) => `[tex]/${p}`);

  // input/tex-base is used instead of input/tex so that the packages config is
  // respected — input/tex bundles all extensions and ignores the packages list.
  mjInstance = await MathJax.init({
    loader: { load: ["input/tex-base", ...extensions, "output/svg"] },
    tex: { packages: allPkgs },
    svg: { fontCache: "none" },
    startup: { typeset: false },
  });

  return mjInstance;
}

function convertToMml(mj: any, tex: string, display: boolean): string {
  const doc = mj.startup.document;
  const node = doc.convert(tex, {
    display,
    end: MathJax._.core.MathItem.STATE.CONVERT,
  });
  const visitor =
    new MathJax._.core.MmlTree.SerializedMmlVisitor.SerializedMmlVisitor();
  return visitor.visitTree(node);
}

export function createMathJaxCompiler(): TexCompiler {
  return {
    name: "mathjax",
    async compile(tex, options) {
      const mj = await ensureInit();
      const wrapped = options.mode === "text" ? `\\text{${tex}}` : tex;
      const mml = convertToMml(mj, wrapped, options.display ?? false);

      const hasError = mml.includes("<merror");
      if (hasError) {
        const match = mml.match(/<merror[^>]*>([\s\S]*?)<\/merror>/);
        const errorText =
          match?.[1]?.replace(/<[^>]+>/g, "").trim() ?? "unknown error";
        return { success: false, error: errorText, output: mml };
      }
      return { success: true, output: mml };
    },
    async dispose() {
      mjInstance = null;
    },
  };
}
