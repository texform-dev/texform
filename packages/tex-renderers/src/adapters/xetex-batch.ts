import { mkdtemp, rm, writeFile, readFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { PACKAGE_MAP } from "../package-map.js";
import type { CompileOptions, CompileResult } from "../types.js";

export interface BatchItem {
  tex: string;
  options: CompileOptions;
}

export interface XeTeXBatchCompiler {
  compileBatch(items: BatchItem[]): Promise<CompileResult[]>;
  dispose(): Promise<void>;
}

// Stable key for grouping items with identical compile options.
function optionsKey(opts: CompileOptions): string {
  return `${opts.mode}|${opts.display ?? false}|${[...opts.packages].sort().join(",")}`;
}

function buildUsepackages(packages: string[]): string {
  return packages
    .flatMap((p) => PACKAGE_MAP[p]?.xetex ?? [])
    .map((pkg) => `\\usepackage{${pkg}}`)
    .join("\n");
}

// Build a single .tex file containing multiple cases separated by markers.
// \typeout writes to the log so we can locate which case the error follows.
function buildBatchTexFile(
  items: BatchItem[],
  usepackages: string,
): string {
  const bodies = items.map((item, i) => {
    const body = item.options.mode === "math" ? `$${item.tex}$` : item.tex;
    return `\\typeout{===CASE_${i}===}\n${body}`;
  });

  return `\\documentclass{article}
${usepackages}
\\begin{document}
${bodies.join("\n\\clearpage\n")}
\\end{document}
`;
}

// Parse the log to find the index of the first case that caused an error.
// Returns the index, or -1 if we cannot tell (treat entire batch as failed).
function findFailedCaseIndex(log: string, batchSize: number): number {
  const lines = log.split("\n");
  let lastSeenCase = -1;

  for (const line of lines) {
    const m = line.match(/===CASE_(\d+)===/);
    if (m) {
      lastSeenCase = parseInt(m[1], 10);
    }
    // xelatex writes the fatal error line starting with "!"
    if (line.startsWith("!") && lastSeenCase >= 0) {
      return lastSeenCase;
    }
  }

  // If an error exists but we never saw a CASE marker before it, blame case 0.
  if (lines.some((l) => l.startsWith("!"))) return 0;

  return -1;
}

async function extractErrorFromLog(logPath: string, stderr: string): Promise<string> {
  try {
    const log = await readFile(logPath, "utf8");
    const errorLine = log.split("\n").find((l) => l.startsWith("!"));
    if (errorLine) return errorLine;
    return log.slice(0, 300);
  } catch {
    return stderr.slice(0, 200) || "unknown xelatex error";
  }
}

// Compile a single item for exact error recovery after a batch failure.
async function compileSingle(item: BatchItem): Promise<CompileResult> {
  const usepackages = buildUsepackages(item.options.packages);
  const body = item.options.mode === "math" ? `$${item.tex}$` : item.tex;
  const texContent = `\\documentclass{article}
${usepackages}
\\begin{document}
${body}
\\end{document}
`;

  const dir = await mkdtemp(join(tmpdir(), "xetex-single-"));
  const texPath = join(dir, "input.tex");
  const logPath = join(dir, "input.log");

  try {
    await writeFile(texPath, texContent);
    const proc = Bun.spawn(
      [
        "xelatex",
        "-interaction=nonstopmode",
        "-halt-on-error",
        "-no-pdf",
        `-output-directory=${dir}`,
        texPath,
      ],
      { stdout: "pipe", stderr: "pipe" },
    );
    const exitCode = await proc.exited;
    const stderr = await new Response(proc.stderr).text();

    if (exitCode !== 0) {
      const error = await extractErrorFromLog(logPath, stderr);
      return { success: false, error };
    }
    return { success: true };
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
}

// Compile a batch of items in one xelatex invocation.
// On failure, falls back to individual compilation to get exact per-item results.
async function compileBatchGroup(items: BatchItem[]): Promise<CompileResult[]> {
  if (items.length === 0) return [];

  const usepackages = buildUsepackages(items[0].options.packages);
  const dir = await mkdtemp(join(tmpdir(), "xetex-batch-"));
  const texPath = join(dir, "batch.tex");
  const logPath = join(dir, "batch.log");

  try {
    await writeFile(texPath, buildBatchTexFile(items, usepackages));

    const proc = Bun.spawn(
      [
        "xelatex",
        "-interaction=nonstopmode",
        "-halt-on-error",
        "-no-pdf",
        `-output-directory=${dir}`,
        texPath,
      ],
      { stdout: "pipe", stderr: "pipe" },
    );
    const exitCode = await proc.exited;

    if (exitCode === 0) {
      // All cases in the batch compiled successfully.
      return items.map(() => ({ success: true }));
    }

    // Batch failed — parse the log to identify the failing case index.
    let log = "";
    try {
      log = await readFile(logPath, "utf8");
    } catch {
      // Log unreadable; fall back to individual compilation for all items.
    }

    const failedIndex = log
      ? findFailedCaseIndex(log, items.length)
      : -1;

    if (failedIndex < 0) {
      // Cannot determine which case failed; run each individually.
      return Promise.all(items.map(compileSingle));
    }

    // Cases before failedIndex definitely passed.
    // failedIndex and all cases after it need individual re-compilation
    // because we halted early and never executed them.
    const results: CompileResult[] = items
      .slice(0, failedIndex)
      .map(() => ({ success: true }));

    const tail = await Promise.all(items.slice(failedIndex).map(compileSingle));
    results.push(...tail);
    return results;
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
}

export function createXeTeXBatchCompiler(options?: {
  batchSize?: number;
  concurrency?: number;
}): XeTeXBatchCompiler {
  const batchSize = options?.batchSize ?? 5;
  const concurrency = options?.concurrency ?? 16;

  return {
    async compileBatch(items: BatchItem[]): Promise<CompileResult[]> {
      if (items.length === 0) return [];

      // Group by compile options so each batch is homogeneous.
      const groups = new Map<string, { indices: number[]; items: BatchItem[] }>();
      for (let i = 0; i < items.length; i++) {
        const key = optionsKey(items[i].options);
        if (!groups.has(key)) groups.set(key, { indices: [], items: [] });
        const g = groups.get(key)!;
        g.indices.push(i);
        g.items.push(items[i]);
      }

      // Split each group into sub-batches and build work units.
      const workUnits: Array<{ resultIndices: number[]; batchItems: BatchItem[] }> = [];
      for (const { indices, items: groupItems } of groups.values()) {
        for (let start = 0; start < groupItems.length; start += batchSize) {
          workUnits.push({
            resultIndices: indices.slice(start, start + batchSize),
            batchItems: groupItems.slice(start, start + batchSize),
          });
        }
      }

      const results: CompileResult[] = new Array(items.length);

      // Process work units with bounded concurrency using a semaphore pattern.
      let next = 0;
      const workers = Array.from({ length: Math.min(concurrency, workUnits.length) }, async () => {
        while (true) {
          const idx = next++;
          if (idx >= workUnits.length) break;
          const { resultIndices, batchItems } = workUnits[idx];
          const batchResults = await compileBatchGroup(batchItems);
          for (let i = 0; i < resultIndices.length; i++) {
            results[resultIndices[i]] = batchResults[i];
          }
        }
      });

      await Promise.all(workers);
      return results;
    },

    async dispose(): Promise<void> {
      // No persistent resources to clean up.
    },
  };
}
