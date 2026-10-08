import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import {
  Document,
  KnowledgeBase,
  Node,
  Parser,
  TransformEngine,
  TexformConfigError,
  TexformEditError,
  TexformConformanceError,
  TexformParseError,
  TexformTransformError,
  listPackages,
  listRules,
  serialize,
  validateArgspec,
} from "../node/index.js";

// Cases shared with the Python binding tests: both must give the same LaTeX or error class.
const bindingCases = JSON.parse(
  readFileSync(new URL("../../../crates/texform/tests/binding_cases.json", import.meta.url), "utf8"),
);
const bindingErrors = {
  conformance: TexformConformanceError,
  edit: TexformEditError,
  parse: TexformParseError,
};
for (const testCase of bindingCases) {
  const knowledgeBase = testCase.packages && new KnowledgeBase({ packages: testCase.packages });
  const document = new Document({ knowledgeBase, mode: testCase.root });
  const method = testCase.call.replace(/_(\w)/g, (_, letter) => letter.toUpperCase());
  let latex;
  try {
    if (method === "serialize") {
      latex = serialize(...testCase.args);
    } else if (method === "fromSyntax") {
      latex = Document.fromSyntax(testCase.args[0], { knowledgeBase }).toLatex();
    } else {
      const options = testCase.mode ? [{ mode: testCase.mode }] : [];
      const node = document[method](...testCase.args, ...options);
      if ("latex" in testCase) {
        document.appendChild(document.root(), node);
        latex = document.toLatex();
      }
    }
  } catch (error) {
    assert.equal(error.constructor, bindingErrors[testCase.error], `${testCase.name}: ${error}`);
    if (testCase.rule) assert.equal(error.rule, testCase.rule, testCase.name);
    continue;
  }
  assert.equal(testCase.error, undefined, `${testCase.name} should fail`);
  assert.equal(latex, testCase.latex, testCase.name);
}

const parsed = validateArgspec("!s");
if (!parsed.valid || parsed.argCount !== 1) {
  throw new Error("validateArgspec contract failed");
}
if (!parsed.parsed?.[0]?.noLeadingSpace) {
  throw new Error("parsed slot should be camelCase");
}

const parser = new Parser();
const missing = parser.knowledgeBase().lookupCommand("__missing__", "math");
if (missing !== null) {
  throw new Error("lookup miss should return null");
}

const frac = parser.knowledgeBase().lookupCommand("frac", "math");
if (frac && !("allowedMode" in frac)) {
  throw new Error("lookup hit should be camelCase");
}

try {
  parser.knowledgeBase().lookupCommand("frac", "bad");
  throw new Error("invalid lookup mode should fail");
} catch (error) {
  if (!(error instanceof TexformConfigError)) {
    throw error;
  }
}

new Parser({
  knowledgeBase: new KnowledgeBase({
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
  }),
}).parse("\\foo{x}", { rejectUnknown: true, abortOnError: true });

try {
  new KnowledgeBase({
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
  new KnowledgeBase({ packages: ["__missing__"] });
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
  throw new Error(
    "serialize options should accept camelCase groupInnerSpacing",
  );
}
const unicodeDoc = parser.parse(String.raw`\text{\%𝒜}`).document;
const unicodeTokenized = unicodeDoc.toTokenizedLatex();
const escaped = unicodeTokenized.tokens.find(
  (token) => token.text === String.raw`\%`,
);
const unicode = unicodeTokenized.tokens.find((token) => token.text === "𝒜");
if (
  unicodeTokenized.latex !== unicodeDoc.toLatex() ||
  escaped?.kind !== "character"
) {
  throw new Error(
    "tokenized serialization should preserve LaTeX and escaped characters",
  );
}
if (
  !unicode ||
  "start_byte" in unicode ||
  unicode.endByte - unicode.startByte !== 4
) {
  throw new Error("token spans should use camelCase UTF-8 byte offsets");
}

const ampersandDoc = parser.parse(String.raw`a \& b`).document;
const staged = [
  ampersandDoc.createAlignmentTab(),
  ampersandDoc.createChar("&"),
];
for (const node of staged) {
  ampersandDoc.appendChild(ampersandDoc.root(), node);
}
if (
  staged[0].kind !== "alignmentTab" ||
  ampersandDoc.toLatex() !== String.raw`a \& b & \&`
) {
  throw new Error("alignment tabs and literal ampersands should stay distinct");
}

const engine = new TransformEngine({ profile: "authoring" });
const normalized = engine.normalize("a''");
if (typeof normalized !== "string") {
  throw new Error("normalize should return a string");
}
const reportedPrimes = engine.normalizeWithReport("a''");
if (reportedPrimes.normalized !== normalized) {
  throw new Error("normalizeWithReport should match plain normalize");
}
if (!("primeRunMerges" in reportedPrimes.report.finalizeAst)) {
  throw new Error("report should expose finalizeAst.primeRunMerges");
}
if (
  "steps" in reportedPrimes.report.finalizeAst ||
  "iterations" in reportedPrimes.report
) {
  throw new Error("report leaked the old shape");
}
if ("lower_attributes" in reportedPrimes.report) {
  throw new Error("report leaked snake_case");
}

const liveParsed = engine.parse("{{x}}").document;
const transformed = engine.transform(liveParsed, {
  rewrite: { enabled: false },
  lowerAttributes: { enabled: false },
  flattenGroups: { enabled: true },
});
if (transformed !== undefined) {
  throw new Error("transform should return undefined");
}
if (liveParsed.toLatex() !== "x") {
  throw new Error("engine.transform should update documents in place");
}
const reportedLive = engine.parse("{{x}}").document;
const transformReport = engine.transformWithReport(reportedLive, {
  rewrite: { enabled: false },
  lowerAttributes: { enabled: false },
  flattenGroups: { enabled: true },
});
if (reportedLive.toLatex() !== "x") {
  throw new Error("transformWithReport should update documents in place");
}
if (!("guardHits" in transformReport.flattenGroups)) {
  throw new Error("transform report should expose flattenGroups.guardHits");
}

const corpusEngine = new TransformEngine({ profile: "corpus" });
const flattenSrc = String.raw`a {} b + \sin {x}`;
const unconfiguredNormalized = corpusEngine.normalize(flattenSrc);
const enabledNormalized = corpusEngine.normalize(flattenSrc, {
  flattenGroups: { enabled: true },
});
if (
  unconfiguredNormalized !== String.raw`a b + \sin x` ||
  enabledNormalized !== unconfiguredNormalized
) {
  throw new Error(
    "corpus normalize flattenGroups overlay should keep profile defaults",
  );
}
const preserveSpacingNormalized = corpusEngine.normalize(flattenSrc, {
  flattenGroups: { preserveRenderedSpacing: true },
});
if (
  !preserveSpacingNormalized.includes("{ }") ||
  !preserveSpacingNormalized.includes(String.raw`\sin {`)
) {
  throw new Error(
    "corpus normalize flattenGroups should honor an explicit preserveRenderedSpacing override",
  );
}

const syntaxDoc = Document.fromSyntax(engine.parse("x").document.toSyntax());
engine.transform(syntaxDoc);
const otherEngine = new TransformEngine({ profile: "authoring" });
engine.transform(otherEngine.parse("x").document);
engine.transform(new Document());
const customKnowledge = new KnowledgeBase({ packages: ["base"] });
const sharedParser = new Parser({ knowledgeBase: customKnowledge });
const sharedEngine = new TransformEngine({
  profile: "authoring",
  knowledgeBase: customKnowledge,
});
const sharedDoc = sharedParser.parse("x").document;
assert(customKnowledge.isSame(sharedDoc.knowledgeBase()));
assert(customKnowledge.isSame(sharedEngine.knowledgeBase()));
assert(sharedDoc.root().isSameNode(sharedDoc.root()));
assert(sharedDoc.root().document().root().isSameNode(sharedDoc.root()));
const clonedDoc = sharedDoc.clone();
assert(!clonedDoc.root().isSameNode(sharedDoc.root()));
assert(clonedDoc.knowledgeBase().isSame(customKnowledge));
sharedEngine.transform(clonedDoc);
sharedEngine.transform(
  Document.fromSyntax(sharedDoc.toSyntax(), { knowledgeBase: customKnowledge }),
);
assert(!customKnowledge.isSame(new KnowledgeBase({ packages: ["base"] })));
expectError(() => engine.transform(sharedDoc), TexformTransformError);
for (const method of ["commands", "environments", "characters"]) {
  const names = customKnowledge[method]("math").map((record) => record.name);
  assert.deepEqual(names, [...names].sort());
}
assert(Array.isArray(customKnowledge.delimiters()));
assert.deepEqual(customKnowledge.packages(), ["base"]);
expectError(() => new Parser({ packages: [] }), TexformConfigError);
expectError(
  () => new TransformEngine({ profile: "authoring", items: [] }),
  TexformConfigError,
);
expectError(() => new Parser({ knowledgeBase: {} }), TexformConfigError);

const incompleteEngine = new TransformEngine({
  profile: "corpus",
  knowledgeBase: new KnowledgeBase({ packages: ["base"] }),
});
const incompleteDocument = incompleteEngine.parse(
  String.raw`\frac{a}{b}\sqrt[`,
  {
    abortOnError: false,
  },
).document;
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
    throw new Error(
      "incomplete-document transform should expose transform kind",
    );
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
if (
  !rootEntry ||
  rootEntry.span.start !== 0 ||
  rootEntry.span.end !== spanSrc.length
) {
  throw new Error("nodeSpans should include a root span covering the source");
}
if (!spanEntries.some((entry) => entry.id === "root.child.0.arg.0.content")) {
  throw new Error("nodeSpans should include argument content paths");
}
if (new Document().nodeSpans().length !== 0) {
  throw new Error(
    "nodeSpans should be empty for documents built without parsing",
  );
}

const packages = listPackages();
if (!Array.isArray(packages) || packages.length === 0) {
  throw new Error("listPackages should return package infos");
}
const basePackage = packages.find((info) => info.name === "base");
if (
  !basePackage ||
  basePackage.commands <= 0 ||
  basePackage.environments <= 0
) {
  throw new Error("listPackages should report base with record counts");
}

const rules = listRules();
const listedRuleKeys = rules.map((rule) => rule.key);
assert.deepEqual(listedRuleKeys, [...listedRuleKeys].sort(), "listRules should sort by key");
assert.equal(new Set(listedRuleKeys).size, listedRuleKeys.length, "listRules keys should be unique");
assert.deepEqual(rules.find((rule) => rule.key === "base/over-to-frac"), {
  key: "base/over-to-frac",
  level: "authoring",
  fidelity: "render",
  summary: "Rewrite infix over to an explicit frac command.",
  enabledByPackages: ["base"],
});

const flattenStrict = {
  enabled: true,
  preserveRenderedSpacing: true,
};
const flattenStructuralOnly = {
  enabled: true,
  preserveRenderedSpacing: false,
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
const normalizedWithRewriteOff = engine.normalize(overSrc, rewriteOff);
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
  () =>
    engine.normalize(overSrc, { flattenGroups: { preserveEmptyGruop: true } }),
  TexformConfigError,
);
assert.match(typoError.message, /flattenGroups\.preserveEmptyGruop/);
assert.match(typoError.message, /preserveRenderedSpacing/);
assert.doesNotMatch(typoError.message, /preserveEmptyGroup/);
assert.doesNotMatch(
  typoError.message,
  /preserveGroupContainingDeclarativeCommand/,
);

const oldFlattenKeys = [
  "preserveGroupContainingDeclarativeCommand",
  "preserveGroupInScriptBaseSlot",
  "preserveGroupInsideEnvBody",
  "preserveGroupContainingInfix",
  "preserveGroupAdjacentToCommandLike",
  "preserveGroupAfterScriptedCommandLike",
  "preserveGroupAsArgumentOfCommand",
  "preserveEmptyGroup",
  "preserveGroupWithLoneAtomSpacingChar",
  "preserveGroupStartingWithAtomSpacingChar",
  "preserveGroupContainingDelimitedPair",
];
for (const key of oldFlattenKeys) {
  const error = expectError(
    () => engine.normalize(overSrc, { flattenGroups: { [key]: false } }),
    TexformConfigError,
  );
  assert.match(error.message, new RegExp(key));
}

const flattenNullOmitted = engine.normalize(overSrc);
assert.equal(
  engine.normalize(overSrc, {
    flattenGroups: { enabled: null, preserveRenderedSpacing: null },
  }),
  flattenNullOmitted,
);
assert.equal(
  engine.normalize(overSrc, { flattenGroups: null }),
  flattenNullOmitted,
);

const rewriteArrayError = expectError(
  () => engine.normalize(overSrc, { rewrite: [] }),
  TexformConfigError,
);
assert.match(rewriteArrayError.message, /expected an object/);

const topArrayError = expectError(
  () => engine.normalize(overSrc, []),
  TexformConfigError,
);
assert.match(topArrayError.message, /expected an object/);

expectError(
  () => engine.normalize(overSrc, { rewrite: { enabled: "yes" } }),
  TexformConfigError,
);

const omittedNormalize = engine.normalize(overSrc);
assert.equal(
  engine.normalize(overSrc, { rewrite: undefined }),
  omittedNormalize,
);
assert.equal(engine.normalize(overSrc, { rewrite: null }), omittedNormalize);

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

expectError(
  () => new KnowledgeBase({ items: [["command", "foo"]] }),
  TexformConfigError,
);

const missingArgspec = expectError(
  () => new KnowledgeBase({ items: [{ target: "command", name: "foo" }] }),
  TexformConfigError,
);
assert.match(missingArgspec.message, /argspec/);

const reportSource = String.raw`a \over b + {\bf x}`;
const plainReportText = engine.normalize(reportSource);
const firstReport = engine.normalizeWithReport(reportSource);
assert.deepEqual(firstReport.report.warnings, []);
const unknownSource = String.raw`\unknown{a}{b}+\unknown{c}`;
const unknownReport = engine.normalizeWithReport(unknownSource);
assert.equal(unknownReport.normalized, engine.normalize(unknownSource));
assert.equal(unknownReport.report.warnings.length, 1);
assert.equal(unknownReport.report.warnings[0].kind, "unknown-command");
assert.equal(unknownReport.report.warnings[0].name, "unknown");
assert.ok(unknownReport.report.warnings[0].message);
assert.ok(unknownReport.report.flattenGroups.guardHits.unknownCommandArguments > 0);
assert.equal(typeof plainReportText, "string");
assert.equal(firstReport.normalized, plainReportText);
assert.deepEqual(Object.keys(firstReport.report).sort(), [
  "finalizeAst",
  "flattenGroups",
  "lowerAttributes",
  "rewrite",
  "warnings",
]);
assert.deepEqual(Object.keys(firstReport.report.rewrite).sort(), [
  "iterations",
  "rules",
]);
assert.ok(firstReport.report.rewrite.iterations > 0);
const ruleKeys = firstReport.report.rewrite.rules.map((rule) => rule.key);
assert.deepEqual(listedRuleKeys, [...listedRuleKeys].sort());
assert.ok(
  firstReport.report.rewrite.rules.some((rule) => rule.appliedCount > 0),
);
for (const rule of firstReport.report.rewrite.rules) {
  assert.deepEqual(Object.keys(rule).sort(), [
    "appliedCount",
    "key",
    "skippedCount",
  ]);
}
assert.deepEqual(Object.keys(firstReport.report.finalizeAst).sort(), [
  "primeRunMerges",
  "textNormalizations",
]);
assert.equal("steps" in firstReport.report.finalizeAst, false);
assert.deepEqual(Object.keys(firstReport.report.flattenGroups.actions).sort(), [
  "inlinedMultiChild",
  "removedEmpty",
  "replacedSingleChild",
  "unwrappedSlot",
]);
assert.deepEqual(
  Object.keys(firstReport.report.flattenGroups.guardHits).sort(),
  [
    "commandContact",
    "commandContactViaScriptedBase",
    "declarativeScope",
    "delimitedPair",
    "emptyGroup",
    "envBody",
    "infixScope",
    "leadingAtomSpacingChar",
    "loneAtomSpacingChar",
    "scriptBase",
    "unknownCommandArguments",
  ],
);
assert.equal("guards" in firstReport.report.flattenGroups, false);
const attributeStats = firstReport.report.lowerAttributes.attributes;
assert.deepEqual(
  attributeStats.map((item) => [item.attr, item.value]),
  [...attributeStats]
    .map((item) => [item.attr, item.value])
    .sort(
      (left, right) =>
        left[0].localeCompare(right[0]) || left[1].localeCompare(right[1]),
    ),
);
for (const item of attributeStats) {
  for (const bucket of ["consumed", "redundant", "emitted"]) {
    assert.deepEqual(Object.keys(item[bucket]).sort(), [
      "declaratives",
      "prefixes",
    ]);
  }
}

const rewriteDisabled = { rewrite: { enabled: false } };
const disabledText = engine.normalize(reportSource, rewriteDisabled);
const disabledReport = engine.normalizeWithReport(
  reportSource,
  rewriteDisabled,
);
assert.equal(disabledText, disabledReport.normalized);
assert.notEqual(disabledText, plainReportText);
assert.equal(disabledReport.report.rewrite.iterations, 0);
assert.deepEqual(disabledReport.report.rewrite.rules, []);
for (const method of [
  engine.normalize.bind(engine),
  engine.normalizeWithReport.bind(engine),
]) {
  expectError(
    () => method(reportSource, { rewriteEnabled: false }),
    TexformConfigError,
  );
  expectError(
    () => method(reportSource, { rewrite: { enabled: "yes" } }),
    TexformConfigError,
  );
}

expectError(() => engine.normalize("{"), TexformParseError);
const afterFailure = engine.normalizeWithReport(reportSource);
assert.equal(afterFailure.normalized, plainReportText);
assert.deepEqual(afterFailure.report, firstReport.report);
assert.equal(engine.normalize(reportSource), plainReportText);

const freshDocument = () => engine.parse(reportSource).document;
const transformedDocument = freshDocument();
assert.equal(engine.transform(transformedDocument), undefined);
assert.equal(transformedDocument.toLatex(), plainReportText);
const incompleteForReport = engine.parse(String.raw`\sqrt[`, {
  abortOnError: false,
}).document;
const incompleteBefore = incompleteForReport.toLatex();
expectError(
  () => engine.transformWithReport(incompleteForReport),
  TexformTransformError,
);
assert.equal(incompleteForReport.toLatex(), incompleteBefore);
const foreignForReport = Document.fromSyntax(freshDocument().toSyntax(), {
  knowledgeBase: new KnowledgeBase(),
});
expectError(
  () => engine.transformWithReport(foreignForReport),
  TexformTransformError,
);
const reportedDocument = freshDocument();
assert.deepEqual(
  engine.transformWithReport(reportedDocument),
  firstReport.report,
);
assert.equal(reportedDocument.toLatex(), plainReportText);
assert.deepEqual(
  engine.transformWithReport(freshDocument()),
  firstReport.report,
);

// CommonJS exposes the same live knowledge and document contract.
const cjs = createRequire(import.meta.url)("../node/index.cjs");
const cjsKnowledge = new cjs.KnowledgeBase({ packages: ["base"] });
const cjsParser = new cjs.Parser({ knowledgeBase: cjsKnowledge });
const cjsEngine = new cjs.TransformEngine({
  profile: "authoring",
  knowledgeBase: cjsKnowledge,
});
const cjsDocument = cjsParser.parse("x").document;
cjsEngine.transform(cjsDocument.clone());
assert(cjsDocument.knowledgeBase().isSame(cjsKnowledge));
assert(cjsDocument.root().document().root().isSameNode(cjsDocument.root()));
assert(new Parser().knowledgeBase().isSame(new Document().knowledgeBase()));
assert(new Document({ mode: "text" }).root().contentMode() === "text");
const records = customKnowledge.commands("math");
records[0].name = "changed";
assert(customKnowledge.commands("math")[0].name !== "changed");
const loadedPackages = customKnowledge.packages();
loadedPackages.push("physics");
assert.deepEqual(customKnowledge.packages(), ["base"]);
expectError(() => new Document({ mode: "invalid" }), TexformConfigError);
expectError(() => new Document({ packages: [] }), TexformConfigError);
expectError(
  () => Document.fromSyntax(doc.toSyntax(), { mode: "text" }),
  TexformConfigError,
);

assert(Object.isFrozen(customKnowledge));
assert.throws(() => { customKnowledge.extra = true; }, TypeError);

// Construction checks knowledge and preserves structured error information.
const buildDoc = new Document();
const buildSqrt = buildDoc.createCommand("sqrt", [null, "x"]);
assert.equal(buildSqrt.arg(0), null);
assert.equal(buildSqrt.arg(1).kind, "Math");
const buildScript = buildDoc.createScripted(buildSqrt, "i", "2");
const buildGroup = buildDoc.createDelimitedGroup("(", ")", [buildScript]);
buildDoc.appendChild(buildDoc.root(), buildGroup);
assert.equal(new Parser().parse(buildDoc.toLatex()).document.hasErrors(), false);
const beforeFailedBuild = buildDoc.toLatex();
assert.throws(() => buildDoc.createCommand("frac", ["x"]), error =>
  error instanceof TexformConformanceError && error instanceof TexformEditError &&
  typeof error.path === "string" && error.rule === "argument_count");
assert.equal(buildDoc.toLatex(), beforeFailedBuild);
assert.throws(() => buildDoc.parseFragment("{"), error =>
  error instanceof TexformParseError && error.diagnostics.length > 0);
const buildOperator = buildDoc.createCommand("operatorname", [null, "sin"]);
assert.equal(buildOperator.arg(0).value, false);
assert.equal(buildOperator.arg(1).kind, "OperatorName");
assert.equal(buildDoc.createCommand("big", ["("]).arg(0).value, "(");
assert.equal(buildDoc.createPrime(2).primeCount(), 2);
assert.equal(buildDoc.createGroup("math", ["x", "y"]).children.length, 2);
assert.equal(buildDoc.createEnvironment("matrix", [], ["x", "y"]).envBody().kind, "group");
const textBuild = new Document({ mode: "text" });
textBuild.appendChild(textBuild.root(), textBuild.createInlineMath(["x"]));
assert.throws(() => buildDoc.createInlineMath(["x"], { mode: "math" }), TexformConformanceError);
assert.throws(() => Document.fromSyntax({ Root: { mode: "Math", children: [{ Prime: { count: 0 } }] } }), TexformConformanceError);
const foreignBuild = new Document().createChar("x");
assert.throws(() => buildDoc.createCommand("sqrt", [null, foreignBuild]), TexformEditError);

const physicsBuild = new Document({ knowledgeBase: new KnowledgeBase({ packages: ["base", "physics"] }) });
const pairedBuild = physicsBuild.createCommand("qty", [{ value: "x", open: "(", close: ")" }]);
const pairedSlot = pairedBuild.argSlots().find(arg => arg?.form.kind === "paired");
assert(pairedSlot);
assert.equal(pairedSlot.form.open, "(");
assert.equal(pairedSlot.form.close, ")");
assert.throws(() => buildDoc.createPrime(-1), TexformConformanceError);
assert.throws(() => buildDoc.createPrime(1.5), TexformConformanceError);
assert.throws(() => new cjs.Document().createCommand("frac", ["x"]), cjs.TexformConformanceError);

for (const api of [{ Document, TexformConformanceError }, cjs]) {
  assert.throws(() => new api.Document().createCommand("bf"), error => {
    assert(error instanceof api.TexformConformanceError);
    assert.equal(error.rule, "command_kind_mismatch");
    assert.match(error.message, /declarative constructor/);
    return true;
  });
}

const tableSnapshot = new Parser().parse(String.raw`\sqrt{x}+\operatorname{sn}`).document.toColumnar();
assert.deepEqual(Object.keys(tableSnapshot).sort(), ["args", "nodes"]);
assert.equal(Object.keys(tableSnapshot.nodes).length, 12);
assert.equal(Object.keys(tableSnapshot.args).length, 9);
for (const table of Object.values(tableSnapshot)) {
  assert(Object.values(table).every(Array.isArray));
  assert.equal(new Set(Object.values(table).map(column => column.length)).size, 1);
}
assert.equal(tableSnapshot.nodes.kind[0], "Root");
assert.equal(tableSnapshot.nodes.parent[0], -1);
assert.equal(tableSnapshot.nodes.slot[0], null);
assert.equal(tableSnapshot.nodes.slot_index[0], -1);
assert.equal(tableSnapshot.nodes.group_kind[0], null);
assert.equal(tableSnapshot.args.form[0], "optional");
assert.equal(tableSnapshot.args.present[0], false);
assert.equal(tableSnapshot.args.value_kind[0], null);
assert.equal(tableSnapshot.args.content[0], -1);
assert(tableSnapshot.args.value_kind.includes("operator_name"));
assert(tableSnapshot.args.value.includes("false"));
const errorTables = Document.fromSyntax({ Root: { mode: "Math", children: [
  { Error: { message: "invalid", snippet: "?" } },
] } }).toColumnar();
assert.deepEqual(errorTables.nodes.value, [null, "?"]);
assert.deepEqual(errorTables.args.owner, []);
assert.deepEqual(new cjs.Document().toColumnar().nodes.kind, ["Root"]);

// Exercise the editing boundary through both distributed wrappers.
for (const api of [{ Document, Parser, TexformEditError, TexformConformanceError }, cjs]) {
  const editDoc = new api.Parser().parse("x_i").document;
  const base = editDoc.root().children[0].scriptBase();
  assert.deepEqual(base.slot(), { kind: "script_base", index: null });
  assert.equal(base.path(), "root.child.0.base");
  assert(editDoc.nodeAt(base.path()).isSameNode(base));
  assert.equal(base.isKnown(), null);
  assert.equal(editDoc.root().slot(), null);
  assert.throws(() => editDoc.nodeAt("root.child.99"), api.TexformEditError);
  const scripted = editDoc.setSuperscript(base, "2");
  assert.equal(scripted.kind, "scripted");
  assert.deepEqual(scripted.superscript().slot(), { kind: "superscript", index: null });
  editDoc.setSubscript(base);
  const collapsed = editDoc.setSuperscript(base, null);
  assert(collapsed.isSameNode(base));
  assert.equal(editDoc.toLatex(), "x");
  assert.deepEqual(base.slot(), { kind: "child", index: 0 });
  const copy = editDoc.cloneNode(base);
  assert.equal(copy.path(), null);
  assert.equal(copy.slot(), null);
  assert(!copy.isSameNode(base));
  assert.equal(editDoc.importNode(base).path(), null);
  const destination = new api.Document();
  const imported = destination.importNode(base);
  destination.appendChild(destination.root(), imported);
  destination.setChar(imported, "y");
  assert.equal(editDoc.toLatex(), "x");
  assert.equal(destination.toLatex(), "y");
  const detachedScript = destination.setSubscript(destination.createChar("z"), "j");
  assert.equal(detachedScript.path(), null);
  assert.equal(destination.setSubscript(detachedScript, undefined).kind, "char");
  const delimiters = destination.createDelimitedGroup("(", ")", ["x"]);
  destination.setDelimiters(delimiters, "[", "]");
  assert.deepEqual(delimiters.groupKind(), { kind: "Delimited", left: "[", right: "]" });
  const prime = destination.createPrime(1);
  destination.setPrimeCount(prime, 3);
  assert.equal(prime.primeCount(), 3);
  for (const invalid of [0, -1, 1.5, 2 ** 32]) {
    assert.throws(() => destination.setPrimeCount(prime, invalid), api.TexformConformanceError);
  }
  const operator = destination.createCommand("operatorname", [null, "sin"]);
  assert.equal(operator.isKnown(), true);
  assert.equal(operator.arg(1).kind, "OperatorName");
  assert.equal(operator.arg(1).node.kind, "group");
  assert.deepEqual(operator.argKind(1), operator.arg(1).form);
  assert.equal(operator.argKind(99), null);
}
const incompatibleImport = new Document({ knowledgeBase: new KnowledgeBase({ packages: [] }) });
const importBefore = incompatibleImport.toLatex();
assert.throws(() => incompatibleImport.importNode(buildOperator), TexformConformanceError);
assert.equal(incompatibleImport.toLatex(), importBefore);
assert.throws(() => buildDoc.cloneNode(foreignBuild), TexformEditError);
const forbiddenScriptHost = new Document({ mode: "text" });
const forbiddenText = forbiddenScriptHost.createText("hello");
forbiddenScriptHost.appendChild(forbiddenScriptHost.root(), forbiddenText);
assert.throws(() => forbiddenScriptHost.setSubscript(forbiddenText, "i"), TexformConformanceError);
assert.equal(forbiddenScriptHost.toLatex(), "hello");
const pairedIndex = pairedBuild.argSlots().findIndex(arg => arg?.form.kind === "paired");
physicsBuild.setArgDelimiters(pairedBuild, pairedIndex, "[", "]");
physicsBuild.setArg(pairedBuild, pairedIndex, "y");
assert.deepEqual(pairedBuild.argKind(pairedIndex), { kind: "paired", open: "[", close: "]" });
assert.equal(pairedBuild.arg(pairedIndex).node.children[0].char, "y");
