import { test, expect } from '@playwright/test';
import { readFile } from 'node:fs/promises';

// @mara checks REQ-EXPLICIT-QUERY-INPUTS
// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
// @mara checks DES-QUERY-COMPOSITION-CONTRACT
test('supplied facts and derived computations match native in the real WASM worker', async ({ page }, info) => {
  test.skip(!process.env.QUERY_FACTS_SNAPSHOT || !process.env.MARA_QUERY_FACTS_REPORT,
    'External-facts proof is opt-in: supply fixture snapshot and executed native report');
  const snapshot = JSON.parse(await readFile(process.env.QUERY_FACTS_SNAPSHOT, 'utf8'));
  const native = JSON.parse(await readFile(process.env.MARA_QUERY_FACTS_REPORT, 'utf8'));
  const requests = [];
  page.context().on('request', request => requests.push(request.url()));
  await page.goto('/');
  const result = await page.evaluate(async ({ snapshot, cases }) => {
    const worker = new Worker('/browser/worker.mjs', { type: 'module' });
    let id = 0;
    const call = (operation, params) => new Promise((resolve, reject) => {
      const requestId = ++id;
      worker.onmessage = ({ data }) => {
        if (data.id !== requestId || data.format_version !== 1) return reject(new Error('uncorrelated worker response'));
        resolve(data);
      };
      worker.onerror = event => reject(new Error(event.message));
      worker.postMessage({ format_version: 1, id: requestId, operation, params });
    });
    try {
      await call('load_snapshot', snapshot);
      const outcomes = {};
      for (const [name, request] of Object.entries(cases)) {
        const response = await call('query_execute', request);
        outcomes[name] = response.ok
          ? { page: response.result }
          : { error: { code: response.error?.error?.code ?? response.error?.code, message: response.error?.error?.message ?? response.error?.message } };
      }
      return outcomes;
    } finally { worker.terminate(); }
  }, { snapshot, cases: buildCases() });
  for (const [name, outcome] of Object.entries(result)) {
    expect(native[name], `native case ${name}`).toBeTruthy();
    expect(outcome, `case ${name}`).toEqual(native[name]);
  }
  expect(requests.every(url => new URL(url).origin === new URL(page.url()).origin)).toBe(true);
  await info.attach('actual-browser-supplied-facts', { body: Buffer.from(JSON.stringify({ result, requests }, null, 2)), contentType: 'application/json' });
});

function text(value) { return { type: 'text', value }; }
function integer(value) { return { type: 'integer', value }; }
function item(value) { return { type: 'item', value }; }
function key(verification, candidate, context) {
  return { verification: item(verification), candidate: text(candidate), context: text(context) };
}
function record(verification, candidate, context, state) {
  return { keys: key(verification, candidate, context), fields: { state: text(state) } };
}
function reported(candidate, records, coverage) {
  return { input: 'reported', identity: 'reported-run', coverage, records };
}
function coverage(candidate) {
  return [key('VER-V1', candidate, 'release'), key('VER-V2', candidate, 'release'), key('VER-V3', candidate, 'release')];
}
function selection(ids) { return { ids }; }
function validation(candidate, facts) {
  return {
    definition_files: ['facts.json'],
    definition: 'urn:fixture:facts:validation',
    selection: selection([candidate]),
    parameters: [{ name: 'candidate', value: text(candidate) }, { name: 'context', value: text('release') }],
    max_rows: 100, limit: 100,
    fact_snapshots: facts, mappings: [],
  };
}
function composed(candidate, facts) {
  return {
    definition_files: ['facts.json'],
    definition: 'urn:fixture:facts:composed',
    selection: selection([candidate]),
    parameters: [{ name: 'candidate', value: text(candidate) }, { name: 'context', value: text('release') }],
    max_rows: 100, limit: 100,
    fact_snapshots: facts, mappings: [],
  };
}
function verifications(candidate, facts) {
  return {
    definition_files: ['facts.json'],
    definition: 'urn:fixture:facts:verifications',
    selection: selection([candidate]),
    parameters: [{ name: 'candidate', value: text(candidate) }, { name: 'context', value: text('release') }],
    max_rows: 100, limit: 100,
    fact_snapshots: facts, mappings: [],
  };
}
function riskRequest() {
  return {
    definition_files: ['facts.json'],
    definition: 'urn:fixture:facts:risk',
    selection: selection(['CAND-C', 'CAND-E']),
    parameters: [], max_rows: 100, limit: 100, fact_snapshots: [],
    mappings: [
      { mapping: 'risk_matrix', entries: [
        { keys: { likelihood: integer(1), consequence: integer(3) }, value: integer(3) },
        { keys: { likelihood: integer(2), consequence: integer(3) }, value: integer(6) },
      ] },
      { mapping: 'risk_band', entries: [
        { keys: { score: integer(3) }, value: text('below') },
        { keys: { score: integer(6) }, value: text('above') },
      ] },
    ],
  };
}
function buildCases() {
  const complete = reported('CAND-C', [
    record('VER-V1', 'CAND-C', 'release', 'pass'),
    record('VER-V2', 'CAND-C', 'release', 'pass'),
    record('VER-V3', 'CAND-C', 'release', 'fail'),
  ], coverage('CAND-C'));
  const missing = reported('CAND-C', [
    record('VER-V1', 'CAND-C', 'release', 'pass'),
    record('VER-V3', 'CAND-C', 'release', 'pass'),
  ], coverage('CAND-C'));
  const partial = reported('CAND-C', [record('VER-V1', 'CAND-C', 'release', 'pass')], [key('VER-V1', 'CAND-C', 'release')]);
  return {
    verifications_complete: verifications('CAND-C', [complete]),
    verifications_missing: verifications('CAND-C', [missing]),
    verifications_partial: verifications('CAND-C', [partial]),
    verifications_absent: verifications('CAND-C', []),
    validation_aggregate: validation('CAND-C', [complete]),
    validation_empty: validation('CAND-EMPTY', [reported('CAND-EMPTY', [], [])]),
    risk_matrix: riskRequest(),
    composed_provenance: composed('CAND-C', [complete]),
  };
}
