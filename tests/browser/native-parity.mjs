// Real native shared-facade half of the Chromium document parity test.
import { readFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
const { createNativeClient } = await import(pathToFileURL(process.env.MARA_NODE_CLIENT));
const input = JSON.parse(await readFile(process.argv[2], 'utf8'));
const client = await createNativeClient({ callerDirectory: input.root, project: input.root,
  mode: 'direct', addonPath: process.env.MARA_NODE_ADDON });
const results = [];
try {
  for (const [method, params] of input.calls) {
    const pages = [];
    let cursor;
    do {
      const result = await client[method]({ ...params, ...(cursor ? { cursor } : {}) });
      pages.push(result);
      cursor = result.has_more ? result.next_cursor : null;
      if (result.has_more && !cursor) throw new Error('Missing owning continuation');
    } while (cursor);
    results.push(pages);
  }
  process.stdout.write(JSON.stringify({ results, capabilities: client.capabilities }));
} finally { await client.close(); }
