// Parsed from YAML specs
export interface CommandEntry {
  name: string;
  kind: "prefix" | "infix" | "declarative";
  argspec: string;
  allowed_mode: "math" | "text" | "both";
  tags: string[];
}

export interface EnvironmentEntry {
  name: string;
  argspec: string;
  body_mode: "math" | "text";
  allowed_mode: "math" | "text" | "both";
  tags: string[];
}

export interface PackageSpec {
  commands: CommandEntry[];
  environments: EnvironmentEntry[];
}

export interface TestRecord {
  package: string;
  name: string;
  type: "command" | "environment";
  argspec: string;
  kind?: "prefix" | "infix" | "declarative";
  body_mode?: "math" | "text";
  allowed_mode: "math" | "text" | "both";
  tags: string[];
}

export interface ParsedSlot {
  required: boolean;
  nullable: boolean;
  no_leading_space: boolean;
  kind: { type: string; mode?: string };
  form: { type: string; open?: any; close?: any; pairs?: any[] };
}

export interface TestCase {
  branch: string;
  positive: boolean;
  tex: string;
  expect: "pass" | "fail" | {
    mathjax: "pass" | "fail";
    katex: "pass" | "fail";
    xetex: "pass" | "fail";
  };
}

export interface RecordTestResult {
  package: string;
  name: string;
  type: "command" | "environment";
  argspec: string;
  support: {
    mathjax: "full" | "partial" | "none";
    katex: "full" | "partial" | "none";
    xetex: "full" | "partial" | "none";
  };
  cases: CaseResult[];
}

export type ErrorCategory = "unsupported" | "syntax_divergence" | "semantic_error";

export interface CaseResult {
  branch: string;
  positive: boolean;
  tex: string;
  expect: "pass" | "fail" | {
    mathjax: "pass" | "fail";
    katex: "pass" | "fail";
    xetex: "pass" | "fail";
  };
  mathjax: boolean;
  katex: boolean;
  xetex: boolean;
  // Only present when at least one renderer fails
  errors?: Partial<Record<"mathjax" | "katex" | "xetex", {
    message: string;
    category: ErrorCategory;
  }>>;
}

export interface ErrorLogEntry {
  package: string;
  name: string;
  branch: string;
  renderer: "mathjax" | "katex" | "xetex";
  tex: string;
  error: string;
}

export interface TestSummary {
  total_records: number;
  total_cases: number;
  by_renderer: Record<string, { full: number; partial: number; none: number }>;
  by_package: Record<string, {
    records: number;
    mathjax: { full: number; partial: number; none: number };
    katex: { full: number; partial: number; none: number };
    xetex: { full: number; partial: number; none: number };
  }>;
}
