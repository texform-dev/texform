import type { TestRecord } from "../types.js";

export function wrapSlot(
  value: string,
  form: { type: string; open?: string; close?: string; pairs?: any[] },
  required: boolean,
): string {
  switch (form.type) {
    case "star": return value;
    case "standard": return required ? `{${value}}` : `[${value}]`;
    case "group": return `{${value}}`;
    case "delimited": return `${form.open ?? "("}${value}${form.close ?? ")"}`;
    case "paired": {
      const pair = form.pairs?.[0];
      return `${pair?.open ?? "("}${value}${pair?.close ?? ")"}`;
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

function environmentBody(record: TestRecord): string {
  if (record.tags.includes("matrix")) return "a & b \\\\\\\\ c & d";
  if (record.tags.includes("math-alignment")) return "a & b";
  return "a";
}

export function environmentBodyForColumns(): string {
  return "a & b";
}
