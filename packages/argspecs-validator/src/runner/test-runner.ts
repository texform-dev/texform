import type { TexCompiler, CompileOptions } from "@texform/tex-renderers";
import type { TestRecord, TestCase, CaseResult, ErrorLogEntry } from "../types.js";
import { classifyError } from "./result-collector.js";
import { resolveMode } from "./orchestrator.js";

type RendererName = "mathjax" | "katex" | "xetex";

export async function runRecord(
  record: TestRecord,
  cases: TestCase[],
  compilers: TexCompiler[],
): Promise<{ results: CaseResult[]; errors: ErrorLogEntry[] }> {
  const results: CaseResult[] = [];
  const errors: ErrorLogEntry[] = [];

  // Track whether each renderer passed on the baseline case,
  // used by classifyError to detect syntax_divergence on variant branches
  const baselinePasses: Record<RendererName, boolean> = {
    mathjax: false,
    katex: false,
    xetex: false,
  };

  for (const tc of cases) {
    const caseResult: CaseResult = {
      branch: tc.branch,
      positive: tc.positive,
      tex: tc.tex,
      expect: tc.expect,
      mathjax: undefined as any,
      katex: undefined as any,
      xetex: undefined as any,
    };

    // Collect per-renderer error info for this case
    const caseErrors: Partial<Record<RendererName, { message: string; category: ReturnType<typeof classifyError> }>> = {};

    for (const compiler of compilers) {
      const rendererName = compiler.name as RendererName;
      const compileOptions: CompileOptions = {
        packages: [record.package],
        mode: resolveMode(record, rendererName),
      };
      const result = await compiler.compile(tc.tex, compileOptions);
      (caseResult as any)[compiler.name] = result.success;

      if (tc.branch === "baseline") {
        baselinePasses[rendererName] = result.success;
      }

      if (!result.success) {
        const errorMsg = result.error ?? "unknown";
        errors.push({
          package: record.package,
          name: record.name,
          branch: tc.branch,
          renderer: compiler.name,
          tex: tc.tex,
          error: errorMsg,
        });

        caseErrors[rendererName] = {
          message: errorMsg,
          category: classifyError(errorMsg, tc.branch, baselinePasses[rendererName]),
        };
      }
    }

    // Only attach errors when at least one renderer failed
    if (Object.keys(caseErrors).length > 0) {
      caseResult.errors = caseErrors;
    }

    results.push(caseResult);
  }

  return { results, errors };
}
