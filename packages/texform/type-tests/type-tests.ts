import type {
  Document,
  NormalizeReportResult,
  ParseConfig,
  Parser,
  TransformConfig,
  TransformEngine,
  TransformReport,
} from "../types/index.d.ts";

// @ts-expect-error — Complete is file-local, not part of the public API
import type { Complete } from "../types/index.d.ts";

declare const engine: TransformEngine;
declare const parser: Parser;
declare const src: string;
declare const doc: Document;

const e: boolean = engine.defaultTransformConfig().rewrite.enabled;
const n: number = engine.defaultTransformConfig().rewrite.maxIterations;
const p: number = parser.defaultParseConfig().maxGroupDepth;

const parseDefaults: ReturnType<Parser["defaultParseConfig"]> = parser.defaultParseConfig();
const transformDefaults: ReturnType<TransformEngine["defaultTransformConfig"]> =
  engine.defaultTransformConfig();
const flattenEnabled: boolean = engine.defaultTransformConfig().flattenGroups.enabled;
const flattenSpacing: boolean =
  engine.defaultTransformConfig().flattenGroups.preserveRenderedSpacing;
const completeFlatten: Complete<TransformConfig>["flattenGroups"] =
  engine.defaultTransformConfig().flattenGroups;
const completeFlattenEnabled: boolean = completeFlatten.enabled;
const completeFlattenSpacing: boolean = completeFlatten.preserveRenderedSpacing;

const overrides: TransformConfig = { rewrite: { enabled: false } };
engine.normalize(src, overrides);
engine.transform(doc, overrides);

const parseOverrides: ParseConfig = { rejectUnknown: true };
parser.parse(src, parseOverrides);

const serializeOptions = { scriptSpacing: "compact" as const, scriptOrder: "sup_first" as const };
doc.toLatex(serializeOptions);
doc.toTokenizedLatex({ groupInnerSpacing: "compact" });

// @ts-expect-error — nested math.scripts is no longer a serialize option
doc.toLatex({ math: { scripts: { order: "sup_first" } } });
// @ts-expect-error — snake_case keys are not accepted in JavaScript
doc.toLatex({ script_spacing: "compact" });

// @ts-expect-error — flat rewriteEnabled is not a normalize overlay key
engine.normalize(src, { rewriteEnabled: false });
// @ts-expect-error — enabled must be a boolean
engine.transform(doc, { rewrite: { enabled: "yes" } });
// @ts-expect-error — old FlattenGroups preserve* keys are not on the public interface
engine.transform(doc, { flattenGroups: { preserveEmptyGroup: true } });
engine.transform(doc, { flattenGroups: { enabled: null, preserveRenderedSpacing: null } });
engine.transform(doc, { flattenGroups: null });
engine.normalize(src, { flattenGroups: { enabled: null } });
engine.normalize(src, { flattenGroups: null });

const normalizedText: string = engine.normalize(src, overrides);
const transformed: void = engine.transform(doc, overrides);
const reported: NormalizeReportResult = engine.normalizeWithReport(src, overrides);
const transformReport: TransformReport = engine.transformWithReport(doc, overrides);
const primeMerges: number = reported.report.finalizeAst.primeRunMerges;
const textNormalizations: number = reported.report.finalizeAst.textNormalizations;
const iterations: number = transformReport.rewrite.iterations;
const applied: number = transformReport.rewrite.rules[0].appliedCount;
const viaScripted: number = transformReport.flattenGroups.guardHits.commandContactViaScriptedBase;
const declaratives: number = reported.report.lowerAttributes.attributes[0].consumed.declaratives;

// @ts-expect-error — plain normalize returns a string
engine.normalize(src).normalized;
// @ts-expect-error — plain transform returns void
engine.transform(doc).rewrite;
// @ts-expect-error — iterations live under rewrite
reported.report.iterations;
// @ts-expect-error — guard counters live under guardHits
reported.report.flattenGroups.guards;
// @ts-expect-error — FinalizeAst no longer nests steps
reported.report.finalizeAst.steps;
// @ts-expect-error — TransformResult was removed
type RemovedReportResult = import("../types/index.d.ts").TransformResult;

void e;
void n;
void p;
void parseDefaults;
void transformDefaults;
void flattenEnabled;
void flattenSpacing;
void completeFlattenEnabled;
void completeFlattenSpacing;
void normalizedText;
void transformed;
void reported;
void transformReport;
void primeMerges;
void textNormalizations;
void iterations;
void applied;
void viaScripted;
void declaratives;
