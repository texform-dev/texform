import type {
  Document,
  ParseConfig,
  Parser,
  TransformConfig,
  TransformEngine,
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

void e;
void n;
void p;
void parseDefaults;
void transformDefaults;
void flattenEnabled;
