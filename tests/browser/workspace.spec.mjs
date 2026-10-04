import { test, expect } from '@playwright/test';
import { mkdtemp, mkdir, readdir, readFile, writeFile, rm } from 'node:fs/promises';
import path from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';

// @mara checks REQ-BROWSER-LOCAL-WORKSPACE
// @mara checks DES-BROWSER-PROJECT-SNAPSHOT
// @mara checks REQ-EXPLICIT-BACKEND-CAPABILITIES
// @mara checks REQ-BROWSER-CHANGE-EXPORT
// @mara checks DES-BROWSER-FILE-CHANGESET
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
    let exported;
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
      // Inject this actual worker; setup/shutdown remain adapter-owned.
      const { createClient, MaraClientError } = await import('./client.mjs');
      const shared = await createClient({
        context: { backend: 'wasm', selection: { fixture: 'explicit-memory-workspace' } },
        async execute(operation, params) {
          const response = await request(operation, params);
          if (!response.ok) throw new MaraClientError('operation', response.error);
          return response.result;
        },
        async close() { worker.terminate(); },
      });
      const caps = shared.capabilities;
      const initialized = await shared.projectInit({ name: 'café 中文 👩🏽‍💻', template: 'engineering' });
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
      const beforeExport = await call('export_changes');
      const rejected = await request('item_update', { reference: 'REQ-A', body: '[[REQ-MISSING]]' });
      assert(!rejected.ok, 'invalid reference accepted');
      assert(JSON.stringify(await call('workspace_sources')) === JSON.stringify(before), 'rejected edit changed state');
      assert(JSON.stringify(await call('export_changes')) === JSON.stringify(beforeExport), 'rejected edit changed export');
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
      const finalSources = await call('workspace_sources');
      const changeSet = await call('export_changes');
      assert(changeSet.format_version === 1 && changeSet.base_identity === snapshot.base_identity,
        'export version/provenance');
      assert(changeSet.changes.length === 2 && changeSet.changes[0].path === 'café/知识.mara.md' &&
        changeSet.changes[1].path === 'moved.mara.md', 'ordered consolidated changes');
      assert(changeSet.changes[0].before === snapshot.files.find(f => f.path === 'café/知识.mara.md').content &&
        changeSet.changes[1].before === null, 'exact preimages');
      assert(changeSet.changes[0].after.includes('REQ-B') && !changeSet.changes[0].after.includes('REQ-A') &&
        !changeSet.changes[1].after.includes('REQ-RENAMED'), 'intermediate operations leaked into export');
      assert(JSON.stringify(await call('export_changes')) === JSON.stringify(changeSet), 'export changed base');
      const match = await call('check_change_set_preconditions', { change_set: changeSet, snapshot });
      assert(match.source_preconditions_match && !match.external_context_verified, 'external context claim');
      for (const mutate of [
        s => { s.files.find(f => f.path === 'checks.yaml').content += '\\n'; },
        s => { s.files.find(f => f.path === '.mara/project.toml').content += '\\n'; },
        s => { s.files.find(f => f.path === '.mara/schema.yaml').content += '\\n'; },
        s => { s.files.find(f => f.path === '.mara/engineering-rules.yaml').content += '\\n'; },
        s => { s.files.push({ path: 'untouched.mara.md', content: '# Extra\\n' });
          s.selection.document_paths.push('untouched.mara.md'); },
        s => { s.selection.document_paths = []; },
        s => { s.selection.ignores.push({ directory: '', root_prefix: [], content: 'ignored/\\n' }); },
      ]) {
        const divergent = structuredClone(snapshot);
        mutate(divergent);
        assert(!(await request('check_change_set_preconditions', {
          change_set: changeSet, snapshot: divergent,
        })).ok, 'divergent whole base accepted');
      }
      const wrongPreimage = structuredClone(changeSet);
      wrongPreimage.changes[0].before += 'wrong';
      assert(!(await request('check_change_set_preconditions', {
        change_set: wrongPreimage, snapshot,
      })).ok, 'forged preimage accepted');
      assert(JSON.stringify(await call('workspace_sources')) === JSON.stringify(finalSources), 'checking wrote memory');
      assert(JSON.stringify(await call('export_changes')) === JSON.stringify(changeSet), 'checking changed export');
      exported = { base: finalSources.base, current: finalSources.current, changeSet };

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
      const codeExport = JSON.stringify(await call('export_changes'));
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
        assert(JSON.stringify(await call('export_changes')) === codeExport, operation + ' changed code export');
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
      assert(shared.capabilities.mutation_persistence === 'memory', 'facade persistence');
      assert((await shared.schemaGet()).kind === 'schema', 'browser-safe facade schema read');
      const custom = structuredClone(codeSnapshot);
      custom.files.find(f => f.path === '.mara/schema.yaml').path = 'custom.yaml';
      custom.files.find(f => f.path === '.mara/project.toml').content =
        custom.files.find(f => f.path === '.mara/project.toml').content.replace('.mara/schema.yaml', 'custom.yaml') +
        '\ninvalid = [';
      await call('load_snapshot', custom);
      const recovered = await shared.projectValidate();
      assert(!recovered.valid && !recovered.evaluation_complete &&
        recovered.diagnostics.some(d => d.code === 'project_invalid'), 'custom-schema configuration recovery');
      try { await shared.projectTransactionRollback(); throw new Error('unsupported work passed'); }
      catch (error) { assert(error instanceof MaraClientError && error.code === 'unsupported_operation', 'typed unsupported error'); }
      const pendingRead = shared.schemaValidate();
      const closing = shared.close();
      await pendingRead;
      await closing;
      assert(shared.close() === closing, 'idempotent shared close');
      try { await shared.schemaGet(); throw new Error('closed client read passed'); }
      catch (error) { assert(error instanceof MaraClientError && error.code === 'client_closed', 'typed closed error'); }
      return { requests, transcript, exported, operations: caps.operations, pageTick, sharedFacade: true, userAgent: navigator.userAgent };
    } finally {
      worker.terminate();
    }
  });
  expect(browserErrors).toEqual([]);
  expect(report.pageTick).toBe(true);
  expect(report.operations).toContain('trace_matrix');
  expect(report.operations).toContain('export_changes');
  // Test-only host materialization. No publisher, journal or filesystem policy is added.
  if (!process.env.TMPDIR || !process.env.MARA_CLI) throw new Error('Set owned TMPDIR and real MARA_CLI');
  const root = await mkdtemp(path.join(path.resolve(process.env.TMPDIR), 'browser-export-'));
  try {
    const { base, current, changeSet } = report.exported;
    for (const file of base.files) {
      const target = path.join(root, file.path);
      await mkdir(path.dirname(target), { recursive: true });
      await writeFile(target, file.content, 'utf8');
    }
    const readSources = async (directory = '') => {
      const files = [];
      for (const entry of await readdir(path.join(root, directory), { withFileTypes: true })) {
        const relative = directory ? directory + '/' + entry.name : entry.name;
        if (entry.isDirectory()) files.push(...await readSources(relative));
        else files.push({ path: relative, content: await readFile(path.join(root, relative), 'utf8') });
      }
      return files.sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
    };
    // Independently reread the entire fixture inventory, including untouched source inputs.
    const actual = { ...base, files: await readSources(), selection: {
      document_paths: (await readSources()).filter(f => f.path.endsWith('.mara.md')).map(f => f.path),
      ignores: [], // This fixture supplies no ignore context or physical ignore files.
    } };
    const checkSources = actual => page.evaluate(async ({ changeSet, actual }) => {
      const worker = new Worker('./worker.mjs', { type: 'module' });
      try {
        return await new Promise((resolve, reject) => {
          worker.onmessage = ({ data }) => resolve(data);
          worker.onerror = event => reject(new Error(event.message));
          worker.postMessage({ format_version: 1, id: 1, operation: 'check_change_set_preconditions',
            params: { change_set: changeSet, snapshot: actual } });
        });
      } finally { worker.terminate(); }
    }, { changeSet, actual });
    // A real untouched-file divergence fails the host's pre-write gate. Preserve
    // the entire divergent tree, including every otherwise matching preimage.
    const untouched = base.files.find(f => f.path === 'checks.yaml');
    await writeFile(path.join(root, untouched.path), untouched.content + '\n', 'utf8');
    const divergentFiles = await readSources();
    const rejectedCheck = await checkSources({ ...actual, files: divergentFiles });
    expect(rejectedCheck.ok).toBe(false);
    expect(await readSources()).toEqual(divergentFiles);
    for (const change of changeSet.changes) {
      expect(divergentFiles.find(f => f.path === change.path)?.content ?? null).toBe(change.before);
    }
    await writeFile(path.join(root, untouched.path), untouched.content, 'utf8');
    const checked = await checkSources({ ...actual, files: await readSources() });
    expect(checked.ok).toBe(true);
    expect(checked.result.external_context_verified).toBe(false);
    // All source checks completed before the first application write. External revision
    // verification/concurrent-writer exclusion belong to real hosts, not this isolated test.
    for (const change of changeSet.changes) {
      const file = actual.files.find(f => f.path === change.path);
      expect(file?.content ?? null).toBe(change.before);
    }
    for (const change of changeSet.changes) {
      const target = path.join(root, change.path);
      if (change.after === null) await rm(target);
      else { await mkdir(path.dirname(target), { recursive: true }); await writeFile(target, change.after, 'utf8'); }
    }
    expect(await readSources()).toEqual([...current.files].sort((a, b) => a.path < b.path ? -1 : 1));
    const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith('GIT_')));
    Object.assign(env, { TMPDIR: path.resolve(process.env.TMPDIR),
      GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null',
      GIT_CEILING_DIRECTORIES: root, XDG_CONFIG_HOME: path.join(root, '.test-config') });
    const args = ['--project', root, '--format', 'json', 'project', 'validate'];
    const result = await promisify(execFile)(process.env.MARA_CLI, args, { cwd: root, env, timeout: 120_000 });
    const validation = JSON.parse(result.stdout);
    expect(validation.valid).toBe(true);
    expect(validation.evaluation_complete).toBe(true);
    await info.attach('native-materialization', { body: JSON.stringify({
      executable: process.env.MARA_CLI, args, base: actual, changeSet, rejectedCheck, checked, validation,
    }, null, 2), contentType: 'application/json' });
  } finally { await rm(root, { recursive: true, force: true }); }
  await info.attach('worker-transcript', {
    body: JSON.stringify({ browserVersion: browser.version(), ...report }, null, 2),
    contentType: 'application/json',
  });
});
