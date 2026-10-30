#!/usr/bin/env node
// Updates the bundled murvix-extension skill (SKILL.md) for both .claude and
// .agents from the canonical copy in the murvix repo.
import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const SOURCE =
  "https://raw.githubusercontent.com/teomurgi/murvix/main/docs/extensions/skills/murvix-extension/SKILL.md";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const targets = [
  join(root, ".claude/skills/murvix-extension/SKILL.md"),
  join(root, ".agents/skills/murvix-extension/SKILL.md"),
];

const res = await fetch(SOURCE);
if (!res.ok) {
  console.error(`Failed to fetch SKILL.md: ${res.status} ${res.statusText}`);
  process.exit(1);
}
const content = await res.text();

for (const target of targets) {
  await mkdir(dirname(target), { recursive: true });
  await writeFile(target, content);
  console.log(`Updated ${target}`);
}
console.log(`Done (${content.length} bytes).`);
