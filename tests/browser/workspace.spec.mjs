import { test, expect } from '@playwright/test';

// @mara checks REQ-BROWSER-LOCAL-WORKSPACE
// @mara checks DES-BROWSER-PROJECT-SNAPSHOT
// @mara checks REQ-EXPLICIT-BACKEND-CAPABILITIES
test('real module worker runs the WASM memory workspace, rules and Unicode', async ({ page, browser }, info) => {
  const browserErrors = [];
  page.on('pageerror', error => browserErrors.push(String(error)));
  await page.goto('http://127.0.0.1:18364/');
  const report = await page.evaluate(async () => {
    const worker = new Worker('./worker.mjs', { type: 'module' });
    let id = 0;
    const pending = new Map();
    const transcript = [];
    const requests = [];
    const assert = (condition, message) => { if (!condition) throw new Error(message); };
    worker.onmessage = ({ data }) => {
      transcript.push(structuredClone(data));
      const handler = pending.get(data.id);
      if (handler) { pending.delete(data.id); handler.resolve(data); }
    };
    worker.onerror = event => {
      for (const handler of pending.values()) handler.reject(new Error(event.message));
      pending.clear();
    };
    const request = (operation, params = {}) => new Promise((resolve, reject) => {
      const next = ++id;
      pending.set(next, { resolve, reject });
      const message = { format_version: 1, id: next, operation, params };
      requests.push(structuredClone(message));
      worker.postMessage(message);
    });
    const call = async (operation, params = {}) => {
      const response = await request(operation, params);
      assert(response.ok, operation + ': ' + JSON.stringify(response.error));
      return response.result;
    };
    try {
      // Requests queued during WASM startup are ordered and leave the page event loop free.
      let pageTick = false;
      setTimeout(() => { pageTick = true; }, 0);
      const [caps, initialized] = await Promise.all([
        call('capabilities'), call('project_init', { name: 'café 中文 👩🏽‍💻', template: 'engineering' }),
      ]);
      assert(pageTick, 'worker computation blocked the page event loop');
      assert(caps.mutation_persistence === 'memory' && !caps.code.indexing &&
        !caps.code.native_index_import && !caps.native_recovery, 'capabilities');
      assert(initialized.created.length === 5, 'engineering initialization');
      const sources = await call('workspace_sources');
      assert(sources.base.files.length === 0, 'initialization advanced base');
      const snapshot = sources.current;
      snapshot.base_identity = 'opaque provenance, not a Git proof';
      snapshot.files.find(f => f.path === '.mara/engineering-rules.yaml').content =
        '- id: rule:draft\n  targetClass: requirement\n  property:\n    - {path: status, hasValue: draft}\n';
      snapshot.files.push({ path: 'checks.yaml', content:
        '- id: rule:status\n  class: requirement\n  property:\n    - {path: status, hasValue: {parameter: expected_status}}\n' });
      snapshot.files.push({ path: 'café/知识.mara.md', content: '# Unicode café 👩🏽‍💻\n\nNarrative.\n' });
      snapshot.selection.document_paths.push('café/知识.mara.md');
      await call('load_snapshot', snapshot);
      assert((await call('workspace_sources')).base.base_identity === snapshot.base_identity, 'new base');
      await call('schema_get');
      await call('schema_list', { kind: 'relation' });
      assert((await call('schema_validate')).valid, 'schema and configured rule definitions');
      const a = await call('item_create', { flavour: 'requirement', id: 'REQ-A',
        file: 'café/知识.mara.md', title: 'Unicode café 👩🏽‍💻',
        fields: [{ key: 'status', value: 'draft' }], body: '中文 café 👩🏽‍💻.' });
      await call('item_create', { flavour: 'requirement', id: 'REQ-B',
        file: 'café/知识.mara.md', title: 'Second', fields: [{ key: 'status', value: 'draft' }],
        body: 'Other source.' });
      assert(/^[0-7][0-9A-HJKMNP-TV-Z]{25}$/.test(a.mid), 'host MID');
      const read = await call('get', { reference: 'REQ-A' });
      assert(read.content.includes('中文 café 👩🏽‍💻'), 'UTF-8 body round trip');
      const search = await call('search', { query: 'café' });
      assert(search.results.length > 0, 'Unicode search');
      const handle = search.results.find(r => r.node.kind === 'section')?.node.reference;
      assert(handle, 'structural discovery handle was not exercised');
      const pageOne = await call('item_list', { limit: 1 });
      assert(pageOne.has_more, 'bounded continuation');
      assert((await call('item_validate', { id: 'REQ-A' })).valid, 'item rules');
      assert((await call('project_validate')).valid, 'project rules pass');
      const trace = await call('trace_matrix', { all: true, rules: ['urn:mara:rule:draft'], limit: 1 });
      assert(trace.evaluation_complete && trace.summaries[0].passed === 2, 'actual configured rules');
      const check = await call('trace_matrix', { ids: ['REQ-A'], check: {
        files: ['checks.yaml'], shape: 'urn:mara:rule:status',
        parameters: { expected_status: 'draft' },
      } });
      assert(check.evaluation_complete && check.summaries[0].passed === 1, 'request-local parameter');
      const before = await call('workspace_sources');
      const rejected = await request('item_update', { reference: 'REQ-A', body: '[[REQ-MISSING]]' });
      assert(!rejected.ok, 'invalid reference accepted');
      assert(JSON.stringify(await call('workspace_sources')) === JSON.stringify(before), 'rejected edit changed state');
      await call('item_list', { limit: 1, cursor: pageOne.next_cursor });
      const edge = { source: 'REQ-A', relation: 'refines', target: 'REQ-B' };
      await call('relation_add', edge);
      const inspection = await call('relation_get', edge);
      await call('related', { reference: 'REQ-A', relations: ['schema:refines'], direction: 'outgoing' });
      await call('relation_remove', { ...edge, occurrence: inspection.occurrences[0].reference });
      await call('item_update', { reference: 'REQ-A', fields: [{ key: 'status', value: 'retired' }] });
      const failedRules = await call('project_validate');
      assert(failedRules.evaluation_complete && !failedRules.valid &&
        failedRules.diagnostics.some(d => d.code === 'rule_failed'), 'rule failure was not computed');
      assert(!(await request('item_list', { limit: 1, cursor: pageOne.next_cursor })).ok, 'stale list cursor');
      assert(!(await request('trace_matrix', { all: true, rules: ['urn:mara:rule:draft'],
        limit: 1, cursor: trace.next_cursor })).ok, 'stale trace cursor');
      assert(!(await request('get', { reference: handle })).ok, 'stale structural handle');
      await call('item_rename', { reference: 'REQ-A', new_id: 'REQ-RENAMED' });
      await call('item_move', { reference: 'REQ-RENAMED', file: 'moved.mara.md' });
      await call('item_delete', { reference: 'REQ-RENAMED' });
      assert((await call('project_validate')).valid, 'post-edit rules');

      const legacy = (await call('workspace_sources')).current;
      legacy.files.push({ path: 'legacy.mara.md', content:
        ':::mara requirement REQ-LEGACY\r\n:title: Legacy café\r\n:status: draft\r\n\r\nBody.\r\n:::\r\n' });
      legacy.selection.document_paths.push('legacy.mara.md');
      await call('load_snapshot', legacy);
      const backfill = await call('project_mid_backfill');
      assert(backfill.changed.length === 1, 'MID backfill');
      assert((await call('project_mid_backfill')).changed.length === 0, 'idempotent backfill');

      const codeSnapshot = (await call('workspace_sources')).current;
      const config = codeSnapshot.files.find(f => f.path === '.mara/project.toml');
      config.content = config.content.replace('format_version = 2', 'format_version = 3') +
        "\n[[code.languages]]\nname='rust'\nextensions=['rs']\ncommand=['must-not-run', '{output}']\n";
      await call('load_snapshot', codeSnapshot);
      const codeBefore = JSON.stringify(await call('workspace_sources'));
      for (const [operation, params] of [
        ['project_init', { name: 'Again' }], ['project_mid_backfill', {}],
        ['item_create', { flavour: 'requirement', id: 'REQ-NEW', file: 'new.mara.md',
          title: 'New', fields: [{ key: 'status', value: 'draft' }], body: 'Body.' }],
        ['item_update', { reference: 'REQ-B', title: 'No' }], ['item_delete', { reference: 'REQ-B' }],
        ['item_move', { reference: 'REQ-B', file: 'new.mara.md' }],
        ['item_rename', { reference: 'REQ-B', new_id: 'REQ-B' }],
        ['relation_add', { source: 'REQ-B', relation: 'refines', target: 'REQ-B' }],
        ['relation_remove', { source: 'REQ-B', relation: 'refines', target: 'REQ-B' }],
      ]) {
        assert(!(await request(operation, params)).ok, operation + ' bypassed code conformance');
        assert(JSON.stringify(await call('workspace_sources')) === codeBefore, operation + ' changed code workspace');
      }
      assert((await call('schema_validate')).valid, 'independent schema check');
      await call('get', { reference: 'REQ-B' });
      const unavailable = await call('project_validate');
      assert(!unavailable.evaluation_complete && !unavailable.valid &&
        unavailable.diagnostics.some(d => d.code === 'evaluation_unavailable'), 'missing code became a pass');
      assert(!(await request('project_transaction_rollback')).ok, 'fake native recovery');

      for (const mutate of [
        s => { s.files[0].path = '../escape'; },
        s => { s.files.push({ ...s.files[0], path: './' + s.files[0].path }); },
        s => { s.files = s.files.filter(f => f.path !== '.mara/schema.yaml'); },
        s => { s.selection.ignores.push({ directory: '', root_prefix: [], content: 'café/\n' }); },
      ]) {
        const invalid = structuredClone(codeSnapshot);
        mutate(invalid);
        assert(!(await request('load_snapshot', invalid)).ok, 'invalid snapshot accepted');
        assert(JSON.stringify(await call('workspace_sources')) === codeBefore, 'failed load lost base or edits');
      }
      const custom = structuredClone(codeSnapshot);
      custom.files.find(f => f.path === '.mara/schema.yaml').path = 'custom.yaml';
      custom.files.find(f => f.path === '.mara/project.toml').content =
        custom.files.find(f => f.path === '.mara/project.toml').content.replace('.mara/schema.yaml', 'custom.yaml') +
        '\ninvalid = [';
      await call('load_snapshot', custom);
      const recovered = await call('project_validate');
      assert(!recovered.valid && !recovered.evaluation_complete &&
        recovered.diagnostics.some(d => d.code === 'project_invalid'), 'custom-schema configuration recovery');
      return { requests, transcript, operations: caps.operations, pageTick, userAgent: navigator.userAgent };
    } finally {
      worker.terminate();
    }
  });
  expect(browserErrors).toEqual([]);
  expect(report.pageTick).toBe(true);
  expect(report.operations).toContain('trace_matrix');
  await info.attach('worker-transcript', {
    body: JSON.stringify({ browserVersion: browser.version(), ...report }, null, 2),
    contentType: 'application/json',
  });
});
