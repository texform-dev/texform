import {
  Document,
  Node,
  Parser,
  TransformEngine,
  TexformConfigError,
  TexformEditError,
  TexformParseError,
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

const engine = new TransformEngine({ profile: "authoring" });
const normalized = engine.normalize("a''");
if (!("finalizeAst" in normalized.report)) {
  throw new Error("report should be camelCase");
}
if ("lower_attributes" in normalized.report) {
  throw new Error("report leaked snake_case");
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
  engine.normalize("\\unknown");
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
