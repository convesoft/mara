import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

assert.equal(Number(process.versions.node.split('.')[0]), 24, 'certify the actual Node 24 baseline');
const publicFacade = Boolean(process.env.MARA_NODE_CLIENT);
const { loadAddon, NativeClientError: PrivateError } = await import(pathToFileURL(process.env.MARA_NODE_LOADER));
const publicModule = publicFacade ? await import(pathToFileURL(process.env.MARA_NODE_CLIENT)) : null;
const NativeClientError = publicModule?.MaraClientError ?? PrivateError;
const createClient = publicFacade ? async options => {
  const client = await publicModule.createNativeClient({ ...options, addonPath: process.env.MARA_NODE_ADDON });
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
} : loadAddon(process.env.MARA_NODE_ADDON);
const base = process.env.MARA_NODE_FIXTURES;
const daemonDirectory = process.env.MARA_NODE_DAEMON;
const covered = new Set();
const rule = '- id: rule:draft\n  targetClass: requirement\n  property:\n    - {path: status, hasValue: draft}\n';
const check = '- id: rule:status\n  class: requirement\n  property:\n    - {path: status, hasValue: {parameter: expected_status}}\n';

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
      assert.equal(initialized.created.length, 2); // Preserve the native owning result.
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
      await call('trace_matrix', { all: true, rules: ['urn:mara:rule:draft'], limit: 1, cursor: trace.next_cursor });
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
