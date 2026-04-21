import type { TexCompiler, CompileOptions, BatchItem, XeTeXBatchCompiler } from "@texform/tex-renderers";
import type { TestRecord, TestCase, CaseResult, ErrorLogEntry } from "../types.js";
import { classifyError } from "./result-collector.js";

type RendererName = "mathjax" | "katex" | "xetex";

// Non-nestable math environments provide their own math mode in XeTeX,
// so the wrapper must use text mode to avoid double $...$ nesting.
export function resolveMode(
  record: TestRecord,
  renderer: RendererName,
): "math" | "text" {
  if (record.allowed_mode === "text") return "text";
  if (
    renderer === "xetex" &&
    record.type === "environment" &&
    record.allowed_mode === "math" &&
    !record.tags.includes("nestable")
  ) {
    return "text";
  }
  return "math";
}

export interface RecordWork {
  record: TestRecord;
  cases: TestCase[];
  caseResults: CaseResult[];
}

export interface RunAllOptions {
  renderers: RendererName[];
  compilers: Map<RendererName, TexCompiler>;
  xetexBatchCompiler?: XeTeXBatchCompiler;
  onProgress?: (msg: string) => void;
}

export async function runAll(
  works: RecordWork[],
  options: RunAllOptions,
): Promise<ErrorLogEntry[]> {
  const allErrors: ErrorLogEntry[] = [];

  for (const renderer of options.renderers) {
    if (renderer === "xetex" && options.xetexBatchCompiler) {
      allErrors.push(...await runXeTexBatch(works, options.xetexBatchCompiler, options.onProgress));
    } else {
      const compiler = options.compilers.get(renderer);
      if (!compiler) continue;
      allErrors.push(...await runSingleRenderer(works, compiler, renderer, options.onProgress));
    }
  }

  return allErrors;
}

async function runSingleRenderer(
  works: RecordWork[],
  compiler: TexCompiler,
  renderer: RendererName,
  onProgress?: (msg: string) => void,
): Promise<ErrorLogEntry[]> {
  const errors: ErrorLogEntry[] = [];
  let completed = 0;

  for (const { record, cases, caseResults } of works) {
    const mode = resolveMode(record, renderer);
    const opts: CompileOptions = { packages: [record.package], mode };
    let baselinePasses = false;

    for (let i = 0; i < cases.length; i++) {
      const tc = cases[i];
      const result = await compiler.compile(tc.tex, opts);
      (caseResults[i] as any)[renderer] = result.success;

      if (tc.branch === "baseline") baselinePasses = result.success;

      if (!result.success) {
        const errorMsg = result.error ?? "unknown";
        errors.push({
          package: record.package, name: record.name,
          branch: tc.branch, renderer, tex: tc.tex, error: errorMsg,
        });
        if (!caseResults[i].errors) caseResults[i].errors = {};
        caseResults[i].errors![renderer] = {
          message: errorMsg,
          category: classifyError(errorMsg, tc.branch, baselinePasses),
        };
      }
    }

    completed++;
    if (onProgress && completed % 50 === 0) {
      onProgress(`  ${renderer}: ${completed}/${works.length} records...`);
    }
  }

  return errors;
}

async function runXeTexBatch(
  works: RecordWork[],
  batchCompiler: XeTeXBatchCompiler,
  onProgress?: (msg: string) => void,
): Promise<ErrorLogEntry[]> {
  const errors: ErrorLogEntry[] = [];

  const flatItems: Array<{ batchItem: BatchItem; workIdx: number; caseIdx: number }> = [];
  for (let wi = 0; wi < works.length; wi++) {
    const { record, cases } = works[wi];
    const mode = resolveMode(record, "xetex");
    for (let ci = 0; ci < cases.length; ci++) {
      flatItems.push({
        batchItem: { tex: cases[ci].tex, options: { packages: [record.package], mode } },
        workIdx: wi,
        caseIdx: ci,
      });
    }
  }

  if (flatItems.length === 0) return errors;

  onProgress?.("Running XeTeX batch compilation...");
  const start = Date.now();

  const xetexResults = await batchCompiler.compileBatch(flatItems.map((f) => f.batchItem));

  const baselineByWork = new Map<number, boolean>();

  for (let i = 0; i < flatItems.length; i++) {
    const { workIdx, caseIdx } = flatItems[i];
    const work = works[workIdx];
    const cr = work.caseResults[caseIdx];
    const res = xetexResults[i];

    (cr as any).xetex = res.success;

    if (work.cases[caseIdx].branch === "baseline") {
      baselineByWork.set(workIdx, res.success);
    }

    if (!res.success) {
      const errorMsg = res.error ?? "unknown";
      errors.push({
        package: work.record.package, name: work.record.name,
        branch: work.cases[caseIdx].branch, renderer: "xetex",
        tex: cr.tex, error: errorMsg,
      });
      if (!cr.errors) cr.errors = {};
      cr.errors.xetex = {
        message: errorMsg,
        category: classifyError(errorMsg, work.cases[caseIdx].branch, baselineByWork.get(workIdx) ?? false),
      };
    }
  }

  const elapsed = ((Date.now() - start) / 1000).toFixed(1);
  onProgress?.(`XeTeX batch done in ${elapsed}s (${flatItems.length} cases)`);

  return errors;
}
