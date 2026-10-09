import { test } from 'node:test';
import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';

// Select the script-free installed artifact, never the source module or a native addon.
if (!process.env.MARA_HTTP_PACKAGE) throw new Error('Set MARA_HTTP_PACKAGE to the disposable installed package root');
const { createHttpClient, MaraClientError } = await import(pathToFileURL(process.env.MARA_HTTP_PACKAGE + '/http.mjs'));
const { applicationVersion } = await import(pathToFileURL(process.env.MARA_HTTP_PACKAGE + '/version.mjs'));
const operations = ['project_init', 'project_mid_backfill', 'project_transaction_rollback', 'project_refresh',
  'schema_get', 'schema_list', 'schema_validate', 'item_list', 'search', 'get', 'related',
  'item_create', 'item_update', 'item_delete', 'item_move', 'item_rename', 'item_validate',
  'project_validate', 'relation_get', 'relation_add', 'relation_remove',
  'query_execute', 'query_gate', 'query_view', 'query_report', 'trace_matrix'];
const negotiation = {
  protocol_version: 1, application_version: applicationVersion,
  result_formats: { discovery: 2, relationship: 1, validation: 1, trace: 1,
    query: 3, gate: 1, query_view: 1, query_report: 1, operation_error: 1 },
  capabilities: { backend: 'native', operations, persistence: 'filesystem', code_indexing: true,
    configured_indexers: true, transaction_rollback: true, force_refresh: true,
    project_sessions: 'reconciled_memory', local_methods: [] },
};
const options = { endpoint: 'http://127.0.0.1:3210/rpc', token: 'fixture-secret', project: '/fixture' };
const typed = (kind, code) => error => error instanceof MaraClientError && error.kind === kind && error.code === code;

// @mara checks DES-JAVASCRIPT-CLIENT-ADAPTERS
// @mara checks REQ-LOCAL-DAEMON-HTTP
test('HTTP transport boundaries retain owning errors, sanitize failures and never replay', async () => {
  const originalFetch = globalThis.fetch;
  const requests = [];
  let reply;
  globalThis.fetch = async (endpoint, init) => {
    const rpc = JSON.parse(init.body);
    requests.push({ endpoint, init, rpc });
    if (rpc.method === 'mara.negotiate') return Response.json({ jsonrpc: '2.0', id: rpc.id, result: negotiation });
    return reply(rpc);
  };
  try {
    for (const changes of [
      { endpoint: 'https://127.0.0.1:3210/rpc' }, { endpoint: 'http://localhost:3210/rpc' },
      { endpoint: 'http://evil.invalid/rpc' }, { endpoint: 'http://127.0.0.1:3210/rpc?token=secret' },
      { endpoint: 'http://user:secret@127.0.0.1:3210/rpc' }, { endpoint: 'http://127.0.0.1:3210/rpc#secret' },
      { token: '' }, { token: 'bad\r\nheader' }, { project: 'relative' }, { applicationVersion: '0.0.0' },
    ]) await assert.rejects(createHttpClient({ ...options, ...changes }), typed('invalid_argument', 'invalid_argument'));
    assert.equal(requests.length, 0);
    const client = await createHttpClient(options);
    assert.equal(client.context.backend, 'http');
    assert.equal(client.context.selection.token, undefined);
    assert.ok(Object.isFrozen(client.capabilities));
    reply = rpc => Response.json({ jsonrpc: '2.0', id: rpc.id, result: { valid: false, evaluation_complete: false } });
    assert.deepEqual(await client.projectValidate(), { valid: false, evaluation_complete: false });
    const query = { definition_files: ['facts.json'], definition: 'urn:fixture:query',
      selection: { ids: ['CAND-1'] }, parameters: [], max_rows: 10, limit: 20,
      fact_snapshots: [{ input: 'reported', identity: 'run-1', coverage: [], records: [] }], mappings: [] };
    const gate = { definition_files: ['facts.json'], gate_file: 'gates.yaml', shape_files: ['gate.ttl'],
      gate: 'ready', scope: { ids: ['CAND-1'] }, context: [], max_rows: 10, fact_snapshots: query.fact_snapshots, mappings: [] };
    await client.queryExecute(query);
    await client.queryGate(gate);
    await client.queryView({ request: query, render: 'markdown' });
    await client.queryReport({ subject: { kind: 'gate', ...gate }, render: 'markdown' });
    reply = rpc => Response.json({ jsonrpc: '2.0', id: rpc.id, result: { evaluation_complete: true } });
    for (const operation of ['query_execute', 'query_gate', 'query_view', 'query_report']) {
      assert.ok(requests.some(request => request.rpc.method === operation), `missing ${operation} dispatch`);
    }
    assert.deepEqual(requests.find(request => request.rpc.method === 'query_view').rpc.params.request, query);
    const last = requests.at(-1);
    assert.equal(last.rpc.params.project, options.project);
    assert.equal(last.init.headers['X-Mara-Application-Version'], negotiation.application_version);
    assert.equal(last.init.headers['X-Mara-Protocol-Version'], '1');
    assert.equal(last.init.credentials, 'omit');
    assert.equal(last.init.redirect, 'error');
    const before = requests.length;
    await assert.rejects(client.itemCreate({ flavour: 'requirement', id: 'REQ-X', file: 'a.mara.md',
      title: 'Too big', body: 'x'.repeat(1024 * 1024) }), typed('invalid_argument', 'request_too_large'));
    await assert.rejects(client.schemaGet({ project: '/elsewhere' }), typed('invalid_argument', 'invalid_argument'));
    assert.equal(requests.length, before);
    const envelope = { format_version: 1, error: { code: 'relation_exists', message: 'Existing edge' },
      edge: { relation: 'refines', source: { kind: 'item', id: 'REQ-A', mid: 'a' },
        target: { kind: 'item', id: 'REQ-B', mid: 'b' }, symmetric: false }, occurrence_count: 1 };
    reply = rpc => Response.json({ jsonrpc: '2.0', id: rpc.id, error: { code: -32000, message: 'Existing edge', data: envelope } });
    await assert.rejects(client.relationAdd({ source: 'REQ-A', relation: 'refines', target: 'REQ-B' }), error => {
      assert.deepEqual(error.envelope, envelope); return typed('operation', 'relation_exists')(error);
    });
    for (const code of [-32700, -32600]) {
      reply = () => Response.json({ jsonrpc: '2.0', id: null, error: { code, message: 'Rejected before dispatch' } });
      await assert.rejects(client.itemUpdate({ reference: 'REQ-A', title: '\ud800' }), typed('invalid_argument', 'invalid_argument'));
    }
    for (const response of [
      rpc => Response.json({ jsonrpc: '2.0', id: rpc.id + 1, result: {} }),
      () => Response.json({ jsonrpc: '2.0', id: null, error: { code: -32000, message: 'Unmatched error' } }),
      () => new Response('broken JSON'),
      rpc => Response.json({ jsonrpc: '2.0', id: rpc.id, error: { code: -32603, message: 'Encoding failure' } }),
      rpc => Response.json({ jsonrpc: '2.0', id: rpc.id, error: { code: -32000, message: 'No owning envelope' } }),
      () => { throw new Error('secret ' + options.token); },
    ]) {
      reply = response;
      const dispatched = requests.length;
      await assert.rejects(client.itemUpdate({ reference: 'REQ-A', title: 'One attempt' }), error => {
        assert.ok(!JSON.stringify(error.envelope).includes(options.token));
        assert.equal(error.envelope.error.operation, 'item_update');
        assert.equal(error.envelope.error.project, options.project);
        return typed('unknown_outcome', 'unknown_outcome')(error);
      });
      assert.equal(requests.length, dispatched + 1);
    }
    for (const [status, kind, code] of [[401, 'authentication', 'authentication_failed'],
      [403, 'authorization', 'authorization_failed'], [503, 'unavailable', 'daemon_unavailable']]) {
      reply = () => new Response('', { status });
      await assert.rejects(client.itemUpdate({ reference: 'REQ-A', title: 'Rejected' }), typed(kind, code));
    }
    reply = rpc => Response.json({ jsonrpc: '2.0', id: rpc.id, result: {} });
    const queued = client.schemaGet();
    const close = client.close();
    await queued; await close;
    assert.equal(client.close(), close);
    await assert.rejects(client.schemaGet(), typed('closed', 'client_closed'));
    assert.ok(requests.every(r => !r.rpc.method.startsWith('daemon_')));
    negotiation.capabilities.local_methods = ['daemon_stop'];
    await assert.rejects(createHttpClient(options), typed('incompatible', 'incompatible_capabilities'));
  } finally { globalThis.fetch = originalFetch; negotiation.capabilities.local_methods = []; }
});
