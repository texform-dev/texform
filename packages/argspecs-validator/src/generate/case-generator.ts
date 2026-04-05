import { parseArgSpec } from "../wasm.js";
import type { TestRecord, TestCase, ParsedSlot } from "../types.js";
import { positivePlaceholder, negativePlaceholder } from "./placeholder.js";
import { wrapSlot, buildCommandTex, buildEnvironmentTex } from "./tex-builder.js";

export function generateCases(record: TestRecord): TestCase[] {
  const slots = parseArgSpec(record.spec);
  if (slots === null) return [];

  const cases: TestCase[] = [];
  const optionalIndices = slots.map((s, i) => ({ s, i })).filter(({ s }) => !s.required);

  // Baseline
  cases.push(makeCase(record, slots, "baseline", new Set(), true));

  // One-hot variations
  for (const { i } of optionalIndices) {
    cases.push(makeCase(record, slots, `vary:${slotLabel(slots[i])}[${i}]`, new Set([i]), true));
  }

  // Maximal
  if (optionalIndices.length > 1) {
    const allOn = new Set(optionalIndices.map(({ i }) => i));
    const maxCase = makeCase(record, slots, "maximal", allOn, true);
    if (!cases.some((c) => c.tex === maxCase.tex)) {
      cases.push(maxCase);
    }
  }

  // Bare token — skip delimiter kind (D params don't accept {}-wrapped form)
  for (let i = 0; i < slots.length; i++) {
    const s = slots[i];
    if (s.required && s.form.type === "standard" && s.kind.type !== "star" && s.kind.type !== "delimiter") {
      cases.push(makeBareCase(record, slots, i));
    }
  }

  // Negative type
  for (let i = 0; i < slots.length; i++) {
    const neg = negativePlaceholder(slots[i]);
    if (neg) cases.push(makeNegativeCase(record, slots, i, neg));
  }

  // Nullable
  for (let i = 0; i < slots.length; i++) {
    if (slots[i].nullable) cases.push(makeNullableCase(record, slots, i));
  }

  return cases;
}

function slotLabel(slot: ParsedSlot): string {
  if (slot.kind.type === "star") return "s";
  if (!slot.required) {
    if (slot.form.type === "standard") return "o";
    if (slot.form.type === "group") return "g";
    return "d";
  }
  return "m";
}

function makeCase(
  record: TestRecord, slots: ParsedSlot[], branch: string,
  activeOptionals: Set<number>, positive: boolean,
): TestCase {
  let letterIdx = 0;
  const filledSlots: string[] = [];
  const bareSlots: string[] = [];

  for (let i = 0; i < slots.length; i++) {
    const s = slots[i];
    if (s.kind.type === "star") {
      if (activeOptionals.has(i)) { filledSlots.push("*"); bareSlots.push("*"); }
      continue;
    }
    if (!s.required && !activeOptionals.has(i)) continue;
    const value = positivePlaceholder(s, letterIdx++);
    filledSlots.push(wrapSlot(value, s.form, s.required, s.kind));
    bareSlots.push(value);
  }

  const tex = record.type === "command"
    ? buildCommandTex(record, filledSlots, bareSlots)
    : buildEnvironmentTex(record, filledSlots);

  return { branch, positive, tex, expect: "pass" };
}

function makeBareCase(record: TestRecord, slots: ParsedSlot[], bareIndex: number): TestCase {
  let letterIdx = 0;
  const filledSlots: string[] = [];

  for (let i = 0; i < slots.length; i++) {
    const s = slots[i];
    if (s.kind.type === "star") continue;
    if (!s.required) continue;
    const value = positivePlaceholder(s, letterIdx++);
    filledSlots.push(i === bareIndex ? value : wrapSlot(value, s.form, s.required, s.kind));
  }

  const tex = record.type === "command"
    ? buildCommandTex(record, filledSlots)
    : buildEnvironmentTex(record, filledSlots);

  return { branch: `bare[${bareIndex}]`, positive: true, tex, expect: "pass" };
}

function makeNegativeCase(
  record: TestRecord, slots: ParsedSlot[], negIndex: number,
  neg: { content: string; expect: TestCase["expect"] },
): TestCase {
  let letterIdx = 0;
  const filledSlots: string[] = [];

  for (let i = 0; i < slots.length; i++) {
    const s = slots[i];
    if (s.kind.type === "star") continue;
    // Skip optional slots EXCEPT the one being negated
    if (!s.required && i !== negIndex) continue;
    if (i === negIndex) {
      filledSlots.push(wrapSlot(neg.content, s.form, s.required, s.kind));
    } else {
      filledSlots.push(wrapSlot(positivePlaceholder(s, letterIdx++), s.form, s.required, s.kind));
    }
  }

  const kindLabel = slots[negIndex].kind.mode === "text" ? "T"
    : slots[negIndex].kind.type === "delimiter" ? "D"
    : slots[negIndex].kind.type === "dimension" ? "L"
    : slots[negIndex].kind.type === "integer" ? "I"
    : slots[negIndex].kind.type === "csname" ? "N"
    : slots[negIndex].kind.type[0].toUpperCase();

  const tex = record.type === "command"
    ? buildCommandTex(record, filledSlots)
    : buildEnvironmentTex(record, filledSlots);

  return { branch: `neg:${kindLabel}[${negIndex}]`, positive: false, tex, expect: neg.expect };
}

function makeNullableCase(record: TestRecord, slots: ParsedSlot[], nullIndex: number): TestCase {
  let letterIdx = 0;
  const filledSlots: string[] = [];

  for (let i = 0; i < slots.length; i++) {
    const s = slots[i];
    if (s.kind.type === "star") continue;
    if (!s.required) continue;
    filledSlots.push(i === nullIndex ? "{}" : wrapSlot(positivePlaceholder(s, letterIdx++), s.form, s.required, s.kind));
  }

  const tex = record.type === "command"
    ? buildCommandTex(record, filledSlots)
    : buildEnvironmentTex(record, filledSlots);

  return { branch: `nullable[${nullIndex}]`, positive: true, tex, expect: "pass" };
}
