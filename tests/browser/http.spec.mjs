import { test, expect } from '@playwright/test';
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import path from 'node:path';
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { once } from 'node:events';

const exec = promisify(execFile);

async function installQueryFactsProject(root) {
  const snapshot = JSON.parse(await readFile(path.resolve('tests/fixtures/query-facts/snapshot.json'), 'utf8'));
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

function queryFacts() {
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

// @mara checks REQ-LOCAL-DAEMON-HTTP
// @mara checks REQ-SHARED-TYPESCRIPT-CLIENT
// @mara checks REQ-EXPLICIT-BACKEND-CAPABILITIES
// @mara checks DES-JAVASCRIPT-CLIENT-ADAPTERS
test('installed HTTP ESM facade uses an authorized real daemon without replay or lifecycle authority', async ({ page, browser, baseURL }, info) => {
  if (!process.env.TMPDIR || !process.env.MARA_CLI || !process.env.MARA_TEST_RUNTIME_DIR) {
    throw new Error('Set owned TMPDIR, private MARA_TEST_RUNTIME_DIR and real MARA_CLI');
  }
  const fixture = await mkdtemp(path.join(process.env.TMPDIR, 'http-browser-'));
  const runtime = await mkdtemp(path.join(process.env.MARA_TEST_RUNTIME_DIR, 'h-'));
  const project = path.join(fixture, 'selected');
  const queryProject = path.join(fixture, 'query-facts');
  const other = path.join(fixture, 'forbidden');
  await mkdir(other);
  await mkdir(queryProject);
  await installQueryFactsProject(queryProject);
  const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith('GIT_') && !key.startsWith('MARA_')));
  Object.assign(env, { TMPDIR: process.env.TMPDIR, CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR,
    GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null', GIT_CEILING_DIRECTORIES: fixture,
    XDG_CONFIG_HOME: path.join(fixture, '.test-config') });
  const cli = args => exec(process.env.MARA_CLI, ['--format', 'json', '--daemon-directory', runtime, ...args],
    { cwd: fixture, env, timeout: 120_000 });
  await exec('git', ['init', '-q', fixture], { cwd: fixture, env });
  const daemon = spawn(process.env.MARA_CLI, ['--execution-mode', 'direct', '--daemon-directory', runtime,
    'daemon', 'run', '--http-bind', '127.0.0.1:0', '--http-origin', baseURL,
    '--http-project', project, '--http-project', queryProject,
    '--http-init-destination', project], { cwd: fixture, env, stdio: 'ignore' });
  const daemonExit = once(daemon, 'exit');
  const pageErrors = [];
  page.on('pageerror', error => pageErrors.push(error.message));
  try {
    let connection;
    const deadline = Date.now() + 10_000;
    while (!connection) {
      try { connection = JSON.parse((await cli(['daemon', 'connection-info'])).stdout); }
      catch {
        if (daemon.exitCode !== null || Date.now() > deadline) throw new Error('Fixture daemon did not become ready');
        await new Promise(resolve => setTimeout(resolve, 20));
      }
    }
    expect(connection.available).toBe(true);
    await page.goto('/');
    const report = await page.evaluate(async ({ endpoint, token, project, queryProject, facts, other }) => {
      const { createHttpClient, MaraClientError } = await import('./http.mjs');
      const assert = (condition, label) => { if (!condition) throw new Error(label); };
      const failure = async (call, kind, code) => {
        try { await call(); throw new Error('Expected failure'); }
        catch (error) {
          assert(error instanceof MaraClientError && error.kind === kind && error.code === code,
            'Expected typed ' + kind + '/' + code);
          assert(!JSON.stringify(error.envelope).includes(token), 'Credential leaked in diagnostic');
          return error.envelope;
        }
      };
      const client = await createHttpClient({ endpoint, token, project });
      window.httpClient = client;
      window.httpFixture = { endpoint, token, project };
      const caps = client.capabilities;
      assert(client.context.backend === 'http' && client.context.selection.project === project &&
        !JSON.stringify(client.context).includes(token), 'Selected project or secret context');
      assert(caps.backend === 'native' && caps.mutation_persistence === 'filesystem' && caps.code.indexing &&
        caps.native_recovery && caps.force_refresh, 'Native capabilities');
      let ticks = 0;
      const timer = setInterval(() => ticks++, 0);
      try {
        const init = await client.projectInit({ template: 'engineering-legacy' });
        assert(init.project.root === project && init.created.length === 5, 'Native initialization');
        const schema = await client.schemaGet();
        assert(schema.kind === 'schema', 'Schema read');
        await client.schemaList({ kind: 'relation' });
        assert((await client.schemaValidate()).valid, 'Schema validation');
        const a = await client.itemCreate({ flavour: 'requirement', id: 'REQ-A', file: 'a.mara.md',
          title: 'HTTP café 中文', fields: [{ key: 'status', value: 'draft' }], body: 'Persistent café 中文.' });
        assert(typeof a.line === 'number' && typeof a.mid === 'string', 'Numeric JSON and MID');
        await failure(() => client.itemCreate({ flavour: 'requirement', id: 'REQ-SURROGATE', file: 'surrogate.mara.md',
          title: '\ud800', fields: [{ key: 'status', value: 'draft' }], body: 'Must not dispatch.' }), 'invalid_argument', 'invalid_argument');
        await client.itemCreate({ flavour: 'requirement', id: 'REQ-B', file: 'a.mara.md',
          title: 'Other', fields: [{ key: 'status', value: 'draft' }], body: 'Other source.' });
        const first = await client.itemList({ limit: 1 });
        assert(first.has_more, 'Bounded first page');
        const listed = [...first.items];
        let page = first;
        while (page.has_more) {
          page = await client.itemList({ limit: 1, cursor: page.next_cursor });
          listed.push(...page.items);
        }
        assert(listed.length === 2 && new Set(listed.map(i => i.id)).size === 2, 'Complete list continuation');
        const searched = await client.search({ query: 'café', limit: 1 });
        assert(searched.format_version === 2 && searched.results.length, 'Search owning result');
        let searchPage = searched;
        while (searchPage.has_more) searchPage = await client.search({ query: 'café', limit: 1, cursor: searchPage.next_cursor });
        const read = await client.get({ reference: 'REQ-A' });
        assert(read.content.includes('Persistent café 中文.'), 'Unicode persisted read');
        let readPage = read;
        while (readPage.has_more) readPage = await client.get({ reference: 'REQ-A', cursor: readPage.next_cursor });
        await client.itemUpdate({ reference: 'REQ-A', title: 'Updated' });
        await failure(() => client.itemUpdate({ reference: 'REQ-A', body: '[[REQ-MISSING]]' }), 'operation', 'operation_failed');
        assert((await client.get({ reference: 'REQ-A' })).content === read.content, 'Rejected edit preserved body');
        await failure(() => client.schemaGet({ project: other }), 'invalid_argument', 'invalid_argument');
        const edge = { source: 'REQ-A', relation: 'refines', target: 'REQ-B' };
        await client.relationAdd(edge);
        const inspection = await client.relationGet({ ...edge, limit: 1 });
        let edgePage = inspection;
        while (edgePage.has_more) edgePage = await client.relationGet({ ...edge, limit: 1, cursor: edgePage.next_cursor });
        const related = await client.related({ reference: 'REQ-A', relations: ['schema:refines'], limit: 1 });
        assert(related.connections.length === 1, 'Owning relationship');
        await failure(() => client.relationAdd(edge), 'operation', 'relation_exists');
        await client.relationRemove({ ...edge, occurrence: inspection.occurrences[0].reference });
        assert((await client.itemValidate({ id: 'REQ-A' })).valid, 'Item validation');
        assert((await client.projectValidate()).valid, 'Project validation');
        await client.itemRename({ reference: 'REQ-A', new_id: 'REQ-RENAMED' });
        await client.itemMove({ reference: 'REQ-RENAMED', file: 'moved.mara.md' });
        await client.itemDelete({ reference: 'REQ-RENAMED' });
        assert((await client.projectMidBackfill()).changed.length === 0, 'MID backfill');
        assert((await client.projectTransactionRollback()).restored.length === 0, 'Real rollback without pending journal');
        assert((await client.projectRefresh()).refreshed, 'Real native refresh');
        await failure(() => createHttpClient({ endpoint, token: 'wrong', project }), 'authentication', 'authentication_failed');
        const forbidden = await createHttpClient({ endpoint, token, project: other });
        try { await failure(() => forbidden.schemaGet(), 'authorization', 'authorization_failed'); }
        finally { await forbidden.close(); }
        await failure(() => createHttpClient({ endpoint: 'http://127.0.0.1:1/rpc', token, project }), 'connection', 'http_connection_failed');
        const queryClient = await createHttpClient({ endpoint, token, project: queryProject });
        let query;
        try {
          const request = {
            definition_files: ['facts.json'], definition: 'urn:fixture:facts:composed',
            selection: { ids: ['CAND-C'] },
            parameters: [
              { name: 'candidate', value: { type: 'text', value: 'CAND-C' } },
              { name: 'context', value: { type: 'text', value: 'release' } },
            ], max_rows: 100, limit: 100, fact_snapshots: [facts], mappings: [],
          };
          const page = await queryClient.queryExecute(request);
          const view = await queryClient.queryView({ request, render: null });
          const validation = await queryClient.itemValidate({ id: 'CAND-C', fact_snapshots: [facts], mappings: [] });
          const gate = {
            definition_files: ['facts.json'], gate_file: 'facts-gates.yaml', shape_files: ['facts-present.ttl'],
            gate: 'require-composed-facts', scope: { ids: ['CAND-C'] },
            context: request.parameters, max_rows: 100, fact_snapshots: [facts], mappings: [],
          };
          const gateResult = await queryClient.queryGate(gate);
          const gateReport = await queryClient.queryReport({ subject: { kind: 'gate', ...gate }, render: null });
          const ruleReport = await queryClient.queryReport({ subject: {
            kind: 'documentation_rules', target: { kind: 'item', id: 'CAND-C' }, fact_snapshots: [facts], mappings: [],
          }, render: null });
          assert(page.evaluation_complete && page.summary.binding_count === 3, 'Definition-v1 named query');
          assert(view.input_identity === page.input_identity && view.summary.binding_count === 3, 'Factual query view');
          assert(validation.valid && validation.query_policies.checks[0].state === 'passed', 'Enabled fact policy');
          assert(gateResult.state === 'passed' && gateReport.checks[0].state === 'passed', 'Supplied-input gate/report');
          assert(ruleReport.checks[0].state === 'passed', 'Supplied-input policy report');
          query = { page, view, validation, gate: gateResult, gateReport, ruleReport };
        } finally { await queryClient.close(); }
        assert(ticks > 0, 'Async browser event-loop progress');
        return { caps, init, listed, read, ticks, browser: navigator.userAgent, query };
      } finally { clearInterval(timer); }
    }, { ...connection, project, queryProject, other, facts: queryFacts() });
    expect(report.query.page.summary.binding_count).toBe(3);
    expect(report.query.view.input_identity).toBe(report.query.page.input_identity);
    expect(report.query.validation.query_policies.checks[0].state).toBe('passed');
    expect(report.query.gate.state).toBe('passed');
    expect(report.query.gateReport.checks[0].state).toBe('passed');
    expect(report.query.ruleReport.checks[0].state).toBe('passed');
    expect(await readFile(path.join(project, 'surrogate.mara.md'), 'utf8').catch(() => null)).toBeNull();
    // Configured/request-local rule results and complete bounded trace/validation are owning data.
    await writeFile(path.join(project, '.mara/engineering-rules.yaml'),
      '- id: rule:draft\n  targetClass: requirement\n  property:\n    - {path: status, hasValue: draft}\n');
    await writeFile(path.join(project, 'checks.yaml'),
      '- id: rule:status\n  class: requirement\n  property:\n    - {path: status, hasValue: {parameter: expected_status}}\n');
    const rules = await page.evaluate(async () => {
      const client = window.httpClient;
      const trace = await client.traceMatrix({ all: true, rules: ['urn:mara:rule:draft'], limit: 1 });
      let page = trace;
      let records = trace.records.length;
      while (page.has_more) {
        page = await client.traceMatrix({ all: true, rules: ['urn:mara:rule:draft'], limit: 1, cursor: page.next_cursor });
        records += page.records.length;
      }
      const check = await client.traceMatrix({ ids: ['REQ-B'], check: {
        files: ['checks.yaml'], shape: 'urn:mara:rule:status', parameters: { expected_status: 'draft' },
      } });
      await client.itemUpdate({ reference: 'REQ-B', fields: [{ key: 'status', value: 'retired' }] });
      const invalid = await client.projectValidate({ limit: 1 });
      let validation = invalid;
      while (validation.has_more) validation = await client.projectValidate({ limit: 1, cursor: validation.next_cursor });
      await client.itemUpdate({ reference: 'REQ-B', fields: [{ key: 'status', value: 'draft' }] });
      return { trace, records, check, invalid };
    });
    expect(rules.trace.evaluation_complete).toBe(true);
    expect(rules.trace.summaries[0].passed).toBe(1);
    expect(rules.records).toBeGreaterThan(1);
    expect(rules.check.summaries[0].passed).toBe(1);
    expect(rules.invalid.valid).toBe(false);
    expect(rules.invalid.evaluation_complete).toBe(true);
    expect(rules.invalid.diagnostics[0].code).toBe('rule_failed');
    // Recover real persisted API-created source through the published journal format.
    // This is a recovery fixture, not an interrupted-publisher certification.
    const rollbackPath = path.join(project, 'a.mara.md');
    const rollbackBefore = await readFile(rollbackPath, 'utf8');
    await writeFile(rollbackPath, '');
    await writeFile(path.join(project, '.mara/transaction.json'), JSON.stringify({
      format_version: 1, changes: [{ path: 'a.mara.md', before: rollbackBefore, after: '', mode: { readonly: false } }],
    }));
    const recovery = await page.evaluate(async () => ({
      rollback: await window.httpClient.projectTransactionRollback(),
      refresh: await window.httpClient.projectRefresh(),
      read: await window.httpClient.get({ reference: 'REQ-B' }),
    }));
    expect(recovery.rollback.restored).toEqual(['a.mara.md']);
    expect(recovery.refresh.refreshed).toBe(true);
    expect(await readFile(rollbackPath, 'utf8')).toBe(rollbackBefore);
    expect(await readFile(path.join(project, '.mara/transaction.json')).catch(() => null)).toBeNull();
    // A real daemon dispatch completes; its response is deliberately lost at the browser boundary.
    let dispatched = 0;
    await page.route(connection.endpoint, async route => {
      if (route.request().postDataJSON()?.method !== 'item_create') return route.continue();
      dispatched++;
      const response = await route.fetch({ maxRetries: 0 });
      expect(response.ok()).toBe(true);
      expect((await response.json()).result.id).toBe('REQ-LOST');
      await response.dispose();
      await route.abort('failed');
    });
    const loss = await page.evaluate(async () => {
      try {
        await window.httpClient.itemCreate({ flavour: 'requirement', id: 'REQ-LOST', file: 'lost.mara.md',
          title: 'Lost response', fields: [{ key: 'status', value: 'draft' }], body: 'Committed once.' });
        throw new Error('Expected unknown outcome');
      } catch (error) { return { kind: error.kind, code: error.code, envelope: error.envelope }; }
    });
    await page.unroute(connection.endpoint);
    expect(loss.kind).toBe('unknown_outcome');
    expect(loss.code).toBe('unknown_outcome');
    expect(loss.envelope.error.message).toContain('Do not retry or replay');
    expect(dispatched).toBe(1);
    expect((await readFile(path.join(project, 'lost.mara.md'), 'utf8')).match(/:::mara requirement REQ-LOST/g)).toHaveLength(1);
    // The actual factory and a later call both reject real incompatible-version responses.
    await page.route(connection.endpoint, route => route.continue({ headers: {
      ...route.request().headers(), 'x-mara-application-version': '0.0.0',
    } }));
    const adapterMismatch = await page.evaluate(async () => {
      const { createHttpClient, MaraClientError } = await import('./http.mjs');
      const errors = [];
      for (const call of [
        () => createHttpClient(window.httpFixture),
        () => window.httpClient.itemUpdate({ reference: 'REQ-B', title: 'Must not change' }),
      ]) {
        try { await call(); throw new Error('Expected incompatibility'); }
        catch (error) {
          if (!(error instanceof MaraClientError)) throw error;
          errors.push({ kind: error.kind, code: error.code, envelope: error.envelope });
        }
      }
      return errors;
    });
    await page.unroute(connection.endpoint);
    expect(adapterMismatch.map(e => [e.kind, e.code])).toEqual([
      ['incompatible', 'incompatible_version'], ['incompatible', 'incompatible_version'],
    ]);
    expect(await readFile(path.join(project, 'a.mara.md'), 'utf8')).not.toContain('Must not change');
    // Exact mismatch is rejected by the real daemon before project mutation.
    const versionFailure = await page.evaluate(async () => {
      const response = await fetch(window.httpFixture.endpoint, { method: 'POST',
        headers: { 'Content-Type': 'application/json', Authorization: 'Bearer ' + window.httpFixture.token,
          'X-Mara-Protocol-Version': '1', 'X-Mara-Application-Version': '0.0.0' },
        body: JSON.stringify({ jsonrpc: '2.0', id: 100, method: 'item_create', params: {
          project: window.httpFixture.project, flavour: 'requirement', id: 'REQ-NO', file: 'no.mara.md', title: 'No', body: 'No',
        } }),
      });
      return response.json();
    });
    expect(versionFailure.error.code).toBe(-32002);
    expect(await readFile(path.join(project, 'no.mara.md'), 'utf8').catch(() => null)).toBeNull();
    // Independent client survives ordered/idempotent close; no HTTP administration is sent.
    const lifecycle = await page.evaluate(async () => {
      const { createHttpClient } = await import('./http.mjs');
      const independent = await createHttpClient(window.httpFixture);
      try {
        const queued = window.httpClient.get({ reference: 'REQ-B' });
        const close = window.httpClient.close();
        await queued;
        await close;
        let code;
        try { await window.httpClient.schemaGet(); } catch (error) { code = error.code; }
        return { idempotent: close === window.httpClient.close(), code, read: await independent.get({ reference: 'REQ-LOST' }) };
      } finally { await independent.close(); }
    });
    expect(lifecycle.idempotent).toBe(true);
    expect(lifecycle.code).toBe('client_closed');
    expect(lifecycle.read.content).toContain('Committed once.');
    const nativeRead = JSON.parse((await cli(['--project', project, 'get', 'REQ-LOST'])).stdout);
    expect(lifecycle.read).toEqual(nativeRead);
    const parity = await page.evaluate(async () => {
      const { createHttpClient } = await import('./http.mjs');
      const client = await createHttpClient(window.httpFixture);
      try {
        return { list: await client.itemList(), search: await client.search({ query: 'Committed' }),
          validation: await client.projectValidate() };
      } finally { await client.close(); }
    });
    expect(parity.list).toEqual(JSON.parse((await cli(['--project', project, 'item', 'list'])).stdout));
    expect(parity.search).toEqual(JSON.parse((await cli(['--project', project, 'search', 'Committed'])).stdout));
    expect(parity.validation).toEqual(JSON.parse((await cli(['--project', project, 'project', 'validate'])).stdout));
    expect(JSON.parse((await cli(['daemon', 'status'])).stdout).state).toBe('running');
    const source = await readFile(path.join(project, 'a.mara.md'), 'utf8');
    expect(source).toContain('REQ-B');
    expect(source).not.toContain(connection.token);
    expect(pageErrors).toEqual([]);
    // Same loopback server, explicitly unallowlisted serialized Origin: browser CORS hides 403.
    await page.goto(baseURL.replace('127.0.0.1', 'localhost') + '/');
    const blockedOrigin = await page.evaluate(async ({ endpoint, token, project }) => {
      const { createHttpClient, MaraClientError } = await import('./http.mjs');
      try { await createHttpClient({ endpoint, token, project }); throw new Error('Expected blocked origin'); }
      catch (error) {
        if (!(error instanceof MaraClientError)) throw error;
        return { kind: error.kind, code: error.code, message: error.message };
      }
    }, { ...connection, project });
    expect(blockedOrigin.kind).toBe('connection');
    expect(blockedOrigin.code).toBe('http_connection_failed');
    expect(blockedOrigin.message).not.toContain(connection.token);
    await info.attach('http-acceptance', { body: JSON.stringify({
      browserVersion: browser.version(), ...report, rules, recovery, parity, loss, dispatched, adapterMismatch, versionFailure, lifecycle, blockedOrigin,
    }, null, 2), contentType: 'application/json' });
  } finally {
    await page.unrouteAll({ behavior: 'ignoreErrors' });
    await page.evaluate(() => window.httpClient?.close()).catch(() => {});
    if (daemon.exitCode === null) {
      try { await cli(['daemon', 'stop']); }
      catch { daemon.kill('SIGTERM'); }
    }
    await daemonExit;
    await rm(runtime, { recursive: true, force: true });
    await rm(fixture, { recursive: true, force: true });
  }
});

// @mara checks REQ-LOCAL-DAEMON-HTTP
// @mara checks DES-DAEMON-LIFECYCLE
// @mara checks DES-LOCAL-DAEMON-PROTOCOL
test('installed daemon refuses HTTP authority before configuration reads or indexer execution', async ({ page, baseURL }, info) => {
  if (!process.env.TMPDIR || !process.env.MARA_CLI || !process.env.MARA_TEST_RUNTIME_DIR) {
    throw new Error('Set owned TMPDIR, private MARA_TEST_RUNTIME_DIR and installed MARA_CLI');
  }
  const fixture = await mkdtemp(path.join(process.env.TMPDIR, 'http-authority-'));
  const runtime = await mkdtemp(path.join(process.env.MARA_TEST_RUNTIME_DIR, 'a-'));
  const repository = path.join(fixture, 'repository');
  const project = path.join(repository, 'selected');
  const descendant = path.join(project, 'descendant');
  const worktree = path.join(fixture, 'other-worktree');
  const forbidden = path.join(fixture, 'forbidden');
  const destination = path.join(fixture, 'init-only');
  const deniedDestination = path.join(project, 'not-authorized');
  const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith('GIT_') && !key.startsWith('MARA_')));
  Object.assign(env, { TMPDIR: process.env.TMPDIR, CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR,
    GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null', GIT_CEILING_DIRECTORIES: fixture,
    XDG_CONFIG_HOME: path.join(fixture, '.test-config') });
  const cli = args => exec(process.env.MARA_CLI, ['--format', 'json', '--daemon-directory', runtime, ...args],
    { cwd: fixture, env, timeout: 120_000 });
  let daemon;
  let daemonExit;
  let logs = '';
  const start = async flags => {
    daemon = spawn(process.env.MARA_CLI, ['--execution-mode', 'direct', '--daemon-directory', runtime,
      'daemon', 'run', ...flags], { cwd: fixture, env, stdio: ['ignore', 'pipe', 'pipe'] });
    daemonExit = once(daemon, 'exit');
    for (const stream of [daemon.stdout, daemon.stderr]) stream.on('data', chunk => { logs += chunk; });
    const deadline = Date.now() + 10_000;
    while (true) {
      try { return JSON.parse((await cli(['daemon', 'connection-info'])).stdout); }
      catch {
        if (daemon.exitCode !== null || Date.now() > deadline) throw new Error('Fixture daemon did not become ready');
        await new Promise(resolve => setTimeout(resolve, 20));
      }
    }
  };
  const stop = async () => {
    if (daemon?.exitCode === null) {
      try { await cli(['daemon', 'stop']); } catch { daemon.kill('SIGTERM'); }
    }
    if (daemonExit) await daemonExit;
  };
  try {
    await mkdir(repository);
    await exec('git', ['init', '-q', repository], { cwd: fixture, env });
    await writeFile(path.join(repository, 'anchor'), 'Fixture-owned worktree identity.\n');
    await exec('git', ['-C', repository, 'add', 'anchor'], { cwd: fixture, env });
    await exec('git', ['-C', repository, '-c', 'user.name=HTTP fixture', '-c', 'user.email=fixture@example.invalid',
      '-c', 'commit.gpgsign=false', 'commit', '-qm', 'Fixture'], { cwd: fixture, env });
    await exec('git', ['-C', repository, 'worktree', 'add', '-q', '--detach', worktree], { cwd: fixture, env });
    for (const root of [project, descendant, worktree, forbidden]) {
      await cli(['--execution-mode', 'direct', '--project', root, 'project', 'init']);
    }
    const disabled = await start([]);
    expect(disabled).toEqual({ available: false, reason: 'http_disabled' });
    expect(JSON.parse((await cli(['daemon', 'status'])).stdout).state).toBe('running');
    await stop();
    const connection = await start(['--http-bind', '127.0.0.1:0', '--http-origin', baseURL,
      '--http-project', project, '--http-init-destination', destination]);
    expect(new URL(connection.endpoint).hostname).toBe('127.0.0.1');
    await page.goto('/');
    // Raw browser fetches retain real Origin/CORS and exact installed artifact versions.
    const rpc = (method, params, options = {}) => page.evaluate(async ({ connection, method, params, options }) => {
      const { applicationVersion } = await import('./version.mjs');
      const headers = { 'Content-Type': 'application/json', 'X-Mara-Protocol-Version': options.protocol ?? '1',
        'X-Mara-Application-Version': options.version ?? applicationVersion };
      if (!options.missingToken) headers.Authorization = 'Bearer ' + (options.token ?? connection.token);
      try {
        const response = await fetch(connection.endpoint, { method: 'POST', headers,
          body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }) });
        const text = await response.text();
        const body = text ? JSON.parse(text) : null; // Access refusals have no RPC body.
        if (JSON.stringify(body).includes(connection.token)) throw new Error('Credential leaked in RPC diagnostic');
        return { status: response.status, body };
      } catch (error) {
        if (options.corsFailure && error instanceof TypeError) return { corsBlocked: true };
        throw error;
      }
    }, { connection, method, params, options });
    // A FIFO cannot produce a normal project result: a config read would hang.
    // Resolve authority using metadata only, including an actual other Git worktree.
    const originals = new Map();
    for (const root of [project, descendant, worktree, forbidden]) {
      const config = path.join(root, '.mara/project.toml');
      originals.set(config, await readFile(config, 'utf8'));
      await rm(config);
      await exec('mkfifo', [config], { cwd: fixture, env });
    }
    try {
      for (const options of [{ missingToken: true }, { token: 'incorrect' }]) {
        expect((await rpc('item_list', { project }, options)).status).toBe(401);
      }
      for (const options of [{ version: '0.0.0' }, { protocol: '99' }]) {
        expect((await rpc('item_list', { project }, options)).body.error.code).toBe(-32002);
      }
      for (const root of [descendant, worktree, forbidden]) {
        for (const method of ['schema_get', 'item_list', 'project_transaction_rollback', 'project_refresh']) {
          expect((await rpc(method, { project: root })).status).toBe(403);
        }
      }
      expect((await rpc('project_init', { project })).status).toBe(403); // root authority is not init authority
      expect((await rpc('project_init', { project: deniedDestination })).status).toBe(403);
      await page.goto(baseURL.replace('127.0.0.1', 'localhost') + '/');
      expect(await rpc('item_list', { project }, { corsFailure: true })).toEqual({ corsBlocked: true });
      await page.goto('/');
      const missingOrigin = await page.request.post(connection.endpoint, { headers: {
        Authorization: 'Bearer ' + connection.token, 'X-Mara-Protocol-Version': '1',
        'X-Mara-Application-Version': JSON.parse(await readFile(path.join(process.env.BROWSER_ASSETS_DIR, 'package.json'), 'utf8')).version,
      }, data: { jsonrpc: '2.0', id: 1, method: 'schema_get', params: { project } } });
      expect(missingOrigin.status()).toBe(403);
    } finally {
      for (const [config, source] of originals) { await rm(config); await writeFile(config, source); }
    }
    expect(await readFile(path.join(deniedDestination, '.mara/project.toml')).catch(() => null)).toBeNull();
    // An actual configured command sentinel, activated by a matching source file.
    for (const root of [descendant, worktree, forbidden]) {
      const config = path.join(root, '.mara/project.toml');
      await writeFile(path.join(root, 'sentinel.rs'), 'pub fn sentinel() {}\n');
      await writeFile(config, originals.get(config).replace('format_version = 1', 'format_version = 3') +
        "\n[[code.languages]]\nname = 'rust'\nextensions = ['rs']\ncommand = ['sh', '-c', 'touch indexer-ran; exit 1', 'sentinel', '{output}']\nposition_encoding = 'utf8'\n");
      expect((await rpc('item_list', { project: root })).status).toBe(403);
      expect(await readFile(path.join(root, 'indexer-ran')).catch(() => null)).toBeNull();
      // Positive control through unchanged native authority proves this command is live.
      await expect(cli(['--execution-mode', 'daemon-required', '--project', root, 'item', 'list'])).rejects.toThrow();
      expect(await readFile(path.join(root, 'indexer-ran'), 'utf8')).toBe('');
    }
    const initialized = await rpc('project_init', { project: destination });
    expect(initialized.body.result.project.root).toBe(destination);
    expect(await readFile(path.join(destination, '.mara/project.toml'), 'utf8')).toContain('[project]');
    expect((await rpc('schema_get', { project: destination })).status).toBe(403); // init authority is not root authority
    for (const method of ['daemon_status', 'daemon_stop', 'daemon_connection_info', 'daemon_set_policy', 'daemon_set_token']) {
      expect((await rpc(method, {})).body.error.code).toBe(-32601);
    }
    const status = (await cli(['daemon', 'status'])).stdout;
    expect(JSON.parse(status).state).toBe('running');
    await stop();
    expect(status + logs).not.toContain(connection.token);
    await info.attach('http-authority', { body: JSON.stringify({ disabled, host: process.platform + '/' + process.arch,
      descendantRefused: true, otherGitWorktreeRefused: true, fifoReadsRefused: true,
      liveIndexerSentinels: 3, separateRootAndDestinationAuthority: true, routineLogsTokenFree: true }), contentType: 'application/json' });
  } finally {
    await stop();
    await rm(runtime, { recursive: true, force: true });
    await rm(fixture, { recursive: true, force: true });
  }
});
