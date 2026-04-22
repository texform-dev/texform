import { readFileSync, existsSync } from "node:fs";
import { join } from "node:path";
import type { TestSummary } from "../types.js";

type RendererName = "mathjax" | "katex" | "xetex";

export function summaryNeedsRefresh(
  outDir: string,
  probeSummary: TestSummary,
  probeRenderers: RendererName[],
): boolean {
  const summaryPath = join(outDir, "summary.json");
  if (!existsSync(summaryPath)) return true;

  const stored: TestSummary = JSON.parse(readFileSync(summaryPath, "utf-8"));

  if (stored.total_records !== probeSummary.total_records) return true;
  if (stored.total_cases !== probeSummary.total_cases) return true;

  for (const r of probeRenderers) {
    const s = stored.by_renderer[r];
    const n = probeSummary.by_renderer[r];
    if (!s || !n) return true;
    if (s.full !== n.full || s.partial !== n.partial || s.none !== n.none) return true;
  }

  const storedPkgs = Object.keys(stored.by_package).sort();
  const newPkgs = Object.keys(probeSummary.by_package).sort();
  if (storedPkgs.length !== newPkgs.length) return true;

  for (let i = 0; i < storedPkgs.length; i++) {
    if (storedPkgs[i] !== newPkgs[i]) return true;
    const sp = stored.by_package[storedPkgs[i]];
    const np = probeSummary.by_package[newPkgs[i]];
    if (sp.records !== np.records) return true;
    for (const r of probeRenderers) {
      if (sp[r].full !== np[r].full || sp[r].partial !== np[r].partial || sp[r].none !== np[r].none) return true;
    }
  }

  return false;
}
