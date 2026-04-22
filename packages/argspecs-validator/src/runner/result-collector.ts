import type { TestRecord, CaseResult, RecordTestResult, TestSummary, ErrorCategory } from "../types.js";

type SupportLevel = "full" | "partial" | "none";
type RendererName = "mathjax" | "katex" | "xetex";
const RENDERERS: RendererName[] = ["mathjax", "katex", "xetex"];

const UNSUPPORTED_PATTERNS = [
  /Undefined control sequence/,
  /No such environment/,
  /Environment \S+ undefined/,
  /Unknown command/,
];

const SYNTAX_DIVERGENCE_PATTERNS = [
  /Illegal unit of measure/,
  /A <box> was supposed to be here/,
];

/**
 * Classify an error message into one of three categories:
 * - unsupported: the renderer does not recognize the command/environment
 * - syntax_divergence: the renderer has different syntax expectations
 * - semantic_error: any other error
 */
export function classifyError(
  errorMessage: string,
  branch: string,
  baselinePasses: boolean,
): ErrorCategory {
  for (const pattern of UNSUPPORTED_PATTERNS) {
    if (pattern.test(errorMessage)) return "unsupported";
  }
  for (const pattern of SYNTAX_DIVERGENCE_PATTERNS) {
    if (pattern.test(errorMessage)) return "syntax_divergence";
  }
  // Heuristic: bare/vary branch fails on a positive case where baseline passes
  if (
    (branch.startsWith("bare[") || branch.startsWith("vary:")) &&
    baselinePasses
  ) {
    return "syntax_divergence";
  }
  if (/Invalid size/.test(errorMessage) && branch.startsWith("bare[")) {
    return "syntax_divergence";
  }
  return "semantic_error";
}

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
  record: TestRecord, cases: CaseResult[],
): RecordTestResult {
  return {
    package: record.package, name: record.name,
    type: record.type, argspec: record.argspec,
    support: computeSupport(cases),
    cases,
  };
}

export function buildSummary(results: RecordTestResult[]): TestSummary {
  const byRenderer: TestSummary["by_renderer"] = {};
  for (const r of RENDERERS) {
    byRenderer[r] = {
      full: results.filter((x) => x.support[r] === "full").length,
      partial: results.filter((x) => x.support[r] === "partial").length,
      none: results.filter((x) => x.support[r] === "none").length,
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
    for (const r of RENDERERS) { pkg[r][result.support[r]]++; }
  }

  return {
    total_records: results.length,
    total_cases: results.reduce((sum, r) => sum + r.cases.length, 0),
    by_renderer: byRenderer,
    by_package: byPackage,
  };
}
