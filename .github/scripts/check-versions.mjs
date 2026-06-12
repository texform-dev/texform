// Release gate: verify tag (when present), Cargo workspace version,
// workspace crate dependency versions, packages/texform/package.json, and
// CHANGELOG.md all agree.
import { readFileSync } from "node:fs";

function fail(message) {
  console.error(`::error::${message}`);
  process.exit(1);
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function readTomlSection(toml, name) {
  const header = new RegExp(`^\\[${escapeRegExp(name)}\\]\\s*$`, "m");
  const match = header.exec(toml);
  if (!match) fail(`${name} not found in Cargo.toml`);

  const bodyStart = match.index + match[0].length;
  const nextHeader = /^\[[^\]\n]+\]\s*$/m.exec(toml.slice(bodyStart));
  const bodyEnd = nextHeader ? bodyStart + nextHeader.index : toml.length;
  const body = toml.slice(bodyStart, bodyEnd);
  const startLine = toml.slice(0, bodyStart).split("\n").length;
  return { body, startLine };
}

function readWorkspaceVersion(toml) {
  const section = readTomlSection(toml, "workspace.package");
  const match = section.body.match(/^\s*version\s*=\s*"([^"]+)"/m);
  if (!match) fail("workspace.package.version not found in Cargo.toml");
  return match[1];
}

function workspaceCrateDependencies(toml) {
  const section = readTomlSection(toml, "workspace.dependencies");
  const deps = [];

  section.body.split("\n").forEach((line, index) => {
    const match = line.match(/^\s*([A-Za-z0-9_-]+)\s*=\s*(\{.*\})\s*$/);
    if (!match) return;

    const [, name, inlineTable] = match;
    const pathMatch = inlineTable.match(/\bpath\s*=\s*"([^"]+)"/);
    if (!pathMatch?.[1].startsWith("crates/")) return;

    const versionMatch = inlineTable.match(/\bversion\s*=\s*"([^"]+)"/);
    deps.push({
      name,
      path: pathMatch[1],
      version: versionMatch?.[1],
      line: section.startLine + index,
    });
  });

  return deps;
}

const cargo = readFileSync("Cargo.toml", "utf8");
const version = readWorkspaceVersion(cargo);

const mismatchedDeps = workspaceCrateDependencies(cargo).filter((dep) => dep.version !== version);
if (mismatchedDeps.length > 0) {
  fail(
    `workspace crate dependency versions differ from ${version}: ${mismatchedDeps
      .map((dep) => `${dep.name} (${dep.path}) is ${dep.version ?? "missing"} at Cargo.toml:${dep.line}`)
      .join(", ")}`,
  );
}

const pkg = JSON.parse(readFileSync("packages/texform/package.json", "utf8"));
if (pkg.version !== version) {
  fail(`packages/texform/package.json is ${pkg.version}, workspace is ${version}`);
}

const changelog = readFileSync("CHANGELOG.md", "utf8");
if (!changelog.includes(`## [${version}]`)) {
  fail(`CHANGELOG.md has no "## [${version}]" section`);
}

const tag = process.env.TAG ?? "";
if (tag && tag !== `v${version}`) {
  fail(`tag ${tag} does not match workspace version v${version}`);
}

console.log(`version check ok: ${version}${tag ? ` (tag ${tag})` : " (no tag, dispatch mode)"}`);
