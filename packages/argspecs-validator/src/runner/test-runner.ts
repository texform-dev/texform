import type { TexCompiler, CompileOptions } from "@texform/tex-renderers";
import type { TestRecord, TestCase, CaseResult, ErrorLogEntry } from "../types.js";

export async function runRecord(
  record: TestRecord,
  cases: TestCase[],
  compilers: TexCompiler[],
): Promise<{ results: CaseResult[]; errors: ErrorLogEntry[] }> {
  const results: CaseResult[] = [];
  const errors: ErrorLogEntry[] = [];

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

    for (const compiler of compilers) {
      // Non-nestable math environments need mode="text" for XeTeX
      // (they provide their own math mode, no $...$ wrapping)
      let mode: "math" | "text" = record.allowed_mode === "text" ? "text" : "math";
      if (
        compiler.name === "xetex" &&
        record.type === "environment" &&
        record.allowed_mode === "math" &&
        !record.tags.includes("nestable")
      ) {
        mode = "text";
      }

      const compileOptions: CompileOptions = {
        packages: [record.package],
        mode,
      };
      const result = await compiler.compile(tc.tex, compileOptions);
      (caseResult as any)[compiler.name] = result.success;

      if (!result.success) {
        errors.push({
          package: record.package,
          name: record.name,
          branch: tc.branch,
          renderer: compiler.name,
          tex: tc.tex,
          error: result.error ?? "unknown",
        });
      }
    }

    results.push(caseResult);
  }

  return { results, errors };
}
