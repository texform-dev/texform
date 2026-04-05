import { resolve, join } from "node:path";
import { mkdirSync, writeFileSync } from "node:fs";
import { parseArgs } from "node:util";
import {
  createMathJaxCompiler, createKaTeXCompiler, createXeTeXCompiler,
  createXeTeXBatchCompiler,
  type TexCompiler,
  type BatchItem,
} from "@texform/tex-renderers";
import { loadSpecs } from "./loader.js";
import { loadCustomTests, customCaseToTestCase } from "./custom-tests.js";
import { generateCases } from "./generate/case-generator.js";
import { runRecord } from "./runner/test-runner.js";
import { buildRecordResult, buildSummary } from "./runner/result-collector.js";
import type { RecordTestResult, ErrorLogEntry, TestRecord } from "./types.js";

const repoRoot = resolve(import.meta.dir, "../../../");

const { values: args } = parseArgs({
  options: {
    package: { type: "string" },
    renderer: { type: "string" },
    name: { type: "string" },
    "dry-run": { type: "boolean", default: false },
    xetex: { type: "boolean", default: false },
    "xetex-batch-size": { type: "string", default: "5" },
    "xetex-concurrency": { type: "string", default: "16" },
    "out-dir": { type: "string", default: join(repoRoot, "out/spec-tests") },
  },
});

// Determine the XeTeX mode to use for a given record (mirrors logic in test-runner.ts).
function xetexMode(record: TestRecord): "math" | "text" {
  if (record.allowed_mode === "text") return "text";
  if (
    record.type === "environment" &&
    record.allowed_mode === "math" &&
    !record.tags.includes("nestable")
  ) {
    return "text";
  }
  return "math";
}

async function main() {
  const specsDir = join(repoRoot, "resources/specs");
  const customTestDir = resolve(import.meta.dir, "../custom-tests");
  const outDir = args["out-dir"]!;

  let records = loadSpecs(specsDir);
  const customMap = loadCustomTests(customTestDir);

  if (args.package) records = records.filter((r) => r.package === args.package);
  if (args.name) records = records.filter((r) => r.name === args.name);

  console.log(`Loaded ${records.length} records`);

  const allResults: RecordTestResult[] = [];
  const allErrors: ErrorLogEntry[] = [];
  let totalCases = 0;

  const recordCases = records.map((record) => {
    const key = `${record.package}/${record.type}/${record.name}`;
    const custom = customMap.get(key);
    const ofatCases = (!custom || !custom.skip_ofat) ? generateCases(record) : [];
    const customCases = custom?.cases.map(customCaseToTestCase) ?? [];
    const cases = [...ofatCases, ...customCases];
    totalCases += cases.length;
    return { record, cases };
  });

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

  // Batch mode: --xetex without --renderer routes XeTeX through the batch compiler.
  // --renderer xetex (or --renderer with any other value) bypasses batch mode and
  // uses the single-compiler path directly via runRecord().
  const useXetexBatch = args.xetex && !args.renderer;

  const mjCompiler = createMathJaxCompiler();
  const ktCompiler = createKaTeXCompiler();
  const baseCompilers: TexCompiler[] = [mjCompiler, ktCompiler];

  // Compilers used inside the main runRecord loop (XeTeX excluded when batch mode is on).
  let compilers: TexCompiler[];
  if (args.renderer) {
    // --renderer: pick exactly one compiler; may include xetex via single-compile path.
    const pool: TexCompiler[] = [...baseCompilers, createXeTeXCompiler()];
    compilers = pool.filter((c) => c.name === args.renderer);
  } else if (useXetexBatch) {
    // batch mode: MathJax + KaTeX only; XeTeX handled after the loop.
    compilers = baseCompilers;
  } else {
    compilers = baseCompilers;
  }

  let completed = 0;
  for (const { record, cases } of recordCases) {
    if (cases.length === 0) {
      allResults.push(buildRecordResult(record, []));
      completed++;
      continue;
    }
    const { results, errors } = await runRecord(record, cases, compilers);
    allResults.push(buildRecordResult(record, results));
    allErrors.push(...errors);
    completed++;
    if (completed % 50 === 0) console.log(`  ${completed}/${records.length} records...`);
  }

  // XeTeX batch pass: collect all items, compile in parallel batches, merge results.
  if (useXetexBatch) {
    console.log("Running XeTeX batch compilation...");
    const xetexStart = Date.now();

    const batchSize = parseInt(args["xetex-batch-size"]!, 10);
    const concurrency = parseInt(args["xetex-concurrency"]!, 10);
    const batchCompiler = createXeTeXBatchCompiler({ batchSize, concurrency });

    // Build a flat list of (item, pointer back to CaseResult) to fill in.
    const flatItems: Array<{
      batchItem: BatchItem;
      recordIdx: number;  // index into allResults
      caseIdx: number;    // index into allResults[recordIdx].cases
    }> = [];

    for (let ri = 0; ri < allResults.length; ri++) {
      const rr = allResults[ri];
      if (rr.cases.length === 0) continue;

      // Find the original record for mode calculation.
      const record = recordCases[ri].record;
      const mode = xetexMode(record);

      for (let ci = 0; ci < rr.cases.length; ci++) {
        const c = rr.cases[ci];
        flatItems.push({
          batchItem: {
            tex: c.tex,
            options: { packages: [record.package], mode },
          },
          recordIdx: ri,
          caseIdx: ci,
        });
      }
    }

    const xetexResults = await batchCompiler.compileBatch(flatItems.map((f) => f.batchItem));

    // Merge XeTeX results back into allResults and collect errors.
    for (let i = 0; i < flatItems.length; i++) {
      const { recordIdx, caseIdx } = flatItems[i];
      const caseResult = allResults[recordIdx].cases[caseIdx];
      const res = xetexResults[i];
      (caseResult as any).xetex = res.success;

      if (!res.success) {
        const record = recordCases[recordIdx].record;
        allErrors.push({
          package: record.package,
          name: record.name,
          branch: caseResult.branch,
          renderer: "xetex",
          tex: caseResult.tex,
          error: res.error ?? "unknown",
        });
      }
    }

    // Recompute support levels now that xetex is populated.
    for (let ri = 0; ri < allResults.length; ri++) {
      const rr = allResults[ri];
      const record = recordCases[ri].record;
      allResults[ri] = buildRecordResult(record, rr.cases);
    }

    await batchCompiler.dispose();
    const elapsed = ((Date.now() - xetexStart) / 1000).toFixed(1);
    console.log(`XeTeX batch done in ${elapsed}s (${flatItems.length} cases, batch=${batchSize}, concurrency=${concurrency})`);
  }

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

  for (const c of compilers) await c.dispose?.();
}

main().catch((e) => { console.error(e); process.exit(1); });
