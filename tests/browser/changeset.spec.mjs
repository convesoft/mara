import { test, expect } from '@playwright/test';
import { mkdtemp, mkdir, readdir, readFile, writeFile, rm } from 'node:fs/promises';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import path from 'node:path';

// @mara checks DES-BROWSER-FILE-CHANGESET
// @mara checks REQ-BROWSER-CHANGE-EXPORT
// @mara implements VER-BROWSER-DOCUMENT-ENGINE
test('real export consolidates edits; WASM checks deletion and empty-content preconditions without publication', async ({ page }, info) => {
  await page.goto('/');
  const report = await page.evaluate(async () => {
    const { createWasmClient } = await import('./browser.mjs');
    const client = await createWasmClient({ snapshot: { format_version: 1, files: [],
      selection: { document_paths: [], ignores: [] } } });
    const assert = (value, message) => { if (!value) throw new Error(message); };
    try {
      await client.projectInit({ name: 'Delta fixture', template: 'minimal' });
      await client.itemCreate({ flavour: 'requirement', id: 'REQ-A', title: 'Original',
        file: 'source.mara.md', body: 'Unicode café 中文.' });
      const base = (await client.workspaceSources()).current;
      base.files.push({ path: 'obsolete.txt', content: 'Original café\r\n' }, { path: 'empty.txt', content: '' });
      await client.loadSnapshot(base);
      for (const title of ['Intermediate', 'Final', 'Original']) await client.itemUpdate({ reference: 'REQ-A', title });
      assert((await client.exportChanges()).changes.length === 0, 'restored file was exported');
      await client.itemUpdate({ reference: 'REQ-A', title: 'Final' });
      await client.itemCreate({ flavour: 'requirement', id: 'REQ-TEMP', title: 'Transient',
        file: 'added.mara.md', body: 'Removed knowledge.' });
      await client.itemDelete({ reference: 'REQ-TEMP' });
      const sources = await client.workspaceSources();
      const changeSet = await client.exportChanges();
      assert(changeSet.changes.length === 2, 'intermediate edits not consolidated');
      const retained = changeSet.changes.find(c => c.path === 'added.mara.md');
      assert(retained.before === null && typeof retained.after === 'string' &&
        !retained.after.includes('REQ-TEMP'), 'retained document confused with absence');
      // Public document mutations deliberately retain containing files. The caller-supplied
      // v1 deletion delta below exercises the actual WASM checker, not an invented exporter
      // operation. Rust consolidation independently covers true absent-current entries.
      const deletionCheckSet = structuredClone(changeSet);
      deletionCheckSet.changes.push({ path: 'empty.txt', before: '', after: null },
        { path: 'new-empty.txt', before: null, after: '' },
        { path: 'obsolete.txt', before: 'Original café\r\n', after: null });
      deletionCheckSet.changes.sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
      const checked = await client.checkChangeSetPreconditions({ change_set: deletionCheckSet, snapshot: base });
      assert(checked.source_preconditions_match && !checked.external_context_verified, 'deletion preconditions');
      const failures = [];
      for (const kind of ['absent-is-not-empty', 'empty-is-not-absent', 'duplicate', 'unsafe', 'unordered', 'unchanged', 'version']) {
        const malformed = structuredClone(deletionCheckSet);
        if (kind === 'absent-is-not-empty') malformed.changes.find(c => c.path === 'empty.txt').before = null;
        if (kind === 'empty-is-not-absent') malformed.changes.find(c => c.path === 'new-empty.txt').before = '';
        if (kind === 'duplicate') malformed.changes.push(malformed.changes[0]);
        if (kind === 'unsafe') malformed.changes[0].path = '../escape';
        if (kind === 'unordered') malformed.changes.reverse();
        if (kind === 'unchanged') malformed.changes[0].after = malformed.changes[0].before;
        if (kind === 'version') malformed.format_version = 2;
        let error;
        try { await client.checkChangeSetPreconditions({ change_set: malformed, snapshot: base }); }
        catch (e) { error = e.envelope; }
        assert(error?.error.code === 'precondition_failed', `${kind} passed`);
        failures.push({ kind, error });
      }
      assert(JSON.stringify(await client.workspaceSources()) === JSON.stringify(sources), 'checking changed memory');
      assert(JSON.stringify(await client.exportChanges()) === JSON.stringify(changeSet), 'checking changed export');
      return { base, sources, changeSet, deletionCheckSet, checked, failures };
    } finally { await client.close(); }
  });
  const root = await mkdtemp(path.join(process.env.TMPDIR, 'deletion-check-'));
  try {
    for (const file of report.base.files) {
      await mkdir(path.dirname(path.join(root, file.path)), { recursive: true });
      await writeFile(path.join(root, file.path), file.content);
    }
    const files = [];
    const readSources = async (directory = '') => {
      for (const entry of await readdir(path.join(root, directory), { withFileTypes: true })) {
        const relative = directory ? `${directory}/${entry.name}` : entry.name;
        if (entry.isDirectory()) await readSources(relative);
        else files.push({ path: relative, content: await readFile(path.join(root, relative), 'utf8') });
      }
    };
    await readSources();
    const fresh = { format_version: 1, files, selection: {
      document_paths: files.filter(f => f.path.endsWith('.mara.md')).map(f => f.path), ignores: [],
    }, base_identity: 'fresh host context is not currency' };
    const checked = await page.evaluate(async ({ changeSet, fresh }) => {
      const { createWasmClient } = await import('./browser.mjs');
      const client = await createWasmClient({ snapshot: fresh });
      try { return await client.checkChangeSetPreconditions({ change_set: changeSet, snapshot: fresh }); }
      finally { await client.close(); }
    }, { changeSet: report.deletionCheckSet, fresh });
    expect(checked.source_preconditions_match).toBe(true);
    for (const change of report.deletionCheckSet.changes) {
      expect(files.find(f => f.path === change.path)?.content ?? null).toBe(change.before);
    }
    // Test-owned application of source data only, after every precondition passed.
    for (const change of report.deletionCheckSet.changes) {
      const target = path.join(root, change.path);
      if (change.after === null) await rm(target);
      else { await mkdir(path.dirname(target), { recursive: true }); await writeFile(target, change.after); }
    }
    expect(await readFile(path.join(root, 'new-empty.txt'), 'utf8')).toBe('');
    await expect(readFile(path.join(root, 'empty.txt'))).rejects.toMatchObject({ code: 'ENOENT' });
    await expect(readFile(path.join(root, 'obsolete.txt'))).rejects.toMatchObject({ code: 'ENOENT' });
    const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith('GIT_')));
    Object.assign(env, { GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null', GIT_CEILING_DIRECTORIES: root,
      XDG_CONFIG_HOME: path.join(root, '.test-config') });
    const args = ['--execution-mode', 'direct', '--project', root, '--format', 'json', 'project', 'validate'];
    const validation = JSON.parse((await promisify(execFile)(process.env.MARA_CLI, args,
      { cwd: root, env, timeout: 120_000 })).stdout);
    expect(validation).toMatchObject({ valid: true, evaluation_complete: true, diagnostics: [], has_more: false });
    await info.attach('export-and-deletion-check', { body: JSON.stringify({ ...report, fresh, checked,
      executable: process.env.MARA_CLI, args, validation }, null, 2), contentType: 'application/json' });
  } finally { await rm(root, { recursive: true, force: true }); }
});
