// Release gate: verify tag (when present), Cargo workspace version,
// packages/texform/package.json, and CHANGELOG.md all agree.
import { readFileSync } from "node:fs";

function fail(message) {
  console.error(`::error::${message}`);
  process.exit(1);
}

const cargo = readFileSync("Cargo.toml", "utf8");
const match = cargo.match(/\[workspace\.package\][^[]*?version\s*=\s*"([^"]+)"/s);
if (!match) fail("workspace.package.version not found in Cargo.toml");
const version = match[1];

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
