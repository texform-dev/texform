import { readFileSync, readdirSync, existsSync } from "node:fs";
import { join, basename } from "node:path";
import yaml from "js-yaml";
import type { TestCase } from "./types.js";

export interface CustomCaseYaml {
  branch: string;
  tex: string;
  expect: "pass" | "fail" | {
    mathjax: "pass" | "fail";
    katex: "pass" | "fail";
    xetex: "pass" | "fail";
  };
}

export interface CustomTestConfig {
  skip_generated: boolean;
  cases: CustomCaseYaml[];
}

interface CustomTestFileYaml {
  commands?: Record<string, {
    skip_generated?: boolean;
    cases: CustomCaseYaml[];
  }>;
  environments?: Record<string, {
    skip_generated?: boolean;
    cases: CustomCaseYaml[];
  }>;
}

/**
 * Load custom test configs from YAML files in the given directory.
 * Returns a map keyed by "package/type/name" (e.g. "base/command/left").
 */
export function loadCustomTests(dir: string): Map<string, CustomTestConfig> {
  const map = new Map<string, CustomTestConfig>();
  if (!existsSync(dir)) return map;

  const files = readdirSync(dir).filter((f) => f.endsWith(".yaml"));
  for (const file of files) {
    const pkg = basename(file, ".yaml");
    const content = readFileSync(join(dir, file), "utf-8");
    const data = yaml.load(content) as CustomTestFileYaml;

    for (const [name, config] of Object.entries(data.commands ?? {})) {
      map.set(`${pkg}/command/${name}`, {
        skip_generated: config.skip_generated ?? false,
        cases: config.cases,
      });
    }

    for (const [name, config] of Object.entries(data.environments ?? {})) {
      map.set(`${pkg}/environment/${name}`, {
        skip_generated: config.skip_generated ?? false,
        cases: config.cases,
      });
    }
  }

  return map;
}

/** Convert a custom YAML case to a TestCase for the runner. */
export function customCaseToTestCase(c: CustomCaseYaml): TestCase {
  const positive = typeof c.expect === "string" ? c.expect === "pass"
    : Object.values(c.expect).some((v) => v === "pass");
  return { branch: c.branch, positive, tex: c.tex, expect: c.expect };
}
