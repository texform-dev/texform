import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import type { CaseResult, RecordTestResult } from "../types.js";

type RendererName = "mathjax" | "katex" | "xetex";

interface RendererSignature {
  pass: boolean;
  error?: {
    message: string;
    category: string;
  };
}

interface CaseSignature {
  branch: string;
  positive: boolean;
  tex: string;
  expect: CaseResult["expect"];
  renderers: Partial<Record<RendererName, RendererSignature>>;
}

interface RecordSignature {
  package: string;
  name: string;
  type: RecordTestResult["type"];
  argspec: string;
  cases: CaseSignature[];
}

export function summaryNeedsRefresh(
  outDir: string,
  probeResults: RecordTestResult[],
  probeRenderers: RendererName[],
): boolean {
  const resultsDir = join(outDir, "results");
  if (!existsSync(resultsDir)) return true;

  const storedResults = readStoredResults(resultsDir);
  if (!storedResults) return true;

  return stableJson(recordSignatures(storedResults, probeRenderers)) !==
    stableJson(recordSignatures(probeResults, probeRenderers));
}

function readStoredResults(resultsDir: string): RecordTestResult[] | undefined {
  try {
    const files = readdirSync(resultsDir)
      .filter((file) => file.endsWith(".jsonl"))
      .sort();
    const records: RecordTestResult[] = [];

    for (const file of files) {
      const content = readFileSync(join(resultsDir, file), "utf-8");
      for (const line of content.split("\n")) {
        if (!line.trim()) continue;
        records.push(JSON.parse(line) as RecordTestResult);
      }
    }

    return records;
  } catch {
    return undefined;
  }
}

function recordSignatures(
  results: RecordTestResult[],
  renderers: RendererName[],
): RecordSignature[] {
  return results
    .map((result) => ({
      package: result.package,
      name: result.name,
      type: result.type,
      argspec: result.argspec,
      cases: result.cases.map((testCase) => caseSignature(testCase, renderers))
        .sort(compareStableJson),
    }))
    .sort(compareRecordSignature);
}

function caseSignature(
  testCase: CaseResult,
  renderers: RendererName[],
): CaseSignature {
  const rendererResults: CaseSignature["renderers"] = {};

  for (const renderer of renderers) {
    rendererResults[renderer] = {
      pass: testCase[renderer],
      error: testCase.errors?.[renderer] ? {
        message: testCase.errors[renderer].message,
        category: testCase.errors[renderer].category,
      } : undefined,
    };
  }

  return {
    branch: testCase.branch,
    positive: testCase.positive,
    tex: testCase.tex,
    expect: testCase.expect,
    renderers: rendererResults,
  };
}

function compareRecordSignature(a: RecordSignature, b: RecordSignature): number {
  return `${a.package}\0${a.type}\0${a.name}\0${a.argspec}`.localeCompare(
    `${b.package}\0${b.type}\0${b.name}\0${b.argspec}`,
  );
}

function compareStableJson(a: unknown, b: unknown): number {
  return stableJson(a).localeCompare(stableJson(b));
}

function stableJson(value: unknown): string {
  if (value === undefined) {
    return "undefined";
  }
  if (Array.isArray(value)) {
    return `[${value.map(stableJson).join(",")}]`;
  }
  if (value && typeof value === "object") {
    const entries = Object.entries(value)
      .filter(([, entryValue]) => entryValue !== undefined)
      .sort(([left], [right]) => left.localeCompare(right));
    return `{${entries
      .map(([key, entryValue]) => `${JSON.stringify(key)}:${stableJson(entryValue)}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
}
