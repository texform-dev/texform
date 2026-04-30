import { describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { summaryNeedsRefresh } from "./summary-check.js";
import type { RecordTestResult, TestSummary } from "../types.js";

type RendererName = "mathjax" | "katex" | "xetex";

const RENDERERS: RendererName[] = ["mathjax", "katex"];

function baseResult(overrides: Partial<RecordTestResult> = {}): RecordTestResult {
  return {
    package: "base",
    name: "frac",
    type: "command",
    argspec: "m m",
    support: {
      mathjax: "full",
      katex: "full",
      xetex: "none",
    },
    cases: [
      {
        branch: "baseline",
        positive: true,
        tex: "\\frac{a}{b}",
        expect: "pass",
        mathjax: true,
        katex: true,
        xetex: false,
      },
      {
        branch: "bare[0]",
        positive: false,
        tex: "\\frac",
        expect: "fail",
        mathjax: false,
        katex: false,
        xetex: false,
        errors: {
          mathjax: {
            message: "Missing argument",
            category: "semantic_error",
          },
          katex: {
            message: "Missing argument",
            category: "semantic_error",
          },
        },
      },
    ],
    ...overrides,
  };
}

function writeStoredResults(outDir: string, results: RecordTestResult[]) {
  const summary: TestSummary = {
    total_records: results.length,
    total_cases: results.reduce((sum, result) => sum + result.cases.length, 0),
    by_renderer: {
      mathjax: { full: 1, partial: 0, none: 0 },
      katex: { full: 1, partial: 0, none: 0 },
      xetex: { full: 0, partial: 0, none: 1 },
    },
    by_package: {
      base: {
        records: 1,
        mathjax: { full: 1, partial: 0, none: 0 },
        katex: { full: 1, partial: 0, none: 0 },
        xetex: { full: 0, partial: 0, none: 1 },
      },
    },
  };

  writeFileSync(
    join(outDir, "summary.json"),
    `${JSON.stringify(summary, null, 2)}\n`,
  );
  writeFileSync(
    join(outDir, "results", "base.jsonl"),
    `${results.map((result) => JSON.stringify(result)).join("\n")}\n`,
  );
}

function withStoredResults<T>(
  storedResults: RecordTestResult[],
  run: (outDir: string) => T,
): T {
  const outDir = mkdtempSync(join(tmpdir(), "argspec-summary-check-"));
  try {
    mkdirSync(join(outDir, "results"), { recursive: true });
    writeStoredResults(outDir, storedResults);
    return run(outDir);
  } finally {
    rmSync(outDir, { recursive: true, force: true });
  }
}

describe("summaryNeedsRefresh", () => {
  test("returns false when probe results match stored results", () => {
    withStoredResults([baseResult()], (outDir) => {
      expect(summaryNeedsRefresh(outDir, [baseResult()], RENDERERS)).toBe(false);
    });
  });

  test("returns true when case text changes but summary counts stay the same", () => {
    const changed = baseResult({
      cases: [
        baseResult().cases[0],
        {
          ...baseResult().cases[1],
          tex: "\\frac{x}",
        },
      ],
    });

    withStoredResults([baseResult()], (outDir) => {
      expect(summaryNeedsRefresh(outDir, [changed], RENDERERS)).toBe(true);
    });
  });

  test("returns true when renderer result changes but summary counts stay the same", () => {
    const changed = baseResult({
      cases: [
        baseResult().cases[0],
        {
          ...baseResult().cases[1],
          katex: true,
          errors: {
            mathjax: {
              message: "Missing argument",
              category: "semantic_error",
            },
          },
        },
      ],
    });

    withStoredResults([baseResult()], (outDir) => {
      expect(summaryNeedsRefresh(outDir, [changed], RENDERERS)).toBe(true);
    });
  });
});
