import assert from 'node:assert/strict';
import test, { after } from 'node:test';
import { chmod, mkdir, readFile, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

assert.equal(Number(process.versions.node.split('.')[0]), 24, 'certify the actual Node 24 baseline');
const publicFacade = Boolean(process.env.MARA_NODE_CLIENT);
const privateModule = publicFacade ? null : await import(pathToFileURL(process.env.MARA_NODE_LOADER));
// Installed proof uses bare public exports and default optional-package resolution.
// Temporarily make its bundled CLI non-executable: addon workflows must still pass.
let publicModule = null;
if (publicFacade) {
  if (process.env.MARA_NODE_INSTALLED) {
    const packageRoot = path.dirname(process.env.MARA_NODE_CLIENT);
    const consumer = path.resolve(packageRoot, '../../..', 'installed-node-proof.mjs');
    await writeFile(consumer, "export * from '@convesoft/mara'; export { createNativeClient as nodeFactory } from '@convesoft/mara/node';\n");
    publicModule = await import(pathToFileURL(consumer));
    assert.equal(publicModule.createNativeClient, publicModule.nodeFactory);
    const names = new Map([['linux:x64', 'mara-linux-x64-gnu'], ['linux:arm64', 'mara-linux-arm64-gnu'], ['darwin:arm64', 'mara-darwin-arm64']]);
    const nativeRoot = path.join(packageRoot, '..', names.get(`${process.platform}:${process.arch}`));
    const cli = path.join(nativeRoot, 'bin/mara');
    const originalMode = (await stat(cli)).mode;
    await chmod(cli, 0o600);
    after(async () => { await chmod(cli, originalMode); });
    assert.equal((await stat(cli)).mode & 0o111, 0, 'installed CLI cannot execute during native workflows');
    console.log('Installed default addon proof with non-executable bundled CLI: ' + nativeRoot);
  } else {
    publicModule = await import(pathToFileURL(process.env.MARA_NODE_CLIENT));
  }
}
const NativeClientError = publicModule?.MaraClientError ?? privateModule.NativeClientError;
const createClient = publicFacade ? async options => {
  const client = await publicModule.createNativeClient({ ...options, ...(process.env.MARA_NODE_INSTALLED ? {} : { addonPath: process.env.MARA_NODE_ADDON }) });
  assert.equal(client.context.backend, 'native');
  assert.equal(client.context.selection.callerDirectory, options.callerDirectory);
  assert.ok(Object.isFrozen(client.capabilities.code));
  return {
    execute(operation, params = {}) {
      if (operation === 'capabilities') return Promise.resolve(client.capabilities);
      const method = operation.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase());
      if (!client[method]) return Promise.reject(new NativeClientError('invalid_argument', {
        format_version: 1, error: { code: 'invalid_argument', message: 'Unknown test operation' },
      }));
      return client[method](params);
    },
    close: () => client.close(),
  };
} : privateModule.loadAddon(process.env.MARA_NODE_ADDON);
const base = process.env.MARA_NODE_FIXTURES;
const daemonDirectory = process.env.MARA_NODE_DAEMON;
const covered = new Set();
const rule = '- id: rule:draft\n  targetClass: requirement\n  property:\n    - {path: status, hasValue: draft}\n';
const check = '- id: rule:status\n  class: requirement\n  property:\n    - {path: status, hasValue: {parameter: expected_status}}\n';

async function installQueryFactsProject(root) {
  const snapshot = JSON.parse(await readFile(new URL('../fixtures/query-facts/snapshot.json', import.meta.url), 'utf8'));
  for (const file of snapshot.files) {
    const target = path.join(root, file.path);
    await mkdir(path.dirname(target), { recursive: true });
    await writeFile(target, file.content);
  }
  const config = path.join(root, '.mara/project.toml');
  await writeFile(config, (await readFile(config, 'utf8')) + `\n[query_policy]\nformat_version = 1\nfiles = ["facts-invariants.yaml"]\ndefinition_files = ["facts.json"]\nshape_files = ["facts-present.ttl"]\nmax_rows = 100\n`);
  await writeFile(path.join(root, 'facts-invariants.yaml'), `format_version: 1\ninvariants:\n  - id: supplied-composed-facts\n    definition: urn:fixture:facts:composed\n    shape: urn:fixture:shape:has-fact-row\n    flavours: [candidate]\n    severity: error\n    parameters:\n      - name: candidate\n        value: {type: text, value: CAND-C}\n      - name: context\n        value: {type: text, value: release}\n`);
  await writeFile(path.join(root, 'facts-gates.yaml'), `format_version: 1\ngates:\n  - id: require-composed-facts\n    empty_roots: not_applicable\n    empty_required: failed\n    conditions:\n      - id: fact-row\n        definition: urn:fixture:facts:composed\n        shape_file: facts-present.ttl\n        shape: urn:fixture:shape:has-fact-row\n        severity: error\n        context_parameters: [candidate, context]\n`);
  await writeFile(path.join(root, 'facts-present.ttl'), `@prefix sh: <http://www.w3.org/ns/shacl#>.\n<urn:fixture:shape:has-fact-row> a sh:NodeShape; sh:property [ sh:path <urn:mara:query:match>; sh:minCount 1 ].\n`);
}

function queryFactSnapshot() {
  const key = verification => ({
    verification: { type: 'item', value: verification },
    candidate: { type: 'text', value: 'CAND-C' }, context: { type: 'text', value: 'release' },
  });
  return {
    input: 'reported', identity: 'reported-run', coverage: ['VER-V1', 'VER-V2', 'VER-V3'].map(key),
    records: [['VER-V1', 'pass'], ['VER-V2', 'pass'], ['VER-V3', 'fail']].map(([verification, state]) => ({
      keys: key(verification), fields: { state: { type: 'text', value: state } },
    })),
  };
}

// @mara checks REQ-NODE-NATIVE-BINDING
// @mara checks REQ-EXPLICIT-BACKEND-CAPABILITIES
for (const mode of ['direct', undefined, 'daemon-required']) {
  test(`real document workflow: ${mode ?? 'default-auto'}`, async () => {
    const root = path.join(base, mode ?? 'auto');
    const client = await createClient({ callerDirectory: base, project: path.basename(root), mode, daemonDirectory });
    const call = async (operation, params = {}) => {
      covered.add(operation);
      return client.execute(operation, params);
    };
    try {
      const caps = await call('capabilities');
      assert.equal(caps.mutation_persistence, 'filesystem');
      assert.equal(caps.code.indexing, true);
      const initialized = await call('project_init', { template: 'engineering' });
      assert.equal(initialized.project.root, root);
      assert.equal(initialized.created.length, 5); // Every installed engineering source is reported.
      assert.ok(await readFile(path.join(root, '.mara/engineering-checks.yaml'), 'utf8'));
      assert.ok(await readFile(path.join(root, '.mara/engineering-execution.yaml'), 'utf8'));
      await writeFile(path.join(root, '.mara/engineering-rules.yaml'), rule);
      await writeFile(path.join(root, 'checks.yaml'), check);
      await mkdir(path.join(root, 'docs'));
      await writeFile(path.join(root, 'docs/a.mara.md'), '# Unicode café 👩🏽‍💻\n\nNarrative.\n');
      await call('schema_get');
      await call('schema_list', { kind: 'relation' });
      assert.equal((await call('schema_validate')).valid, true);
      const create = id => call('item_create', { flavour: 'requirement', id,
        file: 'docs/a.mara.md', title: 'café 中文 👩🏽‍💻', fields: [{ key: 'status', value: 'draft' }],
        body: '中文 café 👩🏽‍💻.' });
      const a = await create('REQ-A');
      await create('REQ-B');
      const read = await call('get', { reference: 'REQ-A' });
      assert.equal(read.format_version, 2);
      assert.match(read.content, /中文 café/);
      assert.ok((await call('search', { query: 'café' })).results.length);
      await call('related', { reference: 'REQ-A' });
      const first = await call('item_list', { limit: 1 });
      assert.equal(first.has_more, true);
      const second = await call('item_list', { limit: 1, cursor: first.next_cursor });
      assert.equal(second.has_more, false);
      assert.notEqual(first.items[0].id, second.items[0].id);
      assert.equal((await call('item_validate', { id: 'REQ-A' })).valid, true);
      assert.equal((await call('project_validate')).valid, true);
      const trace = await call('trace_matrix', { all: true, rules: ['urn:mara:rule:draft'], limit: 1 });
      assert.equal(trace.evaluation_complete, true);
      assert.equal(trace.summaries[0].passed, 2);
      assert.equal(trace.has_more, true);
      let tracePage = trace;
      let traceRecords = trace.records.length;
      while (tracePage.has_more) {
        tracePage = await call('trace_matrix', { all: true, rules: ['urn:mara:rule:draft'], limit: 1, cursor: tracePage.next_cursor });
        assert.equal(tracePage.evaluation_complete, true);
        assert.deepEqual(tracePage.summaries, trace.summaries);
        traceRecords += tracePage.records.length;
      }
      assert.ok(traceRecords > 2, 'consume the complete real trace, not only its second page');
      const local = { ids: ['REQ-A'], check: { files: ['checks.yaml'], shape: 'urn:mara:rule:status',
        parameters: { expected_status: 'draft' } } };
      assert.equal((await call('trace_matrix', local)).summaries[0].passed, 1);
      local.check.parameters.expected_status = 'accepted';
      const failed = await call('trace_matrix', local);
      assert.equal(failed.evaluation_complete, true);
      assert.equal(failed.summaries[0].failed, 1);
      // Result literals remain JSON numbers, including values above 32-bit ranges.
      for (const integer of [1700000000000, -1700000000000, Number.MAX_SAFE_INTEGER]) {
        await writeFile(path.join(root, 'numeric.yaml'),
          `- id: rule:numeric\n  class: requirement\n  property:\n    - {path: status, hasValue: {value: ${integer}, datatype: integer}}\n`);
        const numeric = await call('trace_matrix', { ids: ['REQ-A'],
          check: { files: ['numeric.yaml'], shape: 'urn:mara:rule:numeric' } });
        const literal = numeric.records.find(record =>
          record.kind === 'check' && record.condition?.components?.hasValue)?.condition.components.hasValue.value;
        assert.equal(typeof literal, 'number');
        assert.equal(literal, integer);
        assert.deepEqual(JSON.parse(JSON.stringify(numeric)), numeric);
      }
      const before = await readFile(path.join(root, 'docs/a.mara.md'), 'utf8');
      await assert.rejects(call('item_update', { reference: 'REQ-A', body: '[[REQ-MISSING]]' }),
        error => error instanceof NativeClientError && error.kind === 'operation' &&
          error.envelope.format_version === 1);
      assert.equal(await readFile(path.join(root, 'docs/a.mara.md'), 'utf8'), before);
      await call('item_update', { reference: 'REQ-A', title: 'Updated' });
      await assert.rejects(call('item_list', { limit: 1, cursor: first.next_cursor }), NativeClientError);
      await call('item_rename', { reference: 'REQ-A', new_id: 'REQ-RENAMED' });
      assert.equal((await call('get', { reference: 'REQ-RENAMED' })).node.mid, a.mid);
      if (publicFacade) {
        const original = await readFile(path.join(root, 'docs/a.mara.md'), 'utf8');
        class IndexedFields extends Array { *[Symbol.iterator]() {} }
        const invalidFields = new IndexedFields({ key: 'status', value: () => 'draft' });
        await assert.rejects(call('item_update', { reference: 'REQ-RENAMED',
          title: 'Iterator must not change canonical source', fields: invalidFields }),
        error => error instanceof NativeClientError && error.kind === 'invalid_argument');
        assert.equal(await readFile(path.join(root, 'docs/a.mara.md'), 'utf8'), original);
        // JSON arrays use indexed entries even when iteration would omit them.
        await call('item_update', { reference: 'REQ-RENAMED',
          fields: new IndexedFields({ key: 'status', value: 'retired' }) });
        assert.equal((await call('get', { reference: 'REQ-RENAMED' })).metadata.find(f => f.key === 'status').value, 'retired');
        await call('item_update', { reference: 'REQ-RENAMED', fields: [{ key: 'status', value: 'draft' }] });
        for (const invalidPosition of [NaN, Infinity, -Infinity, () => 1, Symbol('position'), { toJSON: () => null }]) {
          await assert.rejects(call('item_move', { reference: 'REQ-RENAMED', file: 'docs/nonfinite.mara.md', line: invalidPosition }),
            error => error instanceof NativeClientError && error.kind === 'invalid_argument');
        }
        for (const line of [() => 1, Symbol('position')]) {
          await assert.rejects(call('item_create', { flavour: 'requirement', id: 'REQ-INVALID',
            file: 'docs/a.mara.md', title: 'Must not be created', fields: [{ key: 'status', value: 'draft' }], line }),
          error => error instanceof NativeClientError && error.kind === 'invalid_argument');
        }
        assert.equal(await readFile(path.join(root, 'docs/a.mara.md'), 'utf8'), original);
        await assert.rejects(readFile(path.join(root, 'docs/nonfinite.mara.md')), { code: 'ENOENT' });
      }
      await call('item_move', { reference: 'REQ-RENAMED', file: 'docs/b.mara.md' });
      const edge = { source: 'REQ-RENAMED', relation: 'depends_on', target: 'REQ-B' };
      await call('relation_add', edge);
      const inspection = await call('relation_get', edge);
      assert.equal(inspection.format_version, 1);
      await call('relation_remove', { ...edge, occurrence: inspection.occurrences[0].reference });
      await call('item_delete', { reference: 'REQ-B' });
      await call('project_mid_backfill');
      await call('project_transaction_rollback');
      await call('project_refresh');
      // Invalid validation is a returned result, never a rejected operation.
      await writeFile(path.join(root, '.mara/engineering-rules.yaml'), rule.replace('draft}', 'accepted}'));
      const invalid = await call('project_validate', { limit: 1 });
      assert.equal(invalid.valid, false);
      assert.equal(invalid.evaluation_complete, true);
      assert.ok(invalid.summary.errors > 0);
      let validationPage = invalid;
      while (validationPage.has_more) {
        validationPage = await call('project_validate', { limit: 1, cursor: validationPage.next_cursor });
        assert.deepEqual(validationPage.summary, invalid.summary);
        assert.equal(validationPage.evaluation_complete, true);
      }
      await assert.rejects(call('project_validate', { limit: 0 }), error =>
        error instanceof NativeClientError && error.code === 'invalid_argument');
      // Restore configured rule, then exercise consecutive content with a real large read.
      await writeFile(path.join(root, '.mara/engineering-rules.yaml'), rule);
      await call('item_update', { reference: 'REQ-RENAMED', body: 'café 中文 👩🏽‍💻 '.repeat(18000) });
      let ticks = 0;
      const timer = setInterval(() => ticks++, 1);
      let page;
      try { page = await call('get', { reference: 'REQ-RENAMED' }); } finally { clearInterval(timer); }
      assert.ok(ticks > 0, `event loop did not advance during real native read: ${ticks}`);
      assert.equal(page.has_more, true);
      let bytes = Buffer.byteLength(page.content);
      while (page.has_more) {
        page = await call('get', { reference: 'REQ-RENAMED', cursor: page.next_cursor });
        bytes += Buffer.byteLength(page.content);
      }
      assert.ok(bytes > 300000);
      console.log(JSON.stringify({ mode: mode ?? 'default-auto', eventLoopTicks: ticks,
        resultFormats: caps.result_formats, nativeVersion: caps.application_version, node: process.version }));
      // Pending calls finish before close; repeated close and after-close are explicit.
      const pending = call('schema_get');
      const closing = client.close();
      await pending;
      await closing;
      await client.close();
      await assert.rejects(client.execute('schema_get'), error => error.code === 'client_closed');
    } finally { await client.close(); }
    // Shared daemon still serves a different consumer after this handle closes.
    const survivor = await createClient({ callerDirectory: base, project: root, mode: 'daemon-required', daemonDirectory });
    try { assert.equal((await survivor.execute('schema_get')).kind, 'schema'); }
    finally { await survivor.close(); }
  });
}

test('real format-5 consumers run through Node direct, auto and daemon-required adapters', async () => {
  for (const mode of ['direct', undefined, 'daemon-required']) {
    const root = path.join(base, `query-facts-${mode ?? 'auto'}`);
    await mkdir(root, { recursive: true });
    await installQueryFactsProject(root);
    const client = await createClient({ callerDirectory: base, project: root, mode, daemonDirectory });
    const call = async (operation, params) => {
      covered.add(operation);
      return client.execute(operation, params);
    };
    try {
      const facts = queryFactSnapshot();
      const request = {
        definition_files: ['facts.json'], definition: 'urn:fixture:facts:composed',
        selection: { ids: ['CAND-C'] },
        parameters: [
          { name: 'candidate', value: { type: 'text', value: 'CAND-C' } },
          { name: 'context', value: { type: 'text', value: 'release' } },
        ], max_rows: 100, limit: 100, fact_snapshots: [facts], mappings: [],
      };
      const page = await call('query_execute', request);
      assert.equal(page.evaluation_complete, true);
      assert.equal(page.summary.binding_count, 3);
      const view = await call('query_view', { request, render: null });
      assert.equal(view.input_identity, page.input_identity);
      assert.equal(view.summary.binding_count, 3);
      const validation = await call('item_validate', { id: 'CAND-C', fact_snapshots: [facts], mappings: [] });
      assert.equal(validation.valid, true, JSON.stringify(validation));
      assert.equal(validation.query_policies.checks[0].state, 'passed');
      const gate = {
        definition_files: ['facts.json'], gate_file: 'facts-gates.yaml', shape_files: ['facts-present.ttl'],
        gate: 'require-composed-facts', scope: { ids: ['CAND-C'] },
        context: request.parameters, max_rows: 100, fact_snapshots: [facts], mappings: [],
      };
      assert.equal((await call('query_gate', gate)).state, 'passed');
      const gateReport = await call('query_report', { subject: { kind: 'gate', ...gate }, render: null });
      assert.equal(gateReport.checks[0].state, 'passed');
      const ruleReport = await call('query_report', { subject: {
        kind: 'documentation_rules', target: { kind: 'item', id: 'CAND-C' },
        fact_snapshots: [facts], mappings: [],
      } });
      assert.equal(ruleReport.checks[0].state, 'passed');
    } finally { await client.close(); }
  }
});

test('installed compiled TypeScript consumer uses all native routes without an executable CLI',
  { skip: !publicFacade || !process.env.MARA_NODE_INSTALLED }, async () => {
    assert.ok(process.env.MARA_NODE_TYPED_CONSUMER, 'compile installed usage.mts and set MARA_NODE_TYPED_CONSUMER');
    await import(pathToFileURL(process.env.MARA_NODE_TYPED_CONSUMER));
  });

test('routing failures, caller discovery, explicit selection and complete coverage', async () => {
  if (publicFacade) {
    await assert.rejects(createClient({ callerDirectory: '.' }), NativeClientError);
    await assert.rejects(createClient({ callerDirectory: base, mode: 'invalid' }), error => error.code === 'invalid_argument');
  } else {
    assert.throws(() => createClient({ callerDirectory: '.' }), NativeClientError);
    assert.throws(() => createClient({ callerDirectory: base, mode: 'invalid' }), error => error.code === 'invalid_argument');
  }
  const root = path.join(base, 'direct');
  const unbound = await createClient({ callerDirectory: path.join(root, 'docs'), mode: 'daemon-required', daemonDirectory });
  try {
    assert.equal((await unbound.execute('get', { reference: 'REQ-RENAMED' })).node.id, 'REQ-RENAMED');
    assert.equal((await unbound.execute('schema_get', { project: root })).kind, 'schema');
    await assert.rejects(unbound.execute('schema_get', { project: 'relative' }), NativeClientError);
  } finally { await unbound.close(); }
  const bound = await createClient({ callerDirectory: base, project: root, mode: 'direct' });
  try { await assert.rejects(bound.execute('schema_get', { project: root }), NativeClientError); }
  finally { await bound.close(); }
  const absentDirectory = path.join(daemonDirectory, 'absent');
  const required = await createClient({ callerDirectory: base, project: root, mode: 'daemon-required', daemonDirectory: absentDirectory });
  try { await assert.rejects(required.execute('schema_get'), error => error.kind === 'unavailable' && error.code === 'daemon_unavailable'); }
  finally { await required.close(); }
  const auto = await createClient({ callerDirectory: base, project: root, daemonDirectory: absentDirectory });
  try {
    assert.equal((await auto.execute('schema_get')).kind, 'schema');
    const caps = await auto.execute('capabilities');
    assert.deepEqual([...covered].filter(o => o !== 'capabilities').sort(), caps.operations.toSorted());
    await assert.rejects(auto.execute('not_an_operation'), error => error.kind === 'invalid_argument');
    await assert.rejects(auto.execute('get', { reference: 'REQ-RENAMED', extra: true }), NativeClientError);
  } finally { await auto.close(); }
});
