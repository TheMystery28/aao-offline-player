import test from "node:test";
import assert from "node:assert/strict";
import { shouldEnableDebug, createLogger, setDebugEnabled, debugLog } from "../src/debug.js";

function spyOnConsoleLog() {
  const calls = [];
  const original = console.log;
  console.log = function (...args) { calls.push(args); };
  return { calls, restore: function () { console.log = original; } };
}

test("shouldEnableDebug: enabled by query flag", () => {
  assert.equal(shouldEnableDebug(true, false), true);
});

test("shouldEnableDebug: enabled by stored flag", () => {
  assert.equal(shouldEnableDebug(false, true), true);
});

test("shouldEnableDebug: disabled when both flags absent", () => {
  assert.equal(shouldEnableDebug(false, false), false);
});

test("createLogger(false) is a silent no-op", () => {
  const log = createLogger(false);
  const spy = spyOnConsoleLog();
  log("anything");
  assert.equal(spy.calls.length, 0);
  spy.restore();
});

test("createLogger(true) writes to console.log", () => {
  const log = createLogger(true);
  const spy = spyOnConsoleLog();
  log("message", 42);
  assert.equal(spy.calls.length, 1);
  assert.deepEqual(spy.calls[0], ["message", 42]);
  spy.restore();
});

test("debugLog is silent by default in Node (no window)", () => {
  const spy = spyOnConsoleLog();
  debugLog("hidden");
  assert.equal(spy.calls.length, 0);
  spy.restore();
});

test("setDebugEnabled(true) makes debugLog write immediately", () => {
  const spy = spyOnConsoleLog();
  setDebugEnabled(true);
  debugLog("visible");
  assert.equal(spy.calls.length, 1);
  assert.deepEqual(spy.calls[0], ["visible"]);
  setDebugEnabled(false);
  debugLog("hidden again");
  assert.equal(spy.calls.length, 1, "toggle off applies without reload");
  spy.restore();
});
