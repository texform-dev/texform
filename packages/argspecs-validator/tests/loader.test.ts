import { describe, test, expect } from "bun:test";
import { loadSpecs } from "../src/loader.js";
import { resolve } from "node:path";

const SPECS_DIR = resolve(import.meta.dir, "../../../resources/specs");

describe("loadSpecs", () => {
  test("loads all packages", () => {
    const records = loadSpecs(SPECS_DIR);
    expect(records.length).toBeGreaterThan(100);
    const packages = [...new Set(records.map((r) => r.package))];
    expect(packages).toContain("base");
    expect(packages).toContain("ams");
  });

  test("commands have required fields", () => {
    const records = loadSpecs(SPECS_DIR);
    const cmd = records.find((r) => r.name === "frac")!;
    expect(cmd.type).toBe("command");
    expect(cmd.kind).toBe("prefix");
    expect(cmd.argspec).toBe("m m");
    expect(cmd.allowed_mode).toBe("math");
  });

  test("environments have body_mode", () => {
    const records = loadSpecs(SPECS_DIR);
    const env = records.find((r) => r.name === "align" && r.type === "environment")!;
    expect(env.body_mode).toBe("math");
    expect(env.tags).toContain("math-alignment");
  });
});
