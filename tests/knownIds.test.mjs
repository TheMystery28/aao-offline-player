import test from "node:test";
import assert from "node:assert/strict";
import { createKnownIds } from "../src/knownIds.js";

test("add/has roundtrip", () => {
  const ids = createKnownIds();
  ids.add(42);
  assert.equal(ids.has(42), true);
  assert.equal(ids.has(43), false);
});

test("duplicate add is idempotent", () => {
  const ids = createKnownIds();
  ids.add(1);
  ids.add(1);
  assert.equal(ids.size(), 1);
});

test("clear empties the set", () => {
  const ids = createKnownIds();
  ids.add(7);
  ids.clear();
  assert.equal(ids.size(), 0);
  assert.equal(ids.has(7), false);
});
