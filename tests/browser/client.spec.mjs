import { test, expect } from '@playwright/test';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// @mara checks REQ-SHARED-TYPESCRIPT-CLIENT
// @mara checks DES-JAVASCRIPT-CLIENT-ADAPTERS
// @mara checks REQ-BROWSER-LOCAL-WORKSPACE
// @mara checks REQ-EXPLICIT-BACKEND-CAPABILITIES
test('public bundled WASM factory matches the real native shared facade', async ({ page }, info) => {
  const calls = [
    ['projectMidBackfill', {}], ['projectMidBackfill', {}],
    ['schemaGet', {}], ['schemaList', { kind: 'relation' }], ['schemaValidate', { limit: 1 }],
    ['itemList', { limit: 1 }], ['search', { query: 'café', ids: ['REQ-A', 'REQ-B'], limit: 1 }],
    ['get', { reference: 'REQ-A' }], ['related', { reference: 'REQ-A', relations: ['schema:depends_on'], limit: 1 }],
    ['relationGet', { source: 'REQ-A', relation: 'depends_on', target: 'REQ-B', limit: 1 }],
    ['itemValidate', { id: 'REQ-A', limit: 1 }], ['projectValidate', { limit: 1 }],
    ['traceMatrix', { all: true, rules: ['urn:mara:rule:draft'], limit: 1 }],
    ['traceMatrix', { ids: ['REQ-A'], check: { files: ['checks.yaml'], shape: 'urn:mara:rule:root',
      parameters: { expected: 'draft' } }, limit: 1 }],
    ['itemUpdate', { reference: 'REQ-A', title: 'Changed café 中文 👩🏽‍💻' }],
    ['get', { reference: 'REQ-A' }],
    ['schemaList', { kind: 'flavour' }],
    ['schemaGet', { kind: 'flavour', name: 'requirement' }],
    ['schemaGet', { kind: 'relation', name: 'required_by' }],
    ['itemCreate', { flavour: 'requirement', id: 'REQ-C', file: 'created.mara.md',
      title: 'Created café', fields: [{ key: 'status', value: 'draft' }], body: 'Original 中文.' }],
    ['itemUpdate', { reference: 'REQ-C', body: 'Final 中文 👩🏽‍💻.' }],
    ['relationAdd', { source: 'REQ-C', relation: 'refines', target: 'REQ-A' }],
    ['relationGet', { source: 'REQ-C', relation: 'refines', target: 'REQ-A', limit: 1 }],
    ['relationAdd', { source: 'REQ-A', relation: 'refines', target: 'REQ-C' }],
    ['projectValidate', { limit: 1 }],
    ['relationRemove', { source: 'REQ-A', relation: 'refines', target: 'REQ-C' }],
    ['relationRemove', { source: 'REQ-C', relation: 'refines', target: 'REQ-A' }],
    ['itemRename', { reference: 'REQ-C', new_id: 'REQ-RENAMED' }],
    ['itemMove', { reference: 'REQ-RENAMED', file: 'moved.mara.md' }],
    ['get', { reference: 'REQ-RENAMED' }],
    ['itemDelete', { reference: 'REQ-RENAMED' }],
    ['itemUpdate', { reference: 'REQ-A', fields: [{ key: 'status', value: 'retired' }] }],
    ['projectValidate', { limit: 1 }],
    ['traceMatrix', { all: true, rules: ['urn:mara:rule:draft'], limit: 1 }],
    ['traceMatrix', { ids: ['REQ-A'], check: { files: ['checks.yaml'], shape: 'urn:mara:rule:root',
      parameters: { expected: 'retired' } }, limit: 1 }],
    ['itemUpdate', { reference: 'REQ-A', fields: [{ key: 'status', value: 'draft' }] }],
    ['projectValidate', { limit: 1 }],
  ];
  const workers = new Set();
  page.on('worker', worker => { workers.add(worker); worker.on('close', () => workers.delete(worker)); });
  await page.goto('/');
  const browser = await page.evaluate(async calls => {
    const { createWasmClient, MaraClientError } = await import('./browser.mjs');
    const empty = { format_version: 1, files: [], selection: { document_paths: [], ignores: [] } };
    const options = { snapshot: structuredClone(empty) };
    const starting = createWasmClient(options);
    options.snapshot.files.push({ path: '../must-not-be-read', content: 'invalid' });
    const client = await starting;
    const assert = (value, message) => { if (!value) throw new Error(message); };
    try {
      await client.projectInit({ name: 'Parity café', template: 'engineering-legacy' });
      const snapshot = (await client.workspaceSources()).current;
      snapshot.base_identity = 'fixture-original-base';
      snapshot.files.find(f => f.path === '.mara/engineering-rules.yaml').content =
        '- id: rule:draft\n  targetClass: requirement\n  property:\n    - {path: status, hasValue: draft}\n';
      snapshot.files.push({ path: 'checks.yaml', content:
        '- id: rule:root\n  class: requirement\n  and: [rule:literal, rule:linked]\n' +
        '- id: rule:literal\n  property:\n    - {path: status, hasValue: {parameter: expected}}\n' +
        '- id: rule:linked\n  property:\n    - path: depends_on\n      qualifiedValueShape: rule:literal\n      qualifiedMinCount: 1\n' });
      snapshot.files.push({ path: 'café.mara.md', content:
        ':::mara requirement REQ-A\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5FAV\n:title: café A\n:status: draft\n:depends_on: REQ-B\n\n中文 👩🏽‍💻.\n:::\n\n' +
        ':::mara requirement REQ-B\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5FAW\n:title: café B\n:status: draft\n\nTarget.\n:::\n' });
      snapshot.files.push({ path: 'legacy.mara.md', content:
        ':::mara requirement REQ-LEGACY\r\n:title: Legacy café\r\n:status: draft\r\n\r\nBody.\r\n:::\r\n' });
      snapshot.selection.document_paths.push('café.mara.md', 'legacy.mara.md');
      const queuedReplacement = client.loadSnapshot(snapshot);
      snapshot.files[0].content = 'not the invocation snapshot';
      await queuedReplacement;
      const initial = (await client.workspaceSources()).base;
      const results = [];
      for (const [method, params] of calls) {
        const pages = [];
        let cursor;
        do {
          const result = await client[method]({ ...params, ...(cursor ? { cursor } : {}) });
          pages.push(result);
          cursor = result.has_more ? result.next_cursor : null;
          assert(!result.has_more || cursor, 'missing continuation');
        } while (cursor);
        results.push(pages);
      }
      const traces = calls.map(([method], i) => ({ method, pages: results[i] })).filter(o => o.method === 'traceMatrix');
      assert(traces[0].pages.at(-1).evaluation_complete, 'configured rules unavailable');
      assert(traces[1].pages[0].summaries[0].passed === 1, 'nested qualified parameterized check');
      const outcomes = calls.map(([method], i) => ({ method, pages: results[i] }));
      assert(outcomes.some(o => o.method === 'projectValidate' &&
        o.pages.some(p => p.diagnostics.some(d => d.code === 'relation_cycle'))), 'cycle policy not executed');
      assert(outcomes.some(o => o.method === 'projectValidate' &&
        o.pages.some(p => p.diagnostics.some(d => d.code === 'rule_failed'))), 'failed policy not executed');
      assert(outcomes.filter(o => o.method === 'projectMidBackfill')[0].pages[0].changed.length === 1,
        'native/browser legacy backfill not exercised');
      assert(results.at(-1)[0].valid && results.at(-1)[0].evaluation_complete, 'final validation');
      const state = await client.workspaceState();
      const changes = await client.exportChanges();
      assert(state.base_identity === 'fixture-original-base' && changes.changes.length === 4, 'base/export identity');
      assert((await client.checkChangeSetPreconditions({ change_set: changes, snapshot: initial })).source_preconditions_match,
        'whole-base preconditions');
      for (const method of ['projectTransactionRollback', 'projectRefresh']) {
        try { await client[method](); throw new Error('unsupported operation passed'); }
        catch (error) { assert(error instanceof MaraClientError && error.kind === 'unavailable' &&
          error.code === 'unsupported_operation', 'unsupported owning failure'); }
      }
      return { initial, results, finalSources: (await client.workspaceSources()).current,
        capabilities: client.capabilities, context: client.context };
    } finally { await client.close(); }
  }, calls);
  await expect.poll(() => workers.size).toBe(0);
  expect(browser.capabilities.mutation_persistence).toBe('memory');
  expect(browser.context.backend).toBe('wasm');
  if (!process.env.TMPDIR || !process.env.NODE24 || !process.env.MARA_NODE_ADDON || !process.env.MARA_NODE_CLIENT) {
    throw new Error('Set owned TMPDIR, NODE24, MARA_NODE_ADDON and generated MARA_NODE_CLIENT');
  }
  const root = await mkdtemp(path.join(process.env.TMPDIR, 'shared-parity-'));
  try {
    for (const { path: relative, content } of browser.initial.files) {
      const destination = path.join(root, relative);
      await mkdir(path.dirname(destination), { recursive: true });
      await writeFile(destination, content, 'utf8');
    }
    const inputFile = path.join(root, 'parity-input.json');
    await writeFile(inputFile, JSON.stringify({ root, calls }));
    const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith('GIT_')));
    Object.assign(env, { GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null', GIT_CEILING_DIRECTORIES: root,
      XDG_CONFIG_HOME: path.join(root, '.test-config') });
    const helper = fileURLToPath(new URL('./native-parity.mjs', import.meta.url));
    const native = JSON.parse((await promisify(execFile)(process.env.NODE24, [helper, inputFile],
      { cwd: root, env, timeout: 120_000, maxBuffer: 5 * 1024 * 1024 })).stdout);
    // Host context and source-bound opaque cursors differ; domain observations do not.
    const suppliedIdentities = new Set(browser.initial.files.flatMap(file =>
      [...file.content.matchAll(/\b[0-7][0-9A-HJKMNP-TV-Z]{25}\b/g)].map(match => match[0])));
    const normalize = input => {
      const handles = new Map();
      const identities = new Map();
      const visit = value => {
        if (typeof value === 'string') {
          if (value === root) return '.';
          if (value.startsWith(root + '/')) return '.' + value.slice(root.length);
          // Preserve equality/links between owning occurrence/check identifiers,
          // whose fingerprints include host-root context.
          if (value.startsWith('check:') || value.startsWith('occ-') || value.startsWith('mara:node:2:')) {
            if (!handles.has(value)) handles.set(value, `<owning-handle-${handles.size}>`);
            return handles.get(value);
          }
          return value.replace(/\b[0-7][0-9A-HJKMNP-TV-Z]{25}\b/g, mid => {
            if (suppliedIdentities.has(mid)) return mid;
            if (!identities.has(mid)) identities.set(mid, `<generated-identity-${identities.size}>`);
            return identities.get(mid);
          });
        }
        if (Array.isArray(value)) return value.map(visit);
        if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, child]) =>
          [key, key === 'next_cursor' && child !== null ? '<owning-cursor>' : visit(child)]));
        return value;
      };
      return visit(input);
    };
    expect(normalize(native.results)).toEqual(normalize(browser.results));
    expect(normalize(native.files)).toEqual(normalize(browser.finalSources.files));
    expect(native.capabilities.mutation_persistence).toBe('filesystem');
    await info.attach('shared-native-wasm-parity', { body: JSON.stringify({ calls, browser, native }, null, 2),
      contentType: 'application/json' });
  } finally { await rm(root, { recursive: true, force: true }); }
});

// @mara checks DES-JAVASCRIPT-CLIENT-ADAPTERS
test('real startup failure is typed and releases its module worker', async ({ page }) => {
  const workers = new Set();
  page.on('worker', worker => { workers.add(worker); worker.on('close', () => workers.delete(worker)); });
  await page.route('**/mara_wasm_bg.wasm', route => route.abort());
  await page.goto('/');
  const failure = await page.evaluate(async () => {
    const { createWasmClient, MaraClientError } = await import('./browser.mjs');
    try {
      await createWasmClient({ snapshot: { format_version: 1, files: [], selection: { document_paths: [], ignores: [] } } });
      throw new Error('startup succeeded');
    } catch (error) {
      if (!(error instanceof MaraClientError)) throw error;
      return { kind: error.kind, code: error.code };
    }
  });
  expect(failure).toEqual({ kind: 'unavailable', code: 'worker_startup_failed' });
  await expect.poll(() => workers.size).toBe(0);
});
