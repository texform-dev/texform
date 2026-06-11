// Sync the Cargo workspace version into packages/texform/package.json.
// Used by the release-plz workflow to keep the npm package in lockstep.
import { readFileSync, writeFileSync } from "node:fs";

const cargo = readFileSync("Cargo.toml", "utf8");
const match = cargo.match(/\[workspace\.package\][^[]*?version\s*=\s*"([^"]+)"/s);
if (!match) {
  console.error("::error::workspace.package.version not found in Cargo.toml");
  process.exit(1);
}
const version = match[1];

const pkgPath = "packages/texform/package.json";
const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));
if (pkg.version === version) {
  console.log(`package.json already at ${version}`);
} else {
  pkg.version = version;
  writeFileSync(pkgPath, `${JSON.stringify(pkg, null, 2)}\n`);
  console.log(`package.json version -> ${version}`);
}
