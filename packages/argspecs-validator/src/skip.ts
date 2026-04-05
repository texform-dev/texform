import { readFileSync, existsSync } from "node:fs";
import type { TestRecord } from "./types.js";

interface SkipEntry { package: string; name: string; reason: string; }

const AUTO_SKIP_TAGS = ["newdefine", "verbatim"];

// Commands that require specific context (e.g. inside an environment or after
// a particular command) and cannot be tested in isolation.
const CONTEXT_DEPENDENT = new Set([
  "left", "right", "middle",           // must be paired
  "limits", "nolimits",                // must follow an operator
  "cr", "hline", "hdashline",          // only inside tabular/array
  "shoveleft", "shoveright",           // only inside multline
  "leftroot", "uproot",                // only inside \root
  "breakAlign",                        // only inside alignment environments
  "buildrel",                          // special infix: \buildrel X \over Y
  "root",                              // special infix: \root X \of Y
  "mmlToken",                          // MathJax-internal command
  "hfil", "hfill", "hfilll",           // horizontal fill, only in tabular/alignment
]);

export function loadSkipList(path: string): Map<string, string> {
  const map = new Map<string, string>();
  if (!existsSync(path)) return map;
  const lines = readFileSync(path, "utf-8").split("\n").filter(Boolean);
  for (const line of lines) {
    const entry = JSON.parse(line) as SkipEntry;
    map.set(`${entry.package}/${entry.name}`, entry.reason);
  }
  return map;
}

export function shouldSkip(record: TestRecord, manualSkips: Map<string, string>): string | undefined {
  for (const tag of AUTO_SKIP_TAGS) {
    if (record.tags.includes(tag)) return `auto: ${tag}`;
  }
  if (CONTEXT_DEPENDENT.has(record.name)) return "auto: context-dependent";
  return manualSkips.get(`${record.package}/${record.name}`);
}
