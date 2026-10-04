import { createWasmClient, MaraClientError, type WasmClient, type SourceSnapshot,
  type MaraClient, type MemoryProjectInitParams, type FileChangeSet } from '@convesoft/mara/browser';

// @mara checks REQ-SHARED-TYPESCRIPT-CLIENT
// Compiled by standard strict TypeScript, then executed by real Chromium.
export async function typedBrowserUsage(): Promise<void> {
  const snapshot: SourceSnapshot = { format_version: 1, files: [],
    selection: { document_paths: [], ignores: [] }, base_identity: 'typed original' };
  const client: WasmClient = await createWasmClient({ snapshot });
  const shared: MaraClient<MemoryProjectInitParams> = client;
  try {
    await shared.projectInit({ name: 'Typed café 中文 👩🏽‍💻', template: 'engineering' });
    await client.schemaGet();
    await client.schemaList({ kind: 'flavour' });
    await client.schemaValidate();
    const a = await client.itemCreate({ flavour: 'requirement', id: 'REQ-TYPED', file: 'typed.mara.md',
      title: 'Typed', body: 'Unicode 中文', fields: [{ key: 'status', value: 'draft' }] });
    await client.get({ reference: a.id });
    await client.search({ query: 'Unicode' });
    await client.related({ reference: a.id });
    await client.itemList({ limit: 1 });
    await client.itemValidate({ id: a.id });
    await client.projectValidate();
    await client.traceMatrix({ all: true, rules: ['urn:mara:rule:requirement'] });
    await client.itemUpdate({ reference: a.id, title: 'Updated' });
    await client.itemRename({ reference: a.id, new_id: 'REQ-RENAMED' });
    await client.itemMove({ reference: 'REQ-RENAMED', file: 'moved.mara.md' });
    await client.itemDelete({ reference: 'REQ-RENAMED' });
    await client.projectMidBackfill();
    const state = await client.workspaceState();
    const sources = await client.workspaceSources();
    const change_set: FileChangeSet = await client.exportChanges();
    const checked = await client.checkChangeSetPreconditions({ change_set, snapshot: sources.base });
    const notVerified: false = checked.external_context_verified;
    const persistence: 'memory' = state.mutation_persistence;
    void notVerified; void persistence;
    await client.loadSnapshot(sources.current);
    try { await client.projectTransactionRollback(); throw new Error('Native recovery accepted'); }
    catch (error: unknown) {
      if (!(error instanceof MaraClientError) || error.code !== 'unsupported_operation') throw error;
    }
  } finally { await client.close(); }
}
function negativeUsage(client: WasmClient) {
  // @ts-expect-error explicit snapshot required
  createWasmClient();
  // @ts-expect-error worker injection is not public setup
  createWasmClient({ worker: new Worker('worker.mjs'), snapshot: {} });
  // @ts-expect-error name required for memory initialization
  client.projectInit();
  // @ts-expect-error native project overrides are not memory initialization
  client.projectInit({ name: 'Memory', project: '/native' });
  // @ts-expect-error change export is not publication
  client.publishChanges();
  // @ts-expect-error precondition snapshot required
  client.checkChangeSetPreconditions({ change_set: {} });
}
void negativeUsage;
