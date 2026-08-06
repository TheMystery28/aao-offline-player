/**
 * Debug logging gate for the launcher UI.
 *
 * Logging is off by default and enabled by:
 *   - `?debug=1` in the URL, or
 *   - `aao_debug` set in localStorage, or
 *   - `setDebugEnabled(true)` at runtime (future settings UI — applies
 *     immediately, no reload needed).
 *
 * Pure functions are exported for node --test; `debugLog` is safe to import
 * in Node (where `window` is undefined → disabled logger).
 */

const DEBUG_QUERY_RE = /[?&]debug=1([&#]|$)/;
const DEBUG_STORAGE_KEY = "aao_debug";

let enabled = false;

export function shouldEnableDebug(hasQueryFlag, hasStoredFlag) {
  return Boolean(hasQueryFlag) || Boolean(hasStoredFlag);
}

export function createLogger(isEnabled) {
  if (!isEnabled) {
    return function () {};
  }
  return function (...args) {
    console.log.apply(console, args);
  };
}

/** Enable or disable all launcher debug logging at runtime. */
export function setDebugEnabled(value) {
  enabled = Boolean(value);
}

function hasQueryFlag(search) {
  return DEBUG_QUERY_RE.test(search || "");
}

function hasStoredFlag(getItem) {
  try {
    return Boolean(getItem && getItem(DEBUG_STORAGE_KEY));
  } catch (e) {
    return false;
  }
}

function readFlags() {
  if (typeof window === "undefined") {
    return { hasQueryFlag: false, hasStoredFlag: false };
  }
  return {
    hasQueryFlag: hasQueryFlag(window.location.search),
    hasStoredFlag: hasStoredFlag(
      typeof window.localStorage === "object"
        ? window.localStorage.getItem.bind(window.localStorage)
        : null
    )
  };
}

(function init() {
  const flags = readFlags();
  enabled = shouldEnableDebug(flags.hasQueryFlag, flags.hasStoredFlag);
})();

/**
 * Module-level logger. Consults the LIVE flag at call time, so
 * setDebugEnabled() toggles apply immediately without a reload.
 */
export function debugLog(...args) {
  if (enabled) {
    console.log.apply(console, args);
  }
}
