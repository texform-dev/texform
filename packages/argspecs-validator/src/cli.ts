import { resolve, join } from "node:path";
import { mkdirSync, readdirSync, unlinkSync, writeFileSync } from "node:fs";
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
import { summaryNeedsRefresh } from "./runner/summary-check.js";
import type { RecordTestResult, ErrorLogEntry, TestRecord, CaseResult } from "./types.js";

type RendererName = "mathjax" | "katex" | "xetex";

const repoRoot = resolve(import.meta.dir, "../../../");

const { values: args } = parseArgs({
  options: {
    package: { type: "string" },
    renderer: { type: "string" },
    name: { type: "string" },
    record: { type: "string" },
    check: { type: "boolean", default: false },
    "dry-run": { type: "boolean", default: false },
    "xetex-batch-size": { type: "string", default: "5" },
    "xetex-concurrency": { type: "string", default: "16" },
    "out-dir": { type: "string", default: join(repoRoot, "data/argspec-validate-results") },
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

function loadAllCases(customMap: Map<string, any>) {
  const specsDir = join(repoRoot, "resources/specs");
  let records = loadSpecs(specsDir);
  if (args.package) records = records.filter((r) => r.package === args.package);
  if (args.name) records = records.filter((r) => r.name === args.name);

  const recordCases = records.map((record) => ({
    record,
    cases: loadCasesForRecord(record, customMap),
  }));
  return recordCases;
}

function prepareWorks(recordCases: { record: TestRecord; cases: ReturnType<typeof loadCasesForRecord> }[]) {
  return recordCases.map(({ record, cases }) => ({
    record,
    cases,
    caseResults: cases.map((tc) => ({
      branch: tc.branch,
      positive: tc.positive,
      tex: tc.tex,
      expect: tc.expect,
      mathjax: undefined as any,
      katex: undefined as any,
      xetex: undefined as any,
    } satisfies CaseResult)),
  }));
}

function createCompilers(renderers: RendererName[]) {
  const compilers = new Map<RendererName, TexCompiler>();
  if (renderers.includes("mathjax")) compilers.set("mathjax", createMathJaxCompiler());
  if (renderers.includes("katex")) compilers.set("katex", createKaTeXCompiler());
  if (renderers.includes("xetex") && args.renderer === "xetex") {
    compilers.set("xetex", createXeTeXCompiler());
  }
  return compilers;
}

function createBatchCompilerIfNeeded(renderers: RendererName[]) {
  if (renderers.includes("xetex") && !args.renderer) {
    return createXeTeXBatchCompiler({
      batchSize: parseInt(args["xetex-batch-size"]!, 10),
      concurrency: parseInt(args["xetex-concurrency"]!, 10),
    });
  }
  return undefined;
}

function pruneStaleJsonlFiles(dir: string, keepPackages: Set<string>) {
  for (const file of readdirSync(dir)) {
    if (!file.endsWith(".jsonl")) continue;
    const pkg = file.slice(0, -".jsonl".length);
    if (!keepPackages.has(pkg)) {
      unlinkSync(join(dir, file));
    }
  }
}

function writeResults(outDir: string, allResults: RecordTestResult[], allErrors: ErrorLogEntry[]) {
  const resultsDir = join(outDir, "results");
  const errorsDir = join(outDir, "errors");
  mkdirSync(resultsDir, { recursive: true });
  mkdirSync(errorsDir, { recursive: true });

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

  pruneStaleJsonlFiles(resultsDir, new Set(byPkg.keys()));
  pruneStaleJsonlFiles(errorsDir, new Set(errByPkg.keys()));

  for (const [pkg, results] of byPkg) {
    writeFileSync(join(resultsDir, `${pkg}.jsonl`),
      results.map((r) => JSON.stringify(r)).join("\n") + "\n");
  }
  for (const [pkg, errors] of errByPkg) {
    writeFileSync(join(errorsDir, `${pkg}.jsonl`),
      errors.map((e) => JSON.stringify(e)).join("\n") + "\n");
  }

  const summary = buildSummary(allResults);
  writeFileSync(join(outDir, "summary.json"), JSON.stringify(summary, null, 2) + "\n");
  return summary;
}

function printSummary(summary: ReturnType<typeof buildSummary>, outDir: string) {
  console.log(`\n=== Results ===`);
  console.log(`Records: ${summary.total_records}`);
  console.log(`Cases: ${summary.total_cases}`);
  for (const [r, counts] of Object.entries(summary.by_renderer))
    console.log(`  ${r}: full=${counts.full} partial=${counts.partial} none=${counts.none}`);
  console.log(`\nOutput: ${outDir}`);
}

// Normal batch mode
async function runBatchMode(activeRenderers: RendererName[]) {
  const customTestDir = resolve(import.meta.dir, "../custom-tests");
  const customMap = loadCustomTests(customTestDir);
  const outDir = args["out-dir"]!;

  const recordCases = loadAllCases(customMap);
  const totalCases = recordCases.reduce((sum, rc) => sum + rc.cases.length, 0);
  console.log(`Loaded ${recordCases.length} records, ${totalCases} test cases`);

  if (args["dry-run"]) {
    for (const { record, cases } of recordCases) {
      if (cases.length > 0) {
        console.log(`\n${record.package}/${record.name} (${cases.length} cases):`);
        for (const c of cases) console.log(`  ${c.branch}: ${c.tex}`);
      }
    }
    return;
  }

  const works = prepareWorks(recordCases);
  const compilers = createCompilers(activeRenderers);
  const batchCompiler = createBatchCompilerIfNeeded(activeRenderers);

  const allErrors = await runAll(works, {
    renderers: activeRenderers,
    compilers,
    xetexBatchCompiler: batchCompiler,
    onRendererDone: (r, records, cases, ms) =>
      console.log(`  ${r}: ${records} records, ${cases} cases (${(ms / 1000).toFixed(1)}s)`),
  });

  for (const c of compilers.values()) await c.dispose?.();
  if (batchCompiler) await batchCompiler.dispose();

  const allResults = works.map((w) => buildRecordResult(w.record, w.caseResults));
  const summary = writeResults(outDir, allResults, allErrors);
  printSummary(summary, outDir);
}

// --check mode: probe with mathjax+katex, refresh with xetex if changed
async function runCheckMode() {
  const customTestDir = resolve(import.meta.dir, "../custom-tests");
  const customMap = loadCustomTests(customTestDir);
  const outDir = args["out-dir"]!;

  const recordCases = loadAllCases(customMap);
  const totalCases = recordCases.reduce((sum, rc) => sum + rc.cases.length, 0);
  console.log(`[check] Loaded ${recordCases.length} records, ${totalCases} test cases`);

  // Phase 1: probe with mathjax + katex
  const probeRenderers: RendererName[] = ["mathjax", "katex"];
  const works = prepareWorks(recordCases);
  const probeCompilers = createCompilers(probeRenderers);

  console.log("[check] Phase 1: running mathjax + katex...");
  const probeErrors = await runAll(works, {
    renderers: probeRenderers,
    compilers: probeCompilers,
    onRendererDone: (r, records, cases, ms) =>
      console.log(`  ${r}: ${records} records, ${cases} cases (${(ms / 1000).toFixed(1)}s)`),
  });
  for (const c of probeCompilers.values()) await c.dispose?.();

  const probeResults = works.map((w) => buildRecordResult(w.record, w.caseResults));

  if (!summaryNeedsRefresh(outDir, probeResults, probeRenderers)) {
    console.log("[check] Spec validation up to date");
    return;
  }

  // Phase 2: run xetex on the same caseResults (mathjax/katex already filled)
  console.log("[check] Phase 2: results changed, running xetex...");
  const batchCompiler = createXeTeXBatchCompiler({
    batchSize: parseInt(args["xetex-batch-size"]!, 10),
    concurrency: parseInt(args["xetex-concurrency"]!, 10),
  });

  const xetexErrors = await runAll(works, {
    renderers: ["xetex"],
    compilers: new Map(),
    xetexBatchCompiler: batchCompiler,
    onRendererDone: (r, records, cases, ms) =>
      console.log(`  ${r}: ${records} records, ${cases} cases (${(ms / 1000).toFixed(1)}s)`),
  });
  await batchCompiler.dispose();

  const allErrors = [...probeErrors, ...xetexErrors];
  const allResults = works.map((w) => buildRecordResult(w.record, w.caseResults));
  const summary = writeResults(outDir, allResults, allErrors);
  printSummary(summary, outDir);
  console.log("[check] Spec validation results updated");
}

async function main() {
  if (args.check) {
    await runCheckMode();
  } else if (args.record) {
    const activeRenderers = resolveRenderers();
    await runSingleRecord(activeRenderers);
  } else {
    const activeRenderers = resolveRenderers();
    await runBatchMode(activeRenderers);
  }
}

main().catch((e) => { console.error(e); process.exit(1); });
