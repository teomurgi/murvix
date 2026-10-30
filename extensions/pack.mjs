#!/usr/bin/env node
// Packages vendored extensions for the Murvix release marketplace.
//
// Reads each extension's name, version, description, and permissions from its
// package.json `murvix` block (single source of truth), zips the Vite `dist/`
// output, and writes `extensions/manifest.json` — the file the app fetches
// from the release tag to list available extensions.
//
// Workflow:
//   1. cd extensions/files && npm ci && npm run build   (repeat for git)
//   2. node extensions/pack.mjs                          (writes dist/ + manifest.json)
//   3. Commit manifest.json; the release workflow uploads dist/*.zip as
//      GitHub release assets named `<name>-<version>.zip`.
//
// The app constructs each archive's download URL from the release tag and the
// entry's name/version, so the manifest itself carries no URLs: it stays valid
// for every tag that contains it.
import crypto from "node:crypto";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.dirname(fileURLToPath(import.meta.url));

function readPackage(dir) {
  const pkg = JSON.parse(fs.readFileSync(path.join(dir, "package.json"), "utf8"));
  if (typeof pkg.name !== "string" || typeof pkg.version !== "string") {
    throw new Error(`${dir}: package.json needs a name and version`);
  }
  const murvix = pkg.murvix;
  if (murvix === null || typeof murvix !== "object" || Array.isArray(murvix)) {
    throw new Error(`${dir}: package.json needs a "murvix" manifest object`);
  }
  const permissions = murvix.permissions ?? [];
  if (!Array.isArray(permissions) || !permissions.every((p) => typeof p === "string")) {
    throw new Error(`${dir}: murvix.permissions must be an array of strings`);
  }
  return {
    name: pkg.name,
    version: pkg.version,
    description: typeof murvix.description === "string" ? murvix.description : "",
    permissions: [...permissions].sort(),
  };
}

function packExtension(dir) {
  const meta = readPackage(dir);
  const dist = path.join(dir, "dist");
  if (!fs.statSync(dist, { throwIfNoEntry: false })?.isDirectory()) {
    throw new Error(`${meta.name}: 'dist/' not found — run \`npm run build\` first`);
  }
  // The loader prefers dist/ when it exists; the archive ships dist's content
  // at its root so install/extract land a runnable package directly. No
  // symlinks: the loader rejects them and the app refuses to install them.
  const stage = fs.mkdtempSync(path.join(os.tmpdir(), `murvix-pack-${meta.name}-`));
  try {
    execFileSync("zip", ["-X", "-9", "-r", "-q", path.join(stage, "extension.zip"), "."], {
      cwd: dist,
      stdio: ["ignore", "ignore", "inherit"],
    });
    const zip = fs.readFileSync(path.join(stage, "extension.zip"));
    const sha256 = crypto.createHash("sha256").update(zip).digest("hex");
    return { ...meta, zip, sha256, size: zip.length };
  } finally {
    fs.rmSync(stage, { recursive: true, force: true });
  }
}

function main() {
  const names = process.argv.slice(2).filter((a) => !a.startsWith("-"));
  const dirs = (names.length > 0 ? names : ["files", "git"]).map((n) => path.join(root, n));
  const outDir = path.join(root, "dist");
  fs.mkdirSync(outDir, { recursive: true });
  const manifest = [];
  for (const dir of dirs) {
    const { name, version, description, permissions, zip, sha256, size } = packExtension(dir);
    const archive = `${name}-${version}.zip`;
    fs.writeFileSync(path.join(outDir, archive), zip);
    manifest.push({ name, current_version: version, description, permissions, archive, sha256, size });
    console.log(`${name}@${version}  ${size} bytes  sha256=${sha256}  -> dist/${archive}`);
  }
  manifest.sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
  fs.writeFileSync(path.join(root, "manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(`wrote extensions/manifest.json with ${manifest.length} extensions`);
}

main();
