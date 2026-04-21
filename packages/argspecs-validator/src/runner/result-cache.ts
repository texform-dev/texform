import { readFileSync, readdirSync, existsSync } from "node:fs";
import { join } from "node:path";
import type { RecordTestResult, CaseResult, TestCase } from "../types.js";

type RendererName = "mathjax" | "katex" | "xetex";

function cacheKey(pkg: string, name: string, tex: string): string {
  return `${pkg}\0${name}\0${tex}`;
}

export function loadResultCache(outDir: string): Map<string, CaseResult> {
  const resultsDir = join(outDir, "results");
  if (!existsSync(resultsDir)) return new Map();

  const cache = new Map<string, CaseResult>();
  const files = readdirSync(resultsDir).filter((f) => f.endsWith(".jsonl"));

  for (const file of files) {
    const content = readFileSync(join(resultsDir, file), "utf-8");
    for (const line of content.split("\n")) {
      if (!line.trim()) continue;
      const rr: RecordTestResult = JSON.parse(line);
      for (const c of rr.cases) {
        cache.set(cacheKey(rr.package, rr.name, c.tex), c);
      }
    }
  }
  return cache;
}

function isCaseComplete(cached: CaseResult, renderers: RendererName[]): boolean {
  return renderers.every((r) => (cached as any)[r] !== undefined);
}

/**
 * Split cases into fully-cached and needs-run.
 * Method B: if a case is missing ANY renderer result, the whole case is re-run.
 */
export function filterCasesForRun(
  pkg: string,
  name: string,
  cases: TestCase[],
  cache: Map<string, CaseResult>,
  renderers: RendererName[],
  force: boolean,
): { toRun: TestCase[]; fromCache: Map<string, CaseResult> } {
  if (force) return { toRun: cases, fromCache: new Map() };

  const toRun: TestCase[] = [];
  const fromCache = new Map<string, CaseResult>();

  for (const tc of cases) {
    const existing = cache.get(cacheKey(pkg, name, tc.tex));
    if (existing && isCaseComplete(existing, renderers)) {
      fromCache.set(tc.tex, existing);
    } else {
      toRun.push(tc);
    }
  }
  return { toRun, fromCache };
}
