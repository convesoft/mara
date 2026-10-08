import { test, expect } from '@playwright/test';
import { readFile } from 'node:fs/promises';

// @mara checks DES-QUERY-POLICY-ENGINE
// @mara checks DES-QUERY-INPUT-RESULT-CONTRACT
// @mara checks DES-QUERY-CONSUMER-CONTRACT
test('one standard query runs directly and through SHACL in the real WASM worker', async ({ page }, info) => {
  test.skip(!process.env.QUERY_PROOF_SNAPSHOT || !process.env.MARA_QUERY_PROOF_REPORT,
    'Bounded proof is opt-in: supply fixture snapshot and executed native report');
  const snapshot = JSON.parse(await readFile(process.env.QUERY_PROOF_SNAPSHOT, 'utf8'));
  const native = JSON.parse(await readFile(process.env.MARA_QUERY_PROOF_REPORT, 'utf8'));
  const requests = [];
  page.context().on('request', request => requests.push(request.url()));
  await page.goto('/');
  const result = await page.evaluate(async snapshot => {
    const worker = new Worker('/browser/worker.mjs', { type: 'module' });
    let id = 0;
    const call = (operation, params) => new Promise((resolve, reject) => {
      const requestId = ++id;
      worker.onmessage = ({ data }) => {
        if (data.id !== requestId || data.format_version !== 1) return reject(new Error('uncorrelated worker response'));
        if (data.ok) resolve(data.result);
        else reject(new Error(JSON.stringify(data.error)));
      };
      worker.onerror = event => reject(new Error(event.message));
      worker.postMessage({ format_version: 1, id: requestId, operation, params });
    });
    const query = { definition_file: 'supports.query.json', roots: ['SUB-LINKED', 'SUB-EMPTY', '01ARZ3NDEKTSV4RRFFQ69G5F00'], max_rows: 100 };
    const policy = query => ({ query, shape_file: 'coverage.ttl', shape: 'urn:fixture:policy:coverage' });
    try {
      await call('load_snapshot', snapshot);
      const raw = await call('query_backend_execute', query);
      const judged = await call('query_backend_policy', policy(query));
      const limited = await call('query_backend_policy', policy({ ...query, max_rows: 1 }));
      const empty = await call('query_backend_execute', { ...query, roots: [] });
      const rejected = [];
      for (const select of [
        'SELECT ?root ?support ?lifecycle ?amount ?text ?flag ?edge WHERE { SERVICE SILENT <http://127.0.0.1:1/> { ?root ?p ?support } }',
        'SELECT ?root ?support ?lifecycle ?amount ?text ?flag ?edge WHERE { FILTER(1 = 1.0) }',
      ]) {
        const fixture = structuredClone(snapshot);
        const file = fixture.files.find(f => f.path === 'supports.query.json');
        const definition = JSON.parse(file.content);
        definition.select = select;
        file.content = JSON.stringify(definition);
        await call('load_snapshot', fixture);
        try { await call('query_backend_execute', query); rejected.push(false); }
        catch { rejected.push(true); }
      }
      const regressions = {};
      const cases = JSON.parse(snapshot.files.find(f => f.path === 'regressions.json').content);
      for (const variant of cases) {
        const fixture = structuredClone(snapshot);
        fixture.files.find(f => f.path === 'supports.query.json').content = variant.definition_text;
        if (variant.shape_text) fixture.files.find(f => f.path === 'coverage.ttl').content = variant.shape_text;
        await call('load_snapshot', fixture);
        if (variant.reject) {
          const outcomes = [];
          for (const roots of [['SUB-LINKED'], []]) {
            try { await call('query_backend_execute', { ...query, roots }); outcomes.push(false); }
            catch { outcomes.push(true); }
          }
          regressions[variant.name] = { rejected: outcomes };
        } else {
          const evaluation = await call('query_backend_policy', policy(query));
          const repeated = await call('query_backend_policy', policy(query));
          regressions[variant.name] = { evaluation, repeated_identical: JSON.stringify(evaluation) === JSON.stringify(repeated) };
        }
      }
      await call('load_snapshot', snapshot);
      const atMax = await call('query_backend_execute', { ...query, max_rows: 4294967295 });
      let aboveMaxRejected = false;
      try { await call('query_backend_execute', { ...query, max_rows: 4294967296 }); }
      catch { aboveMaxRejected = true; }
      regressions.portable_budget = { at_max: atMax, above_max_rejected: aboveMaxRejected };
      return { raw, policy: judged, limited, empty, rejected, regressions };
    } finally { worker.terminate(); }
  }, snapshot);
  expect(result.raw).toEqual(native.raw);
  expect(result.policy).toEqual(native.policy);
  expect(result.limited).toEqual(native.limited);
  expect(result.regressions).toEqual(native.regressions);
  expect(result.regressions.supported_optional_absence.repeated_identical).toBe(true);
  expect(result.regressions.supported_optional_absence.evaluation.facts.roots[0].bindings).toHaveLength(1);
  for (const [name, outcome] of Object.entries(result.regressions)) {
    if (outcome.rejected) expect(outcome.rejected).toEqual([true, true]);
  }
  expect(result.regressions.row_identity_collision.evaluation.checks.every(c => c.state === 'failed' && c.counts.some(n => n.total === 0))).toBe(true);
  expect(result.regressions.portable_budget.above_max_rejected).toBe(true);
  expect(result.policy.facts).toEqual(result.raw);
  expect(result.limited.evaluation_complete).toBe(false);
  expect(result.limited.checks.every(c => c.state === 'unavailable' && c.counts.length === 0)).toBe(true);
  expect(result.empty.evaluation_complete).toBe(true);
  expect(result.empty.roots).toEqual([]);
  expect(result.rejected).toEqual([true, true]);
  expect(requests.every(url => new URL(url).origin === new URL(page.url()).origin)).toBe(true);
  await info.attach('actual-browser-query-and-shacl', { body: Buffer.from(JSON.stringify({ result, requests }, null, 2)), contentType: 'application/json' });
});
