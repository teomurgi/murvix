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
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { deflateRawSync } from "node:zlib";

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

function collectFiles(dir, prefix, out) {
  const entries = fs
    .readdirSync(dir, { withFileTypes: true })
    .sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
  for (const entry of entries) {
    if (entry.name === ".DS_Store" || entry.name === "Thumbs.db") continue;
    const full = path.join(dir, entry.name);
    if (entry.isSymbolicLink()) throw new Error(`refusing to pack symlink: ${full}`);
    if (entry.isDirectory()) {
      collectFiles(full, `${prefix}${entry.name}/`, out);
    } else if (entry.isFile()) {
      out.push(`${prefix}${entry.name}`);
    }
  }
}

const CRC32_TABLE = new Uint32Array(256);
for (let index = 0; index < CRC32_TABLE.length; index += 1) {
  let value = index;
  for (let bit = 0; bit < 8; bit += 1) {
    value = value & 1 ? (value >>> 1) ^ 0xedb88320 : value >>> 1;
  }
  CRC32_TABLE[index] = value;
}

function crc32(data) {
  let value = 0xffffffff;
  for (const byte of data) {
    value = CRC32_TABLE[(value ^ byte) & 0xff] ^ (value >>> 8);
  }
  return (value ^ 0xffffffff) >>> 0;
}

function createArchive(dist, names) {
  const records = names.map((name) => {
    const data = fs.readFileSync(path.join(dist, name));
    const size = data.length;
    const checksum = crc32(data);
    const compressed = deflateRawSync(data, { level: 9 });
    return { name: Buffer.from(name), size, compressed, crc32: checksum };
  });
  if (records.length > 0xffff) throw new Error("too many files for a ZIP archive");

  const localSize = records.reduce((size, record) => size + 30 + record.name.length + record.compressed.length, 0);
  const centralSize = records.reduce((size, record) => size + 46 + record.name.length, 0);
  const archive = Buffer.alloc(localSize + centralSize + 22);
  const dosDate = 0x2821; // 2000-01-01
  let offset = 0;

  for (const record of records) {
    record.offset = offset;
    archive.writeUInt32LE(0x04034b50, offset);
    archive.writeUInt16LE(20, offset + 4);
    archive.writeUInt16LE(0x0800, offset + 6);
    archive.writeUInt16LE(8, offset + 8);
    archive.writeUInt16LE(0, offset + 10);
    archive.writeUInt16LE(dosDate, offset + 12);
    archive.writeUInt32LE(record.crc32, offset + 14);
    archive.writeUInt32LE(record.compressed.length, offset + 18);
    archive.writeUInt32LE(record.size, offset + 22);
    archive.writeUInt16LE(record.name.length, offset + 26);
    archive.writeUInt16LE(0, offset + 28);
    record.name.copy(archive, offset + 30);
    record.compressed.copy(archive, offset + 30 + record.name.length);
    offset += 30 + record.name.length + record.compressed.length;
  }

  const centralOffset = offset;
  for (const record of records) {
    archive.writeUInt32LE(0x02014b50, offset);
    archive.writeUInt16LE(20, offset + 4);
    archive.writeUInt16LE(20, offset + 6);
    archive.writeUInt16LE(0x0800, offset + 8);
    archive.writeUInt16LE(8, offset + 10);
    archive.writeUInt16LE(0, offset + 12);
    archive.writeUInt16LE(dosDate, offset + 14);
    archive.writeUInt32LE(record.crc32, offset + 16);
    archive.writeUInt32LE(record.compressed.length, offset + 20);
    archive.writeUInt32LE(record.size, offset + 24);
    archive.writeUInt16LE(record.name.length, offset + 28);
    archive.writeUInt16LE(0, offset + 30);
    archive.writeUInt16LE(0, offset + 32);
    archive.writeUInt16LE(0, offset + 34);
    archive.writeUInt16LE(0, offset + 36);
    archive.writeUInt32LE(0, offset + 38);
    archive.writeUInt32LE(record.offset, offset + 42);
    record.name.copy(archive, offset + 46);
    offset += 46 + record.name.length;
  }

  archive.writeUInt32LE(0x06054b50, offset);
  archive.writeUInt16LE(0, offset + 4);
  archive.writeUInt16LE(0, offset + 6);
  archive.writeUInt16LE(records.length, offset + 8);
  archive.writeUInt16LE(records.length, offset + 10);
  archive.writeUInt32LE(centralSize, offset + 12);
  archive.writeUInt32LE(centralOffset, offset + 16);
  archive.writeUInt16LE(0, offset + 20);
  return archive;
}

function packExtension(dir) {
  const meta = readPackage(dir);
  const dist = path.join(dir, "dist");
  if (!fs.statSync(dist, { throwIfNoEntry: false })?.isDirectory()) {
    throw new Error(`${meta.name}: 'dist/' not found — run \`npm run build\` first`);
  }
  // ZIP metadata and compression are generated by Node rather than the
  // host's zip executable so every platform produces the same bytes.
  const names = [];
  collectFiles(dist, "", names);
  const zip = createArchive(dist, names);
  const sha256 = crypto.createHash("sha256").update(zip).digest("hex");
  return { ...meta, zip, sha256, size: zip.length };
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
