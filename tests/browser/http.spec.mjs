import { test, expect } from '@playwright/test';
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import path from 'node:path';
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { once } from 'node:events';

const exec = promisify(execFile);

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
  const other = path.join(fixture, 'forbidden');
  await mkdir(other);
  const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith('GIT_') && !key.startsWith('MARA_')));
  Object.assign(env, { TMPDIR: process.env.TMPDIR, CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR,
    GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null', GIT_CEILING_DIRECTORIES: fixture,
    XDG_CONFIG_HOME: path.join(fixture, '.test-config') });
  const cli = args => exec(process.env.MARA_CLI, ['--format', 'json', '--daemon-directory', runtime, ...args],
    { cwd: fixture, env, timeout: 120_000 });
  await exec('git', ['init', '-q', fixture], { cwd: fixture, env });
  const daemon = spawn(process.env.MARA_CLI, ['--execution-mode', 'direct', '--daemon-directory', runtime,
    'daemon', 'run', '--http-bind', '127.0.0.1:0', '--http-origin', baseURL,
    '--http-project', project, '--http-init-destination', project], { cwd: fixture, env, stdio: 'ignore' });
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
    const report = await page.evaluate(async ({ endpoint, token, project, other }) => {
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
        const init = await client.projectInit({ template: 'engineering' });
        assert(init.project.root === project && init.created.length === 2, 'Native initialization');
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
        assert(ticks > 0, 'Async browser event-loop progress');
        return { caps, init, listed, read, ticks, browser: navigator.userAgent };
      } finally { clearInterval(timer); }
    }, { ...connection, project, other });
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
      browserVersion: browser.version(), ...report, rules, loss, dispatched, adapterMismatch, versionFailure, lifecycle, blockedOrigin,
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
