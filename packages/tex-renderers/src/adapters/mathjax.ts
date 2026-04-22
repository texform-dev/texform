import MathJax from "mathjax";
import { PACKAGE_MAP } from "../package-map.js";
import type { CompileOptions, CompileResult, TexCompiler } from "../types.js";

let mjInstance: any = null;

const DEFAULT_MATHJAX_PACKAGES = ["base", "ams"];

async function ensureInit(): Promise<any> {
  if (mjInstance) return mjInstance;

  const allPkgs = [...new Set(Object.values(PACKAGE_MAP).flatMap((m) => m.mathjax))];
  const extensions = allPkgs
    .filter((p) => p !== "base")
    .map((p) => `[tex]/${p}`);

  // Preload all known extension code once, but compile with package-scoped TeX
  // documents so individual records don't see unrelated package commands.
  mjInstance = await MathJax.init({
    loader: { load: ["input/tex-base", ...extensions, "output/svg"] },
    tex: { packages: DEFAULT_MATHJAX_PACKAGES },
    svg: { fontCache: "none" },
    startup: { typeset: false },
  });

  return mjInstance;
}

function resolveMathJaxPackages(packageIds: string[]): string[] {
  const packages = new Set<string>(DEFAULT_MATHJAX_PACKAGES);

  for (const packageId of packageIds) {
    const mapping = PACKAGE_MAP[packageId];
    if (!mapping) return [];
    for (const mathjaxPkg of mapping.mathjax) {
      packages.add(mathjaxPkg);
    }
  }

  return [...packages];
}

function createDocument(mj: any, packages: string[]): any {
  const tex = new MathJax._.input.tex_ts.TeX({ packages });
  return MathJax._.mathjax.mathjax.document("", {
    InputJax: tex,
    OutputJax: mj.startup.output,
  });
}

function convertToMml(doc: any, tex: string, display: boolean): string {
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
      const packages = resolveMathJaxPackages(options.packages);
      if (packages.length === 0) {
        return {
          success: false,
          error: `Unknown TeX package: ${options.packages.join(", ")}`,
        };
      }
      const doc = createDocument(mj, packages);
      const wrapped = options.mode === "text" ? `\\text{${tex}}` : tex;
      const mml = convertToMml(doc, wrapped, options.display ?? false);

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
