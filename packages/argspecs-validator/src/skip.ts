import { readFileSync, existsSync } from "node:fs";
import type { TestRecord } from "./types.js";

interface SkipEntry { package: string; name: string; reason: string; }

const AUTO_SKIP_TAGS = ["newdefine", "verbatim"];

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
  return manualSkips.get(`${record.package}/${record.name}`);
}
