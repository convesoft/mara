import assert from 'node:assert/strict';
import test from 'node:test';
import { cp, mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { createClient, MaraClientError } from '../../npm/client.mjs';

const caps = { backend: 'test-boundary', operations: ['get', 'schema_get'],
  mutation_persistence: 'memory', code: { indexing: false }, native_recovery: false, result_formats: { discovery: 2 } };
const context = { backend: 'test-boundary', selection: { workspace: 'test-owned' } };

async function installQueryFactsProject(root) {
  const snapshot = JSON.parse(await readFile(new URL('../fixtures/query-facts/snapshot.json', import.meta.url), 'utf8'));
  for (const file of snapshot.files) {
    const target = path.join(root, file.path);
    await mkdir(path.dirname(target), { recursive: true });
    await writeFile(target, file.content);
  }
  const project = path.join(root, '.mara/project.toml');
  await writeFile(project, (await readFile(project, 'utf8')) + `\n[query_policy]\nformat_version = 1\nfiles = ["facts-invariants.yaml"]\ndefinition_files = ["facts.json"]\nshape_files = ["facts-present.ttl"]\nmax_rows = 100\n`);
  await writeFile(path.join(root, 'facts-invariants.yaml'), `format_version: 1\ninvariants:\n  - id: supplied-composed-facts\n    definition: urn:fixture:facts:composed\n    shape: urn:fixture:shape:has-fact-row\n    flavours: [candidate]\n    severity: error\n    parameters:\n      - name: candidate\n        value: {type: text, value: CAND-C}\n      - name: context\n        value: {type: text, value: release}\n`);
  await writeFile(path.join(root, 'facts-gates.yaml'), `format_version: 1\ngates:\n  - id: require-composed-facts\n    empty_roots: not_applicable\n    empty_required: failed\n    conditions:\n      - id: fact-row\n        definition: urn:fixture:facts:composed\n        shape_file: facts-present.ttl\n        shape: urn:fixture:shape:has-fact-row\n        severity: error\n        context_parameters: [candidate, context]\n`);
  await writeFile(path.join(root, 'facts-present.ttl'), `@prefix sh: <http://www.w3.org/ns/shacl#>.\n<urn:fixture:shape:has-fact-row> a sh:NodeShape; sh:property [ sh:path <urn:mara:query:match>; sh:minCount 1 ].\n`);
}

function suppliedFacts() {
  const key = verification => ({
    verification: { type: 'item', value: verification },
    candidate: { type: 'text', value: 'CAND-C' },
    context: { type: 'text', value: 'release' },
  });
  return {
    input: 'reported', identity: 'reported-run',
    coverage: ['VER-V1', 'VER-V2', 'VER-V3'].map(key),
    records: [
      ['VER-V1', 'pass'], ['VER-V2', 'pass'], ['VER-V3', 'fail'],
    ].map(([verification, state]) => ({
      keys: key(verification), fields: { state: { type: 'text', value: state } },
    })),
  };
}

// @mara checks REQ-SHARED-TYPESCRIPT-CLIENT
// These boundary checks supplement, not replace, real addon/browser execution.
test('shared ordering, snapshots, unsupported operations, close and owning failures', async () => {
  const calls = [];
  const envelope = { format_version: 1, error: { code: 'unknown_outcome', message: 'Do not replay; inspect source', operation: 'item_update', project: '/fixture' } };
  const unknown = new MaraClientError('unknown_outcome', envelope);
  const client = await createClient({ context,
    async execute(operation, params) {
      if (operation === 'capabilities') return caps;
      calls.push([operation, params]);
      if (params.reference === 'unknown') throw unknown;
      return { kind: 'schema' };
    },
    async close() { calls.push(['close']); },
  });
  const params = { reference: 'before' };
  const pending = client.get(params);
  params.reference = 'after';
  const failed = client.get({ reference: 'unknown' });
  const following = client.schemaGet();
  await assert.rejects(client.get({ reference: 1n }), error => error.code === 'invalid_argument');
  const cyclic = {}; cyclic.self = cyclic;
  for (const invalid of [NaN, Infinity, -Infinity, () => 1, Symbol('value'), 1n,
    new Date(), new Map(), { toJSON: () => null }, [undefined], cyclic]) {
    await assert.rejects(client.get({ reference: 'unused', nested: { invalid } }), error => error.code === 'invalid_argument');
  }
  await assert.rejects(client.get({ reference: 'unused', [Symbol('key')]: 'lost' }), error => error.code === 'invalid_argument');
  await assert.rejects(client.projectRefresh(), error => error.kind === 'unavailable' && error.code === 'unsupported_operation');
  const closing = client.close();
  assert.equal(client.close(), closing);
  await assert.rejects(client.schemaGet(), error => error.kind === 'closed');
  await pending;
  await assert.rejects(failed, error => error === unknown && error.envelope === envelope);
  await following;
  await closing;
  assert.deepEqual(calls, [['get', { reference: 'before' }], ['get', { reference: 'unknown' }], ['schema_get', {}], ['close']]);
  assert.ok(Object.isFrozen(client.context.selection));
  assert.ok(Object.isFrozen(client.capabilities.operations));
});

test('failed setup releases adapter; close failure remains the same outcome', async () => {
  const error = new MaraClientError('connection', { format_version: 1, error: { code: 'connection_failed', message: 'fixture failure' } });
  let closed = 0;
  await assert.rejects(createClient({ context, async execute() { throw error; }, async close() { closed++; } }), e => e === error);
  assert.equal(closed, 1);
  const client = await createClient({ context, async execute() { return caps; }, async close() { throw error; } });
  const closing = client.close();
  await assert.rejects(closing, e => e === error);
  assert.equal(client.close(), closing);
  await assert.rejects(client.close(), e => e === error);
});

// @mara checks REQ-NODE-NATIVE-BINDING
test('public default package loader, exact-version checks and ESM-only metadata', { skip: !process.env.MARA_NODE_CLIENT }, async () => {
  const root = path.join(process.env.MARA_NODE_FIXTURES, 'package-loader');
  const pkg = path.join(root, 'mara');
  const source = path.dirname(process.env.MARA_NODE_CLIENT);
  await cp(source, pkg, { recursive: true });
  const packageName = new Map([
    ['linux:x64', '@convesoft/mara-linux-x64-gnu'], ['linux:arm64', '@convesoft/mara-linux-arm64-gnu'],
    ['darwin:arm64', '@convesoft/mara-darwin-arm64'],
  ]).get(`${process.platform}:${process.arch}`);
  assert.ok(packageName, 'probe requires an actually supported native host');
  const platform = path.join(pkg, 'node_modules', packageName);
  await mkdir(path.join(platform, 'bin'), { recursive: true });
  const manifest = JSON.parse(await readFile(path.join(pkg, 'package.json'), 'utf8'));
  await writeFile(path.join(platform, 'package.json'), JSON.stringify({ name: packageName, version: manifest.version }));
  await cp(process.env.MARA_NODE_ADDON, path.join(platform, 'bin/mara.node'));
  const { createNativeClient, MaraClientError: PublicError } = await import(pathToFileURL(path.join(pkg, 'node.mjs')));
  assert.equal(manifest.engines.node, '>=24');
  assert.equal(manifest.exports['.'].import, './node.mjs');
  assert.equal(manifest.exports['.'].require, undefined);
  const options = { callerDirectory: root, project: 'project', mode: 'direct' };
  const client = await createNativeClient(options);
  try {
    await client.projectInit();
    assert.equal((await client.projectValidate()).valid, true);
  } finally { await client.close(); }

  const queryRoot = path.join(root, 'query-facts');
  await mkdir(queryRoot, { recursive: true });
  await installQueryFactsProject(queryRoot);
  const queryClient = await createNativeClient({ ...options, project: queryRoot });
  try {
    const facts = suppliedFacts();
    const request = {
      definition_files: ['facts.json'], definition: 'urn:fixture:facts:composed',
      selection: { ids: ['CAND-C'] },
      parameters: [
        { name: 'candidate', value: { type: 'text', value: 'CAND-C' } },
        { name: 'context', value: { type: 'text', value: 'release' } },
      ],
      max_rows: 100, limit: 100, fact_snapshots: [facts], mappings: [],
    };
    const page = await queryClient.queryExecute(request);
    assert.equal(page.evaluation_complete, true);
    assert.equal(page.summary.binding_count, 3);
    const view = await queryClient.queryView({ request, render: null });
    assert.equal(view.input_identity, page.input_identity);
    assert.equal(view.summary.binding_count, 3);
    const validation = await queryClient.itemValidate({ id: 'CAND-C', fact_snapshots: [facts] });
    assert.equal(validation.valid, true);
    assert.equal(validation.query_policies.checks[0].state, 'passed');
    const gate = {
      definition_files: ['facts.json'], gate_file: 'facts-gates.yaml', shape_files: ['facts-present.ttl'],
      gate: 'require-composed-facts', scope: { ids: ['CAND-C'] },
      context: request.parameters, max_rows: 100, fact_snapshots: [facts], mappings: [],
    };
    assert.equal((await queryClient.queryGate(gate)).state, 'passed');
    const gateReport = await queryClient.queryReport({ subject: { kind: 'gate', ...gate }, render: null });
    assert.equal(gateReport.checks[0].state, 'passed');
    const rulesReport = await queryClient.queryReport({ subject: {
      kind: 'documentation_rules', target: { kind: 'item', id: 'CAND-C' },
      fact_snapshots: [facts], mappings: [],
    } });
    assert.equal(rulesReport.checks[0].state, 'passed');
  } finally { await queryClient.close(); }

  await writeFile(path.join(platform, 'package.json'), JSON.stringify({ version: '0.0.0-mismatch' }));
  await assert.rejects(createNativeClient(options), error => error instanceof PublicError && error.kind === 'incompatible' && error.code === 'native_version_mismatch');
  for (const mode of [Symbol('direct'), () => 'direct', 1n]) {
    await assert.rejects(createNativeClient({ ...options, mode, addonPath: process.env.MARA_NODE_ADDON }),
      error => error instanceof PublicError && error.kind === 'invalid_argument' && error.code === 'invalid_argument');
  }
  await assert.rejects(createNativeClient({ ...options, addonPath: 'relative.node' }), error => error.code === 'invalid_argument');
  await assert.rejects(createNativeClient({ ...options, addonPath: path.join(root, 'missing.node') }), error => error.kind === 'unavailable' && error.code === 'native_addon_load_failed');
  // Override bypasses platform discovery, but not real addon version compatibility.
  await writeFile(path.join(pkg, 'package.json'), JSON.stringify({ ...manifest, version: '0.0.0-mismatch' }));
  await assert.rejects(createNativeClient({ ...options, addonPath: process.env.MARA_NODE_ADDON }), error => error.kind === 'incompatible' && error.code === 'native_version_mismatch');
});
