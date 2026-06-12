// Sync the Cargo workspace version into workspace crate dependencies and
// packages/texform/package.json. Used by the release-plz workflow to keep
// release artifacts in lockstep.
import { readFileSync, writeFileSync } from "node:fs";

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
  return {
    body: toml.slice(bodyStart, bodyEnd),
    start: bodyStart,
    end: bodyEnd,
  };
}

function readWorkspaceVersion(toml) {
  const section = readTomlSection(toml, "workspace.package");
  const match = section.body.match(/^\s*version\s*=\s*"([^"]+)"/m);
  if (!match) fail("workspace.package.version not found in Cargo.toml");
  return match[1];
}

function syncWorkspaceCrateDependencyVersions(toml, version) {
  const section = readTomlSection(toml, "workspace.dependencies");
  const lines = section.body.split("\n");
  const updated = [];

  const syncedLines = lines.map((line) => {
    const match = line.match(/^\s*([A-Za-z0-9_-]+)\s*=\s*(\{.*\})\s*$/);
    if (!match) return line;

    const [, name, inlineTable] = match;
    const pathMatch = inlineTable.match(/\bpath\s*=\s*"([^"]+)"/);
    if (!pathMatch?.[1].startsWith("crates/")) return line;

    let nextLine = line;
    if (/\bversion\s*=\s*"[^"]+"/.test(nextLine)) {
      nextLine = nextLine.replace(/\bversion\s*=\s*"[^"]+"/, `version = "${version}"`);
    } else {
      nextLine = nextLine.replace(/\{\s*/, `{ version = "${version}", `);
    }

    updated.push(`${name} (${pathMatch[1]})`);
    return nextLine;
  });

  return {
    cargo: `${toml.slice(0, section.start)}${syncedLines.join("\n")}${toml.slice(section.end)}`,
    updated,
  };
}

const cargo = readFileSync("Cargo.toml", "utf8");
const version = readWorkspaceVersion(cargo);

const { cargo: syncedCargo, updated } = syncWorkspaceCrateDependencyVersions(cargo, version);
if (syncedCargo === cargo) {
  console.log(`workspace crate dependencies already at ${version}`);
} else {
  writeFileSync("Cargo.toml", syncedCargo);
  console.log(`workspace crate dependencies -> ${version}: ${updated.join(", ")}`);
}

const pkgPath = "packages/texform/package.json";
const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));
if (pkg.version === version) {
  console.log(`package.json already at ${version}`);
} else {
  pkg.version = version;
  writeFileSync(pkgPath, `${JSON.stringify(pkg, null, 2)}\n`);
  console.log(`package.json version -> ${version}`);
}
