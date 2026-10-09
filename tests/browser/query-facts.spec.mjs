import { test, expect } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import path from 'node:path';

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

test('composed supplied facts reach worker views, policy, gates and reports', async ({ page }) => {
  const snapshot = JSON.parse(await readFile(path.resolve('tests/fixtures/query-facts/snapshot.json'), 'utf8'));
  const project = snapshot.files.find(file => file.path === '.mara/project.toml');
  project.content += `\n[query_policy]\nformat_version = 1\nfiles = ["facts-invariants.yaml"]\ndefinition_files = ["facts.json"]\nshape_files = ["facts-present.ttl"]\nmax_rows = 100\n`;
  snapshot.files.push(
    { path: 'facts-invariants.yaml', content: `format_version: 1\ninvariants:\n  - id: supplied-composed-facts\n    definition: urn:fixture:facts:composed\n    shape: urn:fixture:shape:has-fact-row\n    flavours: [candidate]\n    severity: error\n    parameters:\n      - name: candidate\n        value: {type: text, value: CAND-C}\n      - name: context\n        value: {type: text, value: release}\n` },
    { path: 'facts-gates.yaml', content: `format_version: 1\ngates:\n  - id: require-composed-facts\n    empty_roots: not_applicable\n    empty_required: failed\n    conditions:\n      - id: fact-row\n        definition: urn:fixture:facts:composed\n        shape_file: facts-present.ttl\n        shape: urn:fixture:shape:has-fact-row\n        severity: error\n        context_parameters: [candidate, context]\n` },
    { path: 'facts-present.ttl', content: `@prefix sh: <http://www.w3.org/ns/shacl#>.\n<urn:fixture:shape:has-fact-row> a sh:NodeShape; sh:property [ sh:path <urn:mara:query:match>; sh:minCount 1 ].\n` },
  );
  const key = verification => ({ verification: item(verification), candidate: text('CAND-C'), context: text('release') });
  const facts = reported('CAND-C', [
    record('VER-V1', 'CAND-C', 'release', 'pass'),
    record('VER-V2', 'CAND-C', 'release', 'pass'),
    record('VER-V3', 'CAND-C', 'release', 'fail'),
  ], ['VER-V1', 'VER-V2', 'VER-V3'].map(key));
  const request = composed('CAND-C', [facts]);
  const gate = {
    definition_files: ['facts.json'], gate_file: 'facts-gates.yaml', shape_files: ['facts-present.ttl'],
    gate: 'require-composed-facts', scope: selection(['CAND-C']),
    context: request.parameters, fact_snapshots: [facts], mappings: [], max_rows: 100,
  };
  await page.goto('/');
  const results = await page.evaluate(async ({ snapshot, request, gate, facts }) => {
    const worker = new Worker('/browser/worker.mjs', { type: 'module' });
    let id = 0;
    const call = (operation, params) => new Promise((resolve, reject) => {
      const requestId = ++id;
      worker.onmessage = ({ data }) => {
        if (data.id !== requestId || data.format_version !== 1) return reject(new Error('uncorrelated worker response'));
        if (!data.ok) return reject(new Error(data.error?.error?.message ?? 'worker operation failed'));
        resolve(data.result);
      };
      worker.onerror = event => reject(new Error(event.message));
      worker.postMessage({ format_version: 1, id: requestId, operation, params });
    });
    try {
      await call('load_snapshot', snapshot);
      const page = await call('query_execute', request);
      const view = await call('query_view', { request, render: null });
      const validation = await call('item_validate', { id: 'CAND-C', fact_snapshots: [facts], mappings: [] });
      const gateResult = await call('query_gate', gate);
      const gateReport = await call('query_report', { subject: { kind: 'gate', ...gate }, render: null });
      const ruleReport = await call('query_report', {
        subject: { kind: 'documentation_rules', target: { kind: 'item', id: 'CAND-C' },
          fact_snapshots: [facts], mappings: [] },
        render: null,
      });
      return { page, view, validation, gateResult, gateReport, ruleReport };
    } finally { worker.terminate(); }
  }, { snapshot, request, gate, facts });
  expect(results.page.evaluation_complete).toBe(true);
  expect(results.page.summary.binding_count).toBe(3);
  expect(results.view.input_identity).toBe(results.page.input_identity);
  expect(results.view.summary.binding_count).toBe(3);
  expect(results.validation.valid).toBe(true);
  expect(results.validation.query_policies.checks[0].state).toBe('passed');
  expect(results.gateResult.state).toBe('passed');
  expect(results.gateReport.checks[0].state).toBe('passed');
  expect(results.ruleReport.checks[0].state).toBe('passed');

  const typed = await page.evaluate(async ({ snapshot, request, gate, facts }) => {
    const { createWasmClient, MaraClientError } = await import('./browser.mjs');
    snapshot.files.push({ path: 'invalid-facts.json', content: 'not valid JSON' });
    const client = await createWasmClient({ snapshot });
    try {
      const page = await client.queryExecute(request);
      const view = await client.queryView({ request, render: null });
      const validation = await client.itemValidate({ id: 'CAND-C', fact_snapshots: [facts], mappings: [] });
      const gateResult = await client.queryGate(gate);
      const gateReport = await client.queryReport({ subject: { kind: 'gate', ...gate }, render: null });
      const ruleReport = await client.queryReport({ subject: {
        kind: 'documentation_rules', target: { kind: 'item', id: 'CAND-C' },
        fact_snapshots: [facts], mappings: [],
      }, render: null });
      const invalidRequest = { ...request, definition_files: ['facts.json', 'invalid-facts.json'] };
      const sourceError = async operation => {
        try { await operation; throw new Error('invalid definition unexpectedly succeeded'); }
        catch (error) {
          if (!(error instanceof MaraClientError)) throw error;
          return { code: error.code, source: error.envelope.error.source };
        }
      };
      const queryError = await sourceError(client.queryExecute(invalidRequest));
      const viewError = await sourceError(client.queryView({ request: invalidRequest, render: null }));
      return { page, view, validation, gateResult, gateReport, ruleReport, queryError, viewError };
    } finally { await client.close(); }
  }, { snapshot, request, gate, facts });
  expect(typed.page.evaluation_complete).toBe(true);
  expect(typed.page.summary.binding_count).toBe(3);
  expect(typed.view.input_identity).toBe(typed.page.input_identity);
  expect(typed.validation.query_policies.checks[0].state).toBe('passed');
  expect(typed.gateResult.state).toBe('passed');
  expect(typed.gateReport.checks[0].state).toBe('passed');
  expect(typed.gateReport.diagnostics).toBeUndefined();
  expect(typed.ruleReport.checks[0].state).toBe('passed');
  expect(typed.ruleReport.roots).toBeUndefined();
  expect(typed.ruleReport.diagnostics).toBeUndefined();
  expect(typed.view.next_cursor).toBeUndefined();
  expect(typed.view.unavailable_reason).toBeUndefined();
  expect(typed.queryError).toEqual({ code: 'invalid_definition', source: 'invalid-facts.json' });
  expect(typed.viewError).toEqual({ code: 'invalid_definition', source: 'invalid-facts.json' });
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
