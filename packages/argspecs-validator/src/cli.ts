import { resolve, join } from "node:path";
import { mkdirSync, writeFileSync } from "node:fs";
import { parseArgs } from "node:util";
import {
  createMathJaxCompiler, createKaTeXCompiler, createXeTeXCompiler,
  createXeTeXBatchCompiler,
  type TexCompiler,
} from "@texform/tex-renderers";
import { loadSpecs } from "./loader.js";
import { loadCustomTests, customCaseToTestCase } from "./custom-tests.js";
import { generateCases } from "./generate/case-generator.js";
import { runRecord } from "./runner/test-runner.js";
import { runAll, type RecordWork } from "./runner/orchestrator.js";
import { buildRecordResult, buildSummary } from "./runner/result-collector.js";
import { loadResultCache, filterCasesForRun } from "./runner/result-cache.js";
import type { RecordTestResult, ErrorLogEntry, TestRecord, CaseResult } from "./types.js";

type RendererName = "mathjax" | "katex" | "xetex";

const repoRoot = resolve(import.meta.dir, "../../../");

const { values: args } = parseArgs({
  options: {
    package: { type: "string" },
    renderer: { type: "string" },
    name: { type: "string" },
    record: { type: "string" },
    force: { type: "boolean", default: false },
    "dry-run": { type: "boolean", default: false },
    "xetex-batch-size": { type: "string", default: "5" },
    "xetex-concurrency": { type: "string", default: "16" },
    "out-dir": { type: "string", default: join(repoRoot, "out/spec-tests") },
  },
});

function resolveRenderers(): RendererName[] {
  if (args.renderer) return [args.renderer as RendererName];
  const renderers: RendererName[] = ["mathjax", "katex"];
  if (Bun.which("xelatex")) {
    renderers.push("xetex");
  } else {
    console.warn("Warning: xelatex not found in PATH, skipping XeTeX renderer");
  }
  return renderers;
}

function loadCasesForRecord(record: TestRecord, customMap: Map<string, any>) {
  const key = `${record.package}/${record.type}/${record.name}`;
  const custom = customMap.get(key);
  const ofatCases = (!custom || !custom.skip_generated) ? generateCases(record) : [];
  const customCases = custom?.cases.map(customCaseToTestCase) ?? [];
  return [...ofatCases, ...customCases];
}

// --record mode: single record, output to stdout
async function runSingleRecord(activeRenderers: RendererName[]) {
  const record: TestRecord = JSON.parse(args.record!);
  const customTestDir = resolve(import.meta.dir, "../custom-tests");
  const customMap = loadCustomTests(customTestDir);
  const cases = loadCasesForRecord(record, customMap);

  if (args["dry-run"]) {
    for (const c of cases) console.log(`${c.branch}: ${c.tex}`);
    return;
  }

  const compilers: TexCompiler[] = [];
  for (const r of activeRenderers) {
    if (r === "mathjax") compilers.push(createMathJaxCompiler());
    else if (r === "katex") compilers.push(createKaTeXCompiler());
    else if (r === "xetex") compilers.push(createXeTeXCompiler());
  }

  const { results } = await runRecord(record, cases, compilers);
  console.log(JSON.stringify(buildRecordResult(record, results)));
  for (const c of compilers) await c.dispose?.();
}

// Normal batch mode
async function runBatchMode(activeRenderers: RendererName[]) {
  const specsDir = join(repoRoot, "resources/specs");
  const customTestDir = resolve(import.meta.dir, "../custom-tests");
  const outDir = args["out-dir"]!;

  let records = loadSpecs(specsDir);
  const customMap = loadCustomTests(customTestDir);

  if (args.package) records = records.filter((r) => r.package === args.package);
  if (args.name) records = records.filter((r) => r.name === args.name);

  console.log(`Loaded ${records.length} records`);

  const recordCases = records.map((record) => ({
    record,
    cases: loadCasesForRecord(record, customMap),
  }));

  const totalCases = recordCases.reduce((sum, rc) => sum + rc.cases.length, 0);
  console.log(`Generated ${totalCases} test cases`);

  if (args["dry-run"]) {
    for (const { record, cases } of recordCases) {
      if (cases.length > 0) {
        console.log(`\n${record.package}/${record.name} (${cases.length} cases):`);
        for (const c of cases) console.log(`  ${c.branch}: ${c.tex}`);
      }
    }
    return;
  }

  // Load cache
  const cache = args.force ? new Map<string, CaseResult>() : loadResultCache(outDir);
  const resultMap = new Map<number, RecordTestResult>();
  const works: Array<RecordWork & { idx: number; allCases: typeof recordCases[0]["cases"] }> = [];
  let cachedCount = 0;

  for (let idx = 0; idx < recordCases.length; idx++) {
    const { record, cases } = recordCases[idx];
    if (cases.length === 0) {
      resultMap.set(idx, buildRecordResult(record, []));
      continue;
    }

    const { toRun, fromCache } = filterCasesForRun(
      record.package, record.name, cases, cache, activeRenderers, !!args.force,
    );

    if (toRun.length === 0) {
      const cachedResults = cases.map((tc) => fromCache.get(tc.tex)!);
      resultMap.set(idx, buildRecordResult(record, cachedResults));
      cachedCount += cases.length;
      continue;
    }

    const caseResults: CaseResult[] = toRun.map((tc) => ({
      branch: tc.branch,
      positive: tc.positive,
      tex: tc.tex,
      expect: tc.expect,
      mathjax: undefined as any,
      katex: undefined as any,
      xetex: undefined as any,
    }));

    works.push({ record, cases: toRun, caseResults, idx, allCases: cases });
  }

  if (cachedCount > 0) console.log(`Skipped ${cachedCount} cached cases`);

  let allErrors: ErrorLogEntry[] = [];

  if (works.length > 0) {
    const compilers = new Map<RendererName, TexCompiler>();
    if (activeRenderers.includes("mathjax")) compilers.set("mathjax", createMathJaxCompiler());
    if (activeRenderers.includes("katex")) compilers.set("katex", createKaTeXCompiler());
    if (activeRenderers.includes("xetex") && args.renderer === "xetex") {
      compilers.set("xetex", createXeTeXCompiler());
    }

    const useXetexBatch = activeRenderers.includes("xetex") && !args.renderer;
    const batchCompiler = useXetexBatch
      ? createXeTeXBatchCompiler({
          batchSize: parseInt(args["xetex-batch-size"]!, 10),
          concurrency: parseInt(args["xetex-concurrency"]!, 10),
        })
      : undefined;

    allErrors = await runAll(
      works.map((w) => ({ record: w.record, cases: w.cases, caseResults: w.caseResults })),
      {
        renderers: activeRenderers,
        compilers,
        xetexBatchCompiler: batchCompiler,
        onProgress: (msg) => console.log(msg),
      },
    );

    // Assemble final results for records that were run
    for (const work of works) {
      const { fromCache } = filterCasesForRun(
        work.record.package, work.record.name, work.allCases,
        cache, activeRenderers, false,
      );

      let runIdx = 0;
      const mergedResults = work.allCases.map((tc) => {
        if (fromCache.has(tc.tex)) return fromCache.get(tc.tex)!;
        return work.caseResults[runIdx++];
      });

      resultMap.set(work.idx, buildRecordResult(work.record, mergedResults));
    }

    for (const c of compilers.values()) await c.dispose?.();
    if (batchCompiler) await batchCompiler.dispose();
  }

  // Assemble in original order
  const allResults = recordCases.map((_, idx) => resultMap.get(idx)!);

  // Write output
  mkdirSync(join(outDir, "results"), { recursive: true });
  mkdirSync(join(outDir, "errors"), { recursive: true });

  const byPkg = new Map<string, RecordTestResult[]>();
  const errByPkg = new Map<string, ErrorLogEntry[]>();
  for (const r of allResults) {
    if (!byPkg.has(r.package)) byPkg.set(r.package, []);
    byPkg.get(r.package)!.push(r);
  }
  for (const e of allErrors) {
    if (!errByPkg.has(e.package)) errByPkg.set(e.package, []);
    errByPkg.get(e.package)!.push(e);
  }

  for (const [pkg, results] of byPkg) {
    writeFileSync(join(outDir, "results", `${pkg}.jsonl`),
      results.map((r) => JSON.stringify(r)).join("\n") + "\n");
  }
  for (const [pkg, errors] of errByPkg) {
    writeFileSync(join(outDir, "errors", `${pkg}.jsonl`),
      errors.map((e) => JSON.stringify(e)).join("\n") + "\n");
  }

  const summary = buildSummary(allResults);
  writeFileSync(join(outDir, "summary.json"), JSON.stringify(summary, null, 2) + "\n");

  console.log(`\n=== Results ===`);
  console.log(`Records: ${summary.total_records}`);
  console.log(`Cases: ${summary.total_cases}`);
  for (const [r, counts] of Object.entries(summary.by_renderer))
    console.log(`  ${r}: full=${counts.full} partial=${counts.partial} none=${counts.none}`);
  console.log(`\nOutput: ${outDir}`);
}

async function main() {
  const activeRenderers = resolveRenderers();
  if (args.record) await runSingleRecord(activeRenderers);
  else await runBatchMode(activeRenderers);
}

main().catch((e) => { console.error(e); process.exit(1); });
