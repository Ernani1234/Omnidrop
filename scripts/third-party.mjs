// Generates docs/THIRD_PARTY.md: every Rust crate linked into the app and
// every npm package bundled into the interface, with its license.
// Usage: node scripts/third-party.mjs
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const metadata = JSON.parse(
  execFileSync("cargo", ["metadata", "--format-version", "1", "--locked"], { maxBuffer: 256 * 1024 * 1024 }).toString(),
);
const packages = new Map(metadata.packages.map((p) => [p.id, p]));
const nodes = new Map(metadata.resolve.nodes.map((n) => [n.id, n]));
const root = metadata.packages.find((p) => p.name === "omnidrop");
const crates = new Map();
const visit = (id) => {
  if (crates.has(id)) return;
  const pkg = packages.get(id);
  crates.set(id, pkg);
  for (const dep of nodes.get(id)?.deps ?? []) {
    // Only what ends up in the shipped binary: normal dependencies.
    if (dep.dep_kinds.some((k) => k.kind === null)) visit(dep.pkg);
  }
};
visit(root.id);

const pkgJson = JSON.parse(readFileSync("package.json", "utf8"));
const npm = new Map();
const visitNpm = (name) => {
  if (npm.has(name)) return;
  const file = join("node_modules", name, "package.json");
  if (!existsSync(file)) return;
  const info = JSON.parse(readFileSync(file, "utf8"));
  npm.set(name, info);
  for (const dep of Object.keys(info.dependencies ?? {})) visitNpm(dep);
  for (const dep of Object.keys(info.peerDependencies ?? {})) if (existsSync(join("node_modules", dep))) visitNpm(dep);
};
Object.keys(pkgJson.dependencies).forEach(visitNpm);

const own = new Set(["omnidrop", "omnidrop-engine", "omnidrop-app"]);
const rustRows = [...crates.values()]
  .filter((p) => !own.has(p.name))
  .sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version))
  .map((p) => `| ${p.name} | ${p.version} | ${p.license ?? p.license_file ?? "see crate"} |`);
const npmRows = [...npm.values()]
  .sort((a, b) => a.name.localeCompare(b.name))
  .map((p) => `| ${p.name} | ${p.version} | ${typeof p.license === "string" ? p.license : p.license?.type ?? "see package"} |`);

const summary = readFileSync("scripts/third-party-summary.md", "utf8");
writeFileSync(
  "docs/THIRD_PARTY.md",
  `${summary}
## Rust crates in the application (${rustRows.length})

| Crate | Version | License |
| --- | --- | --- |
${rustRows.join("\n")}

## npm packages bundled into the interface (${npmRows.length})

| Package | Version | License |
| --- | --- | --- |
${npmRows.join("\n")}
`,
);
console.log(`wrote docs/THIRD_PARTY.md: ${rustRows.length} crates, ${npmRows.length} npm packages`);
