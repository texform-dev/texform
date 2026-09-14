import type {
  Document,
  ParseConfig,
  Parser,
  TransformConfig,
  TransformEngine,
} from "../types/index.d.ts";

declare const engine: TransformEngine;
declare const parser: Parser;
declare const src: string;
declare const doc: Document;

const e: boolean = engine.defaultTransformConfig().rewrite.enabled;
const n: number = engine.defaultTransformConfig().rewrite.maxIterations;
const p: number = parser.defaultParseConfig().maxGroupDepth;

const overrides: TransformConfig = { rewrite: { enabled: false } };
engine.normalize(src, overrides);
engine.transform(doc, overrides);

const parseOverrides: ParseConfig = { rejectUnknown: true };
parser.parse(src, parseOverrides);

// @ts-expect-error — flat rewriteEnabled is not a normalize overlay key
engine.normalize(src, { rewriteEnabled: false });
// @ts-expect-error — enabled must be a boolean
engine.transform(doc, { rewrite: { enabled: "yes" } });

void e;
void n;
void p;
