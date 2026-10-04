import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { createNativeClient, MaraClientError, type MaraClient, type OperationInputs, type OperationResults,
  type ValidationResult, type ClientAdapter, type MemoryProjectInitParams } from '@convesoft/mara';
import { createClient } from '@convesoft/mara/client';

// @mara checks REQ-SHARED-TYPESCRIPT-CLIENT
async function documentUsage(client: MaraClient) {
  const initialized = await client.projectInit({ template: 'engineering' });
  await mkdir(initialized.project.root + '/docs');
  const a = await client.itemCreate({ flavour: 'requirement', id: 'REQ-A', file: 'docs/a.mara.md', title: 'Typed API',
    body: 'Typed owning data.', fields: [{ key: 'status', value: 'draft' }] });
  await client.itemCreate({ flavour: 'requirement', id: 'REQ-B', file: 'docs/a.mara.md', title: 'Target',
    body: 'Target body.', fields: [{ key: 'status', value: 'draft' }] });
  const schema = await client.schemaGet({ kind: 'flavour', name: 'requirement' });
  if (schema.kind === 'flavour') assert.equal(typeof schema.definition.id_prefix, 'string');
  await client.schemaList({ kind: 'relation' });
  const valid: ValidationResult = await client.schemaValidate();
  assert.equal(valid.valid, true);
  const first = await client.itemList({ limit: 1 });
  if (first.has_more) await client.itemList({ limit: 1, cursor: first.next_cursor });
  const found = await client.search({ query: 'Typed' });
  const read = await client.get({ reference: found.results[0]!.node.reference });
  assert.equal(read.node.mid, a.mid);
  const bytes: number = read.content_range.total_bytes;
  assert.ok(bytes > 0);
  const neighbours = await client.related({ reference: a.id });
  for (const { neighbour } of neighbours.connections) {
    if (neighbour.kind === 'item') await client.get({ reference: neighbour.reference });
    if (neighbour.kind === 'external') {
      const address: string = neighbour.address;
      void address;
    }
  }
  await client.itemValidate({ id: a.id });
  await client.itemUpdate({ reference: a.id, title: 'Updated' });
  await client.itemRename({ reference: a.id, new_id: 'REQ-RENAMED' });
  await client.itemMove({ reference: 'REQ-RENAMED', file: 'docs/b.mara.md' });
  const edge = { source: 'REQ-RENAMED', relation: 'depends_on', target: 'REQ-B' };
  await client.relationAdd(edge);
  const inspection = await client.relationGet(edge);
  await client.relationRemove({ ...edge, occurrence: inspection.occurrences[0]!.reference });
  const matrix = await client.traceMatrix({ all: true, rules: ['urn:mara:rule:requirement'] });
  assert.equal(typeof matrix.evaluation_complete, 'boolean');
  await client.itemDelete({ reference: 'REQ-B' });
  await client.projectMidBackfill();
  await client.projectTransactionRollback();
  await client.projectRefresh();
  await writeFile(initialized.project.root + '/.mara/schema.yaml', 'format_version: 999\n');
  // Invalid validation is fulfilled typed data; rejected invocation has a different type.
  const invalid: ValidationResult = await client.projectValidate();
  assert.equal(invalid.valid, false);
  assert.ok(invalid.diagnostics.some(d => d.code === 'format_unsupported'));
  try { await client.get({ reference: 'REQ-MISSING' }); assert.fail('Expected owning failure'); }
  catch (error: unknown) {
    assert.ok(error instanceof MaraClientError);
    const code: string = error.code;
    assert.ok(code);
    assert.equal(error.envelope.format_version, 1);
  }
  const pending = client.schemaValidate();
  const closing: Promise<void> = client.close();
  await pending;
  await closing;
  await assert.rejects(client.schemaValidate(), (error: unknown) => error instanceof MaraClientError && error.kind === 'closed');
}

// Compile-time negative checks: never execute these invalid operations.
function negativeUsage(client: MaraClient) {
  // @ts-expect-error named object required
  client.get('REQ-A');
  // @ts-expect-error reference required
  client.itemUpdate({ title: 'Missing reference' });
  // @ts-expect-error numbers use JS number, not BigInt
  client.itemList({ limit: 1n });
  // @ts-expect-error get has no limit
  client.get({ reference: 'REQ-A', limit: 1 });
  // @ts-expect-error item deletion has no continuation
  client.itemDelete({ reference: 'REQ-A', cursor: 'invalid' });
  // @ts-expect-error native initialization does not accept the memory-only name
  client.projectInit({ name: 'Memory workspace' });
  // @ts-expect-error no daemon-wide shutdown
  client.daemonStop();
}
void negativeUsage;
const input: OperationInputs['search'] = { query: 'Typed', limit: 1 };
const result = null as unknown as OperationResults['search'];
void input; void result;
// Adapter authors use the same operation map without importing Node setup.
const boundary = async (adapter: ClientAdapter): Promise<MaraClient> => createClient(adapter);
void boundary;
async function memoryBoundary(adapter: ClientAdapter<MemoryProjectInitParams>) {
  const client = await createClient<MemoryProjectInitParams>(adapter);
  const result = await client.projectInit({ name: 'Memory workspace', template: 'engineering' });
  const name: string = result.project.name;
  void name;
  // @ts-expect-error memory initialization requires a name
  client.projectInit({ template: 'engineering' });
  // @ts-expect-error memory initialization params cannot be omitted
  client.projectInit();
  // @ts-expect-error native project override is not a memory initialization input
  client.projectInit({ name: 'Memory workspace', project: '/native' });
}
void memoryBoundary;

const callerDirectory = process.env.MARA_NODE_FIXTURES!;
const addonPath = process.env.MARA_NODE_ADDON!;
const client = await createNativeClient({ callerDirectory, project: 'typescript-usage', mode: 'direct', addonPath });
try { await documentUsage(client); } finally { await client.close(); }
console.log('PASS standard TypeScript full-method consumer on real Node addon');
