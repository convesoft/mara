import { createHttpClient, MaraClientError, type FailureKind, type MaraClient,
  type OperationInputs, type OperationResults } from '@convesoft/mara/http';

export async function useHttp(endpoint: string, token: string, project: string): Promise<void> {
  const client: MaraClient = await createHttpClient({ endpoint, token, project });
  const persistence: 'filesystem' | 'memory' = client.capabilities.mutation_persistence;
  const input: OperationInputs['search'] = { query: 'café', limit: 1 };
  const result: OperationResults['search'] = await client.search(input);
  if (result.has_more) await client.search({ ...input, cursor: result.next_cursor });
  const validation = await client.projectValidate();
  const invalid: boolean = !validation.valid; // fulfilled data, not exception
  await client.projectInit({ template: 'engineering' });
  await client.projectMidBackfill();
  await client.projectTransactionRollback();
  await client.projectRefresh();
  await client.schemaGet();
  await client.schemaList({ kind: 'flavour' });
  await client.schemaValidate();
  await client.itemValidate({ id: 'REQ-A' });
  await client.itemList();
  await client.get({ reference: 'REQ-A' });
  await client.related({ reference: 'REQ-A' });
  await client.itemCreate({ flavour: 'requirement', id: 'REQ-A', file: 'a.mara.md', title: 'A', body: 'Body.' });
  await client.itemUpdate({ reference: 'REQ-A', title: 'Updated' });
  await client.itemMove({ reference: 'REQ-A', file: 'b.mara.md' });
  await client.itemRename({ reference: 'REQ-A', new_id: 'REQ-B' });
  await client.itemDelete({ reference: 'REQ-B' });
  const edge = { source: 'REQ-A', relation: 'refines', target: 'REQ-B' };
  await client.relationGet(edge); await client.relationAdd(edge); await client.relationRemove(edge);
  await client.traceMatrix({ all: true });
  await client.close();
  void [persistence, invalid];
}
const accessKinds: FailureKind[] = ['authentication', 'authorization'];
void accessKinds;
export function owningFailure(error: unknown): string | undefined {
  return error instanceof MaraClientError ? error.envelope.error.message : undefined;
}
// @ts-expect-error HTTP setup requires explicit native project
void createHttpClient({ endpoint: 'http://127.0.0.1:3210/rpc', token: 'secret' });
// @ts-expect-error library callers cannot override the artifact application version
void createHttpClient({ endpoint: 'http://127.0.0.1:3210/rpc', token: 'secret', project: '/', applicationVersion: '0.4.0' });
// @ts-expect-error HTTP uses native initialization, not memory workspace naming
void ({} as MaraClient).projectInit({ name: 'memory' });
