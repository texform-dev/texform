import { readFileSync, readdirSync } from "node:fs";
import { join, basename } from "node:path";
import yaml from "js-yaml";
import type { TestRecord, PackageSpec } from "./types.js";

export function loadSpecs(specsDir: string): TestRecord[] {
  const files = readdirSync(specsDir).filter((f) => f.endsWith(".yaml"));
  const records: TestRecord[] = [];

  for (const file of files) {
    const pkg = basename(file, ".yaml");
    const content = readFileSync(join(specsDir, file), "utf-8");
    const spec = yaml.load(content) as PackageSpec;

    for (const cmd of spec.commands ?? []) {
      records.push({
        package: pkg, name: cmd.name, type: "command",
        argspec: cmd.argspec, kind: cmd.kind,
        allowed_mode: cmd.allowed_mode, tags: cmd.tags ?? [],
      });
    }

    for (const env of spec.environments ?? []) {
      records.push({
        package: pkg, name: env.name, type: "environment",
        argspec: env.argspec, body_mode: env.body_mode,
        allowed_mode: env.allowed_mode, tags: env.tags ?? [],
      });
    }
  }
  return records;
}
