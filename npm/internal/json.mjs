// Copy owning JSON without executing toJSON or lossy stringify conversions.
export function snapshotJson(value) {
  // Copy JSON data without letting stringify/toJSON turn invalid inputs into valid mutations.
  // Undefined optional object properties mean omission; array entries must be JSON values.
  const active = new Set();
  const copy = entry => {
    if (entry === null || typeof entry === 'string' || typeof entry === 'boolean') return entry;
    if (typeof entry === 'number' && Number.isFinite(entry)) return entry;
    if (typeof entry !== 'object' || active.has(entry)) throw new Error('Not JSON data');
    const prototype = Object.getPrototypeOf(entry);
    if (!Array.isArray(entry) && prototype !== null && Object.getPrototypeOf(prototype) !== null) {
      throw new Error('Not a JSON object');
    }
    if (Object.getOwnPropertySymbols(entry).some(key => Object.prototype.propertyIsEnumerable.call(entry, key))) {
      throw new Error('Not a JSON key');
    }
    active.add(entry);
    const result = Array.isArray(entry) ? [] : Object.fromEntries(
      Object.entries(entry).filter(([, child]) => child !== undefined).map(([key, child]) => [key, copy(child)]));
    if (Array.isArray(entry)) {
      const length = entry.length;
      for (let index = 0; index < length; index++) result.push(copy(entry[index]));
    }
    active.delete(entry);
    return result;
  };
  return copy(value);
}
