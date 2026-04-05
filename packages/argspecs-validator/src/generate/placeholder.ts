import type { ParsedSlot } from "../types.js";

const LETTERS = "abcdefghijklmnopqrstuvwxyz";

export function positivePlaceholder(slot: ParsedSlot, letterIndex: number): string {
  const letter = LETTERS[letterIndex % 26];
  switch (slot.kind.type) {
    case "content": return letter;
    case "delimiter": return "(";
    case "csname": return letter;
    case "dimension": return "1pt";
    case "integer": return "1";
    case "keyval": return "k=v";
    case "column": return "cc";
    case "star": return "*";
    default: return letter;
  }
}

export function negativePlaceholder(slot: ParsedSlot): {
  content: string;
  expect: "fail" | { mathjax: "pass" | "fail"; katex: "pass" | "fail"; xetex: "pass" | "fail" };
} | null {
  switch (slot.kind.type) {
    case "content":
      if (slot.kind.mode === "text") return { content: "a^2", expect: "fail" };
      return null;
    case "delimiter": return { content: "a", expect: "fail" };
    case "dimension": return { content: "a", expect: "fail" };
    case "integer": return { content: "a", expect: "fail" };
    case "csname":
      return { content: "\\alpha", expect: { mathjax: "pass", katex: "pass", xetex: "fail" } };
    default: return null;
  }
}
