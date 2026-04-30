import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import type { CompileOptions, CompileResult } from "../types.js";
import {
  buildUsepackages,
  compileSingleXeTeX,
  INVALID_CMD_RE,
  spawnXeLaTeX,
} from "./xetex-shared.js";

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

// Scan a successful batch log for per-case "Command invalid" warnings.
function findWarnedCases(log: string): Map<number, string> {
  const result = new Map<number, string>();
  let currentCase = -1;
  for (const line of log.split("\n")) {
    const m = line.match(/===CASE_(\d+)===/);
    if (m) currentCase = parseInt(m[1], 10);
    if (currentCase >= 0 && INVALID_CMD_RE.test(line)) {
      result.set(currentCase, line);
    }
  }
  return result;
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

    const proc = spawnXeLaTeX(texPath, dir);
    const exitCode = await proc.exited;

    if (exitCode === 0) {
      // Check for "invalid command" warnings that don't cause a non-zero exit.
      let log = "";
      try {
        log = await readFile(logPath, "utf8");
      } catch {}

      if (log) {
        const warned = findWarnedCases(log);
        if (warned.size > 0) {
          return items.map((_, i) =>
            warned.has(i)
              ? { success: false, error: warned.get(i)! }
              : { success: true },
          );
        }
      }
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
      return Promise.all(
        items.map((item) =>
          compileSingleXeTeX(item, "xetex-single-", "log-first"),
        ),
      );
    }

    // Cases before failedIndex definitely passed.
    // failedIndex and all cases after it need individual re-compilation
    // because we halted early and never executed them.
    const results: CompileResult[] = items
      .slice(0, failedIndex)
      .map(() => ({ success: true }));

    const tail = await Promise.all(
      items
        .slice(failedIndex)
        .map((item) => compileSingleXeTeX(item, "xetex-single-", "log-first")),
    );
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
