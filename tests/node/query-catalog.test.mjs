import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

// @mara checks REQ-QUERY-ENGINE-SURFACES
test('installed native/shared client executes all twelve catalog witnesses on every native route',
  { skip: !process.env.MARA_QUERY_CATALOG_REPORT }, async () => {
    assert.equal(Number(process.versions.node.split('.')[0]), 24);
    const { createNativeClient } = await import(pathToFileURL(process.env.MARA_NODE_CLIENT));
    const cases = JSON.parse(await readFile(process.env.MARA_QUERY_CATALOG_REPORT, 'utf8'));
    assert.deepEqual([...new Set(cases.map(c => c.witness))].sort((a,b)=>a-b), Array.from({length:12},(_,i)=>i+1));
    const receipts = [];
    for (const [i, c] of cases.entries()) {
      const root = path.join(process.env.MARA_NODE_FIXTURES, 'catalog-' + i);
      for (const file of c.snapshot.files) {
        const target = path.join(root, file.path);
        await mkdir(path.dirname(target), { recursive: true }); await writeFile(target, file.content);
      }
      for (const mode of ['direct', undefined, 'daemon-required']) {
        const client = await createNativeClient({ callerDirectory: root, project: root,
          ...(mode ? {mode} : {}), daemonDirectory: process.env.MARA_NODE_DAEMON });
        try {
          const method = c.operation.replace(/_([a-z])/g, (_,l)=>l.toUpperCase());
          let outcome;
          try { outcome = { result: await client[method](c.params) }; }
          catch (error) { outcome = { error: error.envelope.error }; }
          if (outcome.result) delete outcome.result.project; // physical host context only
          assert.deepEqual(outcome, c.outcome, c.name + ' / ' + (mode ?? 'default-auto'));
          receipts.push({name:c.name, witness:c.witness, mode:mode ?? 'default-auto', outcome});
        } finally { await client.close(); }
      }
    }
    if (process.env.MARA_QUERY_CATALOG_NODE_REPORT) await writeFile(process.env.MARA_QUERY_CATALOG_NODE_REPORT, JSON.stringify(receipts, null, 2));
  });
