import type { TestRecord } from "../types.js";

// Extract the string value from a delimiter token ({ type: "char"|"cmd", value: string }).
function delimStr(tok: any, fallback: string): string {
  if (typeof tok === "string") return tok;
  if (tok && typeof tok.value === "string") return tok.value;
  return fallback;
}

export function wrapSlot(
  value: string,
  form: { type: string; open?: any; close?: any; pairs?: any[] },
  required: boolean,
  kind?: { type: string },
): string {
  // Delimiter-kind parameters are always bare tokens (e.g. \left ( not \left{(}).
  if (kind?.type === "delimiter") return value;
  switch (form.type) {
    case "star": return value;
    case "standard": return required ? `{${value}}` : `[${value}]`;
    case "group": return `{${value}}`;
    case "delimited": return `${delimStr(form.open, "(")}${value}${delimStr(form.close, ")")}`;
    case "paired": {
      const pair = form.pairs?.[0];
      return `${delimStr(pair?.open, "(")}${value}${delimStr(pair?.close, ")")}`;
    }
    default: return `{${value}}`;
  }
}

export function buildCommandTex(
  record: TestRecord,
  filledSlots: string[],
  bareSlots?: string[],
): string {
  const cmd = `\\${record.name}`;
  const args = filledSlots.join("");

  switch (record.kind) {
    case "infix": {
      const infixArgs = (bareSlots ?? filledSlots).join(" ");
      return infixArgs ? `a ${cmd} ${infixArgs} b` : `a ${cmd} b`;
    }
    case "declarative":
      return `{${cmd}${args} a}`;
    default: {
      // Add space between command and args if first arg is a bare token
      // (not wrapped in {}, [], or *), otherwise \frac + a → \fraca
      const sep = args && !/^[{[*]/.test(args) ? " " : "";
      return `${cmd}${sep}${args}`;
    }
  }
}

export function buildEnvironmentTex(
  record: TestRecord,
  filledSlots: string[],
): string {
  const args = filledSlots.join("");
  const body = environmentBody(record);
  return `\\begin{${record.name}}${args}\n${body}\n\\end{${record.name}}`;
}

// Environments that use math-alignment tag but don't allow & (single-column).
const NO_AMPERSAND_ENVS = new Set(["multline", "multline*"]);

function environmentBody(record: TestRecord): string {
  if (record.tags.includes("matrix")) return "a & b \\\\\\\\ c & d";
  if (record.tags.includes("math-alignment") && !NO_AMPERSAND_ENVS.has(record.name))
    return "a & b";
  return "a";
}

export function environmentBodyForColumns(): string {
  return "a & b";
}
