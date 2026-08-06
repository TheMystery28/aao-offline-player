/**
 * Set-backed container for known case IDs.
 * Pure module (no DOM) so it is unit-testable with node --test.
 */
export function createKnownIds() {
  const ids = new Set();
  return {
    /** @param {number} id */
    add: function (id) { ids.add(id); },
    /** @param {number} id @returns {boolean} */
    has: function (id) { return ids.has(id); },
    clear: function () { ids.clear(); },
    /** @returns {number} */
    size: function () { return ids.size; }
  };
}
