import { test, expect } from '@playwright/test';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

// @mara checks REQ-PORTABLE-DOCUMENT-ENGINE
// @mara implements VER-BROWSER-DOCUMENT-ENGINE
for (const template of ['minimal', 'empty', 'engineering', 'engineering-legacy']) test(`installed native/worker initialization parity: ${template}`, async ({ page }, info) => {
  const root = await mkdtemp(path.join(process.env.TMPDIR, 'init-parity-'));
  try {
    const calls = [['projectInit', { template }], ['schemaGet', {}], ['schemaValidate', {}], ['projectMidBackfill', {}]];
    const inputFile = path.join(root, 'parity-input.json');
    await writeFile(inputFile, JSON.stringify({ root, calls }));
    const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith('GIT_')));
    Object.assign(env, { GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null', GIT_CEILING_DIRECTORIES: root,
      XDG_CONFIG_HOME: path.join(root, '.test-config') });
    const native = JSON.parse((await promisify(execFile)(process.env.NODE24,
      [fileURLToPath(new URL('./native-parity.mjs', import.meta.url)), inputFile],
      { cwd: root, env, timeout: 120_000, maxBuffer: 5 * 1024 * 1024 })).stdout);
    await page.goto('/');
    const browser = await page.evaluate(async ({ template, name }) => {
      const { createWasmClient } = await import('./browser.mjs');
      const client = await createWasmClient({ snapshot: { format_version: 1, files: [],
        selection: { document_paths: [], ignores: [] } } });
      try {
        const results = [[await client.projectInit({ template, name })], [await client.schemaGet()],
          [await client.schemaValidate()], [await client.projectMidBackfill()]];
        return { results, files: (await client.workspaceSources()).current.files, changes: await client.exportChanges() };
      } finally { await client.close(); }
    }, { template, name: path.basename(root) });
    const normalize = value => {
      if (typeof value === 'string') return value === root ? '.' : value.startsWith(root + '/') ? value.slice(root.length + 1)
        : value.startsWith('./') ? value.slice(2) : value;
      if (Array.isArray(value)) return value.map(normalize);
      if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, normalize(v)]));
      return value;
    };
    expect(normalize(native.results)).toEqual(normalize(browser.results));
    expect(native.files).toEqual(browser.files);
    expect(browser.changes.changes.every(c => c.before === null && typeof c.after === 'string')).toBe(true);
    await info.attach('initialization-parity', { body: JSON.stringify({ template, root, native, browser }, null, 2), contentType: 'application/json' });
  } finally { await rm(root, { recursive: true, force: true }); }
});

const fields = { status: { type: 'enum', values: ['draft', 'accepted', 'retired'] },
  owner: { type: 'string' }, integer: { type: 'integer' }, double: { type: 'number' },
  boolean: { type: 'boolean' }, revision: { type: 'string' } };
const flavours = Object.fromEntries(['requirement', 'verification', 'evidence'].map((name, i) => [name, {
  description: name, use_when: [name], avoid_when: [], distinguish_from: {},
  id_prefix: ['REQ-', 'VER-', 'EVD-'][i], body: 'required', fields,
}]));
const schema = { format_version: 3, flavours, relations: {
  verifies: { description: 'Checks', source: ['verification'], target: ['requirement'], inverse: 'verified_by',
    cardinality: { outgoing: { maximum: 1, severity: 'error' } } },
  evidences: { description: 'Records', source: ['evidence'], target: ['verification'], inverse: 'evidenced_by' },
  implements: { description: 'Code', source: [], target: ['requirement'], code_source: true, inverse: 'implemented_by' },
} };
const rules = `
- id: rule:coverage
  targetClass: requirement
  whenShape: rule:accepted
  property:
    - path: {inversePath: verifies}
      qualifiedValueShape:
        class: verification
        node: rule:accepted
        property:
          - path: {inversePath: evidences}
            qualifiedValueShape: {class: evidence, node: rule:accepted}
            qualifiedMinCount: 1
      qualifiedMinCount: 1
- {id: rule:accepted, property: [{path: status, hasValue: accepted}]}
- {id: rule:draft, property: [{path: status, hasValue: draft}]}
- {id: rule:retired, property: [{path: status, hasValue: retired}]}
- id: rule:literals
  targetClass: requirement
  and: [rule:fields]
  or: [rule:accepted, rule:draft]
  not: rule:retired
- id: rule:fields
  property:
    - {path: status, datatype: string, minCount: 1, maxCount: 1, in: [accepted, draft]}
    - {path: owner, pattern: '^Unicode café', node: {datatype: string}}
    - {path: integer, datatype: integer, hasValue: {value: 7, datatype: integer}}
    - {path: double, datatype: double, hasValue: {value: 1.5, datatype: double}}
    - {path: boolean, datatype: boolean, hasValue: {value: true, datatype: boolean}}
`;
const checks = `
- id: rule:execution
  class: requirement
  property:
    - path: {inversePath: verifies}
      qualifiedValueShape:
        class: verification
        property:
          - path: {inversePath: evidences}
            qualifiedValueShape:
              class: evidence
              property:
                - path: revision
                  hasValue: {parameter: subject_revision}
                  in: [{parameter: subject_revision}]
            qualifiedMinCount: 1
      qualifiedMinCount: 1
`;
function fixture() {
  let content = '# Unicode café 👩🏽‍💻\n\n';
  for (const [i, flavour, id, status, relations] of [
    [0, 'requirement', 'REQ-A', 'accepted', ''], [1, 'requirement', 'REQ-B', 'accepted', ''],
    [2, 'requirement', 'REQ-DRAFT', 'draft', ''],
    [3, 'verification', 'VER-A', 'accepted', ':verifies: REQ-A\n:verifies: REQ-A\n'],
    [4, 'verification', 'VER-DRAFT', 'draft', ':verifies: REQ-A\n'],
    [5, 'evidence', 'EVD-A', 'accepted', ':evidences: VER-A\n'],
  ]) content += `:::mara ${flavour} ${id}\n:mid: 01ARZ3NDEKTSV4RRFFQ69G5F0${i}\n:title: ${id}\n:status: ${status}\n:owner: Unicode café\n:integer: 7\n:double: 1.5\n:boolean: true\n:revision: abc123\n${relations}\nReal source.\n:::\n\n`;
  return { format_version: 1, base_identity: 'opaque fixture',
    selection: { document_paths: ['items.mara.md'], ignores: [] }, files: [
      { path: '.mara/project.toml', content: "format_version = 2\n[project]\nname = 'Rules parity'\nschema = '.mara/schema.yaml'\n[content]\ninclude = ['**/*.mara.md']\n[rules]\nformat_version = 1\nfiles = ['rules.yaml']\n" },
      { path: '.mara/schema.yaml', content: JSON.stringify(schema) },
      { path: 'items.mara.md', content }, { path: 'rules.yaml', content: rules }, { path: 'checks.yaml', content: checks },
    ] };
}

// @mara checks REQ-PORTABLE-DOCUMENT-ENGINE
// @mara implements VER-BROWSER-DOCUMENT-ENGINE
// @mara checks REQ-EXPLICIT-BACKEND-CAPABILITIES
test('installed worker executes typed, nested, qualified rules and graph policies with native parity', async ({ page }, info) => {
  const snapshot = fixture();
  const check = revision => ({ files: ['checks.yaml'], shape: 'urn:mara:rule:execution',
    parameters: { subject_revision: revision } });
  const calls = [
    ['schemaValidate', { limit: 1 }], ['projectValidate', { limit: 1 }],
    ['itemValidate', { id: 'REQ-A', limit: 1 }],
    ['traceMatrix', { all: true, rules: ['urn:mara:rule:coverage', 'urn:mara:rule:literals'], limit: 1 }],
    ...['abc123', 'wrong', '', 'urn:literal:not-an-iri'].map(revision => ['traceMatrix', {
      ids: ['REQ-A'], check: check(revision), limit: 1 }]),
    ['relationAdd', { source: 'VER-A', relation: 'verifies', target: 'REQ-B' }],
    ['projectValidate', { limit: 1 }],
    ['relationRemove', { source: 'VER-A', relation: 'verifies', target: 'REQ-B' }],
    ['itemUpdate', { reference: 'REQ-A', fields: [{ key: 'integer', value: '8' }] }],
    ['projectValidate', { limit: 1 }],
    ['traceMatrix', { ids: ['REQ-A'], rules: ['urn:mara:rule:literals'], limit: 1 }],
    ['relationGet', { source: 'VER-A', relation: 'verifies', target: 'REQ-A', limit: 1 }],
    ['related', { reference: 'REQ-A', relations: ['schema:verified_by'], limit: 1 }],
    ['search', { query: 'Unicode', limit: 1 }],
  ];
  await page.goto('/');
  const browser = await page.evaluate(async ({ snapshot, calls }) => {
    const { createWasmClient } = await import('./browser.mjs');
    const client = await createWasmClient({ snapshot });
    const results = [];
    try {
      for (const [method, params] of calls) {
        const pages = [];
        let cursor;
        do {
          const result = await client[method]({ ...params, ...(cursor ? { cursor } : {}) });
          pages.push(result);
          if (result.has_more && !result.next_cursor) throw new Error('Missing continuation');
          cursor = result.has_more ? result.next_cursor : null;
        } while (cursor);
        results.push(pages);
      }
      return { results, sources: await client.workspaceSources() };
    } finally { await client.close(); }
  }, { snapshot, calls });
  expect(browser.results[0][0].valid).toBe(true);
  expect(browser.results[1].flatMap(p => p.diagnostics).map(d => d.code)).toEqual(['rule_failed']);
  const summaries = browser.results[3][0].summaries;
  expect(summaries.find(s => s.evaluation.shape === 'urn:mara:rule:coverage')).toMatchObject({ passed: 1, failed: 1, not_applicable: 4 });
  expect(summaries.find(s => s.evaluation.shape === 'urn:mara:rule:literals')).toMatchObject({ passed: 3, failed: 0 });
  expect(browser.results[4][0].summaries[0].passed).toBe(1);
  for (const i of [5, 6, 7]) expect(browser.results[i][0].summaries[0].failed).toBe(1);
  expect(browser.results[9].flatMap(p => p.diagnostics).some(d => d.code === 'relation_cardinality')).toBe(true);
  expect(browser.results[13][0].summaries[0].failed).toBe(1);
  for (const i of [14, 15, 16]) expect(browser.results[i].length).toBeGreaterThan(1);
  expect(browser.results.every(pages => pages.every(p => p.evaluation_complete !== false))).toBe(true);
  const root = await mkdtemp(path.join(process.env.TMPDIR, 'rules-parity-'));
  try {
    for (const file of snapshot.files) {
      await mkdir(path.dirname(path.join(root, file.path)), { recursive: true });
      await writeFile(path.join(root, file.path), file.content);
    }
    const inputFile = path.join(root, 'parity-input.json');
    await writeFile(inputFile, JSON.stringify({ root, calls }));
    const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith('GIT_')));
    Object.assign(env, { GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null', GIT_CEILING_DIRECTORIES: root,
      XDG_CONFIG_HOME: path.join(root, '.test-config') });
    const native = JSON.parse((await promisify(execFile)(process.env.NODE24,
      [fileURLToPath(new URL('./native-parity.mjs', import.meta.url)), inputFile],
      { cwd: root, env, timeout: 120_000, maxBuffer: 10 * 1024 * 1024 })).stdout);
    const normalize = input => {
      const handles = new Map();
      const visit = value => {
        if (typeof value === 'string') {
          if (value === root) return '.';
          if (value.startsWith(root + '/')) return '.' + value.slice(root.length);
          if (value.startsWith('check:') || value.startsWith('occ-')) {
            if (!handles.has(value)) handles.set(value, `<context-handle-${handles.size}>`);
            return handles.get(value);
          }
          return value;
        }
        if (Array.isArray(value)) return value.map(visit);
        if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, child]) =>
          [key, key === 'next_cursor' && child !== null ? '<cursor>' : visit(child)]));
        return value;
      };
      return visit(input);
    };
    expect(normalize(native.results)).toEqual(normalize(browser.results));
    expect(native.files).toEqual(browser.sources.current.files);
    await info.attach('complete-rules-parity', { body: JSON.stringify({ snapshot, calls, browser, native }, null, 2), contentType: 'application/json' });
  } finally { await rm(root, { recursive: true, force: true }); }
});

// @mara checks DES-SEMANTIC-CHANGE-PLANS
// @mara checks REQ-EXPLICIT-BACKEND-CAPABILITIES
// @mara implements VER-BROWSER-DOCUMENT-ENGINE
test('source, heading and block links stay safe; missing code never becomes valid coverage', async ({ page }, info) => {
  await page.goto('/');
  const result = await page.evaluate(async snapshot => {
    const { createWasmClient } = await import('./browser.mjs');
    const assert = (value, message) => { if (!value) throw new Error(message); };
    // Rules intentionally fail but do not block source-conforming edits.
    const file = snapshot.files.find(f => f.path === 'items.mara.md');
    file.content = file.content.replace('Real source.', '## Owned heading\n\n<a name="stable-block"></a>\n\nOwned paragraph 中文.');
    snapshot.files.push({ path: 'links.mara.md', content:
      '[Owned](items.mara.md#owned-heading) [Block](items.mara.md#stable-block)\n' });
    snapshot.selection.document_paths.push('links.mara.md');
    const client = await createWasmClient({ snapshot });
    const rejected = [];
    const unchanged = async (method, params) => {
      const before = JSON.stringify(await client.workspaceSources());
      const delta = JSON.stringify(await client.exportChanges());
      let error;
      try { await client[method](params); } catch (e) { error = e.envelope; }
      assert(error, `${method} unexpectedly passed`);
      assert(JSON.stringify(await client.workspaceSources()) === before, `${method} changed sources`);
      assert(JSON.stringify(await client.exportChanges()) === delta, `${method} changed export`);
      rejected.push({ method, params, error });
    };
    try {
      await unchanged('itemUpdate', { reference: 'REQ-A', body: '## Removed heading\n\nNo target.' });
      await unchanged('itemUpdate', { reference: 'REQ-A', body:
        '## Owned heading\n\n<a name="stable-block"></a>\n\nDifferent paragraph.\n\nOwned paragraph 中文.' });
      await unchanged('itemDelete', { reference: 'REQ-A' });
      await unchanged('itemMove', { reference: 'REQ-A', file: 'moved.mara.md' });
      await unchanged('itemUpdate', { reference: 'REQ-B', body: '[[REQ-NOT-HERE]]' });
      await unchanged('itemCreate', { flavour: 'requirement', id: 'REQ-A', file: 'new.mara.md', title: 'Duplicate', body: 'Body.' });
      await client.itemUpdate({ reference: 'REQ-A', title: 'Source-preserving café' });
      const updated = (await client.workspaceSources()).current.files.find(f => f.path === file.path).content;
      assert(updated === file.content.replace(':title: REQ-A', ':title: Source-preserving café'), 'unrelated source bytes changed');
      const itemChildren = await client.related({ reference: 'REQ-A', relations: ['builtin:contains'], direction: 'outgoing' });
      const heading = itemChildren.connections.find(c => c.neighbour.kind === 'section');
      assert(heading && (await client.get({ reference: heading.neighbour.reference })).content.includes('Owned paragraph'), 'heading navigation');
      const sectionChildren = await client.related({ reference: heading.neighbour.reference,
        relations: ['builtin:contains'], direction: 'outgoing' });
      const blocks = await Promise.all(sectionChildren.connections.filter(c => c.neighbour.kind === 'block')
        .map(c => client.get({ reference: c.neighbour.reference })));
      assert(blocks.some(block => block.content.includes('Owned paragraph 中文')), 'block navigation: ' + JSON.stringify(sectionChildren));
      // A schema-level code relation alone allowed all the preceding edits. A source
      // assertion now requires unavailable code even without a configured indexer.
      const code = (await client.workspaceSources()).current;
      code.files.find(f => f.path === file.path).content = updated.replace(':title: Source-preserving café',
        ':title: Source-preserving café\n:implemented_by: code:source.rs');
      await client.loadSnapshot(code);
      const validation = await client.projectValidate();
      assert(!validation.valid && !validation.evaluation_complete &&
        validation.diagnostics.some(d => d.code === 'evaluation_unavailable'), 'code source became a pass');
      const matrix = await client.traceMatrix({ all: true, rules: ['urn:mara:rule:literals'] });
      assert(!matrix.evaluation_complete && matrix.records.some(r => r.kind === 'issue' &&
        r.diagnostic.code === 'evaluation_unavailable'), 'code matrix became a pass');
      assert((await client.schemaValidate()).valid, 'independent schema check unavailable');
      assert((await client.get({ reference: 'REQ-A' })).content.includes('Owned paragraph'), 'independent read unavailable');
      for (const [method, params] of [
        ['projectInit', { name: 'No' }], ['projectMidBackfill', {}],
        ['itemCreate', { flavour: 'requirement', id: 'REQ-NEW', file: 'new.mara.md', title: 'New', body: 'Body.' }],
        ['itemUpdate', { reference: 'REQ-B', title: 'No' }], ['itemDelete', { reference: 'REQ-B' }],
        ['itemMove', { reference: 'REQ-B', file: 'new.mara.md' }], ['itemRename', { reference: 'REQ-B', new_id: 'REQ-B' }],
        ['relationAdd', { source: 'VER-A', relation: 'verifies', target: 'REQ-B' }],
        ['relationRemove', { source: 'VER-A', relation: 'verifies', target: 'REQ-A' }],
      ]) await unchanged(method, params);
      return { rejected, validation, matrix, updated };
    } finally { await client.close(); }
  }, fixture());
  await info.attach('source-safety-and-unavailable-code', { body: JSON.stringify(result, null, 2), contentType: 'application/json' });
});
