import type { TestRecord, CaseResult, RecordTestResult, TestSummary } from "../types.js";

type SupportLevel = "full" | "partial" | "none";
type RendererName = "mathjax" | "katex" | "xetex";
const RENDERERS: RendererName[] = ["mathjax", "katex", "xetex"];

function computeSupport(cases: CaseResult[]): Record<RendererName, SupportLevel> {
  const result = {} as Record<RendererName, SupportLevel>;

  for (const r of RENDERERS) {
    const positive = cases.filter((c) => c.positive);
    if (positive.length === 0) { result[r] = "none"; continue; }
    const ran = positive.filter((c) => (c as any)[r] !== undefined);
    if (ran.length === 0) { result[r] = "none"; continue; }
    const passed = ran.filter((c) => (c as any)[r] === true).length;
    if (passed === ran.length) result[r] = "full";
    else if (passed === 0) result[r] = "none";
    else result[r] = "partial";
  }

  return result;
}

export function buildRecordResult(
  record: TestRecord, cases: CaseResult[], skip?: string,
): RecordTestResult {
  return {
    package: record.package, name: record.name,
    type: record.type, spec: record.spec,
    ...(skip ? { skip } : {}),
    support: skip ? { mathjax: "none", katex: "none", xetex: "none" } : computeSupport(cases),
    cases: skip ? [] : cases,
  };
}

export function buildSummary(results: RecordTestResult[]): TestSummary {
  const active = results.filter((r) => !r.skip);
  const skipped = results.filter((r) => r.skip);

  const byRenderer: TestSummary["by_renderer"] = {};
  for (const r of RENDERERS) {
    byRenderer[r] = {
      full: active.filter((x) => x.support[r] === "full").length,
      partial: active.filter((x) => x.support[r] === "partial").length,
      none: active.filter((x) => x.support[r] === "none").length,
    };
  }

  const byPackage: TestSummary["by_package"] = {};
  for (const result of results) {
    if (!byPackage[result.package]) {
      byPackage[result.package] = {
        records: 0,
        mathjax: { full: 0, partial: 0, none: 0 },
        katex: { full: 0, partial: 0, none: 0 },
        xetex: { full: 0, partial: 0, none: 0 },
      };
    }
    const pkg = byPackage[result.package];
    pkg.records++;
    if (!result.skip) {
      for (const r of RENDERERS) { pkg[r][result.support[r]]++; }
    }
  }

  return {
    generated_at: new Date().toISOString(),
    total_records: results.length,
    skipped_records: skipped.length,
    total_cases: active.reduce((sum, r) => sum + r.cases.length, 0),
    by_renderer: byRenderer,
    by_package: byPackage,
  };
}
