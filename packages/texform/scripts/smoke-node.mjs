import assert from "node:assert/strict";
import {
  Document,
  Node,
  Parser,
  TransformEngine,
  TexformConfigError,
  TexformEditError,
  TexformParseError,
  TexformTransformError,
  listPackages,
  validateArgspec,
} from "../node/index.js";

const parsed = validateArgspec("!s");
if (!parsed.valid || parsed.argCount !== 1) {
  throw new Error("validateArgspec contract failed");
}
if (!parsed.parsed?.[0]?.noLeadingSpace) {
  throw new Error("parsed slot should be camelCase");
}

const parser = new Parser();
const missing = parser.lookupCommand("__missing__", "math");
if (missing !== null) {
  throw new Error("lookup miss should return null");
}

const frac = parser.lookupCommand("frac", "math");
if (frac && !("allowedMode" in frac)) {
  throw new Error("lookup hit should be camelCase");
}

try {
  parser.lookupCommand("frac", "bad");
  throw new Error("invalid lookup mode should fail");
} catch (error) {
  if (!(error instanceof TexformConfigError)) {
    throw error;
  }
}

new Parser({
  packages: [],
  items: [
    {
      target: "command",
      name: "foo",
      kind: "prefix",
      allowedMode: "math",
      argspec: "m",
    },
  ],
}).parse("\\foo{x}", { rejectUnknown: true, abortOnError: true });

try {
  new Parser({
    items: [
      {
        target: "command",
        name: "foo",
        kind: "prefix",
        allowdMode: "math",
        argspec: "m",
      },
    ],
  });
  throw new Error("unknown ContextItem field should fail");
} catch (error) {
  if (!(error instanceof TexformConfigError)) {
    throw error;
  }
}

try {
  new Parser({ packages: ["__missing__"] });
  throw new Error("unknown parser package should fail");
} catch (error) {
  if (!(error instanceof TexformConfigError)) {
    throw error;
  }
}

const doc = parser.parse("x^{y}").document;
if (!(doc.root() instanceof Node)) {
  throw new Error("document.root should return public Node wrapper");
}
const defaultLatex = doc.toLatex();
const compactLatex = doc.toLatex({
  groupInnerSpacing: "compact",
});
if (defaultLatex === compactLatex) {
  throw new Error("serialize options should accept camelCase groupInnerSpacing");
}
const unicodeDoc = parser.parse(String.raw`\text{\%𝒜}`).document;
const unicodeTokenized = unicodeDoc.toTokenizedLatex();
const escaped = unicodeTokenized.tokens.find((token) => token.text === String.raw`\%`);
const unicode = unicodeTokenized.tokens.find((token) => token.text === "𝒜");
if (unicodeTokenized.latex !== unicodeDoc.toLatex() || escaped?.kind !== "character") {
  throw new Error("tokenized serialization should preserve LaTeX and escaped characters");
}
if (!unicode || "start_byte" in unicode || unicode.endByte - unicode.startByte !== 4) {
  throw new Error("token spans should use camelCase UTF-8 byte offsets");
}

const engine = new TransformEngine({ profile: "authoring" });
const normalized = engine.normalize("a''");
if (!("finalizeAst" in normalized.report)) {
  throw new Error("report should be camelCase");
}
if ("lower_attributes" in normalized.report) {
  throw new Error("report leaked snake_case");
}

const liveParsed = engine.parse("{{x}}").document;
const transformReport = engine.transform(liveParsed, {
  rewrite: { enabled: false },
  lowerAttributes: { enabled: false },
  flattenGroups: { enabled: true },
});
if (liveParsed.toLatex() !== "x") {
  throw new Error("engine.transform should update documents in place");
}
if (!("flattenGroups" in transformReport)) {
  throw new Error("transform report should be camelCase");
}

const corpusEngine = new TransformEngine({ profile: "corpus" });
const flattenSrc = String.raw`a {} b + \sin {x}`;
const unconfiguredNormalized = corpusEngine.normalize(flattenSrc).normalized;
const enabledNormalized = corpusEngine.normalize(flattenSrc, {
  flattenGroups: { enabled: true },
}).normalized;
if (
  unconfiguredNormalized !== String.raw`a b + \sin x` ||
  enabledNormalized !== unconfiguredNormalized
) {
  throw new Error(
    "corpus normalize flattenGroups overlay should keep profile defaults",
  );
}
const preserveEmptyNormalized = corpusEngine.normalize(flattenSrc, {
  flattenGroups: { preserveEmptyGroup: true },
}).normalized;
if (
  !preserveEmptyNormalized.includes("{ }") ||
  !preserveEmptyNormalized.includes(String.raw`\sin x`)
) {
  throw new Error(
    "corpus normalize flattenGroups should honor an explicit guard override",
  );
}

try {
  const syntaxDoc = Document.fromSyntax(engine.parse("x").document.toSyntax());
  engine.transform(syntaxDoc);
  throw new Error("engine.transform should reject syntax-created documents");
} catch (error) {
  if (!(error instanceof TexformTransformError)) {
    throw error;
  }
}

try {
  const otherEngine = new TransformEngine({ profile: "authoring" });
  engine.transform(otherEngine.parse("x").document);
  throw new Error("engine.transform should reject documents from another engine");
} catch (error) {
  if (!(error instanceof TexformTransformError)) {
    throw error;
  }
}

const incompleteEngine = new TransformEngine({ profile: "corpus", packages: ["base"] });
const incompleteDocument = incompleteEngine.parse(String.raw`\frac{a}{b}\sqrt[`, {
  abortOnError: false,
}).document;
if (!incompleteDocument.hasErrors()) {
  throw new Error("incomplete parse should produce a document with errors");
}
const incompleteLatex = incompleteDocument.toLatex();
try {
  incompleteEngine.transform(incompleteDocument);
  throw new Error("engine.transform should reject documents with parse errors");
} catch (error) {
  if (!(error instanceof TexformTransformError)) {
    throw error;
  }
  if (error.kind !== "transform") {
    throw new Error("incomplete-document transform should expose transform kind");
  }
  if (incompleteDocument.toLatex() !== incompleteLatex) {
    throw new Error("rejected transform should leave the document unchanged");
  }
}

try {
  incompleteEngine.normalize(String.raw`\sqrt[`, { abortOnError: false });
  throw new Error("normalize should fail for incomplete input");
} catch (error) {
  if (!(error instanceof TexformParseError)) {
    throw error;
  }
  if (error.kind !== "parse") {
    throw new Error("normalize incomplete input should expose parse kind");
  }
  if (!Array.isArray(error.diagnostics) || error.diagnostics.length === 0) {
    throw new Error("parse error diagnostics missing");
  }
  if (error.document == null) {
    throw new Error("parse error document missing");
  }
}

try {
  new TransformEngine({ profile: "__bad__" });
  throw new Error("unknown transform profile should fail");
} catch (error) {
  if (!(error instanceof TexformConfigError)) {
    throw error;
  }
}

try {
  engine.normalize("{");
  throw new Error("normalize should fail for invalid input");
} catch (error) {
  if (!(error instanceof TexformParseError)) {
    throw error;
  }
  if (!Array.isArray(error.diagnostics)) {
    throw new Error("parse error diagnostics missing");
  }
}

try {
  Document.fromSyntax({ Prime: { count: "x" } });
  throw new Error("Document.fromSyntax should reject invalid syntax");
} catch (error) {
  if (!(error instanceof Error)) {
    throw new Error("Document path should throw a real Error instance");
  }
  if (error.kind !== "parse" || error.name !== "TexformParseError") {
    throw new Error("Document path error should expose kind/name");
  }
}

try {
  const staleDoc = new Document();
  const root = staleDoc.root();
  const child = staleDoc.createChar("x");
  staleDoc.appendChild(root, child);
  staleDoc.remove(child);
  child.kind;
  throw new Error("stale Node access should fail");
} catch (error) {
  if (!(error instanceof TexformEditError)) {
    throw error;
  }
}

try {
  new Document().createCommand("foo", [{ kind: "Boolean", value: "yes" }]);
  throw new Error("invalid ArgValue should fail");
} catch (error) {
  if (!(error instanceof TexformEditError)) {
    throw error;
  }
  if (error.kind !== "edit") {
    throw new Error("ArgValue error should expose edit kind");
  }
}

const spanSrc = "\\frac{a}{b}";
const spanEntries = parser.parse(spanSrc).document.nodeSpans();
if (!Array.isArray(spanEntries) || spanEntries.length === 0) {
  throw new Error("nodeSpans should return entries for parsed documents");
}
const rootEntry = spanEntries.find((entry) => entry.id === "root");
if (!rootEntry || rootEntry.span.start !== 0 || rootEntry.span.end !== spanSrc.length) {
  throw new Error("nodeSpans should include a root span covering the source");
}
if (!spanEntries.some((entry) => entry.id === "root.child.0.arg.0.content")) {
  throw new Error("nodeSpans should include argument content paths");
}
if (new Document().nodeSpans().length !== 0) {
  throw new Error("nodeSpans should be empty for documents built without parsing");
}

const packages = listPackages();
if (!Array.isArray(packages) || packages.length === 0) {
  throw new Error("listPackages should return package infos");
}
const basePackage = packages.find((info) => info.name === "base");
if (!basePackage || basePackage.commands <= 0 || basePackage.environments <= 0) {
  throw new Error("listPackages should report base with record counts");
}

const flattenStrict = {
  enabled: true,
  preserveGroupContainingDeclarativeCommand: true,
  preserveGroupInScriptBaseSlot: true,
  preserveGroupInsideEnvBody: true,
  preserveGroupContainingInfix: true,
  preserveGroupAdjacentToCommandLike: true,
  preserveGroupAsArgumentOfCommand: true,
  preserveGroupAfterScriptedCommandLike: true,
  preserveEmptyGroup: true,
  preserveGroupWithLoneAtomSpacingChar: true,
  preserveGroupStartingWithAtomSpacingChar: true,
  preserveGroupContainingDelimitedPair: true,
};
const flattenStructuralOnly = {
  enabled: true,
  preserveGroupContainingDeclarativeCommand: true,
  preserveGroupInScriptBaseSlot: true,
  preserveGroupInsideEnvBody: true,
  preserveGroupContainingInfix: true,
  preserveGroupAdjacentToCommandLike: false,
  preserveGroupAsArgumentOfCommand: false,
  preserveGroupAfterScriptedCommandLike: false,
  preserveEmptyGroup: false,
  preserveGroupWithLoneAtomSpacingChar: false,
  preserveGroupStartingWithAtomSpacingChar: false,
  preserveGroupContainingDelimitedPair: false,
};
const expectedTransformDefaults = {
  authoring: {
    lowerAttributes: { enabled: true },
    rewrite: { enabled: true, maxIterations: 100 },
    finalizeAst: { enabled: true },
    flattenGroups: flattenStrict,
  },
  faithful: {
    lowerAttributes: { enabled: true },
    rewrite: { enabled: true, maxIterations: 100 },
    finalizeAst: { enabled: true },
    flattenGroups: flattenStrict,
  },
  corpus: {
    lowerAttributes: { enabled: true },
    rewrite: { enabled: true, maxIterations: 100 },
    finalizeAst: { enabled: true },
    flattenGroups: flattenStructuralOnly,
  },
  equiv: {
    lowerAttributes: { enabled: true },
    rewrite: { enabled: true, maxIterations: 100 },
    finalizeAst: { enabled: true },
    flattenGroups: flattenStructuralOnly,
  },
};

const expectedParseDefaults = {
  rejectUnknown: false,
  abortOnError: false,
  maxGroupDepth: 128,
};

assert.deepEqual(parser.defaultParseConfig(), expectedParseDefaults);
assert.deepEqual(engine.defaultParseConfig(), expectedParseDefaults);

for (const profile of Object.keys(expectedTransformDefaults)) {
  const profiled = new TransformEngine({ profile });
  assert.deepEqual(
    profiled.defaultTransformConfig(),
    expectedTransformDefaults[profile],
    `defaultTransformConfig() for ${profile}`,
  );
}

function expectError(fn, ctor) {
  try {
    fn();
  } catch (error) {
    assert.ok(error instanceof ctor, error);
    return error;
  }
  assert.fail(`expected ${ctor.name}`);
}

const rewriteOff = { rewrite: { enabled: false } };
const overSrc = String.raw`a \over b`;
const normalizedWithRewriteOff = engine.normalize(overSrc, rewriteOff).normalized;
const parsedOver = engine.parse(overSrc).document;
engine.transform(parsedOver, rewriteOff);
assert.equal(normalizedWithRewriteOff, parsedOver.toLatex());

const rewriteEnabledError = expectError(
  () => engine.normalize(overSrc, { rewriteEnabled: false }),
  TexformConfigError,
);
assert.match(rewriteEnabledError.message, /rewriteEnabled/);

const snakeKeyError = expectError(
  () => engine.normalize(overSrc, { flatten_groups: { enabled: false } }),
  TexformConfigError,
);
assert.match(snakeKeyError.message, /flatten_groups/);
assert.match(snakeKeyError.message, /camelCase/);

const typoError = expectError(
  () => engine.normalize(overSrc, { flattenGroups: { preserveEmptyGruop: true } }),
  TexformConfigError,
);
assert.match(typoError.message, /flattenGroups\.preserveEmptyGruop/);
assert.match(typoError.message, /preserveGroupContainingDeclarativeCommand/);

const rewriteArrayError = expectError(
  () => engine.normalize(overSrc, { rewrite: [] }),
  TexformConfigError,
);
assert.match(rewriteArrayError.message, /expected an object/);

const topArrayError = expectError(() => engine.normalize(overSrc, []), TexformConfigError);
assert.match(topArrayError.message, /expected an object/);

expectError(
  () => engine.normalize(overSrc, { rewrite: { enabled: "yes" } }),
  TexformConfigError,
);

const omittedNormalize = engine.normalize(overSrc).normalized;
assert.equal(engine.normalize(overSrc, { rewrite: undefined }).normalized, omittedNormalize);
assert.equal(engine.normalize(overSrc, { rewrite: null }).normalized, omittedNormalize);

const defaultLatexAgain = doc.toLatex();
assert.equal(doc.toLatex({ scriptSpacing: undefined }), defaultLatexAgain);
assert.equal(doc.toLatex({ scriptSpacing: null }), defaultLatexAgain);

const nestedError = expectError(
  () => doc.toLatex({ math: { scripts: { order: "sup_first" } } }),
  TexformConfigError,
);
assert.match(nestedError.message, /unknown field `math`/);

const supFirstError = expectError(
  () => doc.toLatex({ scriptOrder: "supFirst" }),
  TexformConfigError,
);
assert.match(supFirstError.message, /scriptOrder/);
assert.match(supFirstError.message, /sub_first/);
assert.match(supFirstError.message, /sup_first/);

expectError(
  () => doc.toLatex({ scriptOrdre: "sup_first" }),
  TexformConfigError,
);

expectError(
  () =>
    new TransformEngine({
      profile: "corpus",
      defaultParseConfig: { rejectUnknown: true },
    }).normalize(String.raw`\notknown`),
  TexformParseError,
);

expectError(() => new Parser({ items: [["command", "foo"]] }), TexformConfigError);

const missingArgspec = expectError(
  () => new Parser({ items: [{ target: "command", name: "foo" }] }),
  TexformConfigError,
);
assert.match(missingArgspec.message, /argspec/);
