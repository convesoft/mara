// Real native shared-facade half of the Chromium document parity test.
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
const { createNativeClient } = await import(pathToFileURL(process.env.MARA_NODE_CLIENT));
const input = JSON.parse(await readFile(process.argv[2], 'utf8'));
const client = await createNativeClient({ callerDirectory: input.root, project: input.root,
  mode: 'direct', ...(process.env.MARA_NODE_INSTALLED ? {} : { addonPath: process.env.MARA_NODE_ADDON }) });
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
  const files = [];
  const readSources = async (directory = '') => {
    for (const entry of await readdir(path.join(input.root, directory), { withFileTypes: true })) {
      const relative = directory ? `${directory}/${entry.name}` : entry.name;
      if (entry.isDirectory()) {
        if (entry.name !== '.test-config') await readSources(relative);
      } else if (relative !== 'parity-input.json' && !entry.name.endsWith('.lock')) {
        files.push({ path: relative, content: await readFile(path.join(input.root, relative), 'utf8') });
      }
    }
  };
  await readSources();
  files.sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
  process.stdout.write(JSON.stringify({ results, files, capabilities: client.capabilities }));
} finally { await client.close(); }
