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
  math: { spacing: { groupInnerSpacing: "compact" } },
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
