import test from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

// Scope: launcher only. engine/ is the upstream AAO player copy — out of scope.
const SRC_DIR = fileURLToPath(new URL("../src/", import.meta.url));
const LOG_RE = /console\.log\s*\(/;

function collectJsFiles(dir, out) {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      collectJsFiles(full, out);
    } else if (entry.endsWith(".js")) {
      out.push(full);
    }
  }
  return out;
}

test("no console.log calls remain in the launcher (src/)", () => {
  const offenders = [];
  for (const file of collectJsFiles(SRC_DIR, [])) {
    const lines = readFileSync(file, "utf8").split("\n");
    lines.forEach(function (line, i) {
      if (LOG_RE.test(line)) {
        offenders.push(file + ":" + (i + 1));
      }
    });
  }
  assert.deepEqual(
    offenders,
    [],
    "gate these sites with debugLog() or they stay silent-unless-debug:\n" + offenders.join("\n")
  );
});
